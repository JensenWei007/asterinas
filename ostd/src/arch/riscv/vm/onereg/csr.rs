// SPDX-License-Identifier: MPL-2.0

use crate::arch::vm::vcpu::VcpuArchState;
use super::{KVM_REG_ARCH_MASK, KVM_REG_SIZE_MASK, KVM_REG_RISCV_SUBTYPE_SHIFT, KVM_REG_RISCV_SUBTYPE_MASK, KVM_REG_RISCV, KVM_REG_SIZE_U64};
use super::KVM_REG_RISCV_CSR;
use crate::arch::vm::types::{VSIP_TO_HVIP_SHIFT, VSIP_VALID_MASK, IRQ_LOCAL_MASK};
use crate::prelude::*;
use crate::Error;

const KVM_REG_RISCV_CSR_GENERAL: usize = 0x0 << KVM_REG_RISCV_SUBTYPE_SHIFT;
const KVM_REG_RISCV_CSR_AIA: usize = 0x1 << KVM_REG_RISCV_SUBTYPE_SHIFT;
const KVM_REG_RISCV_CSR_SMSTATEEN: usize = 0x2 << KVM_REG_RISCV_SUBTYPE_SHIFT;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
struct KvmVcpuCsrReg {
	vsstatus: usize,
	vsie: usize,
	vstvec: usize,
	vsscratch: usize,
	vsepc: usize,
	vscause: usize,
	vstval: usize,
	hvip: usize,
	vsatp: usize,
	scounteren: usize,
	senvcfg: usize,
}

/// Number of CSR slots in the union's slice view.
const CSR_SLICE_LEN: usize = 11;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
struct KvmVcpuCsrSlice {
    slice: [usize; CSR_SLICE_LEN],
}

const _: () = {
    // The union's two views must describe exactly the same bytes.
    assert!(
        size_of::<KvmVcpuCsrReg>() == size_of::<KvmVcpuCsrSlice>(),
        "KvmVcpuCsrReg and KvmVcpuCsrSlice must have the same size",
    );
    assert!(
        align_of::<KvmVcpuCsrReg>() == align_of::<KvmVcpuCsrSlice>(),
        "KvmVcpuCsrReg and KvmVcpuCsrSlice must have the same alignment",
    );
};

#[repr(C)]
#[derive(Clone, Copy)]
pub(in crate::arch::vm) union KvmVcpuCsr {
	reg: KvmVcpuCsrReg,
    slice: KvmVcpuCsrSlice,
}

impl KvmVcpuCsr {
    const SIP: usize = 7;
}

impl Default for KvmVcpuCsr {
    fn default() -> Self {
        KvmVcpuCsr {
            slice: KvmVcpuCsrSlice {
                slice: [0; CSR_SLICE_LEN],
            },
        }
    }
}

impl KvmVcpuCsr {
    // SAFETY: Access to union members is safe.
    fn get_csr(&self, index: usize) -> Result<usize> {
        if index >= CSR_SLICE_LEN {
            return Err(Error::NotFound);
        }
        unsafe { Ok(self.slice.slice[index]) }
    }

    // SAFETY: Access to union members is safe.
    fn set_csr(&mut self, index: usize, value: usize) -> Result<()> {
        if index >= CSR_SLICE_LEN {
            return Err(Error::NotFound);
        }
        unsafe { self.slice.slice[index] = value };
        Ok(())
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_hvip(&self) -> usize {
        unsafe { self.reg.hvip }
    }

    pub(in crate::arch::vm) fn set_hvip(&mut self, value: usize) {
        self.reg.hvip = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_scounteren(&self) -> usize {
        unsafe { self.reg.scounteren }
    }

    pub(in crate::arch::vm) fn set_scounteren(&mut self, value: usize) {
        self.reg.scounteren = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_senvcfg(&self) -> usize {
        unsafe { self.reg.senvcfg }
    }

    pub(in crate::arch::vm) fn set_senvcfg(&mut self, value: usize) {
        self.reg.senvcfg = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_vsie(&self) -> usize {
        unsafe { self.reg.vsie }
    }

    pub(in crate::arch::vm) fn set_vsie(&mut self, value: usize) {
        self.reg.vsie = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_vsstatus(&self) -> usize {
        unsafe { self.reg.vsstatus }
    }

    pub(in crate::arch::vm) fn set_vsstatus(&mut self, value: usize) {
        self.reg.vsstatus = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_vstvec(&self) -> usize {
        unsafe { self.reg.vstvec }
    }

    pub(in crate::arch::vm) fn set_vstvec(&mut self, value: usize) {
        self.reg.vstvec = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_vsscratch(&self) -> usize {
        unsafe { self.reg.vsscratch }
    }

    pub(in crate::arch::vm) fn set_vsscratch(&mut self, value: usize) {
        self.reg.vsscratch = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_vsepc(&self) -> usize {
        unsafe { self.reg.vsepc }
    }

    pub(in crate::arch::vm) fn set_vsepc(&mut self, value: usize) {
        self.reg.vsepc = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_vscause(&self) -> usize {
        unsafe { self.reg.vscause }
    }

    pub(in crate::arch::vm) fn set_vscause(&mut self, value: usize) {
        self.reg.vscause = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_vstval(&self) -> usize {
        unsafe { self.reg.vstval }
    }

    pub(in crate::arch::vm) fn set_vstval(&mut self, value: usize) {
        self.reg.vstval = value;
    }

    // SAFETY: Access to union members is safe.
    pub(in crate::arch::vm) fn get_vsatp(&self) -> usize {
        unsafe { self.reg.vsatp }
    }

    pub(in crate::arch::vm) fn set_vsatp(&mut self, value: usize) {
        self.reg.vsatp = value;
    }
}

impl VcpuArchState {
    pub(super) fn get_csr(&self, reg_id: usize) -> Result<usize> {
        let mut reg_num = reg_id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_CSR);
        let reg_subtype = KVM_REG_RISCV_SUBTYPE_MASK & reg_num;
        reg_num &= !(KVM_REG_RISCV_SUBTYPE_MASK);
        // Subtypes not yet impled: KVM_REG_RISCV_CSR_AIA, KVM_REG_RISCV_CSR_SMSTATEEN
        match reg_subtype {
            KVM_REG_RISCV_CSR_GENERAL => self.get_general_csr(reg_num),
            _ => Err(Error::NotFound),
        }
    }

    fn get_general_csr(&self, reg_num: usize) -> Result<usize> {
        match reg_num {
            KvmVcpuCsr::SIP => Ok(((self.guest_csr.get_hvip() >> VSIP_TO_HVIP_SHIFT) & VSIP_VALID_MASK) & IRQ_LOCAL_MASK),
            _ => self.guest_csr.get_csr(reg_num)
        }
    }

    pub(super) fn set_csr(&mut self, reg_id: usize, reg_val: usize) -> Result<()> {
        let mut reg_num = reg_id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_CSR);
        let reg_subtype = KVM_REG_RISCV_SUBTYPE_MASK & reg_num;
        reg_num &= !(KVM_REG_RISCV_SUBTYPE_MASK);
        // Subtypes not yet impled: KVM_REG_RISCV_CSR_AIA, KVM_REG_RISCV_CSR_SMSTATEEN
        match reg_subtype {
            KVM_REG_RISCV_CSR_GENERAL => self.set_general_csr(reg_num, reg_val)?,
            _ => return Err(Error::NotFound),
        }
        Ok(())
    }

    fn set_general_csr(&mut self, reg_num: usize, reg_val: usize) -> Result<()> {
        match reg_num {
            KvmVcpuCsr::SIP => {
                let reg_val = (reg_val & VSIP_VALID_MASK) << VSIP_TO_HVIP_SHIFT;
                self.guest_csr.set_csr(reg_num, reg_val)?;
            }
            _ => self.guest_csr.set_csr(reg_num, reg_val)?,
        }
        Ok(())
    }

    pub(super) fn copy_csr_reg_vec(&self, mut vec: Option<&mut Vec<usize>>) -> Result<usize> {
        let mut n = 0;
        for i in 0..size_of::<KvmVcpuCsr>()/ size_of::<usize>() {
            let reg = KVM_REG_RISCV | KVM_REG_SIZE_U64 | KVM_REG_RISCV_CSR | i;
            if let Some(vec) = vec.as_deref_mut() {
                vec.push(reg);
            }
            n+=1;
        }
        Ok(n)
    }

    pub(super) fn num_csr_regs(&self) -> Result<usize> {
        self.copy_csr_reg_vec(None)
    }
}


