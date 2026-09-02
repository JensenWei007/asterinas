mod device;
mod guest_address;
mod ioctl;
mod ioeventfd;
mod vcpu;
mod vcpu_file;
mod vm;
mod vm_file;
mod vm_memory;

#[cfg(target_arch = "x86_64")]
mod apic;
#[cfg(target_arch = "x86_64")]
mod cr;
#[cfg(target_arch = "x86_64")]
mod msr;
#[cfg(target_arch = "x86_64")]
mod pio;
#[cfg(target_arch = "x86_64")]
mod cpuid;
#[cfg(target_arch = "x86_64")]
mod irqfd;
#[cfg(target_arch = "x86_64")]
mod kvmclock;
#[cfg(target_arch = "x86_64")]
mod mmio;

#[cfg(target_arch = "riscv64")]
mod aia;


pub use device::HypervisorDevice;

use crate::{device::registry::char, prelude::*};

const KVM_MAJOR: u16 = 10;
const KVM_MINOR: u16 = 232;

pub(super) fn init_in_first_process() -> Result<()> {
    char::register(Arc::new(HypervisorDevice::new()))?;
    Ok(())
}
