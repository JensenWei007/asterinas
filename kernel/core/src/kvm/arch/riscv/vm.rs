use ostd::arch::vm::{enable_virtualization_cpu, disable_virtualization_cpu};

pub(in crate::kvm) struct VmArch {}

impl VmArch {
    pub(in crate::kvm)  fn new() -> Self {
        Self {}
    }

    pub(in crate::kvm)  fn init(&self){
        enable_virtualization_cpu();
    }
}

impl Drop for VmArch {
    fn drop(&mut self) {
        disable_virtualization_cpu()
    }
}
