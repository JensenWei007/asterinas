// SPDX-License-Identifier: MPL-2.0

//! Intel VMX platform lifecycle management.

mod instructions;

#[cfg(all(ktest, feature = "vmx_ktest"))]
use core::sync::atomic::{AtomicU32, Ordering};

use x86::msr::rdmsr;
use x86_64::registers::control::{Cr0, Cr4};

#[cfg(all(ktest, feature = "vmx_ktest"))]
use crate::cpu::CpuId;
use crate::{
    Error,
    cpu::{CpuSet, all_cpus},
    cpu_local, error,
    irq::InterruptLevel,
    mm::{Frame, FrameAllocOptions, paddr_to_vaddr},
    prelude::*,
    sync::{LocalIrqDisabled, Mutex, SpinLock},
};

const IA32_FEATURE_CONTROL: u32 = 0x3a;
const IA32_VMX_BASIC: u32 = 0x480;
const IA32_VMX_CR0_FIXED0: u32 = 0x486;
const IA32_VMX_CR0_FIXED1: u32 = 0x487;
const IA32_VMX_CR4_FIXED0: u32 = 0x488;
const IA32_VMX_CR4_FIXED1: u32 = 0x489;
const CR4_VMXE: u64 = 1 << 13;

static VMX_GUARD_STATE: Mutex<VmxGuardState> = Mutex::new(VmxGuardState::new());

#[cfg(all(ktest, feature = "vmx_ktest"))]
const NO_FAILURE_CPU: u32 = u32::MAX;
#[cfg(all(ktest, feature = "vmx_ktest"))]
static FAIL_VMXON_CPU: AtomicU32 = AtomicU32::new(NO_FAILURE_CPU);
#[cfg(all(ktest, feature = "vmx_ktest"))]
static FAIL_VMXOFF_CPU: AtomicU32 = AtomicU32::new(NO_FAILURE_CPU);

cpu_local! {
    static VMX_CPU_STATE: SpinLock<VmxCpuState, LocalIrqDisabled> =
        SpinLock::new(VmxCpuState::new());
}

struct VmxCpuState {
    enabled: bool,
    region: Option<Frame<()>>,
    last_error: Option<Error>,
}

impl VmxCpuState {
    const fn new() -> Self {
        Self {
            enabled: false,
            region: None,
            last_error: None,
        }
    }
}

struct EnableError {
    error: Error,
    cleanup_complete: bool,
}

struct VmxGuardState {
    active_guards: usize,
    enabled: bool,
    poisoned: bool,
}

impl VmxGuardState {
    const fn new() -> Self {
        Self {
            active_guards: 0,
            enabled: false,
            poisoned: false,
        }
    }
}

/// Keeps VMX operation enabled while the guard exists.
#[cfg_attr(
    not(all(ktest, feature = "vmx_ktest")),
    expect(
        dead_code,
        reason = "Guest execution will acquire this guard in a follow-up PR"
    )
)]
#[must_use]
pub(crate) struct VmxGuard {
    _private: (),
}

/// Acquires a lease on the VMX platform lifecycle.
#[cfg_attr(
    not(all(ktest, feature = "vmx_ktest")),
    expect(
        dead_code,
        reason = "Guest execution will acquire this guard in a follow-up PR"
    )
)]
pub(crate) fn acquire_vmx() -> Result<VmxGuard> {
    if !InterruptLevel::current().is_task_context()
        || !crate::arch::irq::is_local_enabled()
        || crate::smp::IPI_SENDER.get().is_none()
    {
        return Err(Error::InvalidArgs);
    }

    let mut state = VMX_GUARD_STATE.lock();
    if state.poisoned {
        return Err(Error::InvalidArgs);
    }
    if state.active_guards == usize::MAX {
        return Err(Error::NotEnoughResources);
    }

    if !state.enabled {
        match enable_vmx_on_all_cpus() {
            Ok(()) => state.enabled = true,
            Err(enable_error) => {
                state.enabled = !enable_error.cleanup_complete;
                state.poisoned = !enable_error.cleanup_complete;
                return Err(enable_error.error);
            }
        }
    }
    state.active_guards += 1;

    Ok(VmxGuard { _private: () })
}

impl Drop for VmxGuard {
    fn drop(&mut self) {
        debug_assert!(InterruptLevel::current().is_task_context());
        debug_assert!(crate::arch::irq::is_local_enabled());

        let mut state = VMX_GUARD_STATE.lock();
        if state.active_guards == 0 {
            error!("VMX guard state underflow");
            return;
        }
        state.active_guards -= 1;

        if state.active_guards != 0 {
            return;
        }

        match disable_vmx_on_all_cpus() {
            Ok(()) => state.enabled = false,
            Err(err) => {
                error!("failed to disable VMX on all CPUs: {:?}", err);
                state.poisoned = true;
            }
        }
    }
}

fn enable_vmx_on_all_cpus() -> core::result::Result<(), EnableError> {
    prepare_vmxon_regions().map_err(|error| EnableError {
        error,
        cleanup_complete: true,
    })?;

    let targets = CpuSet::new_full();
    let enable_error = match run_on_cpus(&targets, enable_vmx_on_current_cpu) {
        Ok(()) => return Ok(()),
        Err(error) => error,
    };
    cleanup_prepared_regions();

    let rollback_targets = enabled_cpus();
    if rollback_targets.is_empty() {
        return Err(EnableError {
            error: enable_error,
            cleanup_complete: true,
        });
    }

    let rollback_completed = run_on_cpus(&rollback_targets, disable_vmx_on_current_cpu).is_ok();
    cleanup_prepared_regions();

    Err(EnableError {
        error: enable_error,
        cleanup_complete: rollback_completed && enabled_cpus().is_empty(),
    })
}

#[cfg_attr(
    not(all(ktest, feature = "vmx_ktest")),
    expect(
        dead_code,
        reason = "Only the follow-up user of VmxGuard will make its Drop path live"
    )
)]
fn disable_vmx_on_all_cpus() -> Result<()> {
    let targets = enabled_cpus();
    if targets.is_empty() {
        return Err(Error::InvalidArgs);
    }

    let result = run_on_cpus(&targets, disable_vmx_on_current_cpu);
    cleanup_prepared_regions();
    result
}

fn prepare_vmxon_regions() -> Result<()> {
    for cpu in all_cpus() {
        let region = match FrameAllocOptions::new().alloc_frame() {
            Ok(region) => region,
            Err(error) => {
                cleanup_prepared_regions();
                return Err(error);
            }
        };

        let mut state = VMX_CPU_STATE.get_on_cpu(cpu).lock();
        state.region = Some(region);
        state.last_error = None;
    }
    Ok(())
}

fn cleanup_prepared_regions() {
    for cpu in all_cpus() {
        let region = {
            let mut state = VMX_CPU_STATE.get_on_cpu(cpu).lock();
            if state.enabled || state.region.is_none() {
                continue;
            }
            state.last_error = None;
            state.region.take()
        };
        drop(region);
    }
}

fn enabled_cpus() -> CpuSet {
    let mut enabled = CpuSet::new_empty();
    for cpu in all_cpus() {
        if VMX_CPU_STATE.get_on_cpu(cpu).lock().enabled {
            enabled.add(cpu);
        }
    }
    enabled
}

fn run_on_cpus(targets: &CpuSet, handler: fn()) -> Result<()> {
    crate::smp::inter_processor_call(targets, handler).wait();
    first_cpu_error(targets).map_or(Ok(()), Err)
}

fn first_cpu_error(targets: &CpuSet) -> Option<Error> {
    for cpu in targets.iter() {
        let state = VMX_CPU_STATE.get_on_cpu(cpu).lock();
        if let Some(error) = state.last_error {
            return Some(error);
        }
    }
    None
}

fn enable_vmx_on_current_cpu() {
    if let Err(error) = try_enable_vmx_on_current_cpu() {
        let irq_guard = crate::irq::disable_local();
        VMX_CPU_STATE.get_with(&irq_guard).lock().last_error = Some(error);
    }
}

fn try_enable_vmx_on_current_cpu() -> Result<()> {
    let irq_guard = crate::irq::disable_local();
    let state = VMX_CPU_STATE.get_with(&irq_guard);
    let region_paddr = {
        let state = state.lock();
        if state.enabled {
            return Err(Error::InvalidArgs);
        }
        state.region.as_ref().ok_or(Error::InvalidArgs)?.paddr()
    };

    let cr4 = Cr4::read_raw();
    if cr4 & CR4_VMXE != 0 {
        return Err(Error::InvalidArgs);
    }
    let vmx_cr4 = cr4 | CR4_VMXE;
    let revision_id = read_and_validate_capability(vmx_cr4)?;
    initialize_vmxon_region(region_paddr, revision_id);

    // SAFETY: `vmx_cr4` preserves the current `CR4` value, adds only
    // `CR4.VMXE`, and has been checked against the VMX fixed-bit MSRs.
    unsafe { Cr4::write_raw(vmx_cr4) };

    #[cfg(all(ktest, feature = "vmx_ktest"))]
    if failure_injected(&FAIL_VMXON_CPU) {
        // SAFETY: The injected failure occurs before `VMXON`, so this CPU is
        // outside VMX operation.
        unsafe { clear_vmx_enable() };
        return Err(Error::IoError);
    }

    // SAFETY: The capability checks, control-register update, and initialized
    // region above establish the architectural prerequisites for `VMXON`.
    if let Err(error) = unsafe { instructions::vmxon(region_paddr) } {
        // SAFETY: A failed `VMXON` leaves this CPU outside VMX operation.
        unsafe { clear_vmx_enable() };
        return Err(error);
    }

    let mut state = state.lock();
    state.enabled = true;
    state.last_error = None;
    Ok(())
}

fn disable_vmx_on_current_cpu() {
    if let Err(error) = try_disable_vmx_on_current_cpu() {
        let irq_guard = crate::irq::disable_local();
        VMX_CPU_STATE.get_with(&irq_guard).lock().last_error = Some(error);
    }
}

fn try_disable_vmx_on_current_cpu() -> Result<()> {
    let irq_guard = crate::irq::disable_local();
    let state = VMX_CPU_STATE.get_with(&irq_guard);
    {
        let state = state.lock();
        if !state.enabled || state.region.is_none() {
            return Err(Error::InvalidArgs);
        }
    }

    #[cfg(all(ktest, feature = "vmx_ktest"))]
    if failure_injected(&FAIL_VMXOFF_CPU) {
        return Err(Error::IoError);
    }

    // SAFETY: The CPU state is marked as enabled only after a successful
    // `VMXON`, and PR1 does not create or activate any VMCS.
    unsafe { instructions::vmxoff()? };

    // SAFETY: A successful `VMXOFF` leaves this CPU outside VMX operation.
    unsafe { clear_vmx_enable() };

    let mut state = state.lock();
    state.enabled = false;
    state.last_error = None;
    Ok(())
}

/// Clears `CR4.VMXE` without changing any other `CR4` bits.
///
/// # Safety
///
/// The current CPU must be outside VMX operation.
unsafe fn clear_vmx_enable() {
    let cr4 = Cr4::read_raw();
    // SAFETY: The caller guarantees that clearing `CR4.VMXE` is permitted.
    unsafe { Cr4::write_raw(cr4 & !CR4_VMXE) };
}

fn read_and_validate_capability(vmx_cr4: u64) -> Result<u32> {
    const FEATURE_CONTROL_LOCKED: u64 = 1;
    const FEATURE_CONTROL_VMX_OUTSIDE_SMX: u64 = 1 << 2;

    let has_vmx =
        crate::arch::cpu::cpuid::cpuid(1, 0).is_some_and(|result| result.ecx & (1 << 5) != 0);
    if !has_vmx {
        return Err(Error::NotEnoughResources);
    }

    // SAFETY: A CPU that enumerates VMX provides the architectural VMX MSRs
    // read below. This function runs independently on each target CPU.
    let (feature_control, vmx_basic, cr0_fixed0, cr0_fixed1, cr4_fixed0, cr4_fixed1) = unsafe {
        (
            rdmsr(IA32_FEATURE_CONTROL),
            rdmsr(IA32_VMX_BASIC),
            rdmsr(IA32_VMX_CR0_FIXED0),
            rdmsr(IA32_VMX_CR0_FIXED1),
            rdmsr(IA32_VMX_CR4_FIXED0),
            rdmsr(IA32_VMX_CR4_FIXED1),
        )
    };

    let required_feature_control = FEATURE_CONTROL_LOCKED | FEATURE_CONTROL_VMX_OUTSIDE_SMX;
    if feature_control & required_feature_control != required_feature_control {
        return Err(Error::AccessDenied);
    }

    if !control_register_is_valid(Cr0::read_raw(), cr0_fixed0, cr0_fixed1)
        || !control_register_is_valid(vmx_cr4, cr4_fixed0, cr4_fixed1)
    {
        return Err(Error::NotEnoughResources);
    }

    Ok(vmx_basic as u32 & 0x7fff_ffff)
}

fn control_register_is_valid(value: u64, fixed0: u64, fixed1: u64) -> bool {
    value & fixed0 == fixed0 && value & !fixed1 == 0
}

#[cfg(all(ktest, feature = "vmx_ktest"))]
fn failure_injected(failure_cpu: &AtomicU32) -> bool {
    failure_cpu.load(Ordering::Acquire) == u32::from(CpuId::current_racy())
}

fn initialize_vmxon_region(region_paddr: usize, revision_id: u32) {
    let region_ptr = paddr_to_vaddr(region_paddr) as *mut u32;

    // SAFETY: `region_paddr` belongs to the live, exclusively owned frame in
    // the current CPU state. The frame is linearly mapped, page-aligned, and
    // was zero-initialized before this four-byte write.
    unsafe { region_ptr.write(revision_id) };
}

#[cfg(all(ktest, feature = "vmx_ktest"))]
pub(super) mod test_support {
    use super::*;

    #[derive(Clone, Copy)]
    pub(in crate::arch::vm) enum FailurePoint {
        Vmxon,
        Vmxoff,
    }

    pub(in crate::arch::vm) fn inject_failure(point: FailurePoint, cpu: CpuId) {
        let failure_cpu = match point {
            FailurePoint::Vmxon => &FAIL_VMXON_CPU,
            FailurePoint::Vmxoff => &FAIL_VMXOFF_CPU,
        };
        failure_cpu.store(u32::from(cpu), Ordering::Release);
    }

    pub(in crate::arch::vm) fn clear_failures() {
        FAIL_VMXON_CPU.store(NO_FAILURE_CPU, Ordering::Release);
        FAIL_VMXOFF_CPU.store(NO_FAILURE_CPU, Ordering::Release);
    }

    pub(in crate::arch::vm) fn active_guard_count() -> usize {
        VMX_GUARD_STATE.lock().active_guards
    }

    pub(in crate::arch::vm) fn is_poisoned() -> bool {
        VMX_GUARD_STATE.lock().poisoned
    }

    pub(in crate::arch::vm) fn enabled_cpu_count() -> usize {
        all_cpus()
            .filter(|cpu| VMX_CPU_STATE.get_on_cpu(*cpu).lock().enabled)
            .count()
    }

    pub(in crate::arch::vm) fn allocated_region_count() -> usize {
        all_cpus()
            .filter(|cpu| VMX_CPU_STATE.get_on_cpu(*cpu).lock().region.is_some())
            .count()
    }

    pub(in crate::arch::vm) fn recover() -> Result<()> {
        clear_failures();

        let mut state = VMX_GUARD_STATE.lock();
        if state.active_guards != 0 {
            return Err(Error::InvalidArgs);
        }
        if state.enabled {
            disable_vmx_on_all_cpus()?;
            state.enabled = false;
        }
        state.poisoned = false;
        Ok(())
    }
}
