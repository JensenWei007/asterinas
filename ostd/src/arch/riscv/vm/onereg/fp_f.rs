// SPDX-License-Identifier: MPL-2.0

use crate::arch::vm::vcpu::VcpuArchState;
use super::{KVM_REG_ARCH_MASK, KVM_REG_SIZE_MASK, KVM_REG_RISCV, KVM_REG_SIZE_U32};
use super::KVM_REG_RISCV_FP_F;
use crate::arch::cpu::context::FFpuContext;
use crate::prelude::*;
use crate::Error;

impl VcpuArchState {
    pub(super) fn get_fp_f(&self, reg_id: usize) -> Result<usize> {
        let reg_num = reg_id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_FP_F);
        match reg_num {
            32 => Ok(self.guest_context.get_fp_fctx_fcsr()),
            0..32 => Ok(self.guest_context.get_fp_fctx_f(reg_num)),
            _ => Err(Error::NotFound),
        }
    }

    pub(super) fn set_fp_f(&mut self, reg_id: usize, reg_val: usize) -> Result<()> {
        let reg_num = reg_id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_FP_F);
        match reg_num {
            32 => self.guest_context.set_fp_fctx_fcsr(reg_val),
            0..32 => self.guest_context.set_fp_fctx_f(reg_num, reg_val),
            _ => return Err(Error::NotFound),
        }
        Ok(())
    }

    pub(super) fn copy_fp_f_reg_vec(&self, mut vec: Option<&mut Vec<usize>>) -> Result<usize> {
        let mut n = 0;
        for i in 0..size_of::<FFpuContext>()/ size_of::<u32>() {
            let reg = KVM_REG_RISCV | KVM_REG_SIZE_U32 | KVM_REG_RISCV_FP_F | i;
            if let Some(vec) = vec.as_deref_mut() {
                vec.push(reg);
            }
            n+=1;
        }
        Ok(n)
    }

    pub(super) fn num_fp_f_regs(&self) -> Result<usize> {
        self.copy_fp_f_reg_vec(None)
    }
}