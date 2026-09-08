//! Ioctl api compatible with Linux KVM.
//! KVM api: https://www.kernel.org/doc/html/latest/virt/kvm/api.html

use ostd::{
    arch::vm::{
        VcpuDtable as ArchVcpuDtable, VcpuRegs as ArchVcpuRegs, VcpuRunState,
        VcpuSegment as ArchVcpuSegment, VcpuSregs as ArchVcpuSregs,
    },
    cpu::num_cpus,
    mm::VmIo,
    task::Task,
};

use crate::{
    context::current_userspace,
    prelude::*,
    util::ioctl::{InData, InOutData, NoData, OutData, RawIoctl, ioc},
};

const KVM_INTERRUPT_BITMAP_WORDS: usize = (256 + 63) / 64;
const KVM_APIC_REG_SIZE: usize = 0x400;
pub(super) const KVM_MEM_READONLY: u32 = 1 << 1;

pub(super) const KVM_API_VERSION: i32 = 12;
pub(super) const KVM_MAX_VCPUS: i32 = 64;

pub(super) const KVM_CAP_IRQCHIP: usize = 0;
pub(super) const KVM_CAP_USER_MEMORY: usize = 3;
pub(super) const KVM_CAP_NR_VCPUS: usize = 9;
pub(super) const KVM_CAP_NR_MEMSLOTS: usize = 10;
pub(super) const KVM_CAP_MP_STATE: usize = 14;
pub(super) const KVM_CAP_COALESCED_MMIO: usize = 15;
pub(super) const KVM_CAP_DESTROY_MEMORY_REGION_WORKS: usize = 21;
pub(super) const KVM_CAP_IOEVENTFD: usize = 36;
pub(super) const KVM_CAP_ENABLE_CAP: usize = 54;
pub(super) const KVM_CAP_XSAVE: usize = 55;
pub(super) const KVM_CAP_GET_TSC_KHZ: usize = 61;
pub(super) const KVM_CAP_MAX_VCPUS: usize = 66;
pub(super) const KVM_CAP_ONE_REG: usize = 70;
pub(super) const KVM_CAP_TSC_DEADLINE_TIMER: usize = 72;
pub(super) const KVM_CAP_SIGNAL_MSI: usize = 77;
pub(super) const KVM_CAP_READONLY_MEM: usize = 81;

pub(super) const KVM_CAP_ENABLE_CAP_VM: usize = 98;
pub(super) const KVM_CAP_SPLIT_IRQCHIP: usize = 121;
pub(super) const KVM_CAP_IOEVENTFD_ANY_LENGTH: usize = 122;
pub(super) const KVM_CAP_MAX_VCPU_ID: usize = 128;
pub(super) const KVM_CAP_IMMEDIATE_EXIT: usize = 136;
pub(super) const KVM_CAP_COALESCED_PIO: usize = 162;
pub(super) const KVM_CAP_DIRTY_LOG_RING: usize = 192;
pub(super) const KVM_CAP_VM_GPA_BITS: usize = 207;
pub(super) const KVM_CAP_DIRTY_LOG_RING_ACQ_REL: usize = 223;
pub(super) const KVM_CAP_RISCV_MP_STATE_RESET: usize = 242;

pub(super) const KVM_IRQCHIP_PIC_MASTER: u32 = 0;
pub(super) const KVM_IRQCHIP_PIC_SLAVE: u32 = 1;
pub(super) const KVM_IRQCHIP_IOAPIC: u32 = 2;
pub(super) const KVM_IRQ_ROUTING_IRQCHIP: u32 = 1;
pub(super) const KVM_IRQ_ROUTING_MSI: u32 = 2;
pub(super) const KVM_IOEVENTFD_FLAG_DATAMATCH: u32 = 1 << 0;
pub(super) const KVM_IOEVENTFD_FLAG_PIO: u32 = 1 << 1;
pub(super) const KVM_IOEVENTFD_FLAG_DEASSIGN: u32 = 1 << 2;
pub(super) const KVM_IRQFD_FLAG_DEASSIGN: u32 = 1 << 0;
pub(super) const KVM_IRQFD_FLAG_RESAMPLE: u32 = 1 << 1;
pub(super) const KVM_MAX_IRQ_ROUTES: usize = 4096;
pub(super) const KVM_MAX_NR_MEMSLOTS: i32 = 32;
const KVM_IRQCHIP_PAYLOAD_SIZE: usize = 512;

pub(super) const KVM_MP_STATE_RUNNABLE: u32 = 0;
pub(super) const KVM_MP_STATE_UNINITIALIZED: u32 = 1;
pub(super) const KVM_MP_STATE_INIT_RECEIVED: u32 = 2;
pub(super) const KVM_MP_STATE_HALTED: u32 = 3;

pub(super) const KVM_COALESCED_MMIO_PAGE_OFFSET: usize = 2;
pub(super) const KVM_RUN_MMAP_SIZE: usize = (KVM_COALESCED_MMIO_PAGE_OFFSET + 1) * PAGE_SIZE;
pub(super) const KVM_RUN_STRUCT_SIZE: usize = 2352;
const KVM_RUN_EXIT_DATA_OFFSET: usize = 32;
const KVM_RUN_EXIT_DATA_SIZE: usize = KVM_RUN_STRUCT_SIZE - KVM_RUN_EXIT_DATA_OFFSET;

pub(super) const KVM_RUN_IMMEDIATE_EXIT_OFFSET: usize = 1;
pub(super) const KVM_RUN_EXIT_REASON_OFFSET: usize = 8;
pub(super) const KVM_RUN_READY_FOR_INTERRUPT_INJECTION_OFFSET: usize = 12;
pub(super) const KVM_RUN_IF_FLAG_OFFSET: usize = 13;
pub(super) const KVM_RUN_FLAGS_OFFSET: usize = 14;
pub(super) const KVM_RUN_CR8_OFFSET: usize = 16;
pub(super) const KVM_RUN_APIC_BASE_OFFSET: usize = 24;

pub(super) const KVM_RUN_IO_DIRECTION_OFFSET: usize = 32;
pub(super) const KVM_RUN_IO_SIZE_OFFSET: usize = 33;
pub(super) const KVM_RUN_IO_PORT_OFFSET: usize = 34;
pub(super) const KVM_RUN_IO_COUNT_OFFSET: usize = 36;
pub(super) const KVM_RUN_IO_DATA_OFFSET_OFFSET: usize = 40;
pub(super) const KVM_RUN_IO_DATA_OFFSET: usize = 2560;
pub(super) const KVM_RUN_IO_DATA_CAPACITY: usize = PAGE_SIZE;

pub(super) const KVM_RUN_MMIO_PHYS_ADDR_OFFSET: usize = 32;
pub(super) const KVM_RUN_MMIO_DATA_OFFSET: usize = 40;
pub(super) const KVM_RUN_MMIO_LEN_OFFSET: usize = 48;
pub(super) const KVM_RUN_MMIO_IS_WRITE_OFFSET: usize = 52;

pub(super) const KVM_EXIT_IO: u32 = 2;
pub(super) const KVM_EXIT_HLT: u32 = 5;
pub(super) const KVM_EXIT_MMIO: u32 = 6;
pub(super) const KVM_EXIT_SHUTDOWN: u32 = 8;
pub(super) const KVM_EXIT_INTERNAL_ERROR: u32 = 17;

pub(super) const KVM_EXIT_IO_IN: u8 = 0;
pub(super) const KVM_EXIT_IO_OUT: u8 = 1;

pub(super) const IA32_TSC_DEADLINE: u32 = 0x6e0;

// KVM _IO commands may still pass scalar values in the ioctl argument.
// The command word itself encodes no direction or data size for them.

// System ioctls.
pub(super) type GetApiVersion = ioc!(KVM_GET_API_VERSION, 0xAE, 0x00, NoData);
pub(super) type CreateVm = ioc!(KVM_CREATE_VM, 0xAE, 0x01, NoData);
//pub(super) type GetMsrIndexList = ioc!(KVM_GET_MSR_INDEX_LIST, 0xAE, 0x02, InOutData<MsrList>);
pub(super) type CheckExtension = ioc!(KVM_CHECK_EXTENSION, 0xAE, 0x03, NoData);
pub(super) type GetVcpuMmapSize = ioc!(KVM_GET_VCPU_MMAP_SIZE, 0xAE, 0x04, NoData);

// VM ioctls.
pub(super) type CreateVcpu = ioc!(KVM_CREATE_VCPU, 0xAE, 0x41, NoData);
pub(super) type SetUserMemoryRegion = ioc!(
    KVM_SET_USER_MEMORY_REGION,
    0xAE,
    0x46,
    InData<UserMemoryRegion>
);
pub(super) type IoEventFd = ioc!(KVM_IOEVENTFD, 0xAE, 0x79, InData<IoEventFdConfig>);
pub(super) type EnableCap = ioc!(KVM_ENABLE_CAP, 0xAE, 0xa3, InData<EnableCapData>);

// VCPU ioctls.
pub(super) type Run = ioc!(KVM_RUN, 0xAE, 0x80, NoData);
pub(super) type GetOneReg = ioc!(KVM_GET_ONE_REG, 0xAE, 0xAB, OutData<OneReg>);
pub(super) type SetOneReg = ioc!(KVM_SET_ONE_REG, 0xAE, 0xAC, InData<OneReg>);
pub(super) type GetRegList = ioc!(KVM_GET_REG_LIST, 0xAE, 0xB0, OutData<RegList>);
pub(super) type SetMpState = ioc!(KVM_SET_MP_STATE, 0xAE, 0x99, InData<MpState>);

#[cfg(target_arch = "riscv64")]
pub(super) fn check_extension(raw_ioctl: RawIoctl) -> i32 {
    match raw_ioctl.arg() {
        KVM_CAP_IOEVENTFD
        | KVM_CAP_USER_MEMORY
        | KVM_CAP_DESTROY_MEMORY_REGION_WORKS
        | KVM_CAP_COALESCED_MMIO
        | KVM_CAP_COALESCED_PIO
//        | KVM_CAP_READONLY_MEM //TODOWJX: impl this
        | KVM_CAP_MP_STATE
//        | KVM_CAP_SET_GUEST_DEBUG  //TODOWJX: impl this
        | KVM_CAP_IMMEDIATE_EXIT => 1,
        KVM_CAP_NR_VCPUS => 1,//TODO: change 
        KVM_CAP_MAX_VCPUS => KVM_MAX_VCPUS,
        KVM_CAP_NR_MEMSLOTS => KVM_MAX_NR_MEMSLOTS,

        // TODO: Report capabilities from the actual hypervisor implementation.
        _ => 0,
    }
}

pub(super) fn read_vcpu_id(raw_ioctl: RawIoctl) -> Result<u32> {
    Ok(u32::try_from(raw_ioctl.arg())?)
}

/// `struct kvm_userspace_memory_region`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub(super) struct UserMemoryRegion {
    pub slot: u32,
    pub flags: u32,
    pub guest_phys_addr: u64,
    pub memory_size: u64,
    pub userspace_addr: u64,
}

/// The common `struct kvm_ioeventfd`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod)]
pub(super) struct IoEventFdConfig {
    pub datamatch: u64,
    pub addr: u64,
    pub len: u32,
    pub fd: i32,
    pub flags: u32,
    pub pad: [u8; 36],
}

impl Default for IoEventFdConfig {
    fn default() -> Self {
        Self {
            datamatch: 0,
            addr: 0,
            len: 0,
            fd: 0,
            flags: 0,
            pad: [0; 36],
        }
    }
}

/// The common `struct kvm_enable_cap`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod)]
pub(super) struct EnableCapData {
    pub cap: u32,
    pub flags: u32,
    pub args: [u64; 4],
    pub pad: [u8; 64],
}

impl Default for EnableCapData {
    fn default() -> Self {
        Self {
            cap: 0,
            flags: 0,
            args: [0; 4],
            pad: [0; 64],
        }
    }
}

/// The common `struct kvm_mp_state`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub(super) struct MpState {
    pub mp_state: u32,
}

impl From<VcpuRunState> for MpState {
    fn from(state: VcpuRunState) -> Self {
        Self {
            mp_state: match state {
                VcpuRunState::Runnable | VcpuRunState::Running => KVM_MP_STATE_RUNNABLE,
                VcpuRunState::Uninitialized => KVM_MP_STATE_UNINITIALIZED,
                VcpuRunState::WaitForSipi => KVM_MP_STATE_INIT_RECEIVED,
                VcpuRunState::Halted => KVM_MP_STATE_HALTED,
            },
        }
    }
}

impl TryFrom<MpState> for VcpuRunState {
    type Error = Error;

    fn try_from(state: MpState) -> core::result::Result<Self, Self::Error> {
        match state.mp_state {
            KVM_MP_STATE_RUNNABLE => Ok(Self::Runnable),
            KVM_MP_STATE_UNINITIALIZED => Ok(Self::Uninitialized),
            KVM_MP_STATE_INIT_RECEIVED => Ok(Self::WaitForSipi),
            KVM_MP_STATE_HALTED => Ok(Self::Halted),
            _ => Err(Error::with_message(
                Errno::EINVAL,
                "unsupported KVM MP state",
            )),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub struct OneReg {
	id: u64,
	addr: u64,
}

use core::slice;

#[repr(C)]
pub struct RegList {
    /// The number of regs
    pub n: u64,
    // Here is a FLEX_ARRAY, but in rust we do not write it
}

/* 
pub fn get_reg_list(list: *const RegList) -> &'static [u64] {
    unsafe {
        let n = (*list).n as usize;
        let data_ptr = (list as *const u8).add(core::mem::size_of::<u64>()) as *const u64;
        slice::from_raw_parts(data_ptr, n)
    }
}

pub fn get_reg_list_mut(list: *mut RegList) -> &'static mut [u64] {
    unsafe {
        let n = (*list).n as usize;
        let data_ptr = (list as *mut u8).add(core::mem::size_of::<u64>()) as *mut u64;
        slice::from_raw_parts_mut(data_ptr, n)
    }
}
*/

#[cfg(target_arch = "riscv64")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub(super) struct VcpuRegs {
}

#[cfg(target_arch = "riscv64")]
impl From<ArchVcpuRegs> for VcpuRegs {
    fn from(regs: ArchVcpuRegs) -> Self {
        Self {
        }
    }
}

#[cfg(target_arch = "riscv64")]
impl From<VcpuRegs> for ArchVcpuRegs {
    fn from(regs: VcpuRegs) -> Self {
        Self {
        }
    }
}

#[cfg(target_arch = "riscv64")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod)]
pub(super) struct KvmRun {
    pub request_interrupt_window: u8,
    pub immediate_exit: u8,
    pub padding1: [u8; 6],
    pub exit_reason: u32,
    pub ready_for_interrupt_injection: u8,
    pub if_flag: u8,
    pub flags: u16,
    pub cr8: u64,
    pub apic_base: u64,
    pub exit_data: [u8; KVM_RUN_EXIT_DATA_SIZE],
}
