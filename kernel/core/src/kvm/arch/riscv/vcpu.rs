// SPDX-License-Identifier: MPL-2.0

use ostd::arch::vm::{VcpuArchInner, KvmCpuTrap, INSN_OPCODE_MASK, INSN_OPCODE_SHIFT, KvmInsnReturn, CAUSE_IRQ_FLAG};

use crate::kvm::{
    ioctl::{KVM_RUN_EXIT_REASON_OFFSET, KVM_RUN_IMMEDIATE_EXIT_OFFSET, KVM_EXIT_UNKNOWN, KVM_EXIT_MMIO, KVM_EXIT_RISCV_SBI, 
        KVM_EXIT_RISCV_CSR, KVM_RUN_MMIO_IS_WRITE_OFFSET, INSN_OPCODE_SYSTEM, KVM_RUN_MMIO_PHYS_ADDR_OFFSET, KVM_RUN_MMIO_LEN_OFFSET,
        KVM_RUN_MMIO_DATA_OFFSET, EXC_LOAD_GUEST_PAGE_FAULT, EXC_STORE_GUEST_PAGE_FAULT, EXC_VIRTUAL_INST_FAULT, EXC_INST_GUEST_PAGE_FAULT,
        EXC_SUPERVISOR_SYSCALL},
    arch::ioctl::{GetRegList, SetMpState, Interrupt},
    arch::mmio::MmioDecode,
    arch::KvmExitReturn,
    run_page::KvmRun,
};
use ostd::mm::VmIo;
use crate::context::current_userspace;
use crate::util::ioctl::RawIoctl;

use crate::prelude::*;
use crate::dispatch_ioctl;

pub(in crate::kvm) struct VcpuArch {
    /// The ostd part of VcpuArch
    inner: SpinLock<VcpuArchInner>,

    /// MMIO instruction details
	mmio_decode: SpinLock<MmioDecode>,
}

impl VcpuArch {
    pub(in crate::kvm) fn new() -> Self {
        Self {
            inner: SpinLock::new(VcpuArchInner::new()),
            mmio_decode: SpinLock::new(MmioDecode::default()),
        }
    }

    pub(in crate::kvm) fn init(&self) -> Result<()> {
        Ok(self.inner.lock().init()?)
    }

    pub(in crate::kvm) fn get_onereg(&self, reg_id: usize) -> Result<usize> {
        Ok(self.inner.lock().get_one_reg(reg_id)?)
    }

    pub(in crate::kvm) fn set_onereg(&self, reg_id: usize, reg_val: usize) -> Result<()> {
        Ok(self.inner.lock().set_one_reg(reg_id, reg_val)?)
    }

    pub(in crate::kvm) fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        dispatch_ioctl!(match raw_ioctl {
            cmd @ GetRegList => {
                // First, we need to check whether the buffer size provided 
                // by the user is sufficient.
                let mut list = cmd.read()?;
                let n = list.n;
                // Get the actually required buffer size and write to user.
                list.n = self.inner.lock().num_regs()?;
                cmd.write(&list)?;
                // If the size provided by the user is smaller than the actual
                // required size, a response is returned informing the user 
                // that the buffer needs to be enlarged.
                if n < list.n {
                    return Err(Error::new(Errno::E2BIG));
                }
                // The size provided by the user meets the requirements
                // so we copy the regs to the user.
                let mut reg_list = Vec::new();
                self.inner.lock().copy_reg_vec(Some(&mut reg_list))?;
                // We need to write the entire `RegList`, 
                // so we need to prepend `n` to the `vec`.
                reg_list.insert(0, list.n);
                current_userspace!().write_slice(raw_ioctl.arg(), &reg_list)?;
                Ok(0)
            }
            cmd @ SetMpState => {
                // TODO: impl this
                let mut _state = cmd.read()?;
                Ok(0)
            }
            cmd @ Interrupt => {
                let irq = cmd.read()?;
                self.inner.lock().set_interrupt(irq.irq);
                Ok(0)
            }
            _ => {
                let ioctl_nr = raw_ioctl.cmd() & 0xff;
                error!(
                    "kvm: unimplemented VCPU ioctl command: cmd={:#x}, nr={:#x}",
                    raw_ioctl.cmd(),
                    ioctl_nr
                );
                return_errno_with_message!(Errno::ENOTTY, "unknown VCPU ioctl command");
            }
        })
    }

    pub(in crate::kvm) fn run(&self, run: Arc<KvmRun>, page_tabe_root: u64) -> Result<()> {
        let first_run = self.inner.lock().mark_ran_once();
        if first_run {
            self.inner.lock().config_ran_once();
        }

        // Process the data returned after being handled by the user.
        match run.read_run_val::<u32>(KVM_RUN_EXIT_REASON_OFFSET)? {
            KVM_EXIT_MMIO => self.mmio_return(run.clone())?,
            KVM_EXIT_RISCV_SBI => todo!("EXIT_SBI_RETURN is not impled!"),
            KVM_EXIT_RISCV_CSR => todo!("EXIT_CSR_RETURN is not impled!"),
            _ => 0
        };

        // Check whether we need to exit immediately.
        if run.read_run_val::<u8>(KVM_RUN_IMMEDIATE_EXIT_OFFSET)? != 0 {
            return Err(Error::new(Errno::EINTR));
        }

        run.write_run_val::<u32>(KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_UNKNOWN)?;

        self.inner.lock().vcpu_load(page_tabe_root);

        let mut ret = KvmExitReturn::ContinueRun;
        loop {
            if ret == KvmExitReturn::ExitToUserSpace {
                break;
            }

            let guard = ostd::irq::disable_local();

		    // We might have got VCPU interrupts updated asynchronously
		    // so update it in HW.
            self.inner.lock().flush_interrupts();

            // Update HVIP CSR for current CPU
            self.inner.lock().update_hvip();

            // Enter VM and then exit from VM
            let trap = self.enter_exit();

            // Syncup interrupts state with HW
            self.inner.lock().sync_interrupts();

            drop(guard);

            // Handle exit trap
            ret = self.exit(run.clone(), trap)?
        }

        self.inner.lock().vcpu_put();

        Ok(())
    }

    fn virtual_insn(&self, trap: &KvmCpuTrap) -> Result<KvmExitReturn> {
        let insn = trap.get_virt_insn();

        let ret = match (insn & INSN_OPCODE_MASK) >> INSN_OPCODE_SHIFT {
            INSN_OPCODE_SYSTEM => ostd::arch::vm::handle_system_opcode_insn(insn),
            other => todo!("virtual_insn: Other insns({:#x}) is not impled!", other),
        };

        self.handle_insn_return(insn, ret);

        Ok(KvmExitReturn::ContinueRun)
    }

    fn handle_insn_return(&self, insn: usize, ret: KvmInsnReturn) {
        match ret {
            KvmInsnReturn::ContinueNextSepc => self.inner.lock().guest_context.step_sepc(ostd::arch::vm::insn_len(insn)),
            _ => todo!("handle_insn_return: Other insn return is not impled!"),
        }
    }

    /// Emulate MMIO load instruction
    fn mmio_load(&self, run: Arc<KvmRun>, fault_addr: usize, trap_insn: usize) -> Result<KvmExitReturn> {
        self.mmio_decode.lock().reset_with_insn(trap_insn);
        self.mmio_decode.lock().load_decode();

        // Update MMIO details in kvm_run struct
        run.write_run_val::<u8>(KVM_RUN_MMIO_IS_WRITE_OFFSET, &0)?;
        run.write_run_val::<usize>(KVM_RUN_MMIO_PHYS_ADDR_OFFSET, &fault_addr)?;
        run.write_run_val::<u32>(KVM_RUN_MMIO_LEN_OFFSET, &(self.mmio_decode.lock().len as u32))?;
        
        // Exit to userspace for MMIO emulation
        run.write_run_val::<u32>(KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_MMIO)?;

        Ok(KvmExitReturn::ExitToUserSpace)
    }

    /// Emulate MMIO store instruction
    fn mmio_store(&self, run: Arc<KvmRun>, fault_addr: usize, trap_insn: usize) -> Result<KvmExitReturn> {
        self.mmio_decode.lock().reset_with_insn(trap_insn);
        let data = self.mmio_decode.lock().store_decode(&self.inner.lock().guest_context)?;

        // Copy data to kvm_run instance
        match self.mmio_decode.lock().len {
            1 => run.write_run_val::<u8>(KVM_RUN_MMIO_DATA_OFFSET, &(data as u8))?,
            2 => run.write_run_val::<u16>(KVM_RUN_MMIO_DATA_OFFSET, &(data as u16))?,
            4 => run.write_run_val::<u32>(KVM_RUN_MMIO_DATA_OFFSET, &(data as u32))?,
            8 => run.write_run_val::<u64>(KVM_RUN_MMIO_DATA_OFFSET, &(data as u64))?,
            _ => return Err(Error::new(Errno::EOPNOTSUPP)),
        }

        // Update MMIO details in kvm_run struct
        run.write_run_val::<u8>(KVM_RUN_MMIO_IS_WRITE_OFFSET, &1)?;
        run.write_run_val::<usize>(KVM_RUN_MMIO_PHYS_ADDR_OFFSET, &fault_addr)?;
        run.write_run_val::<u32>(KVM_RUN_MMIO_LEN_OFFSET, &(self.mmio_decode.lock().len as u32))?;
        
        // Exit to userspace for MMIO emulation
        run.write_run_val::<u32>(KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_MMIO)?;

        Ok(KvmExitReturn::ExitToUserSpace)
    }

    fn gstage_page_fault(&self, run: Arc<KvmRun>, trap: KvmCpuTrap) -> Result<KvmExitReturn> {
        let fault_addr = trap.get_fault_addr();

        match trap.get_trap_type() {
            EXC_LOAD_GUEST_PAGE_FAULT => self.mmio_load(run, fault_addr, trap.get_trap_insn()),
            EXC_STORE_GUEST_PAGE_FAULT => self.mmio_store(run, fault_addr, trap.get_trap_insn()),
            _ => todo!("gstage_page_fault: Other scause({:#x}) in addr({:#x}) is not impled!", trap.get_trap_type(), fault_addr),
        }
    }

    fn exit(&self, run: Arc<KvmRun>, trap: KvmCpuTrap) -> Result<KvmExitReturn> {
        // If we got host interrupt then do nothing
	    if trap.get_trap_type() & CAUSE_IRQ_FLAG != 0 {
            return Ok(KvmExitReturn::ContinueRun);
        }
        run.write_run_val(KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_UNKNOWN)?;
        
        let mut ret = Err(Error::new(Errno::EFAULT));
        match trap.get_trap_type() {
            EXC_VIRTUAL_INST_FAULT => {
                if self.inner.lock().trap_is_from_guest() {
                    ret = self.virtual_insn(&trap);
                }
            }
            EXC_INST_GUEST_PAGE_FAULT | EXC_LOAD_GUEST_PAGE_FAULT | EXC_STORE_GUEST_PAGE_FAULT => {
                if self.inner.lock().trap_is_from_guest() {
                    ret = self.gstage_page_fault(run, trap);
                }
            }
            EXC_SUPERVISOR_SYSCALL => {
                if self.inner.lock().trap_is_from_guest() {
                    ret = Ok(ostd::arch::vm::handle_sbi_call(&mut self.inner.lock())?.into())
                }
            }
            _ => todo!("VcpuArch exit: Trap({:#x}) is not impled!", trap.get_trap_type()),
        }
        ret
    }

    /// Process MMIO value returned from user-space
    fn mmio_return(&self, run: Arc<KvmRun>) -> Result<usize> {
        if self.mmio_decode.lock().return_handled == 1 {
            return Ok(0);
        }
        self.mmio_decode.lock().return_handled = 1;

        if run.read_run_val::<u8>(KVM_RUN_MMIO_IS_WRITE_OFFSET)? != 1 {
            self.mmio_decode.lock().handle_return(run, &mut self.inner.lock().guest_context)?;
        }
        
        let insn_len = self.mmio_decode.lock().insn_len as usize;
        self.inner.lock().guest_context.step_sepc(insn_len);
        Ok(0)
    }

    fn enter_exit(&self) -> KvmCpuTrap {
        self.inner.lock().swap_in_guest_state();

        let trap = self.inner.lock().enter_exit();

        self.inner.lock().swap_in_host_state();

        trap
    }
}
