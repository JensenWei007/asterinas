use ostd::{
    arch::vm::{GuestExitInfo, VcpuRunState, vcpu::VcpuArch}, task::Task, vm::{GuestMode, GuestRunResult},
    arch::vm::types::*,arch::vm::csr::*,arch::vm::vcpu::*,arch::vm::insn::*,
};
use crate::vm::page_cache::Vmo;

use super::{
    ioctl::*,
    ioeventfd::IoEventAddressSpace,
    vm::Vm,
};
use crate::prelude::*;
use ostd::sync::SpinLock;

pub struct Vcpu {
    id: u32,
    pub(super) vm: Weak<Vm>,
    //pub(super) guest_context: Mutex<GuestContext>,
    guest_mode: GuestMode,
    run_lock: Mutex<()>,
    pub arch: SpinLock<VcpuArch>,
}

impl Vcpu {
    #[cfg(target_arch = "riscv64")]
    pub(super) fn new(id: u32, vm: &Arc<Vm>) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            id,
            vm: Arc::downgrade(vm),
            //guest_context: Mutex::new(GuestContext::new(id)?),
            guest_mode: GuestMode::new()?,
            run_lock: Mutex::new(()),
            arch: SpinLock::new(VcpuArch::new()),
        }))
    }

    pub fn init(&self) {
        self.arch.lock().init();
    }

    pub fn vm(&self) -> Result<Arc<Vm>> {
        self.vm
            .upgrade()
            .ok_or_else(|| Error::with_message(Errno::ENOENT, "vm not found"))
    }

    #[cfg(target_arch = "riscv64")]
    pub(super) fn run<F>(self: &Arc<Self>, run_page: Arc<Vmo>, mut immediate_exit: F) -> Result<Option<GuestExitInfo>>
    where
        F: FnMut() -> Result<bool>,
    {
        let vm = self.vm()?;
        let _run_guard = self.run_lock.lock();

        let at_least_once = self.arch.lock().ran_atleast_once;
        if !at_least_once {
            self.arch.lock().config_ran_once();
        }

        // Mark this VCPU ran at least once
	    self.arch.lock().ran_atleast_once = true;

        match read_run_val::<u32>(run_page.clone(), KVM_RUN_EXIT_REASON_OFFSET)? {
            KVM_EXIT_MMIO => {
                self.mmio_return(run_page.clone())?;
            }
            KVM_EXIT_RISCV_SBI => {
                // TODO
                panic!("EXIT_SBI_RETURN");
            }
            KVM_EXIT_RISCV_CSR => {
                // TODO
                panic!("EXIT_CSR_RETURN");
            }
            _ => {}
        }

        if read_run_val::<u8>(run_page.clone(), KVM_RUN_IMMEDIATE_EXIT_OFFSET)? != 0 {
            return_errno_with_message!(Errno::EINTR, "vcpu wants to run!");
        }

        write_run_val::<u32>(run_page.clone(), KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_UNKNOWN)?;

        let pgd_phys = self.vm()?.memory().guest_mem().root_paddr();
        self.arch.lock().vcpu_load(pgd_phys);

        let mut ret = 1;
        loop {
            if ret < 1 {
                break;
            }

            let guard = ostd::irq::disable_local();

            self.flush_interrupts();
            self.arch.lock().update_hvip();

            let trap = self.enter_exit();

            self.arch.lock().sync_interrupts();

            drop(guard);

            ret = self.exit(run_page.clone(), trap)
        }

        self.arch.lock().vcpu_put();

        Ok(Some(GuestExitInfo{}))
    }

    pub fn set_mp_state(&self, state: MpState) -> Result<()> {
        self.arch.lock().set_mp_state(state);
        Ok(())
    }

    pub fn virtual_insn(&self, trap: &KvmCpuTrap) -> usize {
        let insn = trap.stval;

        match (insn & 0x007c) >> 2 {
            INSN_OPCODE_SYSTEM => {
                self.arch.lock().system_opcode_insn(insn)
            }
            _ => {
                panic!("virtual_insn, default");
            }
        }
    }

    fn mmio_load(&self, run_page: Arc<Vmo>, fault_addr: usize, htinst: usize) -> usize {
        let mut insn = 0;
        let mut insn_le = 0;
        let mut len = 0;
        let mut shift = 0;

        if htinst & 0x1 != 0 {
		    insn = htinst | 0x3;
            if htinst & 0x1 << 1 != 0 {
                insn_le = insn_len(insn);
            } else {
                insn_le = 2;
            }
	    }

        if insn & INSN_MASK_LW == INSN_MATCH_LW {
            len = 4;
            shift = 8 * (size_of::<usize>() - len);
        } else if insn & INSN_MASK_LB == INSN_MATCH_LB {
            len = 1;
            shift = 8 * (size_of::<usize>() - len);
        } else if insn & INSN_MASK_LBU == INSN_MATCH_LBU {
            len = 1;
        } else if insn & INSN_MASK_LD == INSN_MATCH_LD {
            len = 8;
            shift = 8 * (size_of::<usize>() - len);
        } else if insn & INSN_MASK_LWU == INSN_MATCH_LWU {
            len = 4;
        } else if insn & INSN_MASK_LH == INSN_MATCH_LH {
            len = 2;
            shift = 8 * (size_of::<usize>() - len);
        } else if insn & INSN_MASK_LHU == INSN_MATCH_LHU {
            len = 2;
        } else if insn & INSN_MASK_C_LD == INSN_MATCH_C_LD {
            len = 8;
            shift = 8 * (size_of::<usize>() - len);
            insn = (8 + (((insn) >> (2)) & (7))) << 7;
        } else if insn & INSN_MASK_C_LDSP == INSN_MATCH_C_LDSP && (insn >> 7) & 0x1f != 0 {
            len = 8;
            shift = 8 * (size_of::<usize>() - len);
        } else if insn & INSN_MASK_C_LW == INSN_MATCH_C_LW {
            len = 4;
            shift = 8 * (size_of::<usize>() - len);
            insn = (8 + (((insn) >> (2)) & (7))) << 7;
        } else if insn & INSN_MASK_C_LWSP == INSN_MATCH_C_LWSP && (insn >> 7) & 0x1f != 0 {
            len = 4;
            shift = 8 * (size_of::<usize>() - len);
        }

        self.arch.lock().mmio_decode.insn = insn;
        self.arch.lock().mmio_decode.insn_len = insn_le as i32;
        self.arch.lock().mmio_decode.len = len as i32;
        self.arch.lock().mmio_decode.shift = shift as i32;
        self.arch.lock().mmio_decode.return_handled = 0;

        write_run_val::<u8>(run_page.clone(), KVM_RUN_MMIO_IS_WRITE_OFFSET, &0);
        write_run_val::<usize>(run_page.clone(), KVM_RUN_MMIO_PHYS_ADDR_OFFSET, &fault_addr);
        write_run_val::<u32>(run_page.clone(), KVM_RUN_MMIO_LEN_OFFSET, &(len as u32));

        write_run_val::<u32>(run_page.clone(), KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_MMIO);
        0
    }

    fn mmio_store(&self, run_page: Arc<Vmo>, fault_addr: usize, htinst: usize) -> usize {
        let mut insn = 0;
        let mut insn_le = 0;
        let mut len = 0;
        let mut data: usize = 0;

        if htinst & 0x1 != 0 {
		    insn = htinst | 0x3;
            if htinst & 0x1 << 1 != 0 {
                insn_le = insn_len(insn);
            } else {
                insn_le = 2;
            }
	    }

        // Here /8 is / sizeof(usize)
        data = self.arch.lock().guest_context.get_reg(((((insn) >> 17) & 0xF8) as u64) / 8).unwrap();

        if insn & INSN_MASK_SW == INSN_MATCH_SW {
            len = 4;
        } else if insn & INSN_MASK_SB == INSN_MATCH_SB {
            len = 1;
        } else if insn & INSN_MASK_SD == INSN_MATCH_SD {
            len = 8;
        } else if insn & INSN_MASK_SH == INSN_MATCH_SH {
            len = 2;
        } else if insn & INSN_MASK_C_SD == INSN_MATCH_C_SD {
            len = 8;
            data = self.arch.lock().guest_context.get_reg((((((8 + (((insn) >> (2)) & (7))) << 7) << 3) & 0xF8) as u64) / 8 ).unwrap();
        } else if insn & INSN_MASK_C_SDSP == INSN_MATCH_C_SDSP && (insn >> 7) & 0x1f != 0 {
            len = 8;
            data = self.arch.lock().guest_context.get_reg((((((8 + (((insn) >> (2)) & (7))) << 7) << 3) & 0xF8) as u64) / 8 ).unwrap();
        } else if insn & INSN_MASK_C_SW == INSN_MATCH_C_SW {
            len = 4;
            data = self.arch.lock().guest_context.get_reg((((((8 + (((insn) >> (2)) & (7))) << 7) << 3) & 0xF8) as u64) / 8 ).unwrap();
        } else if insn & INSN_MASK_C_SWSP == INSN_MATCH_C_SWSP && (insn >> 7) & 0x1f != 0 {
            len = 4;
            data = self.arch.lock().guest_context.get_reg((((((8 + (((insn) >> (2)) & (7))) << 7) << 3) & 0xF8) as u64) / 8 ).unwrap();
        }

        self.arch.lock().mmio_decode.insn = insn;
        self.arch.lock().mmio_decode.insn_len = insn_le as i32;
        self.arch.lock().mmio_decode.len = len as i32;
        self.arch.lock().mmio_decode.shift = 0;
        self.arch.lock().mmio_decode.return_handled = 0;

        match len {
            1 => {
                write_run_val::<u8>(run_page.clone(), KVM_RUN_MMIO_DATA_OFFSET, &(data as u8));
            }
            2 => {
                write_run_val::<u16>(run_page.clone(), KVM_RUN_MMIO_DATA_OFFSET, &(data as u16));
            }
            4 => {
                write_run_val::<u32>(run_page.clone(), KVM_RUN_MMIO_DATA_OFFSET, &(data as u32));
            }
            8 => {
                write_run_val::<u64>(run_page.clone(), KVM_RUN_MMIO_DATA_OFFSET, &(data as u64));
            }
            _ => {
                panic!("mmio_store");
            }
        }

        write_run_val::<u8>(run_page.clone(), KVM_RUN_MMIO_IS_WRITE_OFFSET, &1);
        write_run_val::<usize>(run_page.clone(), KVM_RUN_MMIO_PHYS_ADDR_OFFSET, &fault_addr);
        write_run_val::<u32>(run_page.clone(), KVM_RUN_MMIO_LEN_OFFSET, &(len as u32));

        write_run_val::<u32>(run_page.clone(), KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_MMIO);
        0
    }

    pub fn gstage_page_fault(&self, run_page: Arc<Vmo>, trap: KvmCpuTrap) -> usize {
        let fault_addr = (trap.htval << 2) | (trap.stval & 0x3);

        match trap.scause {
            EXC_LOAD_GUEST_PAGE_FAULT => {
                self.mmio_load(run_page, fault_addr, trap.htinst)
            }
            EXC_STORE_GUEST_PAGE_FAULT => {
                self.mmio_store(run_page, fault_addr, trap.htinst)
            }
            EXC_INST_GUEST_PAGE_FAULT => {
                error!("====gstage_page_fault, scause: {}, addr: {:x}", trap.scause, fault_addr);
                error!("====gstage_page_fault, gatp: {:x}, addr: {:x}", self.arch.lock().get_gatp(), fault_addr);
                loop{}
                1
            }
            _ => {
                panic!("gstage_page_fault, scause: {}, addr: {}", trap.scause, fault_addr);
            }
        }
    }

    pub fn exit(&self, run_page: Arc<Vmo>, trap: KvmCpuTrap) -> usize {
        /* If we got host interrupt then do nothing */
	    if trap.scause & 0x1 << 63 != 0 {
            return 1;
        }
        write_run_val(run_page.clone(), KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_UNKNOWN);
        
        match trap.scause {
            EXC_VIRTUAL_INST_FAULT => {
                if self.arch.lock().guest_context.hstatus & HSTATUS_SPV != 0 {
                    self.virtual_insn(&trap)
                } else {
                    panic!("exit 1");
                }
            }
            EXC_INST_GUEST_PAGE_FAULT | EXC_LOAD_GUEST_PAGE_FAULT | EXC_STORE_GUEST_PAGE_FAULT => {
                if self.arch.lock().guest_context.hstatus & HSTATUS_SPV != 0 {
                    self.gstage_page_fault(run_page, trap)
                } else {
                    panic!("exit 2");
                }
            }
            EXC_SUPERVISOR_SYSCALL => {
                if self.arch.lock().guest_context.hstatus & HSTATUS_SPV != 0 {
                    self.arch.lock().sbi_call()
                } else {
                    panic!("exit 3");
                }
            }
            //EXC_BREAKPOINT => {
            //    write_run_val(run_page, KVM_RUN_EXIT_REASON_OFFSET, &KVM_EXIT_DEBUG);
            //    0
            //}
            _ => {
                panic!("exit, unknown: {}", trap.scause)
            }
        }
    }

    pub fn mmio_return(&self, run_page: Arc<Vmo>) -> Result<usize> {
        if self.arch.lock().mmio_decode.return_handled == 1 {
            return Ok(0);
        }
        self.arch.lock().mmio_decode.return_handled = 1;
        if read_run_val::<u8>(run_page.clone(), KVM_RUN_MMIO_IS_WRITE_OFFSET)? != 1 {
            let insn = self.arch.lock().mmio_decode.insn;
            let len = self.arch.lock().mmio_decode.len;
            let shift = self.arch.lock().mmio_decode.shift;
            let index = (insn >> 7) & 0x1F;
            match len {
                1 => {
                    let data = read_run_val::<u8>(run_page.clone(), KVM_RUN_MMIO_DATA_OFFSET)?;
                    let wdata = ((((data as usize) << shift) as isize) >> shift) as usize;
                    self.arch.lock().guest_context.set_reg(index, wdata);
                }
                2 => {
                    let data = read_run_val::<u16>(run_page.clone(), KVM_RUN_MMIO_DATA_OFFSET)?;
                    let wdata = ((((data as usize) << shift) as isize) >> shift) as usize;
                    self.arch.lock().guest_context.set_reg(index, wdata);
                }
                4 => {
                    let data = read_run_val::<u32>(run_page.clone(), KVM_RUN_MMIO_DATA_OFFSET)?;
                    let wdata = ((((data as usize) << shift) as isize) >> shift) as usize;
                    self.arch.lock().guest_context.set_reg(index, wdata);
                }
                8 => {
                    let data = read_run_val::<u64>(run_page.clone(), KVM_RUN_MMIO_DATA_OFFSET)?;
                    let wdata = ((((data as usize) << shift) as isize) >> shift) as usize;
                    self.arch.lock().guest_context.set_reg(index, wdata);
                }
                _ => {
                    panic!("mmio return, unknown len: {}!", len);
                }
            }

        }
        let insn_len = self.arch.lock().mmio_decode.insn_len as usize;
        self.arch.lock().guest_context.sepc += insn_len;
        Ok(0)
    }

    pub fn flush_interrupts(&self) {
        let mask = self.arch.lock().irqs_pending_mask;
        if mask != 0 {
            self.arch.lock().irqs_pending_mask = 0;
            let val = self.arch.lock().irqs_pending & mask;

            self.arch.lock().guest_csr.hvip &= !mask as usize;
            self.arch.lock().guest_csr.hvip |= val as usize;
        }
    }

    pub fn enter_exit(&self) -> KvmCpuTrap {
        self.arch.lock().swap_in_guest_state();

        let trap = self.arch.lock().enter_exit();

        self.arch.lock().swap_in_host_state();

        trap
    }
}

impl Drop for Vcpu {
    fn drop(&mut self) {
        debug!("hypervisor: release VCPU {}.", self.id);
    }
}

    fn read_run_val<T: Pod>(run_page:Arc<Vmo>, offset: usize) -> Result<T> {
        let mut value = T::new_zeroed();
        let mut writer = VmWriter::from(value.as_mut_bytes()).to_fallible();
        run_page.read(offset, &mut writer)?;
        Ok(value)
    }

    fn write_run_val<T: Pod>(run_page:Arc<Vmo>, offset: usize, value: &T) -> Result<()> {
        let mut reader = VmReader::from(value.as_bytes()).to_fallible();
        run_page.write(offset, &mut reader)
    }

    fn read_run_bytes(run_page:Arc<Vmo>, offset: usize, buffer: &mut [u8]) -> Result<()> {
        let mut writer = VmWriter::from(buffer).to_fallible();
        run_page.read(offset, &mut writer)
    }

    fn write_run_bytes(run_page:Arc<Vmo>, offset: usize, buffer: &[u8]) -> Result<()> {
        let mut reader = VmReader::from(buffer).to_fallible();
        run_page.write(offset, &mut reader)
    }
