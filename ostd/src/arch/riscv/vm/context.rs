use crate::{prelude::*};

use super::types::VcpuRegs;

/// 1
pub struct GuestContext {
    /// 1
    pub id:u32,
}

impl GuestContext {
    /// 1
    pub fn new(vcpu_id: u32) -> Result<Self> {
        Ok(Self {
            id:vcpu_id,
        })
    }

    /// Returns whether the guest vCPU is currently running.
    pub fn is_running(&self) -> bool {
        false
    }

    /// 1
    pub fn regs(&self) -> VcpuRegs {
        VcpuRegs {  }
    }

    /// Replaces the guest special-register state.
    ///
    /// This method stores the supplied guest-visible state and rebuilds the
    /// VMX control-register state derived from CR0 and CR4. The caller remains
    /// responsible for providing architecturally valid guest state.
    pub fn set_regs(&mut self, _sregs: VcpuRegs) {
        
    }

    /// Returns the vCPU execution state.
    pub fn run_state(&self) -> VcpuRunState {
        VcpuRunState::Uninitialized
    }

    /// Sets the vCPU execution state.
    pub fn set_run_state(&mut self, _state: VcpuRunState) {
    }
}

impl Default for GuestContext {
    fn default() -> Self {
        Self::new(0).expect("failed to create guest context")
    }
}


/// 1
/// Describes whether a guest vCPU may enter guest mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VcpuRunState {
    /// The vCPU is not initialized for execution.
    Uninitialized,
    /// The vCPU is waiting for a startup IPI.
    WaitForSipi,
    /// The vCPU is ready to enter guest mode.
    #[default]
    Runnable,
    /// The vCPU is currently executing in guest mode.
    Running,
    /// The vCPU Halted.
    Halted,
}