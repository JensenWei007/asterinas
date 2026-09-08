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

