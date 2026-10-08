use super::{
    ioctl::{
        EnableCapData,
        KVM_CAP_MAX_VCPU_ID, KVM_CAP_SPLIT_IRQCHIP,
    },
    vcpu::Vcpu,
    vm_memory::VmMemory,
};
use crate::{prelude::*};
use ostd::arch::vm::vm::VmArch;

pub(super) struct Vm {
    pub(super) id: u32,
    memory: VmMemory,
    vcpus: Mutex<BTreeMap<u32, Arc<Vcpu>>>,
    arch: Arc<VmArch>,
}

impl Vm {
    pub fn new(id: u32) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            id,
            memory: VmMemory::new()?,
            vcpus: Mutex::new(BTreeMap::new()),
            arch: VmArch::new(),
        }))
    }

    pub fn init(&self){
        self.arch.init();
    }

    pub(super) fn memory(&self) -> &VmMemory {
        &self.memory
    }

    pub(super) fn create_vcpu(self: &Arc<Self>, vcpu_id: u32) -> Result<Arc<Vcpu>> {
        let mut vcpus = self.vcpus.lock();
        if vcpus.contains_key(&vcpu_id) {
            return_errno_with_message!(Errno::EEXIST, "vCPU already exists");
        }

        #[cfg(target_arch = "riscv64")]
        let vcpu = Vcpu::new(vcpu_id, self)?;
        vcpus.insert(vcpu_id, vcpu.clone());
        drop(vcpus);
        Ok(vcpu)
    }

    pub(super) fn enable_cap(&self, cap: EnableCapData) -> Result<()> {
        match usize::try_from(cap.cap)? {
            KVM_CAP_SPLIT_IRQCHIP => {
                return_errno_with_message!(Errno::EINVAL, "split irqchip is not supported");
            }
            KVM_CAP_MAX_VCPU_ID => Ok(()),
            _ => {
                return_errno_with_message!(Errno::EINVAL, "unsupported VM capability");
            }
        }
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        debug!("kvm: release VM {}.", self.id);
    }
}
