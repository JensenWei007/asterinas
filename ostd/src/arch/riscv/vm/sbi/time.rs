// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_TIME: usize = 0x54494D45;

pub(super) const VCPU_SBI_EXT_TIME: KvmVcpuSbiExtension =
    KvmVcpuSbiExtension::new(SBI_EXT_TIME, SBI_EXT_TIME, &sbi_ext_time_handler);

fn sbi_ext_time_handler(arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    let next_cycle = arch.guest_context.get_a0();
    arch.timer.call_next_event(next_cycle);
    Ok(KvmVcpuSbiReturn::default())
}
