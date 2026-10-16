// SPDX-License-Identifier: MPL-2.0

use crate::arch::vm::vcpu::VcpuArchInner;
use crate::prelude::*;

impl VcpuArchInner {
    pub(super) fn get_isa_ext(&self, _reg_id: usize) -> Result<usize> {
        todo!("OneReg: get_isa_ext is not impled!")
    }
    
    pub(super) fn set_isa_ext(&self, _reg_id: usize, _reg_val: usize) -> Result<()> {
        todo!("OneReg: set_isa_ext is not impled!")
    }

    pub(super) fn copy_isa_ext_reg_vec(&self, mut _vec: Option<&mut Vec<usize>>) -> Result<usize> {
        // todo!("OneReg: copy_isa_ext_reg_vec is not impled!")
        Ok(0)
    }

    pub(super) fn num_isa_ext_regs(&self) -> Result<usize> {
        self.copy_isa_ext_reg_vec(None)
    }
}
