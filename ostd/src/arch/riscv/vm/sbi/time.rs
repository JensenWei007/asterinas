// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_TIME: usize = 0x54494D45;

pub(super) const VCPU_SBI_EXT_TIME: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_TIME, SBI_EXT_TIME, &sbi_ext_time_handler);

fn sbi_ext_time_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
