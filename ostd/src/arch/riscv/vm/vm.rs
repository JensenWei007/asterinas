/// 1
/// 
/// 

use alloc::sync::Arc;

use crate::arch::vm::timer::GuestTimer;

use super::cpu::enable_virtualization_cpu;

/// 1
pub struct VmArch{
    guest_timer: Arc<GuestTimer>
}


impl VmArch {
    /// 1
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            guest_timer: Arc::new(GuestTimer::new()),
        })
    }

    /// 2
    pub fn init(&self){
        enable_virtualization_cpu();
    }
}

