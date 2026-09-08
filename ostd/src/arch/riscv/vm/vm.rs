/// 1
/// 
/// 

use alloc::sync::Arc;

use crate::arch::vm::timer::GuestTimer;

use super::vplic::VPlic;
use super::cpu::enable_virtualization_cpu;

/// 1
pub struct VmArch{
    /// intc
    vplic: Arc<VPlic>,
    guest_timer: Arc<GuestTimer>
}


impl VmArch {
    /// 1
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            vplic: Arc::new(VPlic::new()),
            guest_timer: Arc::new(GuestTimer::new()),
        })
    }

    /// 2
    pub fn init(&self){
        enable_virtualization_cpu();
    }
}

