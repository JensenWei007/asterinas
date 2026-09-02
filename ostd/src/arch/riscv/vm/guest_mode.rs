use crate::{
    prelude::*,
    vm::{GuestPhysMemSpace},
};
use super::{
    context::{GuestContext, },
};

/// Describes why a guest run returned to the kernel client.
pub enum GuestRunResult {
    /// A host interrupt ended this run so the kernel can reach a scheduling point.
    HostInterrupt,
    /// The vCPU is waiting for a startup IPI and was not entered.
    WaitForSipi,
}

/// 1
pub struct GuestMode {}

impl GuestMode {
    /// 1
    pub fn new() -> Result<Self> {
        Ok(Self {
        })
    }

    /// 1
    pub fn execute<I, T>(
        &self,
        _context: &mut GuestContext,
        _guest_mem: &GuestPhysMemSpace,
    ) -> Result<GuestRunResult>
    {
        Ok(GuestRunResult::HostInterrupt)
    }
}

