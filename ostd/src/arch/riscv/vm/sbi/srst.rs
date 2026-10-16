// SPDX-License-Identifier: MPL-2.0

use super::{KvmVcpuSbiExtension, KvmVcpuSbiReturn};
use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

const SBI_EXT_SRST: usize = 0x53525354;

pub(super) const VCPU_SBI_EXT_SRST: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
    extid_start: SBI_EXT_SRST,
    extid_end: SBI_EXT_SRST,
    default_disabled: false,
    handler: &sbi_ext_srst_handler,
    probe: None,
    init: None,
    deinit: None,
    reset: None,
    state_reg_subtype: 0,
    get_state_reg_count: None,
    get_state_reg_id: None,
    get_state_reg: None,
    set_state_reg: None,
};

fn sbi_ext_srst_handler(_arch: &VcpuArchInner) -> Result<KvmVcpuSbiReturn> {
    Ok(KvmVcpuSbiReturn::default())
}
