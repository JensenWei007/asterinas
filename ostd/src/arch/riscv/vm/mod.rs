// SPDX-License-Identifier: MPL-2.0

//! VM (Virtual Machine Extensions) support for riscv.

pub(crate) mod gstage;
mod context;

mod types;

mod csr;
mod timer;
mod cpu;
mod onereg;
mod vcpu;
pub(crate) mod guest_mode;
mod isa;
mod sbi;
mod insn;

pub use vcpu::VcpuArchState;
pub use insn::{KvmInsnReturn, handle_system_opcode_insn, insn_len, INSN_OPCODE_SHIFT, INSN_OPCODE_MASK};
pub use cpu::{enable_virtualization_cpu, disable_virtualization_cpu};
pub use context::VcpuContext;
pub use context::GuestContext;
pub use guest_mode::RiscvGuestHooks;
pub use types::{CAUSE_IRQ_FLAG, GuestExitInfo};
pub use sbi::sbi_ecall;
pub use onereg::{KVM_REG_SIZE_MASK, KVM_REG_SIZE_U32, KVM_REG_SIZE_U64};
