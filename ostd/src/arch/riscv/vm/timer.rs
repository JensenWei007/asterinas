// SPDX-License-Identifier: MPL-2.0

use crate::arch::vm::csr::Vstimecmp;

#[repr(C)]
#[derive(Default)]
pub(super) struct VcpuTimer {
	/// Flag for whether init is done
	init_done: bool,
	/// Flag for whether timer event is configured
	next_set: bool,
	/// Next timer event cycles
	next_cycles: usize,
	// Underlying hrtimer instance, but Asterinas is not support hrtimer now.
	// struct hrtimer hrt;
}

fn kvm_riscv_vcpu_update_vstimecmp(ncycles: usize) -> usize {
	Vstimecmp::write(ncycles);
	0
}

impl VcpuTimer {
	pub(super) fn init(&mut self) {
		self.init_done = true;
		self.next_set = false;
	}

	pub(super) fn sync(&mut self) {
		self.next_cycles = Vstimecmp::read();
	}

	pub(super) fn reset(&mut self) {
		self.next_cycles = usize::MAX;
        self.next_set = false;
	}

	pub(super) fn restore(&mut self) {
        Vstimecmp::write(self.next_cycles);
        self.next_set = false;
	}
}
