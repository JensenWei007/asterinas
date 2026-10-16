// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_SRST: usize = 0x53525354;

pub(super) const VCPU_SBI_EXT_SRST: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_SRST, SBI_EXT_SRST, &sbi_ext_srst_handler);

fn sbi_ext_srst_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
