use alloc::vec::Vec;
use core::{arch::global_asm, u32};
use core::mem::offset_of;

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
use crate::arch::vm::insn::*;
use crate::arch::cpu::context::*;

/// 1
#[repr(C)]
pub struct VcpuArch {
    /// VCPU ran at least once
    pub ran_atleast_once: bool,
    /// Last Host CPU on which Guest VCPU exited
    last_exit_cpu: i32,
    /// ISA feature bits (similar to MISA)
    isa: [u64; 2],
    /// Vendor, Arch, and Implementation details
	pub mvendorid: usize,
	pub marchid: usize,
	pub mimpid: usize,

	/// SSCRATCH, STVEC, and SCOUNTEREN of Host
	pub host_sscratch: usize,
	host_stvec: usize,
	pub host_scounteren: usize,
	pub host_senvcfg: usize,
	host_sstateen0: usize,

    /// CPU context of Host
	host_context: VcpuContext,

	/// CPU context of Guest VCPU
	pub guest_context: VcpuContext,

    /// CPU CSR context of Guest VCPU
	pub guest_csr: KvmVcpuCsr,

	/// CPU Smstateen CSR context of Guest VCPU, TODO
	// struct kvm_vcpu_smstateen_csr smstateen_csr;

	/// CPU reset state of Guest VCPU
	reset_state: SpinLock<KvmVcpuResetState>,

    /*
	 * VCPU interrupts
	 *
	 * The irqs_pending bitmap represents pending interrupts whereas
	 * irqs_pending_mask represents bits changed in irqs_pending. Updates
	 * to these bitmaps are serialized so vcpu interrupt sync/flush cannot
	 * drop a newly injected interrupt while syncing guest-visible HVIP.
	 */
    pub irqs_pending: u64,
    pub irqs_pending_mask: u64,

    /// VCPU Timer
	pub timer: VcpuTimer,

    /// MMIO instruction details
	pub mmio_decode: KvmMmioDecode,

    /// SBI context
	sbi_context: VcpuSbiContext,

    /// VCPU power state
	mp_state: SpinLock<MpState>,

    /// 'static' configurations which are set only once
	cfg: KvmVcpuConfig,
}

/// 1
pub struct KvmCpuTrap {
	pub sepc: usize,
	pub scause: usize,
	pub stval: usize,
	pub htval: usize,
	pub htinst: usize,
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
            irqs_pending: 0,
            irqs_pending_mask: 0,
            timer: VcpuTimer::default(),
            mmio_decode: KvmMmioDecode::default(),
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
        res+=self.num_fp_f_regs();
        res+=self.num_fp_d_regs();
        res+=self.num_sbi_regs();

        res as u64
    }

    /// 3
    pub fn copy_reg_vec(&self, mut vec: Option<&mut Vec<u64>>) {
        self.copy_config_reg_vec(vec.as_deref_mut());
        self.copy_core_reg_vec(vec.as_deref_mut());
        self.copy_csr_reg_vec(vec.as_deref_mut());
        self.copy_fp_f_reg_vec(vec.as_deref_mut());
        self.copy_fp_d_reg_vec(vec.as_deref_mut());
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

    ///1
    pub fn config_ran_once(&mut self) {
        // SSTC
        self.cfg.henvcfg |= ENVCFG_STCE;
        // ZICBOM
        self.cfg.henvcfg |= ENVCFG_CBIE | ENVCFG_CBCFE;
        // ZICBOZ
        self.cfg.henvcfg |= ENVCFG_CBZE;
        // SVADU & SVADE
        self.cfg.henvcfg |= ENVCFG_ADUE;
        // guest_debug
        // self.cfg.hedeleg &= !(1<<3);
    }

    /// 1
    pub fn update_hvip(&self) {
        let hvip = self.guest_csr.hvip;
        unsafe {
            Hvip::write(hvip);
        }
    }

    /// 1
    pub fn swap_in_guest_state(&mut self){
        let scounteren = self.guest_csr.scounteren;
        self.host_scounteren = riscv::register::scounteren::read().bits();
        unsafe {
            riscv::register::scounteren::write(riscv::register::scounteren::Scounteren::from_bits(scounteren));
        }
        let senvcfg = self.guest_csr.senvcfg;
        self.host_senvcfg = riscv::register::senvcfg::read().bits();
        unsafe {
            riscv::register::senvcfg::write(riscv::register::senvcfg::Senvcfg::from_bits(senvcfg));
        }
    }

    /// 1
    pub fn swap_in_host_state(&mut self) {
        let scounteren = self.host_scounteren;
        self.guest_csr.scounteren = riscv::register::scounteren::read().bits();
        unsafe {
            riscv::register::scounteren::write(riscv::register::scounteren::Scounteren::from_bits(scounteren));
        }
        let senvcfg = self.host_senvcfg;
        self.guest_csr.senvcfg = riscv::register::senvcfg::read().bits();
        unsafe {
            riscv::register::senvcfg::write(riscv::register::senvcfg::Senvcfg::from_bits(senvcfg));
        }
    }

    /// 1
    pub fn enter_exit(&mut self) -> KvmCpuTrap {
        unsafe {
            self.host_context.hstatus = Hstatus::swap(self.guest_context.hstatus);

            __kvm_riscv_switch_to(self as *mut _);

            self.guest_context.hstatus = Hstatus::swap(self.host_context.hstatus);

            KvmCpuTrap {
                sepc: self.guest_context.sepc,
                scause: riscv::register::scause::read().bits(),
                stval: riscv::register::stval::read(),
                htval: Htval::read(),
                htinst: Htinst::read(),
            }
        }
    }

    /// 1
    pub fn sync_interrupts(&mut self) {
        unsafe {
            self.guest_csr.vsie = Vsie::read();

            let hvip = Hvip::read();

            if (self.guest_csr.hvip ^ hvip) & (0x1 << 2) != 0 {
                if hvip & (0x1 << 2) != 0 {
                    let should_set = (self.irqs_pending_mask & 0x1 << 2) == 0; 
                    self.irqs_pending_mask |= 0x1 << 2;
                    if should_set {
                        self.irqs_pending |= 0x1 << 2;
                    }
                } else {
                    let should_clear = (self.irqs_pending_mask & 0x1 << 2) == 0; 
                    self.irqs_pending_mask |= 0x1 << 2;
                    if should_clear {
                        self.irqs_pending &= !(0x1 << 2);
                    }
                }
            }
            
            self.timer.sync();
        }
    }

    pub fn system_opcode_insn(&mut self, insn: usize) -> usize {
        let mut ret = 0;
        for i in 0..8 {
            let func = &SYSTEM_OPCODE_FUNCS[i];
            if insn & func.mask == func.should_match {
                let fun = func.func;
                ret = fun(insn);
            }
        }

        match ret {
            1 => {
                self.guest_context.sepc += insn_len(insn);
            }
            _ => {
                panic!("system_opcode_insn");
            }
        }
        ret
    }

    pub fn sbi_call(&mut self) -> usize {
        let index = vcpu_get_sbi_ext_idx(self.guest_context.a7);
        if index !=100 {
            let ext = &SBI_EXT_TABLE[index];
            let func = ext.ext.handler;
        
            let rt = func(self);

            if !rt.uexit {
                self.guest_context.a0 = rt.err_val;
            }

            self.guest_context.sepc += 4;
            self.guest_context.a1 = rt.out_val;
        } else {
            self.guest_context.a0 = usize::MAX - 1;
            self.guest_context.sepc += 4;
            self.guest_context.a1 = 0;
        }
        1
    }

    pub fn config_load(&self) {
        unsafe {
            Hedeleg::write(self.cfg.hedeleg);
            Hideleg::write(self.cfg.hideleg);
            Henvcfg::write(self.cfg.henvcfg);
        }
    }

    pub fn update_hgatp(&self, pgd_paddr: u64) {
        let mut hgatp = 0x9 << 60;
        hgatp |= pgd_paddr >> 12 & (0xFFFFFFFFFFF);
        unsafe {
            Hgatp::write(hgatp as usize);
        }
    }

    fn timer_restore(&mut self) {
        unsafe {
            Vstimecmp::write(self.timer.next_cycles as usize);
            self.timer.next_set = false;
        }
    }

    pub fn vcpu_load(&mut self, pgd_paddr: u64) {
        self.config_load();

        unsafe {
            Vsstatus::write(self.guest_csr.vsstatus);
            Vsie::write(self.guest_csr.vsie);
            Vstvec::write(self.guest_csr.vstvec);
            Vsscratch::write(self.guest_csr.vsscratch);
            Vsepc::write(self.guest_csr.vsepc);
            Vscause::write(self.guest_csr.vscause);
            Vstval::write(self.guest_csr.vstval);
            Hvip::write(self.guest_csr.hvip);
            Vsatp::write(self.guest_csr.vsatp);
        }

        self.update_hgatp(pgd_paddr);

        self.timer_restore();

        // TODO: add fpu
    }

    fn timer_save(&mut self) {
        unsafe {
            Vstimecmp::write(usize::MAX);
        }
    }

    pub fn vcpu_put(&mut self) {
        self.timer_save();

        unsafe {
            self.guest_csr.vsstatus = Vsstatus::read();
            self.guest_csr.vsie = Vsie::read();
            self.guest_csr.vstvec = Vstvec::read();
            self.guest_csr.vsscratch = Vsscratch::read();
            self.guest_csr.vsepc = Vsepc::read();
            self.guest_csr.vscause = Vscause::read();
            self.guest_csr.vstval = Vstval::read();
            self.guest_csr.hvip = Hvip::read();
            self.guest_csr.vsatp = Vsatp::read();
        }
    }

    pub fn set_interrupt(&mut self, irq: u32) {
        if irq == u32::MAX {
            self.irqs_pending |= 0x1<<10;
        } else {
            self.irqs_pending &= !(0x1<<10);
        }
        self.irqs_pending_mask |= 0x1 <<10;
    }

    pub fn get_gatp(&self) -> usize {
        unsafe{Hgatp::read()}
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
                self.get_fp_f(reg)
            }
            KVM_REG_RISCV_FP_D => {
                self.get_fp_d(reg)
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
            KvmCore::PC..=KvmCore::T6 => {
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

    fn get_fp_f(&self, reg: OneReg) -> usize {
        let reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_FP_F);
        match reg_num {
            32 => {
                self.guest_context.fp_fctx.get_fcsr() as usize
            }
            0..32 => {
                self.guest_context.fp_fctx.get_f(reg_num as usize) as usize
            }
            _ => {
                panic!("unknown fp_f reg id!");
            }
        }
    }

    fn get_fp_d(&self, reg: OneReg) -> usize {
        let reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_FP_D);
        match reg_num {
            32 => {
                self.guest_context.fp_dctx.get_fcsr() as usize
            }
            0..32 => {
                self.guest_context.fp_dctx.get_f(reg_num as usize) as usize
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
                self.set_fp_f(reg, reg_val);
            }
            KVM_REG_RISCV_FP_D => {
                self.set_fp_d(reg, reg_val);
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
            KvmCore::PC..=KvmCore::T6 => {
                self.guest_context.set_reg(reg_num as usize, reg_val);
            }
            _ => {
                panic!("unknown core reg id: {:x}, {:x}", reg.id, reg_num);
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

    fn set_fp_f(&mut self, reg: OneReg, reg_val: usize) {
        let reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_FP_F);
        match reg_num {
            32 => {
                self.guest_context.fp_fctx.set_fcsr(reg_val as u32);
            }
            0..32 => {
                self.guest_context.fp_fctx.set_f(reg_num as usize, reg_val as u32);
            }
            _ => {
                panic!("unknown fp_f reg id: {}", reg_num);
            }
        }
    }

    fn set_fp_d(&mut self, reg: OneReg, reg_val: usize) {
        let reg_num = reg.id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_FP_D);
        match reg_num {
            32 => {
                self.guest_context.fp_dctx.set_fcsr(reg_val as u32);
            }
            0..32 => {
                self.guest_context.fp_dctx.set_f(reg_num as usize, reg_val as u64);
            }
            _ => {
                panic!("unknown fp_d reg id: {}", reg_num);
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

    fn copy_fp_f_reg_vec(&self, mut vec: Option<&mut Vec<u64>>){
        let n = self.num_fp_f_regs();
        for i in 0..n {
            let reg = KVM_REG_RISCV as usize 
                            | KVM_REG_SIZE_U32 as usize 
                            | KVM_REG_RISCV_FP_F as usize
                            | i;
            if let Some(vec) = vec.as_deref_mut() {
                vec.push(reg as u64);
            }
        }
    }

    fn num_fp_f_regs(&self) -> usize {
        size_of::<FFpuContext>()/ size_of::<u32>()
    }

    fn copy_fp_d_reg_vec(&self, mut vec: Option<&mut Vec<u64>>){
        let n = self.num_fp_d_regs();
        for i in 0..n-1 {
            let reg = KVM_REG_RISCV as usize 
                            | KVM_REG_SIZE_U64 as usize 
                            | KVM_REG_RISCV_FP_D as usize
                            | i;
            if let Some(vec) = vec.as_deref_mut() {
                vec.push(reg as u64);
            }
        }

        let reg = KVM_REG_RISCV as usize 
                    | KVM_REG_SIZE_U64 as usize 
                    | KVM_REG_RISCV_FP_D as usize
                    | 32;
        if let Some(vec) = vec.as_deref_mut() {
            vec.push(reg as u64);
        }
    }

    fn num_fp_d_regs(&self) -> usize {
        33
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

const KVM_ARCH_HOST_RA: usize = offset_of!(VcpuArch, host_context.ra);
const KVM_ARCH_HOST_SP: usize = offset_of!(VcpuArch, host_context.sp);
const KVM_ARCH_HOST_GP: usize = offset_of!(VcpuArch, host_context.gp);
const KVM_ARCH_HOST_TP: usize = offset_of!(VcpuArch, host_context.tp);
const KVM_ARCH_HOST_S0: usize = offset_of!(VcpuArch, host_context.s0);
const KVM_ARCH_HOST_S1: usize = offset_of!(VcpuArch, host_context.s1);
const KVM_ARCH_HOST_A1: usize = offset_of!(VcpuArch, host_context.a1);
const KVM_ARCH_HOST_A2: usize = offset_of!(VcpuArch, host_context.a2);
const KVM_ARCH_HOST_A3: usize = offset_of!(VcpuArch, host_context.a3);
const KVM_ARCH_HOST_A4: usize = offset_of!(VcpuArch, host_context.a4);
const KVM_ARCH_HOST_A5: usize = offset_of!(VcpuArch, host_context.a5);
const KVM_ARCH_HOST_A6: usize = offset_of!(VcpuArch, host_context.a6);
const KVM_ARCH_HOST_A7: usize = offset_of!(VcpuArch, host_context.a7);
const KVM_ARCH_HOST_S2: usize = offset_of!(VcpuArch, host_context.s2);
const KVM_ARCH_HOST_S3: usize = offset_of!(VcpuArch, host_context.s3);
const KVM_ARCH_HOST_S4: usize = offset_of!(VcpuArch, host_context.s4);
const KVM_ARCH_HOST_S5: usize = offset_of!(VcpuArch, host_context.s5);
const KVM_ARCH_HOST_S6: usize = offset_of!(VcpuArch, host_context.s6);
const KVM_ARCH_HOST_S7: usize = offset_of!(VcpuArch, host_context.s7);
const KVM_ARCH_HOST_S8: usize = offset_of!(VcpuArch, host_context.s8);
const KVM_ARCH_HOST_S9: usize = offset_of!(VcpuArch, host_context.s9);
const KVM_ARCH_HOST_S10: usize = offset_of!(VcpuArch, host_context.s10);
const KVM_ARCH_HOST_S11: usize = offset_of!(VcpuArch, host_context.s11);
const KVM_ARCH_HOST_SSTATUS: usize = offset_of!(VcpuArch, host_context.sstatus);
const KVM_ARCH_HOST_STVEC: usize = offset_of!(VcpuArch, host_stvec);
const KVM_ARCH_HOST_SSCRATCH: usize = offset_of!(VcpuArch, host_sscratch);

global_asm!(
    r#"
    .global KVM_ARCH_HOST_RA
    .set KVM_ARCH_HOST_RA, {KVM_ARCH_HOST_RA}
    .global KVM_ARCH_HOST_SP
    .set KVM_ARCH_HOST_SP, {KVM_ARCH_HOST_SP}
    .global KVM_ARCH_HOST_GP
    .set KVM_ARCH_HOST_GP, {KVM_ARCH_HOST_GP}
    .global KVM_ARCH_HOST_TP
    .set KVM_ARCH_HOST_TP, {KVM_ARCH_HOST_TP}
    .global KVM_ARCH_HOST_S0
    .set KVM_ARCH_HOST_S0, {KVM_ARCH_HOST_S0}
    .global KVM_ARCH_HOST_S1
    .set KVM_ARCH_HOST_S1, {KVM_ARCH_HOST_S1}
    .global KVM_ARCH_HOST_A1
    .set KVM_ARCH_HOST_A1, {KVM_ARCH_HOST_A1}
    .global KVM_ARCH_HOST_A2
    .set KVM_ARCH_HOST_A2, {KVM_ARCH_HOST_A2}
    .global KVM_ARCH_HOST_A3
    .set KVM_ARCH_HOST_A3, {KVM_ARCH_HOST_A3}
    .global KVM_ARCH_HOST_A4
    .set KVM_ARCH_HOST_A4, {KVM_ARCH_HOST_A4}
    .global KVM_ARCH_HOST_A5
    .set KVM_ARCH_HOST_A5, {KVM_ARCH_HOST_A5}
    .global KVM_ARCH_HOST_A6
    .set KVM_ARCH_HOST_A6, {KVM_ARCH_HOST_A6}
    .global KVM_ARCH_HOST_A7
    .set KVM_ARCH_HOST_A7, {KVM_ARCH_HOST_A7}
    .global KVM_ARCH_HOST_S2
    .set KVM_ARCH_HOST_S2, {KVM_ARCH_HOST_S2}
    .global KVM_ARCH_HOST_S3
    .set KVM_ARCH_HOST_S3, {KVM_ARCH_HOST_S3}
    .global KVM_ARCH_HOST_S4
    .set KVM_ARCH_HOST_S4, {KVM_ARCH_HOST_S4}
    .global KVM_ARCH_HOST_S5
    .set KVM_ARCH_HOST_S5, {KVM_ARCH_HOST_S5}
    .global KVM_ARCH_HOST_S6
    .set KVM_ARCH_HOST_S6, {KVM_ARCH_HOST_S6}
    .global KVM_ARCH_HOST_S7
    .set KVM_ARCH_HOST_S7, {KVM_ARCH_HOST_S7}
    .global KVM_ARCH_HOST_S8
    .set KVM_ARCH_HOST_S8, {KVM_ARCH_HOST_S8}
    .global KVM_ARCH_HOST_S9
    .set KVM_ARCH_HOST_S9, {KVM_ARCH_HOST_S9}
    .global KVM_ARCH_HOST_S10
    .set KVM_ARCH_HOST_S10, {KVM_ARCH_HOST_S10}
    .global KVM_ARCH_HOST_S11
    .set KVM_ARCH_HOST_S11, {KVM_ARCH_HOST_S11}
    .global KVM_ARCH_HOST_SSTATUS
    .set KVM_ARCH_HOST_SSTATUS, {KVM_ARCH_HOST_SSTATUS}
    .global KVM_ARCH_HOST_STVEC
    .set KVM_ARCH_HOST_STVEC, {KVM_ARCH_HOST_STVEC}
    .global KVM_ARCH_HOST_SSCRATCH
    .set KVM_ARCH_HOST_SSCRATCH, {KVM_ARCH_HOST_SSCRATCH}
    "#,
    KVM_ARCH_HOST_RA = const KVM_ARCH_HOST_RA,
    KVM_ARCH_HOST_SP = const KVM_ARCH_HOST_SP,
    KVM_ARCH_HOST_GP = const KVM_ARCH_HOST_GP,
    KVM_ARCH_HOST_TP = const KVM_ARCH_HOST_TP,
    KVM_ARCH_HOST_S0 = const KVM_ARCH_HOST_S0,
    KVM_ARCH_HOST_S1 = const KVM_ARCH_HOST_S1,
    KVM_ARCH_HOST_A1 = const KVM_ARCH_HOST_A1,
    KVM_ARCH_HOST_A2 = const KVM_ARCH_HOST_A2,
    KVM_ARCH_HOST_A3 = const KVM_ARCH_HOST_A3,
    KVM_ARCH_HOST_A4 = const KVM_ARCH_HOST_A4,
    KVM_ARCH_HOST_A5 = const KVM_ARCH_HOST_A5,
    KVM_ARCH_HOST_A6 = const KVM_ARCH_HOST_A6,
    KVM_ARCH_HOST_A7 = const KVM_ARCH_HOST_A7,
    KVM_ARCH_HOST_S2 = const KVM_ARCH_HOST_S2,
    KVM_ARCH_HOST_S3 = const KVM_ARCH_HOST_S3,
    KVM_ARCH_HOST_S4 = const KVM_ARCH_HOST_S4,
    KVM_ARCH_HOST_S5 = const KVM_ARCH_HOST_S5,
    KVM_ARCH_HOST_S6 = const KVM_ARCH_HOST_S6,
    KVM_ARCH_HOST_S7 = const KVM_ARCH_HOST_S7,
    KVM_ARCH_HOST_S8 = const KVM_ARCH_HOST_S8,
    KVM_ARCH_HOST_S9 = const KVM_ARCH_HOST_S9,
    KVM_ARCH_HOST_S10 = const KVM_ARCH_HOST_S10,
    KVM_ARCH_HOST_S11 = const KVM_ARCH_HOST_S11,
    KVM_ARCH_HOST_SSTATUS = const KVM_ARCH_HOST_SSTATUS,
    KVM_ARCH_HOST_STVEC = const KVM_ARCH_HOST_STVEC,
    KVM_ARCH_HOST_SSCRATCH = const KVM_ARCH_HOST_SSCRATCH,
);

const KVM_ARCH_GUEST_RA: usize = offset_of!(VcpuArch, guest_context.ra);
const KVM_ARCH_GUEST_SP: usize = offset_of!(VcpuArch, guest_context.sp);
const KVM_ARCH_GUEST_GP: usize = offset_of!(VcpuArch, guest_context.gp);
const KVM_ARCH_GUEST_TP: usize = offset_of!(VcpuArch, guest_context.tp);
const KVM_ARCH_GUEST_T0: usize = offset_of!(VcpuArch, guest_context.t0);
const KVM_ARCH_GUEST_T1: usize = offset_of!(VcpuArch, guest_context.t1);
const KVM_ARCH_GUEST_T2: usize = offset_of!(VcpuArch, guest_context.t2);
const KVM_ARCH_GUEST_S0: usize = offset_of!(VcpuArch, guest_context.s0);
const KVM_ARCH_GUEST_S1: usize = offset_of!(VcpuArch, guest_context.s1);
const KVM_ARCH_GUEST_A0: usize = offset_of!(VcpuArch, guest_context.a0);
const KVM_ARCH_GUEST_A1: usize = offset_of!(VcpuArch, guest_context.a1);
const KVM_ARCH_GUEST_A2: usize = offset_of!(VcpuArch, guest_context.a2);
const KVM_ARCH_GUEST_A3: usize = offset_of!(VcpuArch, guest_context.a3);
const KVM_ARCH_GUEST_A4: usize = offset_of!(VcpuArch, guest_context.a4);
const KVM_ARCH_GUEST_A5: usize = offset_of!(VcpuArch, guest_context.a5);
const KVM_ARCH_GUEST_A6: usize = offset_of!(VcpuArch, guest_context.a6);
const KVM_ARCH_GUEST_A7: usize = offset_of!(VcpuArch, guest_context.a7);
const KVM_ARCH_GUEST_S2: usize = offset_of!(VcpuArch, guest_context.s2);
const KVM_ARCH_GUEST_S3: usize = offset_of!(VcpuArch, guest_context.s3);
const KVM_ARCH_GUEST_S4: usize = offset_of!(VcpuArch, guest_context.s4);
const KVM_ARCH_GUEST_S5: usize = offset_of!(VcpuArch, guest_context.s5);
const KVM_ARCH_GUEST_S6: usize = offset_of!(VcpuArch, guest_context.s6);
const KVM_ARCH_GUEST_S7: usize = offset_of!(VcpuArch, guest_context.s7);
const KVM_ARCH_GUEST_S8: usize = offset_of!(VcpuArch, guest_context.s8);
const KVM_ARCH_GUEST_S9: usize = offset_of!(VcpuArch, guest_context.s9);
const KVM_ARCH_GUEST_S10: usize = offset_of!(VcpuArch, guest_context.s10);
const KVM_ARCH_GUEST_S11: usize = offset_of!(VcpuArch, guest_context.s11);
const KVM_ARCH_GUEST_T3: usize = offset_of!(VcpuArch, guest_context.t3);
const KVM_ARCH_GUEST_T4: usize = offset_of!(VcpuArch, guest_context.t4);
const KVM_ARCH_GUEST_T5: usize = offset_of!(VcpuArch, guest_context.t5);
const KVM_ARCH_GUEST_T6: usize = offset_of!(VcpuArch, guest_context.t6);
const KVM_ARCH_GUEST_SSTATUS: usize = offset_of!(VcpuArch, guest_context.sstatus);
const KVM_ARCH_GUEST_SEPC: usize = offset_of!(VcpuArch, guest_context.sepc);

global_asm!(
    r#"
    .global KVM_ARCH_GUEST_RA
    .set KVM_ARCH_GUEST_RA, {KVM_ARCH_GUEST_RA}
    .global KVM_ARCH_GUEST_SP
    .set KVM_ARCH_GUEST_SP, {KVM_ARCH_GUEST_SP}
    .global KVM_ARCH_GUEST_GP
    .set KVM_ARCH_GUEST_GP, {KVM_ARCH_GUEST_GP}
    .global KVM_ARCH_GUEST_TP
    .set KVM_ARCH_GUEST_TP, {KVM_ARCH_GUEST_TP}
    .global KVM_ARCH_GUEST_T0
    .set KVM_ARCH_GUEST_T0, {KVM_ARCH_GUEST_T0}
    .global KVM_ARCH_GUEST_T1
    .set KVM_ARCH_GUEST_T1, {KVM_ARCH_GUEST_T1}
    .global KVM_ARCH_GUEST_T2
    .set KVM_ARCH_GUEST_T2, {KVM_ARCH_GUEST_T2}
    .global KVM_ARCH_GUEST_S0
    .set KVM_ARCH_GUEST_S0, {KVM_ARCH_GUEST_S0}
    .global KVM_ARCH_GUEST_S1
    .set KVM_ARCH_GUEST_S1, {KVM_ARCH_GUEST_S1}
    .global KVM_ARCH_GUEST_A0
    .set KVM_ARCH_GUEST_A0, {KVM_ARCH_GUEST_A0}
    .global KVM_ARCH_GUEST_A1
    .set KVM_ARCH_GUEST_A1, {KVM_ARCH_GUEST_A1}
    .global KVM_ARCH_GUEST_A2
    .set KVM_ARCH_GUEST_A2, {KVM_ARCH_GUEST_A2}
    .global KVM_ARCH_GUEST_A3
    .set KVM_ARCH_GUEST_A3, {KVM_ARCH_GUEST_A3}
    .global KVM_ARCH_GUEST_A4
    .set KVM_ARCH_GUEST_A4, {KVM_ARCH_GUEST_A4}
    .global KVM_ARCH_GUEST_A5
    .set KVM_ARCH_GUEST_A5, {KVM_ARCH_GUEST_A5}
    .global KVM_ARCH_GUEST_A6
    .set KVM_ARCH_GUEST_A6, {KVM_ARCH_GUEST_A6}
    .global KVM_ARCH_GUEST_A7
    .set KVM_ARCH_GUEST_A7, {KVM_ARCH_GUEST_A7}
    .global KVM_ARCH_GUEST_S2
    .set KVM_ARCH_GUEST_S2, {KVM_ARCH_GUEST_S2}
    .global KVM_ARCH_GUEST_S3
    .set KVM_ARCH_GUEST_S3, {KVM_ARCH_GUEST_S3}
    .global KVM_ARCH_GUEST_S4
    .set KVM_ARCH_GUEST_S4, {KVM_ARCH_GUEST_S4}
    .global KVM_ARCH_GUEST_S5
    .set KVM_ARCH_GUEST_S5, {KVM_ARCH_GUEST_S5}
    .global KVM_ARCH_GUEST_S6
    .set KVM_ARCH_GUEST_S6, {KVM_ARCH_GUEST_S6}
    .global KVM_ARCH_GUEST_S7
    .set KVM_ARCH_GUEST_S7, {KVM_ARCH_GUEST_S7}
    .global KVM_ARCH_GUEST_S8
    .set KVM_ARCH_GUEST_S8, {KVM_ARCH_GUEST_S8}
    .global KVM_ARCH_GUEST_S9
    .set KVM_ARCH_GUEST_S9, {KVM_ARCH_GUEST_S9}
    .global KVM_ARCH_GUEST_S10
    .set KVM_ARCH_GUEST_S10, {KVM_ARCH_GUEST_S10}
    .global KVM_ARCH_GUEST_S11
    .set KVM_ARCH_GUEST_S11, {KVM_ARCH_GUEST_S11}
    .global KVM_ARCH_GUEST_T3
    .set KVM_ARCH_GUEST_T3, {KVM_ARCH_GUEST_T3}
    .global KVM_ARCH_GUEST_T4
    .set KVM_ARCH_GUEST_T4, {KVM_ARCH_GUEST_T4}
    .global KVM_ARCH_GUEST_T5
    .set KVM_ARCH_GUEST_T5, {KVM_ARCH_GUEST_T5}
    .global KVM_ARCH_GUEST_T6
    .set KVM_ARCH_GUEST_T6, {KVM_ARCH_GUEST_T6}
    .global KVM_ARCH_GUEST_SSTATUS
    .set KVM_ARCH_GUEST_SSTATUS, {KVM_ARCH_GUEST_SSTATUS}
    .global KVM_ARCH_GUEST_SEPC
    .set KVM_ARCH_GUEST_SEPC, {KVM_ARCH_GUEST_SEPC}
    "#,
    KVM_ARCH_GUEST_RA = const KVM_ARCH_GUEST_RA,
    KVM_ARCH_GUEST_SP = const KVM_ARCH_GUEST_SP,
    KVM_ARCH_GUEST_GP = const KVM_ARCH_GUEST_GP,
    KVM_ARCH_GUEST_TP = const KVM_ARCH_GUEST_TP,
    KVM_ARCH_GUEST_T0 = const KVM_ARCH_GUEST_T0,
    KVM_ARCH_GUEST_T1 = const KVM_ARCH_GUEST_T1,
    KVM_ARCH_GUEST_T2 = const KVM_ARCH_GUEST_T2,
    KVM_ARCH_GUEST_S0 = const KVM_ARCH_GUEST_S0,
    KVM_ARCH_GUEST_S1 = const KVM_ARCH_GUEST_S1,
    KVM_ARCH_GUEST_A0 = const KVM_ARCH_GUEST_A0,
    KVM_ARCH_GUEST_A1 = const KVM_ARCH_GUEST_A1,
    KVM_ARCH_GUEST_A2 = const KVM_ARCH_GUEST_A2,
    KVM_ARCH_GUEST_A3 = const KVM_ARCH_GUEST_A3,
    KVM_ARCH_GUEST_A4 = const KVM_ARCH_GUEST_A4,
    KVM_ARCH_GUEST_A5 = const KVM_ARCH_GUEST_A5,
    KVM_ARCH_GUEST_A6 = const KVM_ARCH_GUEST_A6,
    KVM_ARCH_GUEST_A7 = const KVM_ARCH_GUEST_A7,
    KVM_ARCH_GUEST_S2 = const KVM_ARCH_GUEST_S2,
    KVM_ARCH_GUEST_S3 = const KVM_ARCH_GUEST_S3,
    KVM_ARCH_GUEST_S4 = const KVM_ARCH_GUEST_S4,
    KVM_ARCH_GUEST_S5 = const KVM_ARCH_GUEST_S5,
    KVM_ARCH_GUEST_S6 = const KVM_ARCH_GUEST_S6,
    KVM_ARCH_GUEST_S7 = const KVM_ARCH_GUEST_S7,
    KVM_ARCH_GUEST_S8 = const KVM_ARCH_GUEST_S8,
    KVM_ARCH_GUEST_S9 = const KVM_ARCH_GUEST_S9,
    KVM_ARCH_GUEST_S10 = const KVM_ARCH_GUEST_S10,
    KVM_ARCH_GUEST_S11 = const KVM_ARCH_GUEST_S11,
    KVM_ARCH_GUEST_T3 = const KVM_ARCH_GUEST_T3,
    KVM_ARCH_GUEST_T4 = const KVM_ARCH_GUEST_T4,
    KVM_ARCH_GUEST_T5 = const KVM_ARCH_GUEST_T5,
    KVM_ARCH_GUEST_T6 = const KVM_ARCH_GUEST_T6,
    KVM_ARCH_GUEST_SSTATUS = const KVM_ARCH_GUEST_SSTATUS,
    KVM_ARCH_GUEST_SEPC = const KVM_ARCH_GUEST_SEPC,
);

global_asm!(include_str!("switch.S"));

unsafe extern "C" {
    unsafe fn __kvm_riscv_switch_to(vcpu: *mut VcpuArch);
}
