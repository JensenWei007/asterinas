// SPDX-License-Identifier: MPL-2.0

//! VM (Virtual Machine Extensions) support for riscv.

pub(crate) mod guest_mode;
pub(crate) mod gstage;
pub(crate) mod interrupt;
pub(crate) mod context;
pub(crate) mod exit;

pub(crate) mod types;

pub use self::{
    context::{GuestContext, VcpuRunState},
    exit::GuestExitInfo,
    types::{
        GuestInterrupt, GuestTimerInstant, VcpuDtable, VcpuRegs, VcpuSegment, VcpuSregs,
    },
    //vmx::VmxExitReason,
};