// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use super::KVM_REG_RISCV_SBI_STA;
use super::VcpuSbiContext;
use crate::prelude::*;

const SBI_EXT_STA: usize = 0x535441;

struct KvmRiscvSbiSta {
	shmem_lo: usize,
	shmem_hi: usize,
}

pub(super) const VCPU_SBI_EXT_STA: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    probe_fn: Some(&sbi_ext_sta_probe),
    reset_fn: Some(&kvm_sbi_ext_sta_reset),
    state_reg_subtype: KVM_REG_RISCV_SBI_STA,
    get_state_reg_count_fn: Some(&sbi_ext_sta_get_state_reg_count),
    get_state_reg_fn: Some(&sbi_ext_sta_get_state_reg),
    set_state_reg_fn: Some(&sbi_ext_sta_set_state_reg),
    ..KvmVcpuSbiExtension::new(SBI_EXT_STA, SBI_EXT_STA, &sbi_ext_sta_handler)
};

fn sbi_ext_sta_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}

fn sbi_ext_sta_probe() -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}

fn kvm_sbi_ext_sta_reset(ctx: &mut VcpuSbiContext) -> Result<usize> {
    ctx.shmem = u64::MAX;
    ctx.last_steal = 0;
    Ok(0)
}

fn sbi_ext_sta_get_state_reg_count() -> usize {
	size_of::<KvmRiscvSbiSta>() / size_of::<usize>()
}

fn sbi_ext_sta_get_state_reg() -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}

fn sbi_ext_sta_set_state_reg() -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
