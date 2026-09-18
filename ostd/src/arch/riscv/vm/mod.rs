// SPDX-License-Identifier: MPL-2.0

//! VM (Virtual Machine Extensions) support for riscv.

pub(crate) mod guest_mode;
pub(crate) mod gstage;
pub(crate) mod interrupt;
pub(crate) mod context;
pub(crate) mod exit;

pub mod types;

pub(crate) mod csr;
pub(crate) mod vmid;
pub(crate) mod tlb;
pub(crate) mod vplic;
pub mod timer;
pub mod vm;
pub mod cpu;
pub mod onereg;
pub mod vcpu;
pub mod isa;
pub mod sbi;

pub use self::{
    context::{GuestContext, VcpuRunState},
    exit::GuestExitInfo,
    types::{
        GuestInterrupt, GuestTimerInstant, VcpuDtable, VcpuRegs, VcpuSegment, VcpuSregs,
    },
    vmid::*,
};

use crate::arch::{cpu::extension::*, vm::interrupt::vintc_init};

/// 1
pub fn kvm_arch_init(){
    crate::error!("kvm_arch_init");
    // TODO: add nacl support

    // Do some check
    // cpu: must enable H.ext
    // sbi: version >=0.2
    // sbi: SBI RFENCE extension should be enabled
    if !has_extensions(IsaExtensions::H) {
        panic!("KVM: should enable H isaext");
    }

    // we use Sv39x4 for gstage now, we can add a dectet fn like linux do.
    // gstage_mode_detect();

    // dectet vmid
    gstage_vmid_detect();

    vintc_init();
}