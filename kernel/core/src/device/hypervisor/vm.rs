use super::{
    ioctl::{
        IoEventFdConfig,
        KVM_CAP_MAX_VCPU_ID, KVM_CAP_SPLIT_IRQCHIP, KVM_IOEVENTFD_FLAG_DEASSIGN,
        KVM_IRQ_ROUTING_IRQCHIP, KVM_IRQ_ROUTING_MSI, KVM_IRQCHIP_IOAPIC, KVM_IRQFD_FLAG_DEASSIGN,
        KVM_IRQFD_FLAG_RESAMPLE,
    },
    ioeventfd::{
        IoEventAddressSpace, IoEventFdBinding, validate_config as validate_ioeventfd_config,
    },
    vcpu::Vcpu,
    vm_memory::VmMemory,
};
use crate::{events::KernelEventFile, prelude::*};
use ostd::arch::vm::vm::VmArch;

const KVM_CLOCK_REALTIME: u32 = 1 << 2;
const KVM_CLOCK_HOST_TSC: u32 = 1 << 3;

pub(super) struct Vm {
    pub(super) id: u32,
    memory: VmMemory,
    vcpus: Mutex<BTreeMap<u32, Arc<Vcpu>>>,
    arch: Arc<VmArch>,
    irqchip_created: Mutex<bool>,
    ioeventfds: Mutex<Vec<Arc<IoEventFdBinding>>>,
}

impl Vm {
    pub fn new(id: u32) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            id,
            memory: VmMemory::new()?,
            vcpus: Mutex::new(BTreeMap::new()),
            arch: VmArch::new(),
            irqchip_created: Mutex::new(false),
            ioeventfds: Mutex::new(Vec::new()),
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

    pub(super) fn configure_ioeventfd(
        &self,
        config: IoEventFdConfig,
        eventfd: Arc<KernelEventFile>,
    ) -> Result<()> {
        validate_ioeventfd_config(&config)?;
        if config.flags & KVM_IOEVENTFD_FLAG_DEASSIGN != 0 {
            let binding = {
                let mut bindings = self.ioeventfds.lock();
                let Some(index) = bindings
                    .iter()
                    .position(|binding| binding.matches_config(&config, &eventfd))
                else {
                    return_errno_with_message!(Errno::ENOENT, "ioeventfd binding does not exist");
                };
                bindings.remove(index)
            };
            binding.deactivate();
            return Ok(());
        }

        let binding = Arc::new(IoEventFdBinding::new(config, eventfd)?);
        let mut bindings = self.ioeventfds.lock();
        if bindings
            .iter()
            .any(|existing| existing.conflicts_with(&binding))
        {
            return_errno_with_message!(Errno::EEXIST, "conflicting ioeventfd binding");
        }
        bindings.push(binding);
        Ok(())
    }

    pub(super) fn signal_ioeventfd(
        &self,
        address_space: IoEventAddressSpace,
        addr: u64,
        len: u32,
        value: u64,
    ) -> bool {
        let bindings = self.ioeventfds.lock();
        let Some(binding) = bindings
            .iter()
            .find(|binding| binding.matches_io(address_space, addr, len, value))
        else {
            return false;
        };
        binding.signal();
        debug!(
            "hypervisor: ioeventfd triggered: address_space={:?}, addr={:#x}, len={}",
            address_space, addr, len
        );
        true
    }

    pub(super) fn has_ioeventfd(
        &self,
        address_space: IoEventAddressSpace,
        addr: u64,
        len: u32,
    ) -> bool {
        self.ioeventfds
            .lock()
            .iter()
            .any(|binding| binding.matches_address(address_space, addr, len))
    }

    /// Signals one matching ioeventfd for every value, or none if the entire
    /// batch cannot be handled in-kernel.
    pub(super) fn signal_ioeventfd_batch(
        &self,
        address_space: IoEventAddressSpace,
        addr: u64,
        len: u32,
        values: &[u64],
    ) -> bool {
        if values.is_empty() {
            return false;
        }

        // Keep the registry locked across both passes. Deassignment therefore
        // cannot turn an all-or-nothing batch into a partially signalled one.
        let bindings = self.ioeventfds.lock();
        let mut routes = Vec::with_capacity(values.len());
        for &value in values {
            let Some(binding) = bindings
                .iter()
                .find(|binding| binding.matches_io(address_space, addr, len, value))
            else {
                return false;
            };
            routes.push(binding.clone());
        }

        let signal_count = routes.len() as u64;
        for binding in routes {
            binding.signal();
        }
        debug!(
            "hypervisor: ioeventfd triggered: address_space={:?}, addr={:#x}, len={}, signals={}",
            address_space, addr, len, signal_count
        );
        true
    }

    fn ensure_irqchip_created(&self) -> Result<()> {
        if *self.irqchip_created.lock() {
            return Ok(());
        }

        return_errno_with_message!(Errno::EINVAL, "in-kernel irqchip has not been created");
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        //let bindings = self.irqfds.get_mut().drain(..).collect::<Vec<_>>();
        //for binding in bindings {
        //    binding.deactivate();
        //}
        debug!("hypervisor: release VM {}.", self.id);
    }
}

#[cfg(ktest)]
mod tests {
    use ostd::prelude::*;

    use super::TscWriteState;

    #[ktest]
    fn zero_tsc_writes_share_one_generation() {
        let mut state = TscWriteState::default();
        let first_offset = state.synchronize(0, 10_000, 1_000);
        let second_offset = state.synchronize(0, 10_250, 1_000);

        assert_eq!(first_offset, -10_000);
        assert_eq!(second_offset, first_offset);
    }

    #[ktest]
    fn nearby_nonzero_tsc_writes_share_one_generation() {
        let mut state = TscWriteState::default();
        let first_offset = state.synchronize(10_000, 50_000, 1_000);
        let second_offset = state.synchronize(10_500, 50_500, 1_000);

        assert_eq!(first_offset, -40_000);
        assert_eq!(second_offset, first_offset);
    }

    #[ktest]
    fn distant_tsc_write_starts_a_new_generation() {
        let mut state = TscWriteState::default();
        let first_offset = state.synchronize(10_000, 50_000, 1_000);
        let second_offset = state.synchronize(20_000, 50_500, 1_000);

        assert_eq!(first_offset, -40_000);
        assert_eq!(second_offset, -30_500);
    }
}
