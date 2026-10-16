// SPDX-License-Identifier: MPL-2.0

use crate::prelude::*;
use crate::Error;
use crate::arch::vm::vcpu::VcpuArchInner;
use super::types::KvmCpuTrap;
use super::onereg::KVM_REG_RISCV_SUBTYPE_SHIFT;
use super::onereg::{KVM_REG_RISCV, KVM_REG_SIZE_U64, KVM_REG_RISCV_SBI_STATE};

mod base;
mod forward;
mod fwft;
mod hsm;
mod ipi;
mod pmu;
mod rfence;
mod srst;
mod sta;
mod susp;
mod time;
mod v01;

use base::VCPU_SBI_EXT_BASE;
use forward::{VCPU_SBI_EXT_DBCN, VCPU_SBI_EXT_EXPERIMENTAL, VCPU_SBI_EXT_MPXY, VCPU_SBI_EXT_VENDOR};
use fwft::{KvmSbiFwft, VCPU_SBI_EXT_FWFT};
use hsm::VCPU_SBI_EXT_HSM;
use ipi::VCPU_SBI_EXT_IPI;
use pmu::VCPU_SBI_EXT_PMU;
use rfence::VCPU_SBI_EXT_RFENCE;
use srst::VCPU_SBI_EXT_SRST;
use sta::VCPU_SBI_EXT_STA;
use susp::VCPU_SBI_EXT_SUSP;
use time::VCPU_SBI_EXT_TIME;
use v01::VCPU_SBI_EXT_V01;

const KVM_REG_RISCV_SBI_STA: usize = 0x0 << KVM_REG_RISCV_SUBTYPE_SHIFT;
const KVM_REG_RISCV_SBI_FWFT: usize = 0x1 << KVM_REG_RISCV_SUBTYPE_SHIFT;

/// The status of SBI extensions
#[derive(Clone, Copy, PartialEq)]
enum KvmRiscvSbiExtStatus {
	Uninitialized,
	Unavailable,
	Enabled,
	Disabled,
}

/// SBI extension IDs specific to KVM. This is not the same as the SBI
/// extension IDs defined by the RISC-V SBI specification.
#[derive(Clone, Copy)]
enum KvmRiscvSbiExtId {
	V01,
	Time,
	Ipi,
	Rfence,
	Srst,
	Hsm,
	Pmu,
	Experimental,
	Vendor,
	Dbcn,
	Sta,
	Susp,
	Fwft,
	Mpxy,
	Max,
}

/// The Sbi context of Vcpu
#[repr(C)]
pub(super) struct VcpuSbiContext {
	return_handled: usize,
	ext_status: [KvmRiscvSbiExtStatus; KvmRiscvSbiExtId::Max as usize + 1],
    /// Firmware feature SBI extension context
	fwft_context: KvmSbiFwft,
    shmem: u64,
	last_steal:u64,
}

impl VcpuSbiContext {
    /// Create a new SbiContext
    pub(super) fn new() -> Self {
        Self {
            return_handled: 0,
            ext_status: [KvmRiscvSbiExtStatus::Uninitialized; KvmRiscvSbiExtId::Max as usize + 1],
            fwft_context: KvmSbiFwft::new(),
            shmem: 0,
            last_steal: 0,
        }
    }

    /// Init the SbiContext
    pub(super) fn init(&mut self) -> Result<()> {
        for &(ext_id, ext) in SBI_EXT_TABLE.iter() {
            let idx = ext_id as usize;
            self.ext_status[idx] = if ext.default_disabled {
                KvmRiscvSbiExtStatus::Disabled
            } else {
                KvmRiscvSbiExtStatus::Enabled
            };
            if let Some(func) = ext.init {
                func(self)?;
            }
        }
        Ok(())
    }
}

struct KvmRiscvSbiExtensionEntry {
	ext_idx: KvmRiscvSbiExtId,
	ext: &'static KvmVcpuSbiExtension,
}

/// The return value of SbiHandler
#[derive(Clone, Copy, Debug, Default)]
struct KvmVcpuSbiReturn {
	pub out_val: usize,
	pub err_val: usize,
	pub utrap: KvmCpuTrap,
	pub uexit: bool,
}

enum KvmVcpuSbiReturnError {
    Failure	= -1,
    NotSupported = -2,
    InvalidParam = -3,
    Denied = -4,
    InvalidAddress	= -5,
    AlreadyAvailable = -6,
    AlreadyStarted = -7,
    AlreadyStopped = -8,
    NoShmem = -9,
    InvalidState = -10,
    BadRange = -11,
    Timeout = -12,
    Io = -13,
    DeniedLocked = -14,
}

type SbiHandlerFunction = dyn Fn(&VcpuArchInner) -> Result<KvmVcpuSbiReturn> + Sync + Send + 'static;
type SbiProbeFunction = dyn Fn() -> Result<KvmVcpuSbiReturn> + Sync + Send + 'static;
type SbiInitFunction = dyn Fn(&mut VcpuSbiContext) -> Result<usize> + Sync + Send + 'static;
type SbiDeinitFunction = dyn Fn() -> Result<KvmVcpuSbiReturn> + Sync + Send + 'static;
type SbiResetFunction = dyn Fn(&mut VcpuSbiContext) -> Result<usize> + Sync + Send + 'static;
type SbiGetStateRegIdFunction = dyn Fn(usize) -> Result<usize> + Sync + Send + 'static;
type SbiGetStateRegCountFunction = dyn Fn() -> usize + Sync + Send + 'static;
type SbiGetStateRegFunction = dyn Fn() -> Result<KvmVcpuSbiReturn> + Sync + Send + 'static;
type SbiSetStateRegFunction = dyn Fn() -> Result<KvmVcpuSbiReturn> + Sync + Send + 'static;

struct KvmVcpuSbiExtension {
	extid_start: usize,
	extid_end: usize,

	default_disabled: bool,

	/// SBI extension handler. It can be defined for a given extension or group of
	/// extension. But it should always return linux error codes rather than SBI
	/// specific error codes.
	handler: &'static SbiHandlerFunction,

    /// Extension specific probe function
    probe: Option<&'static SbiProbeFunction>,

	/// Init/deinit function called once during VCPU init/destroy. These
	/// might be use if the SBI extensions need to allocate or do specific
	/// init time only configuration.
	init: Option<&'static SbiInitFunction>,
    deinit: Option<&'static SbiDeinitFunction>,

    reset: Option<&'static SbiResetFunction>,

    state_reg_subtype: usize,
    get_state_reg_count: Option<&'static SbiGetStateRegCountFunction>,
	get_state_reg_id: Option<&'static SbiGetStateRegIdFunction>,
    get_state_reg: Option<&'static SbiGetStateRegFunction>,
    set_state_reg: Option<&'static SbiSetStateRegFunction>,
}

impl KvmVcpuSbiExtension {
    /// Create an SBI extension with default optional fields.
    ///
    /// `default_disabled`, `probe`, `init`, `deinit`, `reset`,
    /// `state_reg_subtype`, and all state-reg hooks default to
    /// disabled/zero; use struct update syntax to override only the
    /// fields an extension actually needs:
    ///
    /// ```ignore
    /// KvmVcpuSbiExtension {
    ///     default_disabled: true,
    ///     init: Some(&sbi_ext_x_init),
    ///     ..KvmVcpuSbiExtension::new(SBI_EXT_X, SBI_EXT_X, &sbi_ext_x_handler)
    /// }
    /// ```
    pub(super) const fn new(
        extid_start: usize,
        extid_end: usize,
        handler: &'static SbiHandlerFunction,
    ) -> Self {
        Self {
            extid_start,
            extid_end,
            default_disabled: false,
            handler,
            probe: None,
            init: None,
            deinit: None,
            reset: None,
            state_reg_subtype: 0,
            get_state_reg_count: None,
            get_state_reg_id: None,
            get_state_reg: None,
            set_state_reg: None,
        }
    }
}

const SBI_EXT_TABLE_SIZE: usize = 15;

/// `SBI_EXT_TABLE`: each entry pairs an extension with its
/// `KvmRiscvSbiExtId` slot. `VCPU_SBI_EXT_BASE` intentionally maps to
/// `KvmRiscvSbiExtId::Max` (the sentinel slot), so BASE does not occupy
/// a regular `ext_status` slot.
const SBI_EXT_TABLE: [(KvmRiscvSbiExtId, &KvmVcpuSbiExtension); SBI_EXT_TABLE_SIZE] = [
    (KvmRiscvSbiExtId::V01,          &VCPU_SBI_EXT_V01),
    (KvmRiscvSbiExtId::Max,          &VCPU_SBI_EXT_BASE),
    (KvmRiscvSbiExtId::Time,         &VCPU_SBI_EXT_TIME),
    (KvmRiscvSbiExtId::Ipi,          &VCPU_SBI_EXT_IPI),
    (KvmRiscvSbiExtId::Rfence,       &VCPU_SBI_EXT_RFENCE),
    (KvmRiscvSbiExtId::Srst,         &VCPU_SBI_EXT_SRST),
    (KvmRiscvSbiExtId::Hsm,          &VCPU_SBI_EXT_HSM),
    (KvmRiscvSbiExtId::Pmu,          &VCPU_SBI_EXT_PMU),
    (KvmRiscvSbiExtId::Dbcn,         &VCPU_SBI_EXT_DBCN),
    (KvmRiscvSbiExtId::Susp,         &VCPU_SBI_EXT_SUSP),
    (KvmRiscvSbiExtId::Sta,          &VCPU_SBI_EXT_STA),
    (KvmRiscvSbiExtId::Fwft,         &VCPU_SBI_EXT_FWFT),
    (KvmRiscvSbiExtId::Mpxy,         &VCPU_SBI_EXT_MPXY),
    (KvmRiscvSbiExtId::Experimental, &VCPU_SBI_EXT_EXPERIMENTAL),
    (KvmRiscvSbiExtId::Vendor,       &VCPU_SBI_EXT_VENDOR),
];

fn vcpu_get_sbi_ext_idx(idx: usize) -> Result<usize> {
    SBI_EXT_TABLE
        .iter()
        .position(|(_, ext)| (ext.extid_start..=ext.extid_end).contains(&idx))
        .ok_or(Error::NotFound)
}

pub(super) fn reset_sbi(inner: &mut VcpuArchInner) -> Result<()> {
    for &(ext_id, ext) in SBI_EXT_TABLE.iter() {
        if let Some(func) = ext.reset {
            if inner.sbi_context.ext_status[ext_id as usize] != KvmRiscvSbiExtStatus::Enabled {
                func(&mut inner.sbi_context)?;
            }
        }
    }
    Ok(())
}

/// Handle vm's sbi_call
pub fn handle_sbi_call(inner: &mut VcpuArchInner) -> Result<usize> {
    let index = vcpu_get_sbi_ext_idx(inner.guest_context.get_a7());
    if let Ok(index) = index {
        let (_, ext) = &SBI_EXT_TABLE[index];
        let func = ext.handler;

        let rt = func(inner)?;

        if !rt.uexit {
            inner.guest_context.set_a0(rt.err_val);
        }

        inner.guest_context.step_sepc(4);
        inner.guest_context.set_a1(rt.out_val);
    } else {
        inner.guest_context.set_a0(usize::MAX - 1);
        inner.guest_context.step_sepc(4);
        inner.guest_context.set_a1(0);
    }
    Ok(1)
}

pub(super) fn state_reg_count(inner: &VcpuArchInner, mut vec: Option<&mut Vec<usize>>) -> Result<usize> {
    let mut n = 0;

    for &(ext_id, ext) in SBI_EXT_TABLE.iter() {
        if ext.get_state_reg_count.is_none()
            || inner.sbi_context.ext_status[ext_id as usize] != KvmRiscvSbiExtStatus::Enabled {
            continue;
        }

        if let Some(func) = ext.get_state_reg_count {
            let state_reg_count = func();

            for j in 0..state_reg_count {
                if let Some(vec) = vec.as_deref_mut() {
                    if let Some(func) = ext.get_state_reg_id {
                        vec.push(func(j)?);
                    } else {
                        let reg = KVM_REG_RISCV | KVM_REG_SIZE_U64 | KVM_REG_RISCV_SBI_STATE
                            | ext.state_reg_subtype | j;
                        vec.push(reg);
                    }
                }
            }
            n += state_reg_count;
        }
    }

    Ok(n)
}
