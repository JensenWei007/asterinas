use ostd::{
    arch::vm::{GuestExitInfo, VcpuRunState, vcpu::VcpuArch}, task::Task, vm::{GuestMode, GuestRunResult},
    arch::vm::types::*,
};

use super::{
    ioctl::{VcpuRegs},
    ioeventfd::IoEventAddressSpace,
    vm::Vm,
};
use crate::prelude::*;
use ostd::sync::SpinLock;

pub struct Vcpu {
    id: u32,
    pub(super) vm: Weak<Vm>,
    //pub(super) guest_context: Mutex<GuestContext>,
    guest_mode: GuestMode,
    run_lock: Mutex<()>,
    pub arch: SpinLock<VcpuArch>,
}

impl Vcpu {
    #[cfg(target_arch = "riscv64")]
    pub(super) fn new(id: u32, vm: &Arc<Vm>) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            id,
            vm: Arc::downgrade(vm),
            //guest_context: Mutex::new(GuestContext::new(id)?),
            guest_mode: GuestMode::new()?,
            run_lock: Mutex::new(()),
            arch: SpinLock::new(VcpuArch::new()),
        }))
    }

    pub fn init(&self) {
        self.arch.lock().init();
    }

    pub fn vm(&self) -> Result<Arc<Vm>> {
        self.vm
            .upgrade()
            .ok_or_else(|| Error::with_message(Errno::ENOENT, "vm not found"))
    }

    #[cfg(target_arch = "riscv64")]
    pub(super) fn run<F>(self: &Arc<Self>, mut immediate_exit: F) -> Result<Option<GuestExitInfo>>
    where
        F: FnMut() -> Result<bool>,
    {
        let vm = self.vm()?;
        let _run_guard = self.run_lock.lock();

        loop {
            if immediate_exit()? {
                return Ok(None);
            }

            /* 
            let run_result = {
                let mut context = self.guest_context();
                match self.guest_mode.execute(
                    &mut context,
                    vm.memory().guest_mem(),
                ) {
                    Ok(exit_info) => exit_info,
                    Err(err) => {
                        error!("hypervisor: GuestMode::execute failed: {:?}", err);
                        return Err(err.into());
                    }
                }
            };
            */
            return Ok(None);
        }
    }

    /* 
    pub fn get_mp_state(&self) -> Result<MpState> {
        Ok(self.guest_context.lock().run_state().into())
    }
    */

    pub fn set_mp_state(&self, state: MpState) -> Result<()> {
        self.arch.lock().set_mp_state(state);
        Ok(())
    }
}

impl Drop for Vcpu {
    fn drop(&mut self) {
        debug!("hypervisor: release VCPU {}.", self.id);
    }
}
