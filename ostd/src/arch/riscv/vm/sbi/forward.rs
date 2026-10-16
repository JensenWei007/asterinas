// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_DBCN: usize = 0x4442434E;
const SBI_EXT_NACL: usize = 0x4E41434C;
const SBI_EXT_MPXY: usize = 0x4D505859;
const SBI_EXT_DBTR: usize = 0x44425452;
const SBI_EXT_EXPERIMENTAL_START: usize = 0x08000000;
const SBI_EXT_EXPERIMENTAL_END: usize = 0x08FFFFFF;
const SBI_EXT_VENDOR_START: usize = 0x09000000;
const SBI_EXT_VENDOR_END: usize = 0x09FFFFFF;

pub(super) const VCPU_SBI_EXT_DBCN: KvmVcpuSbiExtension =KvmVcpuSbiExtension {
    default_disabled: true,
    ..KvmVcpuSbiExtension::new(SBI_EXT_DBCN, SBI_EXT_DBCN, &sbi_forward_handler)
};

pub(super) const VCPU_SBI_EXT_MPXY: KvmVcpuSbiExtension =KvmVcpuSbiExtension {
    default_disabled: true,
    ..KvmVcpuSbiExtension::new(SBI_EXT_MPXY, SBI_EXT_MPXY, &sbi_forward_handler)
};

pub(super) const VCPU_SBI_EXT_EXPERIMENTAL: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_EXPERIMENTAL_START, SBI_EXT_EXPERIMENTAL_END, &sbi_forward_handler);

pub(super) const VCPU_SBI_EXT_VENDOR: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_VENDOR_START, SBI_EXT_VENDOR_END, &sbi_forward_handler);

fn sbi_forward_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
