// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_RFENCE: usize = 0x52464E43;

pub(super) const VCPU_SBI_EXT_RFENCE: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_RFENCE, SBI_EXT_RFENCE, &sbi_ext_rfence_handler);

fn sbi_ext_rfence_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
