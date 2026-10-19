// SPDX-License-Identifier: MPL-2.0

use crate::arch::cpu::extension::{IsaExtensions, has_extensions};

pub(super) const KVM_RISCV_BASE_ISA_MASK: usize = 0x0000000003FFFFFF;

/// ISA extension IDs specific to KVM.
/// This is not the same as the host ISA extension IDs,
/// as that is internal to the host and should not be exposed to the guest.
/// This should always be contiguous
/// to keep the mapping simple in the KVM implementation.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum KvmRiscvIsaExtId {
    A = 0,
    C,
    D,
    F,
    H,
    I,
    M,
    SVPBMT,
    SSTC,
    SVINVAL,
    ZIHINTPAUSE,
    ZICBOM,
    ZICBOZ,
    ZBB,
    SSAIA,
    V,
    SVNAPOT,
    ZBA,
    ZBS,
    ZICNTR,
    ZICSR,
    ZIFENCEI,
    ZIHPM,
    SMSTATEEN,
    ZICOND,
    ZBC,
    ZBKB,
    ZBKC,
    ZBKX,
    ZKND,
    ZKNE,
    ZKNH,
    ZKR,
    ZKSED,
    ZKSH,
    ZKT,
    ZVBB,
    ZVBC,
    ZVKB,
    ZVKG,
    ZVKNED,
    ZVKNHA,
    ZVKNHB,
    ZVKSED,
    ZVKSH,
    ZVKT,
    ZFH,
    ZFHMIN,
    ZIHINTNTL,
    ZVFH,
    ZVFHMIN,
    ZFA,
    ZTSO,
    ZACAS,
    SSCOFPMF,
    ZIMOP,
    ZCA,
    ZCB,
    ZCD,
    ZCF,
    ZCMOP,
    ZAWRS,
    SMNPM,
    SSNPM,
    SVADE,
    SVADU,
    SVVPTC,
    ZABHA,
    ZICCRSE,
    ZAAMO,
    ZALRSC,
    ZICBOP,
    ZFBFMIN,
    ZVFBFMIN,
    ZVFBFWMA,
    ZCLSD,
    ZILSD,
    ZALASR,
    MAX,
}

impl KvmRiscvIsaExtId {
    /// Map a KVM-specific ISA extension ID to the host's `IsaExtensions`.
    ///
    /// Keep this `match` as the single source of truth; do not add a
    /// parallel array, as the two orderings will drift.
    fn to_isa_ext(self) -> IsaExtensions {
        match self {
            KvmRiscvIsaExtId::A => IsaExtensions::A,
            KvmRiscvIsaExtId::C => IsaExtensions::C,
            KvmRiscvIsaExtId::D => IsaExtensions::D,
            KvmRiscvIsaExtId::F => IsaExtensions::F,
            KvmRiscvIsaExtId::H => IsaExtensions::H,
            KvmRiscvIsaExtId::I => IsaExtensions::I,
            KvmRiscvIsaExtId::M => IsaExtensions::M,
            KvmRiscvIsaExtId::SVPBMT => IsaExtensions::SVPBMT,
            KvmRiscvIsaExtId::SSTC => IsaExtensions::SSTC,
            KvmRiscvIsaExtId::SVINVAL => IsaExtensions::SVINVAL,
            KvmRiscvIsaExtId::ZIHINTPAUSE => IsaExtensions::ZIHINTPAUSE,
            KvmRiscvIsaExtId::ZICBOM => IsaExtensions::ZICBOM,
            KvmRiscvIsaExtId::ZICBOZ => IsaExtensions::ZICBOZ,
            KvmRiscvIsaExtId::ZBB => IsaExtensions::ZBB,
            KvmRiscvIsaExtId::SSAIA => IsaExtensions::SSAIA,
            KvmRiscvIsaExtId::V => IsaExtensions::V,
            KvmRiscvIsaExtId::SVNAPOT => IsaExtensions::SVNAPOT,
            KvmRiscvIsaExtId::ZBA => IsaExtensions::ZBA,
            KvmRiscvIsaExtId::ZBS => IsaExtensions::ZBS,
            KvmRiscvIsaExtId::ZICNTR => IsaExtensions::ZICNTR,
            KvmRiscvIsaExtId::ZICSR => IsaExtensions::ZICSR,
            KvmRiscvIsaExtId::ZIFENCEI => IsaExtensions::ZIFENCEI,
            KvmRiscvIsaExtId::ZIHPM => IsaExtensions::ZIHPM,
            KvmRiscvIsaExtId::SMSTATEEN => IsaExtensions::SMSTATEEN,
            KvmRiscvIsaExtId::ZICOND => IsaExtensions::ZICOND,
            KvmRiscvIsaExtId::ZBC => IsaExtensions::ZBC,
            KvmRiscvIsaExtId::ZBKB => IsaExtensions::ZBKB,
            KvmRiscvIsaExtId::ZBKC => IsaExtensions::ZBKC,
            KvmRiscvIsaExtId::ZBKX => IsaExtensions::ZBKX,
            KvmRiscvIsaExtId::ZKND => IsaExtensions::ZKND,
            KvmRiscvIsaExtId::ZKNE => IsaExtensions::ZKNE,
            KvmRiscvIsaExtId::ZKNH => IsaExtensions::ZKNH,
            KvmRiscvIsaExtId::ZKR => IsaExtensions::ZKR,
            KvmRiscvIsaExtId::ZKSED => IsaExtensions::ZKSED,
            KvmRiscvIsaExtId::ZKSH => IsaExtensions::ZKSH,
            KvmRiscvIsaExtId::ZKT => IsaExtensions::ZKT,
            KvmRiscvIsaExtId::ZVBB => IsaExtensions::ZVBB,
            KvmRiscvIsaExtId::ZVBC => IsaExtensions::ZVBC,
            KvmRiscvIsaExtId::ZVKB => IsaExtensions::ZVKB,
            KvmRiscvIsaExtId::ZVKG => IsaExtensions::ZVKG,
            KvmRiscvIsaExtId::ZVKNED => IsaExtensions::ZVKNED,
            KvmRiscvIsaExtId::ZVKNHA => IsaExtensions::ZVKNHA,
            KvmRiscvIsaExtId::ZVKNHB => IsaExtensions::ZVKNHB,
            KvmRiscvIsaExtId::ZVKSED => IsaExtensions::ZVKSED,
            KvmRiscvIsaExtId::ZVKSH => IsaExtensions::ZVKSH,
            KvmRiscvIsaExtId::ZVKT => IsaExtensions::ZVKT,
            KvmRiscvIsaExtId::ZFH => IsaExtensions::ZFH,
            KvmRiscvIsaExtId::ZFHMIN => IsaExtensions::ZFHMIN,
            KvmRiscvIsaExtId::ZIHINTNTL => IsaExtensions::ZIHINTNTL,
            KvmRiscvIsaExtId::ZVFH => IsaExtensions::ZVFH,
            KvmRiscvIsaExtId::ZVFHMIN => IsaExtensions::ZVFHMIN,
            KvmRiscvIsaExtId::ZFA => IsaExtensions::ZFA,
            KvmRiscvIsaExtId::ZTSO => IsaExtensions::ZTSO,
            KvmRiscvIsaExtId::ZACAS => IsaExtensions::ZACAS,
            KvmRiscvIsaExtId::SSCOFPMF => IsaExtensions::SSCOFPMF,
            KvmRiscvIsaExtId::ZIMOP => IsaExtensions::ZIMOP,
            KvmRiscvIsaExtId::ZCA => IsaExtensions::ZCA,
            KvmRiscvIsaExtId::ZCB => IsaExtensions::ZCB,
            KvmRiscvIsaExtId::ZCD => IsaExtensions::ZCD,
            KvmRiscvIsaExtId::ZCF => IsaExtensions::ZCF,
            KvmRiscvIsaExtId::ZCMOP => IsaExtensions::ZCMOP,
            KvmRiscvIsaExtId::ZAWRS => IsaExtensions::ZAWRS,
            KvmRiscvIsaExtId::SMNPM => IsaExtensions::SMNPM,
            KvmRiscvIsaExtId::SSNPM => IsaExtensions::SSNPM,
            KvmRiscvIsaExtId::SVADE => IsaExtensions::SVADE,
            KvmRiscvIsaExtId::SVADU => IsaExtensions::SVADU,
            KvmRiscvIsaExtId::SVVPTC => IsaExtensions::SVVPTC,
            KvmRiscvIsaExtId::ZABHA => IsaExtensions::ZABHA,
            KvmRiscvIsaExtId::ZICCRSE => IsaExtensions::ZICCRSE,
            KvmRiscvIsaExtId::ZAAMO => IsaExtensions::ZAAMO,
            KvmRiscvIsaExtId::ZALRSC => IsaExtensions::ZALRSC,
            KvmRiscvIsaExtId::ZICBOP => IsaExtensions::ZICBOP,
            KvmRiscvIsaExtId::ZFBFMIN => IsaExtensions::ZFBFMIN,
            KvmRiscvIsaExtId::ZVFBFMIN => IsaExtensions::ZVFBFMIN,
            KvmRiscvIsaExtId::ZVFBFWMA => IsaExtensions::ZVFBFWMA,
            KvmRiscvIsaExtId::ZCLSD => IsaExtensions::ZCLSD,
            KvmRiscvIsaExtId::ZILSD => IsaExtensions::ZILSD,
            KvmRiscvIsaExtId::ZALASR => IsaExtensions::ZALASR,
            KvmRiscvIsaExtId::MAX => unreachable!("MAX is not a real ISA extension"),
        }
    }

    /// Convert a raw KVM ISA extension ID into the enum variant.
    ///
    /// Returns `None` for `MAX` and any out-of-range value.
    fn from_raw(raw: u32) -> Option<Self> {
        if raw >= Self::MAX as u32 {
            return None;
        }
        // SAFETY: `Self` is `#[repr(u32)]` and contiguous from 0 to MAX-1.
        Some(unsafe { core::mem::transmute::<u32, Self>(raw) })
    }

    /// Number of real (non-`MAX`) variants.
    const COUNT: u32 = Self::MAX as u32;
}

fn kvm_riscv_isa_check_host(kvm_ext: KvmRiscvIsaExtId) -> Option<IsaExtensions> {
    let mapped = kvm_ext.to_isa_ext();

    let host_ext = match mapped {
        IsaExtensions::SMNPM => IsaExtensions::SSNPM,
        other => other,
    };

    if !has_extensions(host_ext) {
        return None;
    }

    Some(mapped)
}

fn kvm_riscv_isa_enable_allowed(ext: IsaExtensions) -> bool {
    match ext {
        IsaExtensions::H | IsaExtensions::SSCOFPMF |IsaExtensions::V => false,
        _ => has_extensions(ext),
    }
}

fn set_bit(isa: &mut [u64; 2], bit: usize) {
    if bit < 64 {
        isa[0] |= 1<<bit
    } else {
        isa[1] |= 1<<(bit-64)
    }
}

pub(super) fn kvm_riscv_vcpu_setup_isa(isa: &mut [u64; 2]) {
    for raw in 0..KvmRiscvIsaExtId::COUNT {
        let Some(ext) = KvmRiscvIsaExtId::from_raw(raw) else {
            continue;
        };

        if let Some(isa_ext) = kvm_riscv_isa_check_host(ext) {
            if kvm_riscv_isa_enable_allowed(isa_ext) {
                set_bit(isa, isa_ext.bits().trailing_zeros() as usize);
            }
        }
    }
}
