// SPDX-License-Identifier: MPL-2.0

use crate::arch::vm::csr::Vstimecmp;

/// The Timer of Vcpu.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct VcpuTimer {
	/// Next timer event cycles.
	next_cycles: usize,
}

impl VcpuTimer {
	/// Init the timer.
	pub fn init(&mut self) {
		self.next_cycles = 0;
	}

	pub(super) fn sync(&mut self) {
		self.next_cycles = Vstimecmp::read();
	}

	/// Reset the timer.
	pub fn reset(&mut self) {
		self.next_cycles = usize::MAX;
	}

	pub(super) fn save(&self) {
		Vstimecmp::write(usize::MAX);
	}

	pub(super) fn restore(&mut self) {
        Vstimecmp::write(self.next_cycles);
	}
}
