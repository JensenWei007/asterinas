// SPDX-License-Identifier: MPL-2.0

use crate::arch::cpu::extension::{IsaExtensions, has_extensions};
use crate::arch::vm::csr::Vstimecmp;

type TimerNextEventFunction = dyn Fn(usize) -> usize + Sync + Send + 'static;

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

	/// Flag to check if sstc is enabled or not */
	sstc_enabled: bool,
	/// A function pointer to switch between stimecmp or hrtimer at runtime */
	timer_next_event: Option<&'static TimerNextEventFunction>,
}

fn kvm_riscv_vcpu_update_vstimecmp(ncycles: usize) -> usize {
	Vstimecmp::write(ncycles);
	0
}

impl VcpuTimer {
	pub(super) fn init(&mut self) {
		self.init_done = true;
		self.next_set = false;

		if has_extensions(IsaExtensions::SSTC) {
			self.sstc_enabled = true;
			self.timer_next_event = Some(&kvm_riscv_vcpu_update_vstimecmp);
		}
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

	pub(super) fn call_next_event(&self, next_cycle: usize) {
		let func = self.timer_next_event.unwrap();
    	func(next_cycle);
	}
}
