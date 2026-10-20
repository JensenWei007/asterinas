// SPDX-License-Identifier: MPL-2.0

use crate::Error;
use crate::prelude::*;
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
    fn to_isa_ext(self) -> Result<IsaExtensions> {
        match self {
            KvmRiscvIsaExtId::A => Ok(IsaExtensions::A),
            KvmRiscvIsaExtId::C => Ok(IsaExtensions::C),
            KvmRiscvIsaExtId::D => Ok(IsaExtensions::D),
            KvmRiscvIsaExtId::F => Ok(IsaExtensions::F),
            KvmRiscvIsaExtId::H => Ok(IsaExtensions::H),
            KvmRiscvIsaExtId::I => Ok(IsaExtensions::I),
            KvmRiscvIsaExtId::M => Ok(IsaExtensions::M),
            KvmRiscvIsaExtId::SVPBMT => Ok(IsaExtensions::SVPBMT),
            KvmRiscvIsaExtId::SSTC => Ok(IsaExtensions::SSTC),
            KvmRiscvIsaExtId::SVINVAL => Ok(IsaExtensions::SVINVAL),
            KvmRiscvIsaExtId::ZIHINTPAUSE => Ok(IsaExtensions::ZIHINTPAUSE),
            KvmRiscvIsaExtId::ZICBOM => Ok(IsaExtensions::ZICBOM),
            KvmRiscvIsaExtId::ZICBOZ => Ok(IsaExtensions::ZICBOZ),
            KvmRiscvIsaExtId::ZBB => Ok(IsaExtensions::ZBB),
            KvmRiscvIsaExtId::SSAIA => Ok(IsaExtensions::SSAIA),
            KvmRiscvIsaExtId::V => Ok(IsaExtensions::V),
            KvmRiscvIsaExtId::SVNAPOT => Ok(IsaExtensions::SVNAPOT),
            KvmRiscvIsaExtId::ZBA => Ok(IsaExtensions::ZBA),
            KvmRiscvIsaExtId::ZBS => Ok(IsaExtensions::ZBS),
            KvmRiscvIsaExtId::ZICNTR => Ok(IsaExtensions::ZICNTR),
            KvmRiscvIsaExtId::ZICSR => Ok(IsaExtensions::ZICSR),
            KvmRiscvIsaExtId::ZIFENCEI => Ok(IsaExtensions::ZIFENCEI),
            KvmRiscvIsaExtId::ZIHPM => Ok(IsaExtensions::ZIHPM),
            KvmRiscvIsaExtId::SMSTATEEN => Ok(IsaExtensions::SMSTATEEN),
            KvmRiscvIsaExtId::ZICOND => Ok(IsaExtensions::ZICOND),
            KvmRiscvIsaExtId::ZBC => Ok(IsaExtensions::ZBC),
            KvmRiscvIsaExtId::ZBKB => Ok(IsaExtensions::ZBKB),
            KvmRiscvIsaExtId::ZBKC => Ok(IsaExtensions::ZBKC),
            KvmRiscvIsaExtId::ZBKX => Ok(IsaExtensions::ZBKX),
            KvmRiscvIsaExtId::ZKND => Ok(IsaExtensions::ZKND),
            KvmRiscvIsaExtId::ZKNE => Ok(IsaExtensions::ZKNE),
            KvmRiscvIsaExtId::ZKNH => Ok(IsaExtensions::ZKNH),
            KvmRiscvIsaExtId::ZKR => Ok(IsaExtensions::ZKR),
            KvmRiscvIsaExtId::ZKSED => Ok(IsaExtensions::ZKSED),
            KvmRiscvIsaExtId::ZKSH => Ok(IsaExtensions::ZKSH),
            KvmRiscvIsaExtId::ZKT => Ok(IsaExtensions::ZKT),
            KvmRiscvIsaExtId::ZVBB => Ok(IsaExtensions::ZVBB),
            KvmRiscvIsaExtId::ZVBC => Ok(IsaExtensions::ZVBC),
            KvmRiscvIsaExtId::ZVKB => Ok(IsaExtensions::ZVKB),
            KvmRiscvIsaExtId::ZVKG => Ok(IsaExtensions::ZVKG),
            KvmRiscvIsaExtId::ZVKNED => Ok(IsaExtensions::ZVKNED),
            KvmRiscvIsaExtId::ZVKNHA => Ok(IsaExtensions::ZVKNHA),
            KvmRiscvIsaExtId::ZVKNHB => Ok(IsaExtensions::ZVKNHB),
            KvmRiscvIsaExtId::ZVKSED => Ok(IsaExtensions::ZVKSED),
            KvmRiscvIsaExtId::ZVKSH => Ok(IsaExtensions::ZVKSH),
            KvmRiscvIsaExtId::ZVKT => Ok(IsaExtensions::ZVKT),
            KvmRiscvIsaExtId::ZFH => Ok(IsaExtensions::ZFH),
            KvmRiscvIsaExtId::ZFHMIN => Ok(IsaExtensions::ZFHMIN),
            KvmRiscvIsaExtId::ZIHINTNTL => Ok(IsaExtensions::ZIHINTNTL),
            KvmRiscvIsaExtId::ZVFH => Ok(IsaExtensions::ZVFH),
            KvmRiscvIsaExtId::ZVFHMIN => Ok(IsaExtensions::ZVFHMIN),
            KvmRiscvIsaExtId::ZFA => Ok(IsaExtensions::ZFA),
            KvmRiscvIsaExtId::ZTSO => Ok(IsaExtensions::ZTSO),
            KvmRiscvIsaExtId::ZACAS => Ok(IsaExtensions::ZACAS),
            KvmRiscvIsaExtId::SSCOFPMF => Ok(IsaExtensions::SSCOFPMF),
            KvmRiscvIsaExtId::ZIMOP => Ok(IsaExtensions::ZIMOP),
            KvmRiscvIsaExtId::ZCA => Ok(IsaExtensions::ZCA),
            KvmRiscvIsaExtId::ZCB => Ok(IsaExtensions::ZCB),
            KvmRiscvIsaExtId::ZCD => Ok(IsaExtensions::ZCD),
            KvmRiscvIsaExtId::ZCF => Ok(IsaExtensions::ZCF),
            KvmRiscvIsaExtId::ZCMOP => Ok(IsaExtensions::ZCMOP),
            KvmRiscvIsaExtId::ZAWRS => Ok(IsaExtensions::ZAWRS),
            KvmRiscvIsaExtId::SMNPM => Ok(IsaExtensions::SMNPM),
            KvmRiscvIsaExtId::SSNPM => Ok(IsaExtensions::SSNPM),
            KvmRiscvIsaExtId::SVADE => Ok(IsaExtensions::SVADE),
            KvmRiscvIsaExtId::SVADU => Ok(IsaExtensions::SVADU),
            KvmRiscvIsaExtId::SVVPTC => Ok(IsaExtensions::SVVPTC),
            KvmRiscvIsaExtId::ZABHA => Ok(IsaExtensions::ZABHA),
            KvmRiscvIsaExtId::ZICCRSE => Ok(IsaExtensions::ZICCRSE),
            KvmRiscvIsaExtId::ZAAMO => Ok(IsaExtensions::ZAAMO),
            KvmRiscvIsaExtId::ZALRSC => Ok(IsaExtensions::ZALRSC),
            KvmRiscvIsaExtId::ZICBOP => Ok(IsaExtensions::ZICBOP),
            KvmRiscvIsaExtId::ZFBFMIN => Ok(IsaExtensions::ZFBFMIN),
            KvmRiscvIsaExtId::ZVFBFMIN => Ok(IsaExtensions::ZVFBFMIN),
            KvmRiscvIsaExtId::ZVFBFWMA => Ok(IsaExtensions::ZVFBFWMA),
            KvmRiscvIsaExtId::ZCLSD => Ok(IsaExtensions::ZCLSD),
            KvmRiscvIsaExtId::ZILSD => Ok(IsaExtensions::ZILSD),
            KvmRiscvIsaExtId::ZALASR => Ok(IsaExtensions::ZALASR),
            KvmRiscvIsaExtId::MAX => Err(Error::InvalidArgs)
        }
    }

    /// Convert a raw KVM ISA extension ID into the enum variant.
    ///
    /// Returns `None` for any out-of-range value.
    fn from_raw(raw: u32) -> Result<Self> {
        if raw > Self::ZALASR as u32 {
            return Err(Error::InvalidArgs);
        }
        // SAFETY: `Self` is `#[repr(u32)]` and contiguous from 0 to MAX-1.
        Ok(unsafe { core::mem::transmute::<u32, Self>(raw) })
    }

    /// Number of real variants.
    const COUNT: u32 = Self::ZALASR as u32 + 1;
}

fn kvm_riscv_isa_check_host(kvm_ext: KvmRiscvIsaExtId) -> Result<IsaExtensions> {
    let mapped = kvm_ext.to_isa_ext()?;

    let host_ext = match mapped {
        IsaExtensions::SMNPM => IsaExtensions::SSNPM,
        other => other,
    };

    if !has_extensions(host_ext) {
        return Err(Error::InvalidArgs);
    }

    Ok(mapped)
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

pub(super) fn kvm_riscv_vcpu_setup_isa(isa: &mut u128) -> Result<()> {
    for raw in 0..KvmRiscvIsaExtId::COUNT {
        let ext = KvmRiscvIsaExtId::from_raw(raw)?;

        if let Ok(isa_ext) = kvm_riscv_isa_check_host(ext) {
            if kvm_riscv_isa_enable_allowed(isa_ext) {
                *isa |= 0x1 << (isa_ext.bits().trailing_zeros() as usize);
            }
        }
    }
    Ok(())
}
