// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_0_1_SET_TIMER: usize = 0x0;
const SBI_EXT_0_1_CONSOLE_PUTCHAR: usize = 0x1;
const SBI_EXT_0_1_CONSOLE_GETCHAR: usize = 0x2;
const SBI_EXT_0_1_CLEAR_IPI: usize = 0x3;
const SBI_EXT_0_1_SEND_IPI: usize = 0x4;
const SBI_EXT_0_1_REMOTE_FENCE_I: usize = 0x5;
const SBI_EXT_0_1_REMOTE_SFENCE_VMA: usize = 0x6;
const SBI_EXT_0_1_REMOTE_SFENCE_VMA_ASID: usize = 0x7;
const SBI_EXT_0_1_SHUTDOWN: usize = 0x8;

pub(super) const VCPU_SBI_EXT_V01: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_0_1_SET_TIMER, SBI_EXT_0_1_SHUTDOWN, &sbi_ext_v01_handler);

fn sbi_ext_v01_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
