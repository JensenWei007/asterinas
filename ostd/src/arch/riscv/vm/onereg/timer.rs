// SPDX-License-Identifier: MPL-2.0

use crate::arch::vm::vcpu::VcpuArchInner;
use super::{KVM_REG_ARCH_MASK, KVM_REG_SIZE_MASK};
use super::KVM_REG_RISCV_TIMER;
use crate::arch::timer::get_timebase_freq;
use crate::prelude::*;

struct KvmRiscvTimer {
	frequency: usize,
	time: usize,
	compare: usize,
	state: usize,
}

impl KvmRiscvTimer {
    const FREQUENCY: usize = 0;
    const TIME: usize = Self::FREQUENCY + 1;
    const COMPARE: usize = Self::TIME + 1;
    const STATE: usize = Self::COMPARE + 1;
}

impl VcpuArchInner {
    pub(super) fn get_timer(&self, reg_id: usize) -> Result<usize> {
        let reg_num = reg_id & !(KVM_REG_ARCH_MASK | KVM_REG_SIZE_MASK | KVM_REG_RISCV_TIMER);
        match reg_num {
            KvmRiscvTimer::FREQUENCY => Ok(get_timebase_freq() as usize),
            KvmRiscvTimer::TIME => todo!("OneReg: get_timer for TIME is not impled!"),
            KvmRiscvTimer::COMPARE => todo!("OneReg: get_timer for COMPARE is not impled!"),
            KvmRiscvTimer::STATE => todo!("OneReg: get_timer for STATE is not impled!"),
            _ => Err(crate::Error::NotFound),
        }
    }
    
    pub(super) fn set_timer(&self, _reg_id: usize, _reg_val: usize) -> Result<()> {
        todo!("OneReg: set_timer is not impled!")
    }

    pub(super) fn copy_timer_reg_vec(&self, mut _vec: Option<&mut Vec<usize>>) -> Result<usize> {
        // todo!("OneReg: copy_timer_reg_vec is not impled!")
        Ok(0)
    }

    pub(super) fn num_timer_regs(&self) -> Result<usize> {
        self.copy_timer_reg_vec(None)
    }
}
