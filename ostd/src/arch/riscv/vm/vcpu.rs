use crate::prelude::*;
use super::onereg::KvmVcpuCsr;
use crate::arch::{cpu::extension::{IsaExtensions, has_extensions}, vm::context::VcpuContext};

use crate::arch::vm::types::{KvmCpuTrap, KvmVcpuConfig, KVM_HEDELEG_DEFAULT, KVM_HIDELEG_DEFAULT, IRQ_VS_SOFT, HGATP_MODE_LEVEL_48, HGATP_PPN, HGATP_MODE_SHIFT};
use crate::arch::vm::timer::VcpuTimer;
use crate::arch::vm::sbi::{VcpuSbiContext, reset_sbi};
use crate::arch::vm::isa::kvm_riscv_vcpu_setup_isa;
use crate::arch::vm::csr::{SR_SPP, SR_SPIE, SR_FS, SR_FS_INITIAL, HSTATUS_VTW, HSTATUS_SPVP, HSTATUS_SPV, ENVCFG_STCE, ENVCFG_CBIE, ENVCFG_CBCFE, ENVCFG_CBZE, ENVCFG_ADUE, 
    Hvip, Hstatus, Htval, Htinst, Vsie, Hedeleg, Hideleg, Henvcfg, Hgatp, Vsstatus, Vstvec, Vsscratch, Vsepc, Vscause, Vstval, Vsatp, Vstimecmp};
use crate::arch::vm::gstage::PAGE_SHIFT;

/// The ostd part of VcpuArch
#[repr(C)]
pub struct VcpuArchInner {
    /// whether VCPU ran at least once
    ran_atleast_once: bool,
    /// ISA feature bits (similar to MISA)
    pub(super) isa: [u64; 2],
    /// Vendor details
	pub(super) mvendorid: usize,
    /// Arch details
	pub(super) marchid: usize,
    /// Implementation details
	pub(super) mimpid: usize,

	/// SSCRATCH, STVEC, SCOUNTEREN, SENVCFG, and SSTATEEN0 of Host
	pub(super) host_sscratch: usize,
	pub(super) host_stvec: usize,
	host_scounteren: usize,
	host_senvcfg: usize,
	host_sstateen0: usize,

    /// CPU context of Host
	pub(super) host_context: VcpuContext,

	/// CPU context of Guest VCPU
	pub guest_context: VcpuContext,

    /// CPU CSR context of Guest VCPU
	pub(super) guest_csr: KvmVcpuCsr,

	/// ### VCPU interrupts
	/// The irqs_pending bitmap represents pending interrupts whereas
	/// irqs_pending_mask represents bits changed in irqs_pending. Updates
	/// to these bitmaps are serialized so vcpu interrupt sync/flush cannot
	/// drop a newly injected interrupt while syncing guest-visible HVIP.
    /// 
    /// Note: If we do not use AIA to implement interrupt virtualization, 
    /// all guest interrupts must originate from the user-space VMM (e.g., QEMU) 
    /// via `SET_INTERRUPT`. However, the VMM's invocation of the `ioctl` requires 
    /// CPU resources; this means that in a single-core environment, unless 
    /// the `vcpu_run` thread yields control, the guest will never receive the interrupt.
    /// 
    /// Note: Generally, interrupts are associated with specific interrupt numbers,
    /// however, we do not need to concern ourselves with the specific number—only 
    /// whether an interrupt has occurred. This is why the `set_interrupt` function 
    /// does not distinguish between interrupt numbers. We simply need to notify the guest 
    /// that an interrupt is pending; the guest then independently queries the PLIC via MMIO 
    /// to determine the specific interrupt number. Since the PLIC is managed by 
    /// the userspace VMM(e.g. QEMU), all the necessary setup has already been handled.
    irqs_pending: u64,
    irqs_pending_mask: u64,

    /// VCPU Timer
	pub(super) timer: VcpuTimer,

    /// SBI context
	pub(super) sbi_context: VcpuSbiContext,

    /// 'static' configurations which are set only once
	cfg: KvmVcpuConfig,
}

impl VcpuArchInner {
    /// Create a New VcpuArchInner
    pub fn new() -> Self {
        Self {
            ran_atleast_once: false,
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
            irqs_pending: 0,
            irqs_pending_mask: 0,
            timer: VcpuTimer::default(),
            sbi_context: VcpuSbiContext::new(),
            cfg: KvmVcpuConfig::default(),
        }
    }

    /// Init Vcpu
    pub fn init(&mut self) -> Result<()> {
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
        self.sbi_context.init()?;

        // Reset VCPU
        self.reset(false)?;

        Ok(())
    }

    /// Check whether vcpu has ran once
    pub fn is_ran_at_least_once(&self) -> bool {
        self.ran_atleast_once
    }

    /// Mark vcpu has ran once
    pub fn ran_at_least_once(&mut self) {
        self.ran_atleast_once = true;
    }

    fn reset(&mut self, _kvm_sbi_reset: bool) -> Result<()> {
        self.context_reset();
        self.fp_reset();
        self.timer_reset();
        self.sbi_reset()?;
        Ok(())
    }

    fn context_reset(&mut self) {
        self.guest_context = VcpuContext::default();
        self.guest_csr = KvmVcpuCsr::default();

        // Setup reset state of shadow SSTATUS and HSTATUS CSRs
        self.guest_context.set_sstatus(SR_SPP | SR_SPIE);

        let mut hstatus = self.guest_context.get_hstatus();
        hstatus |= HSTATUS_VTW;
        hstatus |= HSTATUS_SPVP;
        hstatus |= HSTATUS_SPV;
        self.guest_context.set_hstatus(hstatus);
    }

    fn fp_reset(&mut self) {
        let mut sstatus = self.guest_context.get_sstatus();
        sstatus &= !SR_FS;

        if has_extensions(IsaExtensions::F) || has_extensions(IsaExtensions::D) {
            // Set this bit to allow guest use fp_reg and fp_insn
            sstatus |= SR_FS_INITIAL;
        }
        self.guest_context.set_sstatus(sstatus);
    }

    fn timer_reset(&mut self) {
        self.timer.reset();
    }

    fn sbi_reset(&mut self) -> Result<()> {
        reset_sbi(self)
    }

    /// Set config regs to vcpu which has ran once
    pub fn config_ran_once(&mut self) {
        // SSTC
        self.cfg.henvcfg |= ENVCFG_STCE;
        // ZICBOM
        self.cfg.henvcfg |= ENVCFG_CBIE | ENVCFG_CBCFE;
        // ZICBOZ
        self.cfg.henvcfg |= ENVCFG_CBZE;
        // SVADU & SVADE
        self.cfg.henvcfg |= ENVCFG_ADUE;
    }

    /// Update vcpu's hvip reg
    pub fn update_hvip(&self) {
        Hvip::write(self.guest_csr.get_hvip());
    }

    /// Save host csrs in host_context and load guest csrs
    // SAFETY: Accessing CSR registers using riscv crate is safe.
    pub fn swap_in_guest_state(&mut self){
        let scounteren = self.guest_csr.get_scounteren();
        self.host_scounteren = riscv::register::scounteren::read().bits();
        unsafe {
            riscv::register::scounteren::write(riscv::register::scounteren::Scounteren::from_bits(scounteren));
        }
        let senvcfg = self.guest_csr.get_senvcfg();
        self.host_senvcfg = riscv::register::senvcfg::read().bits();
        unsafe {
            riscv::register::senvcfg::write(riscv::register::senvcfg::Senvcfg::from_bits(senvcfg));
        }
    }

    /// Save guest csrs in guset_context and load host csrs
    // SAFETY: Accessing CSR registers using riscv crate is safe.
    pub fn swap_in_host_state(&mut self) {
        let scounteren = self.host_scounteren;
        self.guest_csr.set_scounteren(riscv::register::scounteren::read().bits());
        unsafe {
            riscv::register::scounteren::write(riscv::register::scounteren::Scounteren::from_bits(scounteren));
        }
        let senvcfg = self.host_senvcfg;
        self.guest_csr.set_senvcfg(riscv::register::senvcfg::read().bits());
        unsafe {
            riscv::register::senvcfg::write(riscv::register::senvcfg::Senvcfg::from_bits(senvcfg));
        }
    }

    /// Enter VM and will back to host.
    /// The trap info will be returned in KvmCpuTrap
    pub fn enter_exit(&mut self) -> KvmCpuTrap {
        self.host_context.set_hstatus(Hstatus::swap(self.guest_context.get_hstatus()));
        
        // SAFETY: It is safe to enter VM.
        unsafe {
            __kvm_riscv_switch_to(self as *mut _);
        }

        self.guest_context.set_hstatus(Hstatus::swap(self.host_context.get_hstatus()));

        KvmCpuTrap::new(self.guest_context.get_sepc(), riscv::register::scause::read().bits(), riscv::register::stval::read(), Htval::read(), Htinst::read())
    }

    /// We might get interrupts when we handle vcpu's trap in host(eg. in wfi_insn's yield).
    /// Now we should sync interrupts to vcpu after trap is handled.
    /// 
    /// Note: For details on how the host handles guest interrupts, 
    /// please refer to the comments for `irqs_pending`.
    pub fn sync_interrupts(&mut self) {
            self.guest_csr.set_vsie(Vsie::read());

            let hvip = Hvip::read();

            if (self.guest_csr.get_hvip() ^ hvip) & (0x1 << IRQ_VS_SOFT) != 0 {
                if hvip & (0x1 << IRQ_VS_SOFT) != 0 {
                    let should_set = (self.irqs_pending_mask & 0x1 << IRQ_VS_SOFT) == 0; 
                    self.irqs_pending_mask |= 0x1 << IRQ_VS_SOFT;
                    if should_set {
                        self.irqs_pending |= 0x1 << IRQ_VS_SOFT;
                    }
                } else {
                    let should_clear = (self.irqs_pending_mask & 0x1 << IRQ_VS_SOFT) == 0; 
                    self.irqs_pending_mask |= 0x1 << IRQ_VS_SOFT;
                    if should_clear {
                        self.irqs_pending &= !(0x1 << IRQ_VS_SOFT);
                    }
                }
            }
            
            self.timer.sync();
    }
    
    fn config_load(&self) {
        Hedeleg::write(self.cfg.hedeleg);
        Hideleg::write(self.cfg.hideleg);
        Henvcfg::write(self.cfg.henvcfg);
    }

    /// Update HGATP reg which is used in G-stage addr trans.
    fn update_hgatp(&self, pgd_paddr: u64) {
        let mut hgatp = HGATP_MODE_LEVEL_48 << HGATP_MODE_SHIFT;
        hgatp |= pgd_paddr as usize >> PAGE_SHIFT & HGATP_PPN;
        Hgatp::write(hgatp);
    }

    /// Check whether the trap is from guest
    pub fn trap_is_from_guest(&self) -> bool {
        self.guest_context.get_hstatus() & HSTATUS_SPV != 0
    }

    fn timer_restore(&mut self) {
        self.timer.restore();
    }

    /// Now we have sync guest's interrupt in `VcpuArchInner.irqs_pending`
    /// We should flush it to guest's csr regs before we enter VM.
    /// 
    /// Note: For details on how the host handles guest interrupts, 
    /// please refer to the comments for `irqs_pending`.
    pub fn flush_interrupts(&mut self) {
        let mask = self.irqs_pending_mask;
        if mask != 0 {
            self.irqs_pending_mask = 0;
            let val = self.irqs_pending & mask;

            let mut hvip = self.guest_csr.get_hvip();
            hvip &= !mask as usize;
            hvip |= val as usize;
            self.guest_csr.set_hvip(hvip);
        }
    }

    /// Load Vcpu
    pub fn vcpu_load(&mut self, pgd_paddr: u64) {
        self.config_load();

        Vsstatus::write(self.guest_csr.get_vsstatus());
        Vsie::write(self.guest_csr.get_vsie());
        Vstvec::write(self.guest_csr.get_vstvec());
        Vsscratch::write(self.guest_csr.get_vsscratch());
        Vsepc::write(self.guest_csr.get_vsepc());
        Vscause::write(self.guest_csr.get_vscause());
        Vstval::write(self.guest_csr.get_vstval());
        Hvip::write(self.guest_csr.get_hvip());
        Vsatp::write(self.guest_csr.get_vsatp());

        self.update_hgatp(pgd_paddr);

        self.timer_restore();
    }

    fn timer_save(&mut self) {
        Vstimecmp::write(usize::MAX);
    }

    /// Put Vcpu
    pub fn vcpu_put(&mut self) {
        self.timer_save();

        self.guest_csr.set_vsstatus(Vsstatus::read());
        self.guest_csr.set_vsie(Vsie::read());
        self.guest_csr.set_vstvec(Vstvec::read());
        self.guest_csr.set_vsscratch(Vsscratch::read());
        self.guest_csr.set_vsepc(Vsepc::read());
        self.guest_csr.set_vscause(Vscause::read());
        self.guest_csr.set_vstval(Vstval::read());
        self.guest_csr.set_hvip(Hvip::read());
        self.guest_csr.set_vsatp(Vsatp::read());
    }

    /// The VMM(from user-space, e.g. QEMU) told us guest has a interrupt.
    /// So we save the interrupt in `VcpuArchInner.irqs_pending`.
    /// 
    /// Note: For details on how the host handles guest interrupts, 
    /// please refer to the comments for `irqs_pending`.
    pub fn set_interrupt(&mut self, irq: u32) {
        if irq == u32::MAX {
            self.irqs_pending |= 0x1<<10;
        } else {
            self.irqs_pending &= !(0x1<<10);
        }
        self.irqs_pending_mask |= 0x1 <<10;
    }
}

unsafe extern "C" {
    // We only use this pointer to read/write guest_context/host_context.
    // So we can use #[allow(improper_ctypes)] to mask the parts of the struct 
    // that are not FFI-safe and would otherwise trigger warnings.
    #[allow(improper_ctypes)]
    unsafe fn __kvm_riscv_switch_to(vcpu: *mut VcpuArchInner);
}
