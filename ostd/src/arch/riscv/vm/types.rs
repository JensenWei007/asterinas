// SPDX-License-Identifier: MPL-2.0

pub(super) const HGATP_MODE_SHIFT:usize = 60;
pub(super) const HGATP_MODE_48:usize = 0x9000000000000000;
pub(super) const HGATP_MODE_LEVEL_48:usize = 0x9;
pub(super) const HGATP_PPN:usize = 0xFFFFFFFFFFF;

const IRQ_S_SOFT:usize =		1;
pub(super) const IRQ_VS_SOFT:usize =		2;
const IRQ_S_TIMER:usize =		5;
const IRQ_S_EXT:usize =		9;
const IRQ_PMU_OVF:usize =		13;
pub(super) const IRQ_LOCAL_MASK:usize = 0x3FFF;

/// A flag indicating whether an interruption has occurred(from host).
pub const CAUSE_IRQ_FLAG: usize = 0x1 << 63;

pub(super) const VSIP_TO_HVIP_SHIFT: usize = 	IRQ_VS_SOFT - IRQ_S_SOFT;
pub(super) const VSIP_VALID_MASK:usize =	(1 << IRQ_S_SOFT) | (1 << IRQ_S_TIMER) | (1 << IRQ_S_EXT) | (1 << IRQ_PMU_OVF);

pub(super) const KVM_HEDELEG_DEFAULT:usize = 0xB30D;
pub(super) const KVM_HIDELEG_DEFAULT:usize = 0x444;

#[derive(Clone, Copy, Debug, Default, Pod)]
pub(super) struct KvmVcpuConfig {
	pub henvcfg: usize,
	pub hstateen0: usize,
	pub hedeleg: usize,
	pub hideleg: usize,
}

/// VCPU trap information
#[derive(Clone, Copy, Debug, Default)]
pub struct KvmCpuTrap {
	sepc: usize,
	scause: usize,
	stval: usize,
	htval: usize,
	htinst: usize,
}

impl KvmCpuTrap {
	pub(super) fn new(sepc:usize, scause: usize, stval: usize, htval: usize, htinst:usize) -> Self {
		Self {
			sepc,
			scause,
			stval,
			htval,
			htinst,
		}
	}

	/// Get the virt_insn from stval reg
	pub fn get_virt_insn(&self) -> usize {
		self.stval
	}

	/// Get the trap_insn from stval reg
	pub fn get_trap_insn(&self) -> usize {
		self.htinst
	}

	/// Get fault_addr from htval and stval reg
	pub fn get_fault_addr(&self) -> usize {
		(self.htval << 2) | (self.stval & 0x3)
	}

	/// Get the type of trap
	pub fn get_trap_type(&self) -> usize {
		self.scause
	}
}
