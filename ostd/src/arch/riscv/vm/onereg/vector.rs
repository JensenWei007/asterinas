// SPDX-License-Identifier: MPL-2.0

use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

impl VcpuArchInner {
    pub(super) fn get_vector(&self, _reg_id: usize) -> Result<usize> {
        todo!("OneReg: get_vector is not impled!")
    }
    
    pub(super) fn set_vector(&self, _reg_id: usize, _reg_val: usize) -> Result<()> {
        todo!("OneReg: set_vector is not impled!")
    }

    pub(super) fn copy_vector_reg_vec(&self, mut _vec: Option<&mut Vec<usize>>) -> Result<usize> {
        // todo!("OneReg: copy_vector_reg_vec is not impled!")
        Ok(0)
    }

    pub(super) fn num_vector_regs(&self) -> Result<usize> {
        self.copy_vector_reg_vec(None)
    }
}
