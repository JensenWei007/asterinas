use crate::arch::cpu::extension::{IsaExtensions, has_extensions};
use crate::arch::vm::csr::*;

/// timer
/// 
/// 


pub struct GuestTimer {
	/// Mult values to get nanoseconds from cycles
	nsec_mult: u32,
    /// Shift values to get nanoseconds from cycles
	nsec_shift: u32,
	/// Time delta value
	time_delta: u64
}

impl GuestTimer {
    /// new
    pub fn new() -> Self {
        Self {
            nsec_mult: 0,
            nsec_shift: 0,
            time_delta: riscv::register::time::read64(),
        }
    }
}

/// 1
pub type TimerNextEventFunction = dyn Fn(u64) -> usize + Sync + Send + 'static;

/// 1
#[derive(Default)]
pub struct VcpuTimer {
	/// Flag for whether init is done
	init_done: bool,
	/// Flag for whether timer event is configured
	pub next_set: bool,
	/// Next timer event cycles
	pub next_cycles: u64,
	/// Underlying hrtimer instance
	// struct hrtimer hrt;

	/// Flag to check if sstc is enabled or not */
	sstc_enabled: bool,
	// A function pointer to switch between stimecmp or hrtimer at runtime */
	timer_next_event: Option<&'static TimerNextEventFunction>,
}

fn kvm_riscv_vcpu_update_vstimecmp(ncycles: u64) -> usize {
	unsafe {
		Vstimecmp::write(ncycles as usize);
	}
	0
}

impl VcpuTimer {
	/// 1
	pub fn init(&mut self) {
		self.init_done = true;
		self.next_set = false;

		if has_extensions(IsaExtensions::SSTC) {
			self.sstc_enabled = true;
			self.timer_next_event = Some(&kvm_riscv_vcpu_update_vstimecmp);
		} else {
			self.sstc_enabled = false;
			panic!("VcpuTimer: SSTC should be enabled!");
		}
	}
}

