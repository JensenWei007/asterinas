// SPDX-License-Identifier: MPL-2.0

use crate::arch::vm::vcpu::VcpuArchInner;
use super::{KVM_REG_ARCH_MASK, KVM_REG_SIZE_MASK, KVM_REG_RISCV, KVM_REG_SIZE_U64};
use super::KVM_REG_RISCV_FP_D;
use crate::prelude::*;
use crate::Error;

impl VcpuArchInner {
    pub(super) fn get_fp_d(&self, reg_id: usize) -> Result<usize> {
        let reg_num = reg_id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_FP_D);
        match reg_num {
            32 => Ok(self.guest_context.get_fp_dctx_fcsr()),
            0..32 => Ok(self.guest_context.get_fp_dctx_f(reg_num)),
            _ => Err(Error::NotFound),
        }
    }

    pub(super) fn set_fp_d(&mut self, reg_id: usize, reg_val: usize) -> Result<()> {
        let reg_num = reg_id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_FP_D);
        match reg_num {
            32 => self.guest_context.set_fp_dctx_fcsr(reg_val),
            0..32 =>self.guest_context.set_fp_dctx_f(reg_num, reg_val),
            _ => return Err(Error::NotFound),
        }
        Ok(())
    }

    pub(super) fn copy_fp_d_reg_vec(&self, mut vec: Option<&mut Vec<usize>>){
        let n = self.num_fp_d_regs();
        for i in 0..n-1 {
            let reg = KVM_REG_RISCV | KVM_REG_SIZE_U64 | KVM_REG_RISCV_FP_D | i;
            if let Some(vec) = vec.as_deref_mut() {
                vec.push(reg);
            }
        }

        let reg = KVM_REG_RISCV | KVM_REG_SIZE_U64 | KVM_REG_RISCV_FP_D | 32;
        if let Some(vec) = vec.as_deref_mut() {
            vec.push(reg);
        }
    }

    pub(super) fn num_fp_d_regs(&self) -> usize {
        33
    }
}
