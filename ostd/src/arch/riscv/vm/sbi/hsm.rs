// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_HSM: usize = 0x48534D;

pub(super) const VCPU_SBI_EXT_HSM: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_HSM, SBI_EXT_HSM, &sbi_ext_hsm_handler);

fn sbi_ext_hsm_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
