// SPDX-License-Identifier: MPL-2.0

//! KVM device (kvm) support.
//!
//! KVM device with major number 10.
//! Devices appear as `/dev/kvm`

use ostd::arch::vm::kvm_arch_init;

mod device;
mod ioctl;
mod vcpu;
mod vcpu_file;
mod vm;
mod vm_file;
mod vm_memory;

use device::KvmDevice;

use crate::{device::registry::char, prelude::*};

const KVM_MAJOR: u16 = 10;
const KVM_MINOR: u16 = 232;

pub(super) fn init_in_first_process() -> Result<()> {
    kvm_arch_init();
    char::register(Arc::new(KvmDevice::new()))?;
    Ok(())
}
