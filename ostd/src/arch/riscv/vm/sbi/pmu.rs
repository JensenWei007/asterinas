// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_PMU: usize = 0x504D55;

pub(super) const VCPU_SBI_EXT_PMU: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    probe_fn: Some(&sbi_ext_pmu_probe),
    ..KvmVcpuSbiExtension::new(SBI_EXT_PMU, SBI_EXT_PMU, &sbi_ext_pmu_handler)
};

fn sbi_ext_pmu_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}

fn sbi_ext_pmu_probe() -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
