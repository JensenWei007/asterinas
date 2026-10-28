// SPDX-License-Identifier: MPL-2.0

//! VM (Virtual Machine Extensions) support for riscv.

pub(crate) mod gstage;
mod context;

mod types;
mod exit;

mod csr;
mod timer;
mod cpu;
mod vcpu;
pub(crate) mod guest_mode;

pub use vcpu::VcpuArchState;
pub use cpu::{enable_virtualization_cpu, disable_virtualization_cpu};
pub use context::{VcpuContext, GuestContext, KvmVcpuCsr};
pub use guest_mode::RiscvGuestHooks;
pub use types::{CAUSE_IRQ_FLAG, HGATP_MODE_48, HGATP_MODE_SHIFT, VSIP_TO_HVIP_SHIFT, VSIP_VALID_MASK, IRQ_LOCAL_MASK};
pub use exit::GuestExitInfo;
