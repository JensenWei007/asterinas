/// 1
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestTimerInstant {
    /// 1
    pub tsc: u64,
}
/// 1
pub struct GuestInterrupt {}
/// 1
pub struct VcpuDtable {}
/// 1
pub struct VcpuRegs {}
/// 1
pub struct VcpuSegment {}
/// 1
pub struct VcpuSregs {}

pub const RISCV_ISA_EXT_MAX: usize = 128;

pub const SATP_PPN:u64 = 0x00000FFFFFFFFFFF;
pub const SATP_MODE_39:u64 = 0x8000000000000000;
pub const SATP_MODE_48:u64 = 0x9000000000000000;
pub const SATP_MODE_57:u64 = 0xa000000000000000;
pub const SATP_MODE_SHIFT:u64 = 	60;
pub const SATP_ASID_BITS:u64 = 	16;
pub const SATP_ASID_SHIFT:u64 = 	44;
pub const SATP_ASID_MASK:u64 = 0xFFFF;

pub const IRQ_S_SOFT:u64 =		1;
pub const IRQ_VS_SOFT:u64 =		2;
pub const IRQ_M_SOFT:u64 =		3;
pub const IRQ_S_TIMER:u64 =		5;
pub const IRQ_VS_TIMER:u64 =		6;
pub const IRQ_M_TIMER:u64 =		7;
pub const IRQ_S_EXT:u64 =		9;
pub const IRQ_VS_EXT:u64 =		10;
pub const IRQ_M_EXT:u64 =		11;
pub const IRQ_S_GEXT:u64 =		12;
pub const IRQ_PMU_OVF:u64 =		13;
pub const IRQ_LOCAL_MAX:u64 = IRQ_PMU_OVF + 1;
pub const IRQ_LOCAL_MASK:u64 = 0x3FFF;

pub const VSIP_TO_HVIP_SHIFT: u64 = 	IRQ_VS_SOFT - IRQ_S_SOFT;
pub const VSIP_VALID_MASK:u64 =	(1 << IRQ_S_SOFT) | (1 << IRQ_S_TIMER) | (1 << IRQ_S_EXT) | (1 << IRQ_PMU_OVF);

pub const KVM_HEDELEG_DEFAULT:usize = 0xB30D;
pub const KVM_HIDELEG_DEFAULT:usize = 0x444;

#[derive(Clone, Copy, Debug, Default, Pod)]
pub struct KvmVcpuResetState {
	pc: usize,
	a1: usize,
}

/// The common `struct kvm_mp_state`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub struct MpState {
    pub mp_state: u32,
}

#[derive(Clone, Copy, Debug, Default, Pod)]
pub struct KvmVcpuConfig {
	pub henvcfg: usize,
	pub hstateen0: usize,
	pub hedeleg: usize,
	pub hideleg: usize,
}
