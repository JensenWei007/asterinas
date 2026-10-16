// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_SUSP: usize = 0x53555350;

pub(super) const VCPU_SBI_EXT_SUSP: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    default_disabled: true,
    ..KvmVcpuSbiExtension::new(SBI_EXT_SUSP, SBI_EXT_SUSP, &sbi_ext_susp_handler)
};

fn sbi_ext_susp_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
