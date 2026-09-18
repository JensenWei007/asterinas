use alloc::vec::Vec;

///1

use crate::sync::SpinLock;
use super::onereg::*;
use crate::arch::{cpu::extension::{IsaExtensions, has_extensions}, vm::context::VcpuContext};
use crate::arch::mm::cmo_block_size;
use crate::arch::vm::types::*;
use crate::arch::vm::timer::*;
use crate::arch::timer::get_timebase_freq;
use crate::arch::vm::sbi::*;
use crate::arch::vm::isa::*;
use crate::arch::vm::csr::*;

/// 1
pub struct VcpuArch {
    /// VCPU ran at least once
    ran_atleast_once: bool,
    /// Last Host CPU on which Guest VCPU exited
    last_exit_cpu: i32,
    /// ISA feature bits (similar to MISA)
    isa: [u64; 2],
    /// Vendor, Arch, and Implementation details
	mvendorid: usize,
	marchid: usize,
	mimpid: usize,

	/// SSCRATCH, STVEC, and SCOUNTEREN of Host
	host_sscratch: usize,
	host_stvec: usize,
	host_scounteren: usize,
	host_senvcfg: usize,
	host_sstateen0: usize,

    /// CPU context of Host
	host_context: VcpuContext,

	/// CPU context of Guest VCPU
	guest_context: VcpuContext,

    /// CPU CSR context of Guest VCPU
	guest_csr: KvmVcpuCsr,

	/// CPU Smstateen CSR context of Guest VCPU, TODO
	// struct kvm_vcpu_smstateen_csr smstateen_csr;

	/// CPU reset state of Guest VCPU
	reset_state: SpinLock<KvmVcpuResetState>,

    /// VCPU Timer
	timer: VcpuTimer,

    /// SBI context
	sbi_context: VcpuSbiContext,

    /// VCPU power state
	mp_state: SpinLock<MpState>,

    /// 'static' configurations which are set only once
	cfg: KvmVcpuConfig,
}

impl VcpuArch {
    /// 1
    pub fn new() -> Self {
        Self {
            ran_atleast_once: false,
            last_exit_cpu: 0,
            isa: [0,0],
            mvendorid: 0,
            marchid: 0,
            mimpid: 0,
            host_sscratch: 0,
            host_stvec: 0,
            host_scounteren: 0,
            host_senvcfg: 0,
            host_sstateen0: 0,
            host_context: VcpuContext::default(),
            guest_context: VcpuContext::default(),
            guest_csr: KvmVcpuCsr::default(),
            reset_state: SpinLock::new(KvmVcpuResetState::default()),
            timer: VcpuTimer::default(),
            sbi_context: VcpuSbiContext::new(),
            mp_state: SpinLock::new(MpState::default()),
            cfg: KvmVcpuConfig::default(),
        }
    }

    /// 1
    pub fn init(&mut self) {
        // Setup VCPU config
        self.cfg.hedeleg = KVM_HEDELEG_DEFAULT;
        self.cfg.hideleg = KVM_HIDELEG_DEFAULT;

        // Setup ISA features available to VCPU
        kvm_riscv_vcpu_setup_isa(&mut self.isa);

        // Setup vendor, arch, and implementation details
	    self.mvendorid = sbi_rt::get_mvendorid();
	    self.marchid = sbi_rt::get_marchid();
	    self.mimpid = sbi_rt::get_mimpid();

        // Setup VCPU timer
        self.timer.init();

        // Setup SBI extensions
	    // NOTE: This must be the last thing to be initialized.
        self.sbi_context.init();

        // Reset VCPU
        self.reset(false);
    }

    /// 2
    pub fn num_regs(&self) -> u64 {
        let mut res = 0;

        res+=self.num_config_regs();
        res+=self.num_core_regs();
        res+=self.num_csr_regs();
        res+=self.num_sbi_regs();

        res as u64
    }

    /// 3
    pub fn copy_reg_vec(&self, mut vec: Option<&mut Vec<u64>>) {
        self.copy_config_reg_vec(vec.as_deref_mut());
        self.copy_core_reg_vec(vec.as_deref_mut());
        self.copy_csr_reg_vec(vec.as_deref_mut());
        self.copy_sbi_reg_vec(vec.as_deref_mut());
    }

    /// 4
    pub fn set_mp_state(&self, mp_state: MpState) {
        *self.mp_state.lock() = mp_state;
    }

    /// 1
    pub fn reset(&mut self, _kvm_sbi_reset: bool) {
        self.last_exit_cpu = -1;

        self.context_reset();
        self.timer_reset();
        self.sbi_reset();
    }

    /// 1
    pub fn context_reset(&mut self) {
        self.guest_context = VcpuContext::default();
        self.guest_csr = KvmVcpuCsr::default();

        // Setup reset state of shadow SSTATUS and HSTATUS CSRs
	    self.guest_context.sstatus = SR_SPP | SR_SPIE;

	    self.guest_context.hstatus |= HSTATUS_VTW;
	    self.guest_context.hstatus |= HSTATUS_SPVP;
	    self.guest_context.hstatus |= HSTATUS_SPV;
    }

    /// 1
    pub fn timer_reset(&mut self) {
        self.timer.next_cycles = u64::MAX;
        self.timer.next_set = false;
    }

    /// 1
    pub fn sbi_reset(&mut self) {
        for i in 0..15 {
            let entry = &SBI_EXT_TABLE[i];
            if let Some(func) = entry.ext.reset {
                if self.sbi_context.ext_status[entry.ext_idx as usize] != KvmRiscvSbiExtStatus::KVM_RISCV_SBI_EXT_STATUS_ENABLED {
                    func(&mut self.sbi_context);
                }
            }
        }
    }
}

impl VcpuArch {
    /// 1
    pub fn get_one_reg(&self, reg: OneReg) -> usize {
        match reg.id & KVM_REG_RISCV_TYPE_MASK {
            KVM_REG_RISCV_CONFIG => {
                self.get_config(reg)
            }
            KVM_REG_RISCV_CORE => {
                self.get_core(reg)
            }
            KVM_REG_RISCV_CSR => {
                self.get_csr(reg)
            }
            KVM_REG_RISCV_TIMER => {
                self.get_timer(reg)
            }
            KVM_REG_RISCV_FP_F => {
                panic!("get_fp_f")
            }
            KVM_REG_RISCV_FP_D => {
                panic!("get_fp_D")
            }
            KVM_REG_RISCV_ISA_EXT => {
                panic!("get_isa_ext")
            }
            KVM_REG_RISCV_SBI_EXT => {
                panic!("get_sbi_ext")
            }
            KVM_REG_RISCV_VECTOR => {
                panic!("get_vector")
            }
            KVM_REG_RISCV_SBI_STATE => {
                panic!("get_sbi_state")
            }
            _ => {
                crate::error!("unknown onereg id!");
                0
            }
        }
    }

    fn get_config(&self, reg: OneReg)->usize{
        match reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_CONFIG) {
            KvmConfig::ISA => {
                (KVM_RISCV_BASE_ISA_MASK & self.isa[0]) as usize
            }
            KvmConfig::ZICBOM_BLOCK_SIZE => {
                if has_extensions(IsaExtensions::ZICBOM) 
                    {cmo_block_size()}
                else 
                    {0}
            }
            KvmConfig::MVENDORID => {
                self.mvendorid
            }
            KvmConfig::MARCHID => {
                self.marchid
            }
            KvmConfig::MIMPID => {
                self.mimpid
            }
            KvmConfig::ZICBOZ_BLOCK_SIZE => {
                if has_extensions(IsaExtensions::ZICBOZ) 
                    {0}// TODO: add this isa support
                else 
                    {0}
            }
            KvmConfig::SATP_MODE => {
                (SATP_MODE_39 >> SATP_MODE_SHIFT) as usize
            }
            KvmConfig::ZICBOP_BLOCK_SIZE => {
                if has_extensions(IsaExtensions::ZICBOP) 
                    {0}// TODO: add this isa support
                else 
                    {0}
            }
            _ => {
                panic!("unknown config reg id!");
            }
        }
    }

    fn get_core(&self, reg: OneReg)->usize{
        let reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_CORE);
        match reg_num {
            KvmCore::PC => {
                self.guest_context.sepc
            }
            KvmCore::MODE => {
                (self.guest_context.sstatus & SR_SPP as usize == 1) as usize
            }
            KvmCore::PC..KvmCore::T6 => {
                self.guest_context.get_reg(reg_num).unwrap()
            }
            _ => {
                panic!("unknown core reg id!");
            }
        }
    }

    fn get_csr(&self, reg: OneReg) -> usize{
        let mut reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_CSR);
        let reg_subtype = KVM_REG_RISCV_SUBTYPE_MASK & reg_num;
        reg_num &= !(KVM_REG_RISCV_SUBTYPE_MASK);
        match reg_subtype {
            KVM_REG_RISCV_CSR_GENERAL => {
                self.get_general_csr(reg_num)
            }
            KVM_REG_RISCV_CSR_AIA => {
                panic!("SSAIA isa support is disabled!");
            }
            KVM_REG_RISCV_CSR_SMSTATEEN => {
                panic!("SMSTATEEN isa support is disabled!");
            }
            _ => {
                panic!("unknown csr reg id!");
            }
        }
    }

    fn get_general_csr(&self, reg_num: u64) -> usize {
        match reg_num {
            KvmVcpuCsr::SIP => {
                ((self.guest_csr.hvip >> VSIP_TO_HVIP_SHIFT) & VSIP_VALID_MASK as usize) & IRQ_LOCAL_MASK as usize
            }
            _ => {
                self.guest_csr.get_csr(reg_num).unwrap()
            }
        }
    }

    fn get_timer(&self, reg: OneReg) -> usize {
        let reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_TIMER);
        match reg_num {
            KvmRiscvTimer::FREQUENCY => {
                get_timebase_freq() as usize
            }
            KvmRiscvTimer::TIME => {
                panic!("KvmRiscvTimer::TIME is disabled!");
            }
            KvmRiscvTimer::COMPARE => {
                panic!("KvmRiscvTimer::COMP is disabled!");
            }
            KvmRiscvTimer::STATE => {
                panic!("KvmRiscvTimer::STATE is disabled!");
            }
            _ => {
                panic!("unknown timer reg id!");
            }
        }
    }
}


impl VcpuArch {
    /// 1
    pub fn set_one_reg(&mut self, reg: OneReg, reg_val: usize) {
        match reg.id & KVM_REG_RISCV_TYPE_MASK {
            KVM_REG_RISCV_CONFIG => {
                panic!("set_config")
            }
            KVM_REG_RISCV_CORE => {
                self.set_core(reg, reg_val);
            }
            KVM_REG_RISCV_CSR => {
                self.set_csr(reg, reg_val);
            }
            KVM_REG_RISCV_TIMER => {
                panic!("set_timer")
            }
            KVM_REG_RISCV_FP_F => {
                panic!("set_fp_f")
            }
            KVM_REG_RISCV_FP_D => {
                panic!("set_fp_d")
            }
            KVM_REG_RISCV_ISA_EXT => {
                panic!("set_isa_ext")
            }
            KVM_REG_RISCV_SBI_EXT => {
                panic!("set_sbi_ext")
            }
            KVM_REG_RISCV_VECTOR => {
                panic!("set_vector")
            }
            KVM_REG_RISCV_SBI_STATE => {
                panic!("set_sbi_state")
            }
            _ => {
                crate::error!("unknown onereg id!");
            }
        }
    }

    fn set_core(&mut self, reg: OneReg, reg_val: usize){
        let reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_CORE);
        match reg_num {
            KvmCore::PC => {
                self.guest_context.sepc = reg_val;
            }
            KvmCore::MODE => {
                if reg_val == 1 {
                    self.guest_context.sstatus |= SR_SPP as usize;
                } else {
                    self.guest_context.sstatus &= SR_SPP as usize;
                }
            }
            KvmCore::PC..KvmCore::T6 => {
                self.guest_context.set_reg(reg_num as usize, reg_val);
            }
            _ => {
                panic!("unknown core reg id!");
            }
        }
        
    }

    fn set_csr(&mut self, reg: OneReg, reg_val: usize){
        let mut reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_CSR);
        let reg_subtype = KVM_REG_RISCV_SUBTYPE_MASK & reg_num;
        reg_num &= !(KVM_REG_RISCV_SUBTYPE_MASK);
        match reg_subtype {
            KVM_REG_RISCV_CSR_GENERAL => {
                self.set_general_csr(reg_num, reg_val)
            }
            KVM_REG_RISCV_CSR_AIA => {
                panic!("SSAIA isa support is disabled!");
            }
            KVM_REG_RISCV_CSR_SMSTATEEN => {
                panic!("SMSTATEEN isa support is disabled!");
            }
            _ => {
                panic!("unknown csr reg id!");
            }
        }
    }

    fn set_general_csr(&mut self, reg_num: u64, reg_val: usize) {
        match reg_num {
            KvmVcpuCsr::SIP => {
                let reg_val = (reg_val & VSIP_VALID_MASK as usize) << VSIP_TO_HVIP_SHIFT;
                self.guest_csr.set_csr(reg_num as usize, reg_val);
            }
            _ => {
                self.guest_csr.set_csr(reg_num as usize, reg_val);
            }
        }
    }
}

impl VcpuArch {
    fn copy_config_reg_vec(&self, mut vec: Option<&mut Vec<u64>>) -> usize {
        let mut n = 0;
        for i in 0..(size_of::<KvmConfig>()/ size_of::<usize>()) {
            let reg = KVM_REG_RISCV as usize 
                            | KVM_REG_SIZE_U64 as usize 
                            | KVM_REG_RISCV_CONFIG as usize
                            | i;
            if let Some(vec) = vec.as_deref_mut() {
                vec.push(reg as u64);
            }
            n+=1;
        }
        n
    }

    fn num_config_regs(&self) -> usize {
        self.copy_config_reg_vec(None)
    }

    fn copy_core_reg_vec(&self, mut vec: Option<&mut Vec<u64>>){
        let n = self.num_core_regs();
        for i in 0..n {
            let reg = KVM_REG_RISCV as usize 
                            | KVM_REG_SIZE_U64 as usize 
                            | KVM_REG_RISCV_CORE as usize
                            | i;
            if let Some(vec) = vec.as_deref_mut() {
                vec.push(reg as u64);
            }
        }
    }

    fn num_core_regs(&self) -> usize {
        size_of::<KvmCore>()/ size_of::<usize>()
    }

    fn copy_csr_reg_vec(&self, mut vec: Option<&mut Vec<u64>>){
        let n = self.num_csr_regs();
        for i in 0..n {
            let reg = KVM_REG_RISCV as usize 
                            | KVM_REG_SIZE_U64 as usize 
                            | KVM_REG_RISCV_CSR as usize
                            | i;
            if let Some(vec) = vec.as_deref_mut() {
                vec.push(reg as u64);
            }
        }
    }

    fn num_csr_regs(&self) -> usize {
        // TODO: add vcpu SSAIA, SMSTATEEN isa ext
        size_of::<KvmVcpuCsr>()/ size_of::<usize>()
    }

    fn copy_sbi_reg_vec(&self, mut vec: Option<&mut Vec<u64>>) -> usize{
        let mut n = 0;

        // copy fp.d.f regs
        for i in 0..15 {
            let entry = &SBI_EXT_TABLE[i];

            if entry.ext.get_state_reg_count.is_none() 
            || self.sbi_context.ext_status[entry.ext_idx as usize] != KvmRiscvSbiExtStatus::KVM_RISCV_SBI_EXT_STATUS_ENABLED {
                continue;
            }

            if let Some(func) = entry.ext.get_state_reg_count {
                let state_reg_count = func();

                for j in 0..state_reg_count {
                    if let Some(vec) = vec.as_deref_mut() {
                        if let Some(func) = entry.ext.get_state_reg_id {
                            vec.push(func(j) as u64);
                        } else {
                            let reg = KVM_REG_RISCV as usize 
                            | KVM_REG_SIZE_U64 as usize 
                            | KVM_REG_RISCV_SBI_STATE as usize
                            | entry.ext.state_reg_subtype
                            | j;
                            vec.push(reg as u64);
                        }
                    }
                }
                n+=state_reg_count;
            }
        }

        n
    }

    fn num_sbi_regs(&self) -> usize {
        self.copy_sbi_reg_vec(None)
    }
}
