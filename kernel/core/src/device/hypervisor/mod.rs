mod device;
mod guest_address;
mod ioctl;
mod ioeventfd;
mod vcpu;
mod vcpu_file;
mod vm;
mod vm_file;
mod vm_memory;

pub use device::HypervisorDevice;

use ostd::arch::vm::kvm_arch_init;

use crate::{device::registry::char, prelude::*};

const KVM_MAJOR: u16 = 10;
const KVM_MINOR: u16 = 232;

pub(super) fn init_in_first_process() -> Result<()> {
    kvm_arch_init();
    char::register(Arc::new(HypervisorDevice::new()))?;
    Ok(())
}
