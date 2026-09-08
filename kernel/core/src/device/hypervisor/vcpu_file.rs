// SPDX-License-Identifier: MPL-2.0

//! VCPU file descriptor implementation

use ostd::arch::vm::{GuestExitInfo};

pub(super) use super::vcpu::Vcpu;
use super::{
    ioctl::*,
    //mmio::{MmioDirection, decode_current_mmio_instruction},
    //pio::{PioDirection, PioOperation},
    //vcpu::{PendingMmioOperation, PendingOperation, PendingPioOperation},
    vm::Vm,
};
use crate::{
    fs::{
        file::{AccessMode, FileCommon, FileLike, Mappable, StatusFlags, file_table::FdFlags},
        pseudofs::AnonInodeFs,
    },
    prelude::*,
    process::{posix_thread::AsPosixThread, signal::HandlePendingSignal},
    util::ioctl::{RawIoctl, dispatch_ioctl},
    vm::page_cache::{Vmo, VmoOptions},
};

/// VCPU file descriptor
pub struct VcpuFile {
    vm: Arc<Vm>,
    vcpu: Arc<Vcpu>,
    run_page: Arc<Vmo>,
    //pending_operation: Mutex<Option<PendingOperation>>,
    common: FileCommon,
    #[cfg(target_arch = "x86_64")]
    compat_state: Mutex<VcpuCompatState>,
}

// Compatibility state for KVM ioctls that are accepted but not wired into
// guest execution yet. QEMU copies these GET results back into CPUX86State,
// so keep the last SET value instead of returning a fresh default state.
#[cfg(target_arch = "x86_64")]
struct VcpuCompatState {
    debug_regs: DebugRegs,
    vcpu_events: VcpuEvents,
    xsave: XsaveState,
}

#[cfg(target_arch = "x86_64")]
impl Default for VcpuCompatState {
    fn default() -> Self {
        Self {
            debug_regs: default_debug_regs(),
            vcpu_events: VcpuEvents::default(),
            xsave: XsaveState::default(),
        }
    }
}

impl VcpuFile {
    /// Creates a new VCPU file
    pub fn new(vm: Arc<Vm>, vcpu_id: u32) -> Result<Self> {
        let run_page = VmoOptions::new(KVM_RUN_MMAP_SIZE).alloc()?;
        let vcpu = vm.create_vcpu(vcpu_id)?;
        let pseudo_path = AnonInodeFs::new_path(|_| "anon_inode:[hypervisor-vcpu]".to_string());
        Ok(Self {
            vm,
            vcpu,
            run_page,
            //pending_operation: Mutex::new(None),
            common: FileCommon::new(pseudo_path, AccessMode::O_RDWR, StatusFlags::empty()),
            #[cfg(target_arch = "x86_64")]
            compat_state: Mutex::new(VcpuCompatState::default()),
        })
    }
}

impl FileLike for VcpuFile {
    fn read(&self, _writer: &mut VmWriter) -> Result<usize> {
        return_errno_with_message!(Errno::EINVAL, "cannot read from VCPU file");
    }

    fn write(&self, _reader: &mut VmReader) -> Result<usize> {
        return_errno_with_message!(Errno::EINVAL, "cannot write to VCPU file");
    }

    #[cfg(target_arch = "riscv64")]
    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        dispatch_ioctl!(match raw_ioctl {
            Run => {
                self.ioctl_run()
            }
            cmd @ GetOneReg => {
                //let regs = self.vcpu.get_regs()?;
                //cmd.write(&regs)?;
                Ok(0)
            }
            cmd @ SetOneReg => {
                //let regs = cmd.read()?;
                //self.vcpu.set_regs(regs)?;
                Ok(0)
            }
            cmd @ GetRegList => {
                //let state = self.vcpu.get_mp_state()?;
                //cmd.write(&state)?;
                Ok(0)
            }
            cmd @ SetMpState => {
                let state = cmd.read()?;
                self.vcpu.set_mp_state(state)?;
                Ok(0)
            }
            _ => {
                let ioctl_nr = raw_ioctl.cmd() & 0xff;
                error!(
                    "hypervisor: unimplemented VCPU ioctl command: cmd={:#x}, nr={:#x}",
                    raw_ioctl.cmd(),
                    ioctl_nr
                );
                return_errno_with_message!(Errno::ENOTTY, "unknown VCPU ioctl command");
            }
        })
    }

    fn common(&self) -> &FileCommon {
        &self.common
    }

    fn mappable(&self) -> Result<Mappable> {
        Ok(Mappable::Vmo(self.run_page.clone()))
    }

    fn dump_proc_fdinfo(self: Arc<Self>, _fd_flags: FdFlags) -> Box<dyn core::fmt::Display> {
        Box::new("hypervisor_vcpu\n")
    }
}

#[cfg(target_arch = "x86_64")]
fn default_debug_regs() -> DebugRegs {
    DebugRegs {
        dr6: 0xffff0ff0,
        dr7: 0x400,
        ..DebugRegs::default()
    }
}

impl VcpuFile {
    fn ioctl_run(&self) -> Result<i32> {
        #[cfg(target_arch = "x86_64")]
        self.complete_pending_operation()?;
        if self.immediate_exit()? {
            return_errno_with_message!(Errno::EINTR, "KVM_RUN interrupted by immediate_exit");
        }

        let Some(exit_info) = self.vcpu.run(|| self.run_interrupted())? else {
            return_errno_with_message!(Errno::EINTR, "KVM_RUN was interrupted");
        };
        self.write_exit_to_run_page(exit_info)?;
        Ok(0)
    }

    #[cfg(target_arch = "x86_64")]
    fn complete_pending_operation(&self) -> Result<()> {
        let Some(operation) = self.pending_operation.lock().take() else {
            return Ok(());
        };

        if let Err(err) = self.complete_operation(operation) {
            *self.pending_operation.lock() = Some(operation);
            return Err(err);
        }
        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    fn complete_operation(&self, operation: PendingOperation) -> Result<()> {
        match operation {
            PendingOperation::Pio(pio) => self.complete_pio_operation(pio),
            PendingOperation::Mmio(mmio) => self.complete_mmio_operation(mmio),
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn complete_pio_operation(&self, operation: PendingPioOperation) -> Result<()> {
        let input_data = if operation.operation.direction() == PioDirection::In {
            let data_len = usize::try_from(operation.count)?
                .checked_mul(usize::from(operation.operation.size()))
                .ok_or_else(|| Error::new(Errno::EOVERFLOW))?;
            let mut bytes = vec![0_u8; data_len];
            self.read_run_bytes(KVM_RUN_IO_DATA_OFFSET, &mut bytes)?;
            Some(bytes)
        } else {
            None
        };

        self.vcpu
            .complete_pio_operation(operation, input_data.as_deref())
    }

    #[cfg(target_arch = "x86_64")]
    fn complete_mmio_operation(&self, operation: PendingMmioOperation) -> Result<()> {
        let instruction = operation.instruction;
        let read_value = if instruction.direction() == MmioDirection::Read {
            let size = instruction.size() as usize;
            let mut bytes = [0_u8; size_of::<u64>()];
            self.read_run_bytes(KVM_RUN_MMIO_DATA_OFFSET, &mut bytes[..size])?;
            Some(u64::from_le_bytes(bytes))
        } else {
            None
        };

        self.vcpu.complete_mmio_operation(operation, read_value)
    }

    fn immediate_exit(&self) -> Result<bool> {
        let immediate_exit = self.read_run_val::<u8>(KVM_RUN_IMMEDIATE_EXIT_OFFSET)?;
        Ok(immediate_exit != 0)
    }

    fn run_interrupted(&self) -> Result<bool> {
        if self.immediate_exit()? {
            return Ok(true);
        }

        // QEMU normally kicks a running vCPU with a POSIX signal. Asterinas
        // delivers that signal after the syscall returns, so KVM_RUN must
        // notice the pending signal itself and return EINTR first.
        let thread = current_thread!();
        Ok(thread
            .as_posix_thread()
            .is_some_and(HandlePendingSignal::has_pending))
    }

    #[cfg(target_arch = "x86_64")]
    fn write_exit_to_run_page(&self, exit_info: GuestExitInfo) -> Result<()> {
        self.clear_run_output()?;
        self.write_common_run_state()?;

        match VmxExitReason::try_from(exit_info.exit_reason) {
            Ok(VmxExitReason::IO_INSTRUCTION) => self.write_io_exit(exit_info),
            Ok(VmxExitReason::EPT_VIOLATION) => self.write_mmio_exit(exit_info),
            Ok(VmxExitReason::HLT) => self.write_simple_exit(KVM_EXIT_HLT),
            Ok(VmxExitReason::TRIPLE_FAULT) => self.write_simple_exit(KVM_EXIT_SHUTDOWN),
            _ => self.write_internal_error_exit(exit_info),
        }
    }

    #[cfg(target_arch = "riscv64")]
    fn write_exit_to_run_page(&self, _exit_info: GuestExitInfo) -> Result<()> {
        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    fn write_common_run_state(&self) -> Result<()> {
        // These fields are maintained in the safe context cache. Avoid a full
        // VMCS synchronization on every userspace-visible VM exit.
        let apic_base = self.vcpu.guest_context().sregs().apic_base;
        let mut state = [0_u8; 20];
        state[12..20].copy_from_slice(&apic_base.to_le_bytes());
        self.write_run_bytes(KVM_RUN_READY_FOR_INTERRUPT_INJECTION_OFFSET, &state)
    }

    fn write_simple_exit(&self, exit_reason: u32) -> Result<()> {
        self.write_run_val(KVM_RUN_EXIT_REASON_OFFSET, &exit_reason)
    }

    #[cfg(target_arch = "x86_64")]
    fn write_io_exit(&self, exit_info: GuestExitInfo) -> Result<()> {
        let (operation, count, data) = {
            let context = self.vcpu.guest_context();
            let Some(operation) = PioOperation::decode(&context, self.vm.memory(), &exit_info)?
            else {
                return self.write_internal_error_exit(exit_info);
            };
            let count = operation.batch_count(&context, KVM_RUN_IO_DATA_CAPACITY);
            if count == 0 {
                return self.write_internal_error_exit(exit_info);
            }
            let data = if operation.direction() == PioDirection::Out {
                operation.output_data(&context, self.vm.memory(), count)?
            } else {
                let data_len = usize::try_from(count)?
                    .checked_mul(usize::from(operation.size()))
                    .ok_or_else(|| Error::new(Errno::EOVERFLOW))?;
                vec![0_u8; data_len]
            };
            (operation, count, data)
        };
        let kvm_direction = match operation.direction() {
            PioDirection::In => KVM_EXIT_IO_IN,
            PioDirection::Out => KVM_EXIT_IO_OUT,
        };
        let size = operation.size();
        let port = operation.port();
        let data_offset = KVM_RUN_IO_DATA_OFFSET as u64;

        self.write_simple_exit(KVM_EXIT_IO)?;
        let mut io = [0_u8; 16];
        io[0] = kvm_direction;
        io[1] = size;
        io[2..4].copy_from_slice(&port.to_le_bytes());
        io[4..8].copy_from_slice(&count.to_le_bytes());
        io[8..16].copy_from_slice(&data_offset.to_le_bytes());
        self.write_run_bytes(KVM_RUN_IO_DIRECTION_OFFSET, &io)?;

        self.write_run_bytes(KVM_RUN_IO_DATA_OFFSET, &data)?;

        *self.pending_operation.lock() = Some(PendingOperation::Pio(PendingPioOperation {
            operation,
            count,
        }));
        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    fn write_mmio_exit(&self, exit_info: GuestExitInfo) -> Result<()> {
        let direction = match exit_info.exit_qualification & 0b111 {
            0b001 => MmioDirection::Read,
            0b010 => MmioDirection::Write,
            _ => return self.write_internal_error_exit(exit_info),
        };
        let context = self.vcpu.guest_context();
        let instruction = decode_current_mmio_instruction(&context, self.vm.memory())?;
        let Some(instruction) = instruction else {
            return self.write_internal_error_exit(exit_info);
        };
        if instruction.direction() != direction {
            return self.write_internal_error_exit(exit_info);
        }

        let size = instruction.size();
        let len = u32::from(size);
        let is_write = u8::from(direction == MmioDirection::Write);
        let mut data = [0_u8; 8];
        if direction == MmioDirection::Write {
            let Some(value) = instruction.write_value(&context) else {
                return self.write_internal_error_exit(exit_info);
            };
            data[..size as usize].copy_from_slice(&value.to_le_bytes()[..size as usize]);
        }
        drop(context);

        self.write_simple_exit(KVM_EXIT_MMIO)?;
        let mut mmio = [0_u8; 24];
        mmio[0..8].copy_from_slice(&(exit_info.guest_phys_addr as u64).to_le_bytes());
        mmio[8..16].copy_from_slice(&data);
        mmio[16..20].copy_from_slice(&len.to_le_bytes());
        mmio[20] = is_write;
        self.write_run_bytes(KVM_RUN_MMIO_PHYS_ADDR_OFFSET, &mmio)?;

        *self.pending_operation.lock() =
            Some(PendingOperation::Mmio(PendingMmioOperation { instruction }));
        Ok(())
    }

    fn write_internal_error_exit(&self, exit_info: GuestExitInfo) -> Result<()> {
        /* 
        warn!(
            "hypervisor: unsupported VM exit for KVM_RUN: reason={:#x}, len={}, rip={:#x}, \
             gpa={:#x}, qualification={:#x}",
            exit_info.exit_reason,
            exit_info.instruction_len,
            exit_info.guest_rip,
            exit_info.guest_phys_addr,
            exit_info.exit_qualification,
        );*/
        self.write_simple_exit(KVM_EXIT_INTERNAL_ERROR)
    }

    fn read_run_val<T: Pod>(&self, offset: usize) -> Result<T> {
        let mut value = T::new_zeroed();
        let mut writer = VmWriter::from(value.as_mut_bytes()).to_fallible();
        self.run_page.read(offset, &mut writer)?;
        Ok(value)
    }

    fn write_run_val<T: Pod>(&self, offset: usize, value: &T) -> Result<()> {
        let mut reader = VmReader::from(value.as_bytes()).to_fallible();
        self.run_page.write(offset, &mut reader)
    }

    fn read_run_bytes(&self, offset: usize, buffer: &mut [u8]) -> Result<()> {
        let mut writer = VmWriter::from(buffer).to_fallible();
        self.run_page.read(offset, &mut writer)
    }

    fn write_run_bytes(&self, offset: usize, buffer: &[u8]) -> Result<()> {
        let mut reader = VmReader::from(buffer).to_fallible();
        self.run_page.write(offset, &mut reader)
    }

    #[cfg(target_arch = "x86_64")]
    fn clear_run_output(&self) -> Result<()> {
        static ZERO_PAGE: [u8; PAGE_SIZE] = [0; PAGE_SIZE];

        let mut offset = KVM_RUN_EXIT_REASON_OFFSET;
        while offset < KVM_RUN_STRUCT_SIZE {
            let len = (KVM_RUN_STRUCT_SIZE - offset).min(PAGE_SIZE);
            let mut reader = VmReader::from(&ZERO_PAGE[..len]).to_fallible();
            self.run_page.write(offset, &mut reader)?;
            offset += len;
        }
        Ok(())
    }
}

impl crate::process::signal::Pollable for VcpuFile {
    fn poll(
        &self,
        _mask: crate::events::IoEvents,
        _poller: Option<&mut crate::process::signal::PollHandle>,
    ) -> crate::events::IoEvents {
        // VCPUs don't support polling
        crate::events::IoEvents::empty()
    }
}
