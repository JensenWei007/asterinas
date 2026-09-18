/// 1

use crate::arch::cpu::context::*;

/// 1
pub struct GuestContext{}


/// Describes whether a guest vCPU may enter guest mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VcpuRunState {
    /// The vCPU is not initialized for execution.
    Uninitialized,
    /// The vCPU is waiting for a startup IPI.
    WaitForSipi,
    /// The vCPU is ready to enter guest mode.
    #[default]
    Runnable,
    /// The vCPU is currently executing in guest mode.
    Running,
    /// The vCPU Halted.
    Halted,
}

/// 1
#[derive(Default)]
pub struct VcpuContext {
    pub zero: usize,
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
    pub sepc: usize,
	pub sstatus: usize,
	pub hstatus: usize,
    pub fp_fctx: FFpuContext,
    pub fp_dctx: DFpuContext,
    pub fp_qctx: QFpuContext,
	//__riscv_v_ext_state vector;
}

pub const IDX_ZERO: usize = 0;
pub const IDX_RA: usize = 1;
pub const IDX_SP: usize = 2;
pub const IDX_GP: usize = 3;
pub const IDX_TP: usize = 4;
pub const IDX_T0: usize = 5;
pub const IDX_T1: usize = 6;
pub const IDX_T2: usize = 7;
pub const IDX_S0: usize = 8;
pub const IDX_S1: usize = 9;
pub const IDX_A0: usize = 10;
pub const IDX_A1: usize = 11;
pub const IDX_A2: usize = 12;
pub const IDX_A3: usize = 13;
pub const IDX_A4: usize = 14;
pub const IDX_A5: usize = 15;
pub const IDX_A6: usize = 16;
pub const IDX_A7: usize = 17;
pub const IDX_S2: usize = 18;
pub const IDX_S3: usize = 19;
pub const IDX_S4: usize = 20;
pub const IDX_S5: usize = 21;
pub const IDX_S6: usize = 22;
pub const IDX_S7: usize = 23;
pub const IDX_S8: usize = 24;
pub const IDX_S9: usize = 25;
pub const IDX_S10: usize = 26;
pub const IDX_S11: usize = 27;
pub const IDX_T3: usize = 28;
pub const IDX_T4: usize = 29;
pub const IDX_T5: usize = 30;
pub const IDX_T6: usize = 31;
pub const IDX_SEPC: usize = 32;
pub const IDX_SSTATUS: usize = 33;
pub const IDX_HSTATUS: usize = 34;
pub const REG_COUNT: usize = 35;

impl VcpuContext {
    /// 通过索引获取寄存器值
    pub fn get_reg(&self, index: u64) -> Option<usize> {
        match index {
            0 => Some(self.zero),
            1 => Some(self.ra),
            2 => Some(self.sp),
            3 => Some(self.gp),
            4 => Some(self.tp),
            5 => Some(self.t0),
            6 => Some(self.t1),
            7 => Some(self.t2),
            8 => Some(self.s0),
            9 => Some(self.s1),
            10 => Some(self.a0),
            11 => Some(self.a1),
            12 => Some(self.a2),
            13 => Some(self.a3),
            14 => Some(self.a4),
            15 => Some(self.a5),
            16 => Some(self.a6),
            17 => Some(self.a7),
            18 => Some(self.s2),
            19 => Some(self.s3),
            20 => Some(self.s4),
            21 => Some(self.s5),
            22 => Some(self.s6),
            23 => Some(self.s7),
            24 => Some(self.s8),
            25 => Some(self.s9),
            26 => Some(self.s10),
            27 => Some(self.s11),
            28 => Some(self.t3),
            29 => Some(self.t4),
            30 => Some(self.t5),
            31 => Some(self.t6),
            32 => Some(self.sepc),
            33 => Some(self.sstatus),
            34 => Some(self.hstatus),
            _ => None,
        }
    }

    /// 通过索引设置寄存器值
    pub fn set_reg(&mut self, index: usize, value: usize) -> bool {
        match index {
            0 => { self.zero = value; true }
            1 => { self.ra = value; true }
            2 => { self.sp = value; true }
            3 => { self.gp = value; true }
            4 => { self.tp = value; true }
            5 => { self.t0 = value; true }
            6 => { self.t1 = value; true }
            7 => { self.t2 = value; true }
            8 => { self.s0 = value; true }
            9 => { self.s1 = value; true }
            10 => { self.a0 = value; true }
            11 => { self.a1 = value; true }
            12 => { self.a2 = value; true }
            13 => { self.a3 = value; true }
            14 => { self.a4 = value; true }
            15 => { self.a5 = value; true }
            16 => { self.a6 = value; true }
            17 => { self.a7 = value; true }
            18 => { self.s2 = value; true }
            19 => { self.s3 = value; true }
            20 => { self.s4 = value; true }
            21 => { self.s5 = value; true }
            22 => { self.s6 = value; true }
            23 => { self.s7 = value; true }
            24 => { self.s8 = value; true }
            25 => { self.s9 = value; true }
            26 => { self.s10 = value; true }
            27 => { self.s11 = value; true }
            28 => { self.t3 = value; true }
            29 => { self.t4 = value; true }
            30 => { self.t5 = value; true }
            31 => { self.t6 = value; true }
            32 => { self.sepc = value; true }
            33 => { self.sstatus = value; true }
            34 => { self.hstatus = value; true }
            _ => false,
        }
    }
}
