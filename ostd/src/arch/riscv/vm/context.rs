// SPDX-License-Identifier: MPL-2.0

use core::{arch::global_asm, u32};
use core::mem::offset_of;
use super::vcpu::VcpuArchInner;
use crate::arch::cpu::context::{FFpuContext, DFpuContext, QFpuContext};
use crate::prelude::*;
use crate::Error;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct VcpuContextReg {
    zero: usize,
    ra: usize,
    sp: usize,
    gp: usize,
    tp: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    s0: usize,
    s1: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,
    sepc: usize,
	sstatus: usize,
	hstatus: usize,
    fp_fctx: FFpuContext,
    fp_dctx: DFpuContext,
    fp_qctx: QFpuContext,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VcpuContextSlice {
    regs_1: [usize; 35],
    regs_2: [u32; 33],
    regs_3: [u64; 32],
    regs_4: u32,
    regs_5: [u64; 64],
    regs_6: u32,
}

// Logical register index layout for the RISC-V vCPU context.
//
// These indices are the flat, stable numbering used by the KVM ABI.
// They do NOT directly index any sub-array; each sub-array is indexed
// by `logical_index - <BASE>`.
//
// The layout mirrors `VcpuContextReg` field-for-field:
//
//   [CORE_REGS_BASE, CORE_REGS_END)   -> regs_1 (usize, 35)
//       x0..x31 (32) + sepc + sstatus + hstatus
//   [FP_F_REGS_BASE, FP_F_REGS_END)   -> regs_2 (u32,   33)
//       f0..f31 (32) + fcsr
//   [FP_D_REGS_BASE, FP_D_REGS_END)   -> regs_3 (u64,   32)
//       f0..f31 (32)
//   [FP_D_FCSR_INDEX]                 -> regs_4 (u32)
//       D-extension fcsr
//   [FP_Q_REGS_BASE, FP_Q_REGS_END)   -> regs_5 (u64,   64)
//       f0..f63 (64)
//   [FP_Q_FCSR_INDEX]                 -> regs_6 (u32)
//       Q-extension fcsr
// Number of integer/core registers exposed as `usize` (`regs_1`).
// 32 GPRs + sepc + sstatus + hstatus.
const CORE_REGS_LEN: usize = 35;
const CORE_REGS_BASE: usize = 0;
const CORE_REGS_END: usize = CORE_REGS_BASE + CORE_REGS_LEN; // 35

// `regs_2`: FFpuContext == 32 x u32 `f` + 1 x u32 `fcsr`.
const FP_F_REGS_LEN: usize = 33;
const FP_F_REGS_BASE: usize = CORE_REGS_END; // 35
const FP_F_REGS_END: usize = FP_F_REGS_BASE + FP_F_REGS_LEN; // 68

// `regs_3`: DFpuContext's `f: [u64; 32]`.
const FP_D_REGS_LEN: usize = 32;
const FP_D_REGS_BASE: usize = FP_F_REGS_END; // 68
const FP_D_REGS_END: usize = FP_D_REGS_BASE + FP_D_REGS_LEN; // 100

// `regs_4`: DFpuContext's `fcsr: u32` (split out by struct padding).
const FP_D_FCSR_INDEX: usize = FP_D_REGS_END; // 100

// `regs_5`: QFpuContext's `f: [u64; 64]`.
const FP_Q_REGS_LEN: usize = 64;
const FP_Q_REGS_BASE: usize = FP_D_FCSR_INDEX + 1; // 101
const FP_Q_REGS_END: usize = FP_Q_REGS_BASE + FP_Q_REGS_LEN; // 165

// `regs_6`: QFpuContext's `fcsr: u32` (split out by struct padding).
const FP_Q_FCSR_INDEX: usize = FP_Q_REGS_END; // 165

const _: () = {
    // The union's two views must describe exactly the same bytes.
    assert!(
        size_of::<VcpuContextSlice>() == size_of::<VcpuContextReg>(),
        "VcpuContextSlice and VcpuContextReg must have the same size",
    );
    assert!(
        align_of::<VcpuContextSlice>() == align_of::<VcpuContextReg>(),
        "VcpuContextSlice and VcpuContextReg must have the same alignment",
    );
};

/// The Context of Vcpu, include some regs.
#[repr(C)]
pub union VcpuContext {
    slice: VcpuContextSlice,
    reg: VcpuContextReg,
}

impl Default for VcpuContext {
    fn default() -> Self {
        VcpuContext {
            slice: VcpuContextSlice {
                regs_1: [0; 35],
                regs_2: [0; 33],
                regs_3: [0; 32],
                regs_4: 0,
                regs_5: [0; 64],
                regs_6: 0,
            },
        }
    }
}

impl VcpuContext {
    /// Get reg by logical index.
    ///
    /// The index layout is described by the `*_BASE` / `*_LEN` constants
    /// above; each sub-array is indexed by `index - base`.
    // SAFETY: Access to union members is safe.
    pub fn get_reg(&self, index: usize) -> Result<usize> {
        unsafe {
            match index {
                CORE_REGS_BASE..CORE_REGS_END => Ok(self.slice.regs_1[index - CORE_REGS_BASE]),
                FP_F_REGS_BASE..FP_F_REGS_END => Ok(self.slice.regs_2[index - FP_F_REGS_BASE] as usize),
                FP_D_REGS_BASE..FP_D_REGS_END => Ok(self.slice.regs_3[index - FP_D_REGS_BASE] as usize),
                FP_D_FCSR_INDEX => Ok(self.slice.regs_4 as usize),
                FP_Q_REGS_BASE..FP_Q_REGS_END => Ok(self.slice.regs_5[index - FP_Q_REGS_BASE] as usize),
                FP_Q_FCSR_INDEX => Ok(self.slice.regs_6 as usize),
                _ => Err(Error::NotFound),
            }
        }
    }

    /// Set reg by logical index.
    ///
    /// See [`Self::get_reg`] for the index layout.
    // SAFETY: Access to union members is safe.
    pub fn set_reg(&mut self, index: usize, value: usize) -> Result<()> {
        unsafe {
            match index {
                CORE_REGS_BASE..CORE_REGS_END => self.slice.regs_1[index - CORE_REGS_BASE] = value,
                FP_F_REGS_BASE..FP_F_REGS_END => self.slice.regs_2[index - FP_F_REGS_BASE] = value as u32,
                FP_D_REGS_BASE..FP_D_REGS_END => self.slice.regs_3[index - FP_D_REGS_BASE] = value as u64,
                FP_D_FCSR_INDEX => self.slice.regs_4 = value as u32,
                FP_Q_REGS_BASE..FP_Q_REGS_END => self.slice.regs_5[index - FP_Q_REGS_BASE] = value as u64,
                FP_Q_FCSR_INDEX => self.slice.regs_6 = value as u32,
                _ => return Err(Error::NotFound),
            }
        }
        Ok(())
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_a0(&self) -> usize {
        unsafe{ self.reg.a0 }
    }

    pub(super) fn set_a0(&mut self, a0: usize) {
        self.reg.a0 = a0;
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_a1(&self) -> usize {
        unsafe{ self.reg.a1 }
    }

    pub(super) fn set_a1(&mut self, a1: usize) {
        self.reg.a1 = a1;
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_a6(&self) -> usize {
        unsafe{ self.reg.a6 }
    }

    pub(super) fn set_a6(&mut self, a6: usize) {
        self.reg.a6 = a6;
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_a7(&self) -> usize {
        unsafe{ self.reg.a7 }
    }

    pub(super) fn set_a7(&mut self, a7: usize) {
        self.reg.a7 = a7;
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_sepc(&self) -> usize {
        unsafe{ self.reg.sepc }
    }

    pub(super) fn set_sepc(&mut self, sepc: usize) {
        self.reg.sepc = sepc;
    }

    /// Increase the SEPC reg by several bytes
    // SAFETY: Access to union members is safe.
    pub fn step_sepc(&mut self, bytes: usize) {
        unsafe {self.reg.sepc += bytes}
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_sstatus(&self) -> usize {
        unsafe{ self.reg.sstatus }
    }

    pub(super) fn set_sstatus(&mut self, sstatus: usize) {
        self.reg.sstatus = sstatus;
    }

    /// Get hstatus reg
    // SAFETY: Access to union members is safe.
    pub fn get_hstatus(&self) -> usize {
        unsafe{ self.reg.hstatus }
    }

    pub(super) fn set_hstatus(&mut self, hstatus: usize) {
        self.reg.hstatus = hstatus;
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_fp_fctx_f(&self, index: usize) -> usize {
        unsafe{ self.reg.fp_fctx.get_f(index) as usize }
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn set_fp_fctx_f(&mut self, index: usize, f: usize) {
        unsafe{ self.reg.fp_fctx.set_f(index, f as u32) }
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_fp_fctx_fcsr(&self) -> usize {
        unsafe{ self.reg.fp_fctx.get_fcsr() as usize }
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn set_fp_fctx_fcsr(&mut self, fcsr: usize) {
        unsafe{ self.reg.fp_fctx.set_fcsr(fcsr as u32) }
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_fp_dctx_f(&self, index: usize) -> usize {
        unsafe{ self.reg.fp_dctx.get_f(index) as usize }
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn set_fp_dctx_f(&mut self, index: usize, f: usize) {
        unsafe{ self.reg.fp_dctx.set_f(index, f as u64) }
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn get_fp_dctx_fcsr(&self) -> usize {
        unsafe{ self.reg.fp_dctx.get_fcsr() as usize }
    }

    // SAFETY: Access to union members is safe.
    pub(super) fn set_fp_dctx_fcsr(&mut self, fcsr: usize) {
        unsafe{ self.reg.fp_dctx.set_fcsr(fcsr as u32) }
    }
}

macro_rules! kvm_arch_offset {
    (
        $(
            $name:ident => $($field:ident).+
        ),+ $(,)?
    ) => {
        global_asm!(
            concat!(
                $(
                    ".global ", stringify!($name), "\n",
                    ".set ",    stringify!($name), ", {", stringify!($name), "}\n",
                )+
            ),
            $(
                $name = const offset_of!(VcpuArchInner, $($field).+),
            )+
        );
    };
}

// We will continue to make changes to VcpuArchInner in the foreseeable future; 
// using the definition below allows us to avoid the impact of these changes on 
// overall correctness.
kvm_arch_offset! {
    KVM_ARCH_HOST_RA       => host_context.reg.ra,
    KVM_ARCH_HOST_SP       => host_context.reg.sp,
    KVM_ARCH_HOST_GP       => host_context.reg.gp,
    KVM_ARCH_HOST_TP       => host_context.reg.tp,
    KVM_ARCH_HOST_S0       => host_context.reg.s0,
    KVM_ARCH_HOST_S1       => host_context.reg.s1,
    KVM_ARCH_HOST_A1       => host_context.reg.a1,
    KVM_ARCH_HOST_A2       => host_context.reg.a2,
    KVM_ARCH_HOST_A3       => host_context.reg.a3,
    KVM_ARCH_HOST_A4       => host_context.reg.a4,
    KVM_ARCH_HOST_A5       => host_context.reg.a5,
    KVM_ARCH_HOST_A6       => host_context.reg.a6,
    KVM_ARCH_HOST_A7       => host_context.reg.a7,
    KVM_ARCH_HOST_S2       => host_context.reg.s2,
    KVM_ARCH_HOST_S3       => host_context.reg.s3,
    KVM_ARCH_HOST_S4       => host_context.reg.s4,
    KVM_ARCH_HOST_S5       => host_context.reg.s5,
    KVM_ARCH_HOST_S6       => host_context.reg.s6,
    KVM_ARCH_HOST_S7       => host_context.reg.s7,
    KVM_ARCH_HOST_S8       => host_context.reg.s8,
    KVM_ARCH_HOST_S9       => host_context.reg.s9,
    KVM_ARCH_HOST_S10      => host_context.reg.s10,
    KVM_ARCH_HOST_S11      => host_context.reg.s11,
    KVM_ARCH_HOST_SSTATUS  => host_context.reg.sstatus,
    KVM_ARCH_HOST_STVEC    => host_stvec,
    KVM_ARCH_HOST_SSCRATCH => host_sscratch,

    KVM_ARCH_GUEST_RA       => guest_context.reg.ra,
    KVM_ARCH_GUEST_SP       => guest_context.reg.sp,
    KVM_ARCH_GUEST_GP       => guest_context.reg.gp,
    KVM_ARCH_GUEST_TP       => guest_context.reg.tp,
    KVM_ARCH_GUEST_T0       => guest_context.reg.t0,
    KVM_ARCH_GUEST_T1       => guest_context.reg.t1,
    KVM_ARCH_GUEST_T2       => guest_context.reg.t2,
    KVM_ARCH_GUEST_S0       => guest_context.reg.s0,
    KVM_ARCH_GUEST_S1       => guest_context.reg.s1,
    KVM_ARCH_GUEST_A0       => guest_context.reg.a0,
    KVM_ARCH_GUEST_A1       => guest_context.reg.a1,
    KVM_ARCH_GUEST_A2       => guest_context.reg.a2,
    KVM_ARCH_GUEST_A3       => guest_context.reg.a3,
    KVM_ARCH_GUEST_A4       => guest_context.reg.a4,
    KVM_ARCH_GUEST_A5       => guest_context.reg.a5,
    KVM_ARCH_GUEST_A6       => guest_context.reg.a6,
    KVM_ARCH_GUEST_A7       => guest_context.reg.a7,
    KVM_ARCH_GUEST_S2       => guest_context.reg.s2,
    KVM_ARCH_GUEST_S3       => guest_context.reg.s3,
    KVM_ARCH_GUEST_S4       => guest_context.reg.s4,
    KVM_ARCH_GUEST_S5       => guest_context.reg.s5,
    KVM_ARCH_GUEST_S6       => guest_context.reg.s6,
    KVM_ARCH_GUEST_S7       => guest_context.reg.s7,
    KVM_ARCH_GUEST_S8       => guest_context.reg.s8,
    KVM_ARCH_GUEST_S9       => guest_context.reg.s9,
    KVM_ARCH_GUEST_S10      => guest_context.reg.s10,
    KVM_ARCH_GUEST_S11      => guest_context.reg.s11,
    KVM_ARCH_GUEST_T3       => guest_context.reg.t3,
    KVM_ARCH_GUEST_T4       => guest_context.reg.t4,
    KVM_ARCH_GUEST_T5       => guest_context.reg.t5,
    KVM_ARCH_GUEST_T6       => guest_context.reg.t6,
    KVM_ARCH_GUEST_SSTATUS  => guest_context.reg.sstatus,
    KVM_ARCH_GUEST_SEPC     => guest_context.reg.sepc,
}

global_asm!(include_str!("switch.S"));
