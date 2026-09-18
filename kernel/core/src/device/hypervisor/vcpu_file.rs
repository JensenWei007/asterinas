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
use ostd::mm::VmIo;
use crate::{
    fs::{
        file::{AccessMode, FileCommon, FileLike, Mappable, StatusFlags, file_table::FdFlags},
        pseudofs::AnonInodeFs,
    },
    prelude::*,
    process::{posix_thread::AsPosixThread, signal::HandlePendingSignal},
    util::ioctl::{RawIoctl, dispatch_ioctl},
    vm::page_cache::{Vmo, VmoOptions},
    context::current_userspace,
};

/// VCPU file descriptor
pub struct VcpuFile {
    vm: Arc<Vm>,
    vcpu: Arc<Vcpu>,
    run_page: Arc<Vmo>,
    //pending_operation: Mutex<Option<PendingOperation>>,
    common: FileCommon,
}


impl VcpuFile {
    /// Creates a new VCPU file
    pub fn new(vm: Arc<Vm>, vcpu_id: u32) -> Result<Self> {
        let run_page = VmoOptions::new(KVM_RUN_MMAP_SIZE).alloc()?;
        let vcpu = vm.create_vcpu(vcpu_id)?;
        vcpu.init();
        let pseudo_path = AnonInodeFs::new_path(|_| "anon_inode:[hypervisor-vcpu]".to_string());
        Ok(Self {
            vm,
            vcpu,
            run_page,
            //pending_operation: Mutex::new(None),
            common: FileCommon::new(pseudo_path, AccessMode::O_RDWR, StatusFlags::empty()),
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
                let regs = cmd.read()?;
                let reg_val: usize = self.vcpu.arch.lock().get_one_reg(regs);
                current_userspace!().write_val(regs.addr as usize, &reg_val)?;
                Ok(0)
            }
            cmd @ SetOneReg => {
                let regs = cmd.read()?;
                let reg_val = current_userspace!().read_val(regs.addr as usize)?;
                self.vcpu.arch.lock().set_one_reg(regs, reg_val);
                Ok(0)
            }
            cmd @ GetRegList => {
                let mut list = cmd.read()?;
                let n = list.n;
                list.n = self.vcpu.arch.lock().num_regs();
                cmd.write(&list)?;
                if n < list.n {
                    return_errno_with_message!(Errno::E2BIG, "GetReglist vec is too small!");
                }
                let mut reg_list = Vec::new();
                self.vcpu.arch.lock().copy_reg_vec(Some(&mut reg_list));
                reg_list.insert(0, list.n);
                current_userspace!().write_slice(raw_ioctl.arg(), &reg_list)?;
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

    #[cfg(target_arch = "riscv64")]
    fn write_exit_to_run_page(&self, _exit_info: GuestExitInfo) -> Result<()> {
        Ok(())
    }

    fn write_simple_exit(&self, exit_reason: u32) -> Result<()> {
        self.write_run_val(KVM_RUN_EXIT_REASON_OFFSET, &exit_reason)
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
