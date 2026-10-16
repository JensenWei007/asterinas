// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use super::KVM_REG_RISCV_SBI_FWFT;
use super::VcpuSbiContext;
use crate::prelude::*;

const SBI_EXT_FWFT: usize = 0x46574654;

pub(super) const VCPU_SBI_EXT_FWFT: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    init_fn: Some(&kvm_sbi_ext_fwft_init),
    deinit_fn: Some(&sbi_ext_fwft_deinit),
    reset_fn: Some(&kvm_sbi_ext_fwft_reset),
    state_reg_subtype: KVM_REG_RISCV_SBI_FWFT,
    get_state_reg_count_fn: Some(&sbi_ext_fwft_get_state_reg_count),
    get_state_reg_id_fn: Some(&sbi_ext_fwft_get_state_reg_id),
    get_state_reg_fn: Some(&sbi_ext_fwft_get_state_reg),
    set_state_reg_fn: Some(&sbi_ext_fwft_set_state_reg),
    ..KvmVcpuSbiExtension::new(SBI_EXT_FWFT, SBI_EXT_FWFT, &sbi_ext_fwft_handler)
};

const SBI_FWFT_FEATURES_SIZE: usize = 2;

const SBI_FWFT_FEATURES: [KvmSbiFwftFeature; SBI_FWFT_FEATURES_SIZE] = [
    KvmSbiFwftFeature {
        id: SbiFwftFeature::SbiFwftMisalignedExcDeleg,
        first_reg_num: 0,
        supported_fn: &kvm_sbi_fwft_misaligned_delegation_supported,
        init_fn: None,
        reset_fn: None,
        set_fn: None,
        get_fn: None,
    },
    KvmSbiFwftFeature {
        id: SbiFwftFeature::SbiFwftPointerMaskingPmlen,
        first_reg_num: 3,
        supported_fn: &kvm_sbi_fwft_pointer_masking_pmlen_supported,
        init_fn: Some(&kvm_sbi_fwft_pointer_masking_pmlen_init),
        reset_fn: None,
        set_fn: None,
        get_fn: None,
    },
];

enum SbiFwftFeature {
	SbiFwftMisalignedExcDeleg		= 0x0,
	SbiFwftLandingPad			= 0x1,
	SbiFwftShadowStack			= 0x2,
	SbiFwftDoubleTrap			= 0x3,
	SbiFwftPteAdHwUpdating		= 0x4,
	SbiFwftPointerMaskingPmlen		= 0x5,
	SbiFwftLocalReservedStart		= 0x6,
	SbiFwftLocalReservedEnd		= 0x3fffffff,
	SbiFwftLocalPlatformStart		= 0x40000000,
	SbiFwftLocalPlatformEnd		= 0x7fffffff,

	SbiFwftGlobalReservedStart		= 0x80000000,
	SbiFwftGlobalReservedEnd		= 0xbfffffff,
	SbiFwftGlobalPlatformStart		= 0xc0000000,
	SbiFwftGlobalPlatformEnd		= 0xffffffff,
}

type FwftSupportFunction = dyn Fn(&VcpuSbiContext) -> usize + Sync + Send + 'static;
type FwftInitFunction = dyn Fn(&mut VcpuSbiContext) -> Result<usize> + Sync + Send + 'static;
type FwftResetFunction = dyn Fn(&mut VcpuSbiContext) -> Result<usize> + Sync + Send + 'static;
type FwftSetFunction = dyn Fn(&mut VcpuSbiContext) -> Result<usize> + Sync + Send + 'static;
type FwftGetFunction = dyn Fn(&mut VcpuSbiContext) -> Result<usize> + Sync + Send + 'static;


struct KvmSbiFwftFeature {
	/// Feature ID
	id: SbiFwftFeature,

	/// ONE_REG index of the first ONE_REG register
	first_reg_num: usize,

	/// Check if the feature is supported on the vcpu.
	/// This callback is optional, if not provided the feature is assumed to
	/// supported
	supported_fn: &'static FwftSupportFunction,

	/// Probe and initialize the feature on the vcpu.
	/// This callback is optional. If provided, it will be called during
	/// vcpu initialization to probe the feature availability and perform
	/// any necessary initialization. Returns true if the feature is supported
	/// and initialized successfully, false otherwise.
	init_fn: Option<&'static FwftInitFunction>,

	/// Reset the feature value irrespective whether feature is supported or not.
	/// This callback is mandatory
	reset_fn: Option<&'static FwftResetFunction>,

	/// Set the feature value
	/// Return SBI_SUCCESS on success or an SBI error (SBI_ERR_*).
	/// This callback is mandatory
	set_fn: Option<&'static FwftSetFunction>,

	/// Get the feature current value
	/// Return SBI_SUCCESS on success or an SBI error (SBI_ERR_*).
	/// This callback is mandatory
    get_fn: Option<&'static FwftGetFunction>,
}

struct KvmSbiFwftConfig {
	feature: &'static KvmSbiFwftFeature,
	supported: bool,
	enabled: bool,
	flags: usize,
}

#[repr(C)]
pub(super) struct KvmSbiFwft {
	configs: Vec<KvmSbiFwftConfig>,
	have_vs_pmlen_7: bool,
	have_vs_pmlen_16: bool,
}

impl KvmSbiFwft {
    pub(super) fn new() -> Self {
        let configs = SBI_FWFT_FEATURES
            .iter()
            .map(|feature| KvmSbiFwftConfig {
                feature,
                supported: false,
                enabled: false,
                flags: 0,
            })
            .collect();

        Self {
            configs,
            have_vs_pmlen_7: false,
            have_vs_pmlen_16: false,
        }
    }
}

fn sbi_ext_fwft_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}

fn kvm_sbi_fwft_misaligned_delegation_supported(_ctx: &VcpuSbiContext) -> usize {
    0
}

fn kvm_sbi_fwft_pointer_masking_pmlen_supported(_ctx: &VcpuSbiContext) -> usize {
    0
}

fn kvm_sbi_fwft_pointer_masking_pmlen_init(_ctx: &mut VcpuSbiContext) -> Result<usize> {
    Ok(0)
}

fn kvm_sbi_ext_fwft_init(ctx: &mut VcpuSbiContext) -> Result<usize> {
    for i in 0..SBI_FWFT_FEATURES_SIZE {
        let (supported_fn, init_fn) = {
            let feature = &ctx.fwft_context.configs[i].feature;
            (feature.supported_fn, feature.init_fn)
        };

        let mut supported = supported_fn(ctx) == 1;
        if supported {
            if let Some(init) = init_fn {
                supported = init(ctx)? == 1;
            }
        }

        let conf = &mut ctx.fwft_context.configs[i];
        conf.supported = supported;
        conf.enabled = supported;
    }
    Ok(0)
}

fn sbi_ext_fwft_deinit() -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}

fn kvm_sbi_ext_fwft_reset(_ctx: &mut VcpuSbiContext) -> Result<usize> {
    Ok(0)
}

fn sbi_ext_fwft_get_state_reg_count() -> usize {
    0
}

fn sbi_ext_fwft_get_state_reg_id(_index: usize) -> Result<usize> {
    Ok(0)
}

fn sbi_ext_fwft_get_state_reg() -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}

fn sbi_ext_fwft_set_state_reg() -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
