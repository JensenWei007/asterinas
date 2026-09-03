use crate::sync::SpinLock;
use crate::cpu::num_cpus;

use super::csr::*;
use super::gstage::*;
use super::tlb::*;

static VMID_VERSION: SpinLock<usize> = SpinLock::new(1);
static VMID_NEXT: SpinLock<usize> = SpinLock::new(0);
static VMID_BITS: SpinLock<usize> = SpinLock::new(0);
static VMID_LOCK: SpinLock<usize> = SpinLock::new(0);

/// Vmid for KVM
pub struct Vmid {
    /// Writes to vmid_version and vmid happen with vmid_lock held
	/// whereas reads happen without any lock held.
    pub vmid_version: usize,
    /// 1
    pub vmid: usize,
}

/// 1
pub fn gstage_vmid_detect(){
    // Figure-out number of VMID bits in HW
    let mode = (HgatpMode::from_pgd_levels(GSTAGE_MAX_PGD_LEVELS) as usize) << HGATP_MODE_SHIFT;
    let val = mode | HGATP_VMID;
    let mut vmid_bits = VMID_BITS.lock();
    unsafe{
        Hgatp::write(val);
        *vmid_bits = Hgatp::read();
        *vmid_bits = (*vmid_bits & HGATP_VMID) >> HGATP_VMID_SHIFT;
        *vmid_bits = fls(*vmid_bits);
        Hgatp::write(0);
    }

    // We polluted local TLB so flush all guest TLB
    local_hfence_gvma_all();

    // We don't use VMID bits if they are not sufficient
    if (1 << *vmid_bits) < num_cpus(){
        *vmid_bits = 0;
    }
}


/// 1
pub fn fls(x: usize) -> usize {
    if x == 0 {
        return 0;
    }
    usize::BITS as usize - x.leading_zeros() as usize
}