// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn, KvmVcpuSbiReturnError};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_BASE: usize = 0x10;

const SBI_EXT_BASE_GET_SPEC_VERSION: usize = 0;
const SBI_EXT_BASE_GET_IMP_ID: usize = 1;
const SBI_EXT_BASE_GET_IMP_VERSION: usize = 2;
const SBI_EXT_BASE_PROBE_EXT: usize = 3;
const SBI_EXT_BASE_GET_MVENDORID: usize = 4;
const SBI_EXT_BASE_GET_MARCHID: usize = 5;
const SBI_EXT_BASE_GET_MIMPID: usize = 6;

pub(super) const VCPU_SBI_EXT_BASE: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_BASE, SBI_EXT_BASE, &sbi_ext_base_handler);

fn sbi_ext_base_handler(arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    let mut rt = KvmVcpuSbiReturn::default();

    match arch.guest_context.get_a6() {
        SBI_EXT_BASE_GET_SPEC_VERSION => rt.out_val = 3 << 24,
        SBI_EXT_BASE_GET_IMP_ID => rt.out_val = 3,
        SBI_EXT_BASE_GET_IMP_VERSION => rt.out_val = 459264,
        SBI_EXT_BASE_PROBE_EXT => rt.out_val = 0,
        SBI_EXT_BASE_GET_MVENDORID => rt.out_val = arch.mvendorid,
        SBI_EXT_BASE_GET_MARCHID => rt.out_val = arch.marchid,
        SBI_EXT_BASE_GET_MIMPID => rt.out_val = arch.mimpid,
        _ => rt.err_val = KvmVcpuSbiReturnError::NotSupported as usize,
    }

    Ok(rt)
}
