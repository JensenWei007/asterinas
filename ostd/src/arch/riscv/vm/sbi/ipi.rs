// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_IPI: usize = 0x735049;

pub(super) const VCPU_SBI_EXT_IPI: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_IPI, SBI_EXT_IPI, &sbi_ext_ipi_handler);

fn sbi_ext_ipi_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
