/// 1
use alloc::{sync::Arc, vec::Vec};
use super::onereg::*;
use alloc::vec;

/// 1
#[derive(Clone, Copy, PartialEq)]
pub enum KvmRiscvSbiExtStatus {
	KVM_RISCV_SBI_EXT_STATUS_UNINITIALIZED,
	KVM_RISCV_SBI_EXT_STATUS_UNAVAILABLE,
	KVM_RISCV_SBI_EXT_STATUS_ENABLED,
	KVM_RISCV_SBI_EXT_STATUS_DISABLED,
}

/// SBI extension IDs specific to KVM. This is not the same as the SBI
/// extension IDs defined by the RISC-V SBI specification.
#[derive(Clone, Copy)]
pub enum KVM_RISCV_SBI_EXT_ID {
	KVM_RISCV_SBI_EXT_V01 = 0,
	KVM_RISCV_SBI_EXT_TIME,
	KVM_RISCV_SBI_EXT_IPI,
	KVM_RISCV_SBI_EXT_RFENCE,
	KVM_RISCV_SBI_EXT_SRST,
	KVM_RISCV_SBI_EXT_HSM,
	KVM_RISCV_SBI_EXT_PMU,
	KVM_RISCV_SBI_EXT_EXPERIMENTAL,
	KVM_RISCV_SBI_EXT_VENDOR,
	KVM_RISCV_SBI_EXT_DBCN,
	KVM_RISCV_SBI_EXT_STA,
	KVM_RISCV_SBI_EXT_SUSP,
	KVM_RISCV_SBI_EXT_FWFT,
	KVM_RISCV_SBI_EXT_MPXY,
	KVM_RISCV_SBI_EXT_MAX,
}

pub const SBI_EXT_0_1_SET_TIMER: usize = 0x0;
	pub const SBI_EXT_0_1_CONSOLE_PUTCHAR: usize = 0x1;
	pub const SBI_EXT_0_1_CONSOLE_GETCHAR: usize = 0x2;
	pub const SBI_EXT_0_1_CLEAR_IPI: usize = 0x3;
	pub const SBI_EXT_0_1_SEND_IPI: usize = 0x4;
	pub const SBI_EXT_0_1_REMOTE_FENCE_I: usize = 0x5;
	pub const SBI_EXT_0_1_REMOTE_SFENCE_VMA: usize = 0x6;
	pub const SBI_EXT_0_1_REMOTE_SFENCE_VMA_ASID: usize = 0x7;
	pub const SBI_EXT_0_1_SHUTDOWN: usize = 0x8;
	pub const SBI_EXT_BASE: usize = 0x10;
	pub const SBI_EXT_TIME: usize = 0x54494D45;
	pub const SBI_EXT_IPI: usize = 0x735049;
	pub const SBI_EXT_RFENCE: usize = 0x52464E43;
	pub const SBI_EXT_HSM: usize = 0x48534D;
	pub const SBI_EXT_SRST: usize = 0x53525354;
	pub const SBI_EXT_SUSP: usize = 0x53555350;
	pub const SBI_EXT_PMU: usize = 0x504D55;
	pub const SBI_EXT_DBCN: usize = 0x4442434E;
	pub const SBI_EXT_STA: usize = 0x535441;
	pub const SBI_EXT_NACL: usize = 0x4E41434C;
	pub const SBI_EXT_FWFT: usize = 0x46574654;
	pub const SBI_EXT_MPXY: usize = 0x4D505859;
	pub const SBI_EXT_DBTR: usize = 0x44425452;
	pub const SBI_EXT_EXPERIMENTAL_START: usize = 0x08000000;
	pub const SBI_EXT_EXPERIMENTAL_END: usize = 0x08FFFFFF;
	pub const SBI_EXT_VENDOR_START: usize = 0x09000000;
	pub const SBI_EXT_VENDOR_END: usize = 0x09FFFFFF;

/// 1
pub struct VcpuSbiContext {
	pub return_handled: usize,
	pub ext_status: [KvmRiscvSbiExtStatus; KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_MAX as usize],
    /// Firmware feature SBI extension context
	pub fwft_context: KvmSbiFwft,
    pub shmem: u64,
	pub last_steal:u64,
}

impl VcpuSbiContext {
    /// 1
    pub fn new() -> Self {
        Self {
            return_handled: 0,
            ext_status: [KvmRiscvSbiExtStatus::KVM_RISCV_SBI_EXT_STATUS_UNINITIALIZED; KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_MAX as usize],
            fwft_context: KvmSbiFwft::new(),
            shmem: 0,
            last_steal: 0,
        }
    }

    /// 1
    pub fn init(&mut self) {
        for i in 0..15 {
            let entry = &SBI_EXT_TABLE[i];
            let idx = entry.ext_idx as usize;

            if entry.ext.default_disabled {
                self.ext_status[idx] = KvmRiscvSbiExtStatus::KVM_RISCV_SBI_EXT_STATUS_DISABLED;
            } else {
                self.ext_status[idx] = KvmRiscvSbiExtStatus::KVM_RISCV_SBI_EXT_STATUS_ENABLED;
            }

            if let Some(func) = entry.ext.init {
                func(self);
            }
        }
    }
}

/// 1
pub struct KvmRiscvSbiExtensionEntry {
	pub ext_idx: KVM_RISCV_SBI_EXT_ID,
	pub ext: &'static KvmVcpuSbiExtension,
}

/// 1
pub struct KvmCpuTrap {
	sepc: usize,
	scause: usize,
	stval: usize,
	htval: usize,
	htinst: usize,
}

/// 1
pub struct KvmVcpuSbiReturn {
	out_val: usize,
	err_val: usize,
	utrap: Arc<KvmCpuTrap>,
	uexit: bool,
}

pub enum SbiFwftFeature {
	SBI_FWFT_MISALIGNED_EXC_DELEG		= 0x0,
	SBI_FWFT_LANDING_PAD			= 0x1,
	SBI_FWFT_SHADOW_STACK			= 0x2,
	SBI_FWFT_DOUBLE_TRAP			= 0x3,
	SBI_FWFT_PTE_AD_HW_UPDATING		= 0x4,
	SBI_FWFT_POINTER_MASKING_PMLEN		= 0x5,
	SBI_FWFT_LOCAL_RESERVED_START		= 0x6,
	SBI_FWFT_LOCAL_RESERVED_END		= 0x3fffffff,
	SBI_FWFT_LOCAL_PLATFORM_START		= 0x40000000,
	SBI_FWFT_LOCAL_PLATFORM_END		= 0x7fffffff,

	SBI_FWFT_GLOBAL_RESERVED_START		= 0x80000000,
	SBI_FWFT_GLOBAL_RESERVED_END		= 0xbfffffff,
	SBI_FWFT_GLOBAL_PLATFORM_START		= 0xc0000000,
	SBI_FWFT_GLOBAL_PLATFORM_END		= 0xffffffff,
}

pub type FwftSupportFunction = dyn Fn(&VcpuSbiContext) -> usize + Sync + Send + 'static;
pub type FwftInitFunction = dyn Fn(&mut VcpuSbiContext) -> usize + Sync + Send + 'static;

pub struct KvmSbiFwftFeature {
	/// Feature ID
	id: SbiFwftFeature,

	/// ONE_REG index of the first ONE_REG register
	first_reg_num: usize,

	/// Check if the feature is supported on the vcpu.
	/// This callback is optional, if not provided the feature is assumed to
	/// supported
	supported: &'static FwftSupportFunction,

	/// Probe and initialize the feature on the vcpu.
	/// This callback is optional. If provided, it will be called during
	/// vcpu initialization to probe the feature availability and perform
	/// any necessary initialization. Returns true if the feature is supported
	/// and initialized successfully, false otherwise.
	init: Option<&'static FwftInitFunction>,

	/// Reset the feature value irrespective whether feature is supported or not.
	/// This callback is mandatory
	reset: Option<&'static FwftInitFunction>,

	/// Set the feature value
	/// Return SBI_SUCCESS on success or an SBI error (SBI_ERR_*).
	/// This callback is mandatory
	set: Option<&'static FwftInitFunction>,

	/// Get the feature current value
	/// Return SBI_SUCCESS on success or an SBI error (SBI_ERR_*).
	/// This callback is mandatory
    get: Option<&'static FwftInitFunction>,
}

/// 1
pub struct KvmSbiFwftConfig {
	feature: &'static KvmSbiFwftFeature,
	supported: bool,
	enabled: bool,
	flags: usize,
}

/// FWFT data structure per vcpu
pub struct KvmSbiFwft {
	configs: Vec<KvmSbiFwftConfig>,
	have_vs_pmlen_7: bool,
	have_vs_pmlen_16: bool,
}

impl KvmSbiFwft {
    /// 1
    pub fn new() -> Self {
        Self {
            configs: vec![
                KvmSbiFwftConfig {
                    feature: &SBI_FWFT_FEATURES[0],
                    supported: false,
                    enabled: false,
                    flags: 0,
                },
                KvmSbiFwftConfig {
                    feature: &SBI_FWFT_FEATURES[1],
                    supported: false,
                    enabled: false,
                    flags: 0,
                },
            ],
            have_vs_pmlen_16: false,
            have_vs_pmlen_7: false,
        }
    }
}

/// A type alias for the Sbi Handler callback function.
pub type SbiHandlerFunction = dyn Fn(&KvmVcpuSbiReturn) -> usize + Sync + Send + 'static;
pub type SbiGetStateRegIdFunction = dyn Fn(usize) -> usize + Sync + Send + 'static;
pub type SbiGetStateRegCountFunction = dyn Fn() -> usize + Sync + Send + 'static;
pub type SbiInitFunction = dyn Fn(&mut VcpuSbiContext) -> usize + Sync + Send + 'static;
pub type SbiResetFunction = dyn Fn(&mut VcpuSbiContext) -> usize + Sync + Send + 'static;

/// 1
pub struct KvmVcpuSbiExtension {
	extid_start: usize,
	extid_end: usize,

	default_disabled: bool,

	/// SBI extension handler. It can be defined for a given extension or group of
	/// extension. But it should always return linux error codes rather than SBI
	/// specific error codes.
	handler: &'static SbiHandlerFunction,

	/// Init/deinit function called once during VCPU init/destroy. These
	/// might be use if the SBI extensions need to allocate or do specific
	/// init time only configuration.
	init: Option<&'static SbiInitFunction>,
	//void (*deinit)(struct kvm_vcpu *vcpu);

    pub reset: Option<&'static SbiResetFunction>,

    pub state_reg_subtype: usize,
    pub get_state_reg_count: Option<&'static SbiGetStateRegCountFunction>,
	pub get_state_reg_id: Option<&'static SbiGetStateRegIdFunction>,
}

const VCPU_SBI_EXT_V01: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_0_1_SET_TIMER,
    extid_end: SBI_EXT_0_1_SHUTDOWN,
    default_disabled: false,
    handler: &sbi_ext_v01_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_BASE: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_BASE,
    extid_end: SBI_EXT_BASE,
    default_disabled: false,
    handler: &sbi_ext_base_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_TIME: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_TIME,
    extid_end: SBI_EXT_TIME,
    default_disabled: false,
    handler: &sbi_ext_time_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_IPI: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_IPI,
    extid_end: SBI_EXT_IPI,
    default_disabled: false,
    handler: &sbi_ext_ipi_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_RFENCE: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_RFENCE,
    extid_end: SBI_EXT_RFENCE,
    default_disabled: false,
    handler: &sbi_ext_rfence_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_SRST: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_SRST,
    extid_end: SBI_EXT_SRST,
    default_disabled: false,
    handler: &sbi_ext_srst_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_HSM: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_HSM,
    extid_end: SBI_EXT_HSM,
    default_disabled: false,
    handler: &sbi_ext_hsm_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_PMU: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_PMU,
    extid_end: SBI_EXT_PMU,
    default_disabled: false,
    handler: &sbi_ext_pmu_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
    // TODO: add probe
};

const VCPU_SBI_EXT_DBCN: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_DBCN,
    extid_end: SBI_EXT_DBCN,
    default_disabled: true,
    handler: &sbi_forward_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_SUSP: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_SUSP,
    extid_end: SBI_EXT_SUSP,
    default_disabled: true,
    handler: &sbi_ext_susp_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_STA: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_STA,
    extid_end: SBI_EXT_STA,
    default_disabled: false,
    handler: &sbi_ext_sta_handler,
    init: None,
    reset: Some(&kvm_sbi_ext_sta_reset),
    state_reg_subtype: KVM_REG_RISCV_SBI_STA,
    get_state_reg_count: Some(&sbi_ext_sta_get_state_reg_count),
    get_state_reg_id: None,
    // TODO: add probe, reset, state_reg_subtype, get_state_reg_count, get_state_reg, set_state_reg
};

const VCPU_SBI_EXT_FWFT: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_FWFT,
    extid_end: SBI_EXT_FWFT,
    default_disabled: false,
    handler: &sbi_ext_fwft_handler,
    init: Some(&kvm_sbi_ext_fwft_init),
    reset: Some(&kvm_sbi_ext_fwft_reset),
    state_reg_subtype: KVM_REG_RISCV_SBI_FWFT,
    get_state_reg_count: None,
    get_state_reg_id: None,
    // TODO: add init, deinit, reset, state_reg_subtype, get_state_reg_count, get_state_reg_id, get_state_reg, set_state_reg
};

const VCPU_SBI_EXT_MPXY: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_MPXY,
    extid_end: SBI_EXT_MPXY,
    default_disabled: true,
    handler: &sbi_forward_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_EXPERIMENTAL: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_EXPERIMENTAL_START,
    extid_end: SBI_EXT_EXPERIMENTAL_END,
    default_disabled: false,
    handler: &sbi_forward_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

const VCPU_SBI_EXT_VENDOR: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_VENDOR_START,
    extid_end: SBI_EXT_VENDOR_END,
    default_disabled: false,
    handler: &sbi_forward_handler,
    init: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
};

/// 1
pub const SBI_EXT_TABLE: [KvmRiscvSbiExtensionEntry; 15] = [
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_V01,
        ext: &VCPU_SBI_EXT_V01,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_MAX,
        ext: &VCPU_SBI_EXT_BASE,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_TIME,
        ext: &VCPU_SBI_EXT_TIME,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_IPI,
        ext: &VCPU_SBI_EXT_IPI,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_RFENCE,
        ext: &VCPU_SBI_EXT_RFENCE,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_SRST,
        ext: &VCPU_SBI_EXT_SRST,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_HSM,
        ext: &VCPU_SBI_EXT_HSM,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_PMU,
        ext: &VCPU_SBI_EXT_PMU,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_DBCN,
        ext: &VCPU_SBI_EXT_DBCN,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_SUSP,
        ext: &VCPU_SBI_EXT_SUSP,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_STA,
        ext: &VCPU_SBI_EXT_STA,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_FWFT,
        ext: &VCPU_SBI_EXT_FWFT,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_MPXY,
        ext: &VCPU_SBI_EXT_MPXY,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_EXPERIMENTAL,
        ext: &VCPU_SBI_EXT_EXPERIMENTAL,
    },
    KvmRiscvSbiExtensionEntry {
        ext_idx: KVM_RISCV_SBI_EXT_ID::KVM_RISCV_SBI_EXT_VENDOR,
        ext: &VCPU_SBI_EXT_VENDOR,
    },
];

/// 1
pub const SBI_FWFT_FEATURES: [KvmSbiFwftFeature; 2] = [
    KvmSbiFwftFeature {
        id: SbiFwftFeature::SBI_FWFT_MISALIGNED_EXC_DELEG,
        first_reg_num: 0,
        supported: &kvm_sbi_fwft_misaligned_delegation_supported,
        init: None,
        reset: None,
        set: None,
        get: None,
    },
    KvmSbiFwftFeature {
        id: SbiFwftFeature::SBI_FWFT_POINTER_MASKING_PMLEN,
        first_reg_num: 3,
        supported: &kvm_sbi_fwft_pointer_masking_pmlen_supported,
        init: Some(&kvm_sbi_fwft_pointer_masking_pmlen_init),
        reset: None,
        set: None,
        get: None,
    },
];

pub fn sbi_ext_v01_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_base_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_time_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_ipi_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_rfence_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_srst_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_hsm_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_pmu_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_forward_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_susp_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_sta_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn sbi_ext_fwft_handler(_ret: &KvmVcpuSbiReturn) -> usize {
    0
}

pub fn vcpu_get_sbi_ext_idx(idx: usize) -> usize {
	for i in 0..15 {
        if SBI_EXT_TABLE[i].ext_idx as usize == idx {
            return i;
        }
    }
	100
}

pub fn sbi_ext_sta_get_state_reg_count() -> usize {
	return size_of::<KvmRiscvSbiSta>() / size_of::<usize>();
}

pub fn kvm_sbi_fwft_misaligned_delegation_supported(_ctx: &VcpuSbiContext) -> usize {
    0
}

pub fn kvm_sbi_fwft_pointer_masking_pmlen_supported(_ctx: &VcpuSbiContext) -> usize {
    0
}

pub fn kvm_sbi_fwft_pointer_masking_pmlen_init(ctx: &mut VcpuSbiContext) -> usize {
    0
}

pub fn kvm_sbi_ext_fwft_init(ctx: &mut VcpuSbiContext) -> usize {
    for i in 0..2 {
        let (supported_fn, init_fn) = {
            let feature = &ctx.fwft_context.configs[i].feature;
            (feature.supported, feature.init)
        };

        let mut supported = supported_fn(ctx) == 1;
        if supported {
            if let Some(init) = init_fn {
                supported = init(ctx) == 1;
            }
        }

        let conf = &mut ctx.fwft_context.configs[i];
        conf.supported = supported;
        conf.enabled = supported;
    }
    0
}

pub fn kvm_sbi_ext_fwft_reset(_ctx: &mut VcpuSbiContext) -> usize {
    0
}

pub fn kvm_sbi_ext_sta_reset(ctx: &mut VcpuSbiContext) -> usize {
    ctx.shmem = u64::MAX;
    ctx.last_steal = 0;
    0
}
