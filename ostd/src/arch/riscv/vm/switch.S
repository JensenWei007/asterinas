// switchto

.extern KVM_ARCH_HOST_RA
.extern KVM_ARCH_HOST_SP
.extern KVM_ARCH_HOST_GP
.extern KVM_ARCH_HOST_TP
.extern KVM_ARCH_HOST_S0
.extern KVM_ARCH_HOST_S1
.extern KVM_ARCH_HOST_A1
.extern KVM_ARCH_HOST_A2
.extern KVM_ARCH_HOST_A3
.extern KVM_ARCH_HOST_A4
.extern KVM_ARCH_HOST_A5
.extern KVM_ARCH_HOST_A6
.extern KVM_ARCH_HOST_A7
.extern KVM_ARCH_HOST_S2
.extern KVM_ARCH_HOST_S3
.extern KVM_ARCH_HOST_S4
.extern KVM_ARCH_HOST_S5
.extern KVM_ARCH_HOST_S6
.extern KVM_ARCH_HOST_S7
.extern KVM_ARCH_HOST_S8
.extern KVM_ARCH_HOST_S9
.extern KVM_ARCH_HOST_S10
.extern KVM_ARCH_HOST_S11
.extern KVM_ARCH_HOST_SSTATUS
.extern KVM_ARCH_HOST_STVEC
.extern KVM_ARCH_HOST_SSCRATCH

.extern KVM_ARCH_GUEST_RA
.extern KVM_ARCH_GUEST_SP
.extern KVM_ARCH_GUEST_GP
.extern KVM_ARCH_GUEST_TP
.extern KVM_ARCH_GUEST_T0
.extern KVM_ARCH_GUEST_T1
.extern KVM_ARCH_GUEST_T2
.extern KVM_ARCH_GUEST_S0
.extern KVM_ARCH_GUEST_S1
.extern KVM_ARCH_GUEST_A0
.extern KVM_ARCH_GUEST_A1
.extern KVM_ARCH_GUEST_A2
.extern KVM_ARCH_GUEST_A3
.extern KVM_ARCH_GUEST_A4
.extern KVM_ARCH_GUEST_A5
.extern KVM_ARCH_GUEST_A6
.extern KVM_ARCH_GUEST_A7
.extern KVM_ARCH_GUEST_S2
.extern KVM_ARCH_GUEST_S3
.extern KVM_ARCH_GUEST_S4
.extern KVM_ARCH_GUEST_S5
.extern KVM_ARCH_GUEST_S6
.extern KVM_ARCH_GUEST_S7
.extern KVM_ARCH_GUEST_S8
.extern KVM_ARCH_GUEST_S9
.extern KVM_ARCH_GUEST_S10
.extern KVM_ARCH_GUEST_S11
.extern KVM_ARCH_GUEST_T3
.extern KVM_ARCH_GUEST_T4
.extern KVM_ARCH_GUEST_T5
.extern KVM_ARCH_GUEST_T6
.extern KVM_ARCH_GUEST_SSTATUS
.extern KVM_ARCH_GUEST_SEPC

.macro SAVE_HOST_GPRS
	/* Save Host GPRs (except A0 and T0-T6) */
	sd	ra, (KVM_ARCH_HOST_RA)(a0)
	sd	sp, (KVM_ARCH_HOST_SP)(a0)
	sd	gp, (KVM_ARCH_HOST_GP)(a0)
	sd	tp, (KVM_ARCH_HOST_TP)(a0)
	sd	s0, (KVM_ARCH_HOST_S0)(a0)
	sd	s1, (KVM_ARCH_HOST_S1)(a0)
	sd	a1, (KVM_ARCH_HOST_A1)(a0)
	sd	a2, (KVM_ARCH_HOST_A2)(a0)
	sd	a3, (KVM_ARCH_HOST_A3)(a0)
	sd	a4, (KVM_ARCH_HOST_A4)(a0)
	sd	a5, (KVM_ARCH_HOST_A5)(a0)
	sd	a6, (KVM_ARCH_HOST_A6)(a0)
	sd	a7, (KVM_ARCH_HOST_A7)(a0)
	sd	s2, (KVM_ARCH_HOST_S2)(a0)
	sd	s3, (KVM_ARCH_HOST_S3)(a0)
	sd	s4, (KVM_ARCH_HOST_S4)(a0)
	sd	s5, (KVM_ARCH_HOST_S5)(a0)
	sd	s6, (KVM_ARCH_HOST_S6)(a0)
	sd	s7, (KVM_ARCH_HOST_S7)(a0)
	sd	s8, (KVM_ARCH_HOST_S8)(a0)
	sd	s9, (KVM_ARCH_HOST_S9)(a0)
	sd	s10, (KVM_ARCH_HOST_S10)(a0)
	sd	s11, (KVM_ARCH_HOST_S11)(a0)
.endm

.macro SAVE_HOST_AND_RESTORE_GUEST_CSRS __resume_addr
	/* Load Guest CSR values */
	ld	t0, (KVM_ARCH_GUEST_SSTATUS)(a0)
	la	t1, \__resume_addr
	ld	t2, (KVM_ARCH_GUEST_SEPC)(a0)

	/* Save Host and Restore Guest SSTATUS */
	csrrw	t0, 0x100, t0

	/* Save Host STVEC and change it to return path */
	csrrw	t1, 0x105, t1

	/* Restore Guest SEPC */
	csrw	0x141, t2

	/* Save Host SSCRATCH and change it to struct kvm_vcpu_arch pointer */
	csrrw	t3, 0x140, a0

	/* Store Host CSR values */
	sd	t0, (KVM_ARCH_HOST_SSTATUS)(a0)
	sd	t1, (KVM_ARCH_HOST_STVEC)(a0)
	sd	t3, (KVM_ARCH_HOST_SSCRATCH)(a0)
.endm

.macro RESTORE_GUEST_GPRS
	/* Restore Guest GPRs (except A0) */
	ld	ra, (KVM_ARCH_GUEST_RA)(a0)
	ld	sp, (KVM_ARCH_GUEST_SP)(a0)
	ld	gp, (KVM_ARCH_GUEST_GP)(a0)
	ld	tp, (KVM_ARCH_GUEST_TP)(a0)
	ld	t0, (KVM_ARCH_GUEST_T0)(a0)
	ld	t1, (KVM_ARCH_GUEST_T1)(a0)
	ld	t2, (KVM_ARCH_GUEST_T2)(a0)
	ld	s0, (KVM_ARCH_GUEST_S0)(a0)
	ld	s1, (KVM_ARCH_GUEST_S1)(a0)
	ld	a1, (KVM_ARCH_GUEST_A1)(a0)
	ld	a2, (KVM_ARCH_GUEST_A2)(a0)
	ld	a3, (KVM_ARCH_GUEST_A3)(a0)
	ld	a4, (KVM_ARCH_GUEST_A4)(a0)
	ld	a5, (KVM_ARCH_GUEST_A5)(a0)
	ld	a6, (KVM_ARCH_GUEST_A6)(a0)
	ld	a7, (KVM_ARCH_GUEST_A7)(a0)
	ld	s2, (KVM_ARCH_GUEST_S2)(a0)
	ld	s3, (KVM_ARCH_GUEST_S3)(a0)
	ld	s4, (KVM_ARCH_GUEST_S4)(a0)
	ld	s5, (KVM_ARCH_GUEST_S5)(a0)
	ld	s6, (KVM_ARCH_GUEST_S6)(a0)
	ld	s7, (KVM_ARCH_GUEST_S7)(a0)
	ld	s8, (KVM_ARCH_GUEST_S8)(a0)
	ld	s9, (KVM_ARCH_GUEST_S9)(a0)
	ld	s10, (KVM_ARCH_GUEST_S10)(a0)
	ld	s11, (KVM_ARCH_GUEST_S11)(a0)
	ld	t3, (KVM_ARCH_GUEST_T3)(a0)
	ld	t4, (KVM_ARCH_GUEST_T4)(a0)
	ld	t5, (KVM_ARCH_GUEST_T5)(a0)
	ld	t6, (KVM_ARCH_GUEST_T6)(a0)

	/* Restore Guest A0 */
	ld	a0, (KVM_ARCH_GUEST_A0)(a0)
.endm

.macro SAVE_GUEST_GPRS
	/* Swap Guest A0 with SSCRATCH */
	csrrw	a0, 0x140, a0

	/* Save Guest GPRs (except A0) */
	sd	ra, (KVM_ARCH_GUEST_RA)(a0)
	sd	sp, (KVM_ARCH_GUEST_SP)(a0)
	sd	gp, (KVM_ARCH_GUEST_GP)(a0)
	sd	tp, (KVM_ARCH_GUEST_TP)(a0)
	sd	t0, (KVM_ARCH_GUEST_T0)(a0)
	sd	t1, (KVM_ARCH_GUEST_T1)(a0)
	sd	t2, (KVM_ARCH_GUEST_T2)(a0)
	sd	s0, (KVM_ARCH_GUEST_S0)(a0)
	sd	s1, (KVM_ARCH_GUEST_S1)(a0)
	sd	a1, (KVM_ARCH_GUEST_A1)(a0)
	sd	a2, (KVM_ARCH_GUEST_A2)(a0)
	sd	a3, (KVM_ARCH_GUEST_A3)(a0)
	sd	a4, (KVM_ARCH_GUEST_A4)(a0)
	sd	a5, (KVM_ARCH_GUEST_A5)(a0)
	sd	a6, (KVM_ARCH_GUEST_A6)(a0)
	sd	a7, (KVM_ARCH_GUEST_A7)(a0)
	sd	s2, (KVM_ARCH_GUEST_S2)(a0)
	sd	s3, (KVM_ARCH_GUEST_S3)(a0)
	sd	s4, (KVM_ARCH_GUEST_S4)(a0)
	sd	s5, (KVM_ARCH_GUEST_S5)(a0)
	sd	s6, (KVM_ARCH_GUEST_S6)(a0)
	sd	s7, (KVM_ARCH_GUEST_S7)(a0)
	sd	s8, (KVM_ARCH_GUEST_S8)(a0)
	sd	s9, (KVM_ARCH_GUEST_S9)(a0)
	sd	s10, (KVM_ARCH_GUEST_S10)(a0)
	sd	s11, (KVM_ARCH_GUEST_S11)(a0)
	sd	t3, (KVM_ARCH_GUEST_T3)(a0)
	sd	t4, (KVM_ARCH_GUEST_T4)(a0)
	sd	t5, (KVM_ARCH_GUEST_T5)(a0)
	sd	t6, (KVM_ARCH_GUEST_T6)(a0)
.endm

.macro SAVE_GUEST_AND_RESTORE_HOST_CSRS
	/* Load Host CSR values */
	ld	t0, (KVM_ARCH_HOST_STVEC)(a0)
	ld	t1, (KVM_ARCH_HOST_SSCRATCH)(a0)
	ld	t2, (KVM_ARCH_HOST_SSTATUS)(a0)

	/* Save Guest A0 and Restore Host SSCRATCH */
	csrrw	t1, 0x140, t1

	/* Save Guest SEPC */
	csrr	t3, 0x141

	/* Restore Host STVEC */
	csrw	0x105, t0

	/* Save Guest and Restore Host SSTATUS */
	csrrw	t2, 0x100, t2

	/* Store Guest CSR values */
	sd	t1, (KVM_ARCH_GUEST_A0)(a0)
	sd	t2, (KVM_ARCH_GUEST_SSTATUS)(a0)
	sd	t3, (KVM_ARCH_GUEST_SEPC)(a0)
.endm

.macro RESTORE_HOST_GPRS
	/* Restore Host GPRs (except A0 and T0-T6) */
	ld	ra, (KVM_ARCH_HOST_RA)(a0)
	ld	sp, (KVM_ARCH_HOST_SP)(a0)
	ld	gp, (KVM_ARCH_HOST_GP)(a0)
	ld	tp, (KVM_ARCH_HOST_TP)(a0)
	ld	s0, (KVM_ARCH_HOST_S0)(a0)
	ld	s1, (KVM_ARCH_HOST_S1)(a0)
	ld	a1, (KVM_ARCH_HOST_A1)(a0)
	ld	a2, (KVM_ARCH_HOST_A2)(a0)
	ld	a3, (KVM_ARCH_HOST_A3)(a0)
	ld	a4, (KVM_ARCH_HOST_A4)(a0)
	ld	a5, (KVM_ARCH_HOST_A5)(a0)
	ld	a6, (KVM_ARCH_HOST_A6)(a0)
	ld	a7, (KVM_ARCH_HOST_A7)(a0)
	ld	s2, (KVM_ARCH_HOST_S2)(a0)
	ld	s3, (KVM_ARCH_HOST_S3)(a0)
	ld	s4, (KVM_ARCH_HOST_S4)(a0)
	ld	s5, (KVM_ARCH_HOST_S5)(a0)
	ld	s6, (KVM_ARCH_HOST_S6)(a0)
	ld	s7, (KVM_ARCH_HOST_S7)(a0)
	ld	s8, (KVM_ARCH_HOST_S8)(a0)
	ld	s9, (KVM_ARCH_HOST_S9)(a0)
	ld	s10, (KVM_ARCH_HOST_S10)(a0)
	ld	s11, (KVM_ARCH_HOST_S11)(a0)
.endm

	.text
	.altmacro
	.option norelax

	/*
	 * Parameters:
	 * A0 <= Pointer to struct kvm_vcpu_arch
	 */

.global __kvm_riscv_switch_to
.type __kvm_riscv_switch_to, @function
.balign 4
__kvm_riscv_switch_to:

	SAVE_HOST_GPRS

	SAVE_HOST_AND_RESTORE_GUEST_CSRS .Lkvm_switch_return

	RESTORE_GUEST_GPRS

	/* Resume Guest using SRET */
	sret

	/* Back to Host */
	.align 2
.Lkvm_switch_return:
	SAVE_GUEST_GPRS

	SAVE_GUEST_AND_RESTORE_HOST_CSRS

	RESTORE_HOST_GPRS

	/* Return to rust code */
	ret
.set .L__sym_size___kvm_riscv_switch_to, .-__kvm_riscv_switch_to ;
.size __kvm_riscv_switch_to, .L__sym_size___kvm_riscv_switch_to



/*
.global __kvm_riscv_fp_f_save
.type __kvm_riscv_fp_f_save, @function
.balign 4
__kvm_riscv_fp_f_save:
	csrr t2, CSR_SSTATUS
	li t1, SR_FS
	csrs CSR_SSTATUS, t1
	frcsr t0
	fsw f0,  KVM_ARCH_FP_F_F0(a0)
	fsw f1,  KVM_ARCH_FP_F_F1(a0)
	fsw f2,  KVM_ARCH_FP_F_F2(a0)
	fsw f3,  KVM_ARCH_FP_F_F3(a0)
	fsw f4,  KVM_ARCH_FP_F_F4(a0)
	fsw f5,  KVM_ARCH_FP_F_F5(a0)
	fsw f6,  KVM_ARCH_FP_F_F6(a0)
	fsw f7,  KVM_ARCH_FP_F_F7(a0)
	fsw f8,  KVM_ARCH_FP_F_F8(a0)
	fsw f9,  KVM_ARCH_FP_F_F9(a0)
	fsw f10, KVM_ARCH_FP_F_F10(a0)
	fsw f11, KVM_ARCH_FP_F_F11(a0)
	fsw f12, KVM_ARCH_FP_F_F12(a0)
	fsw f13, KVM_ARCH_FP_F_F13(a0)
	fsw f14, KVM_ARCH_FP_F_F14(a0)
	fsw f15, KVM_ARCH_FP_F_F15(a0)
	fsw f16, KVM_ARCH_FP_F_F16(a0)
	fsw f17, KVM_ARCH_FP_F_F17(a0)
	fsw f18, KVM_ARCH_FP_F_F18(a0)
	fsw f19, KVM_ARCH_FP_F_F19(a0)
	fsw f20, KVM_ARCH_FP_F_F20(a0)
	fsw f21, KVM_ARCH_FP_F_F21(a0)
	fsw f22, KVM_ARCH_FP_F_F22(a0)
	fsw f23, KVM_ARCH_FP_F_F23(a0)
	fsw f24, KVM_ARCH_FP_F_F24(a0)
	fsw f25, KVM_ARCH_FP_F_F25(a0)
	fsw f26, KVM_ARCH_FP_F_F26(a0)
	fsw f27, KVM_ARCH_FP_F_F27(a0)
	fsw f28, KVM_ARCH_FP_F_F28(a0)
	fsw f29, KVM_ARCH_FP_F_F29(a0)
	fsw f30, KVM_ARCH_FP_F_F30(a0)
	fsw f31, KVM_ARCH_FP_F_F31(a0)
	sw t0, KVM_ARCH_FP_F_FCSR(a0)
	csrw CSR_SSTATUS, t2
	ret
SYM_FUNC_END(__kvm_riscv_fp_f_save)

SYM_FUNC_START(__kvm_riscv_fp_d_save)
	csrr t2, CSR_SSTATUS
	li t1, SR_FS
	csrs CSR_SSTATUS, t1
	frcsr t0
	fsd f0,  KVM_ARCH_FP_D_F0(a0)
	fsd f1,  KVM_ARCH_FP_D_F1(a0)
	fsd f2,  KVM_ARCH_FP_D_F2(a0)
	fsd f3,  KVM_ARCH_FP_D_F3(a0)
	fsd f4,  KVM_ARCH_FP_D_F4(a0)
	fsd f5,  KVM_ARCH_FP_D_F5(a0)
	fsd f6,  KVM_ARCH_FP_D_F6(a0)
	fsd f7,  KVM_ARCH_FP_D_F7(a0)
	fsd f8,  KVM_ARCH_FP_D_F8(a0)
	fsd f9,  KVM_ARCH_FP_D_F9(a0)
	fsd f10, KVM_ARCH_FP_D_F10(a0)
	fsd f11, KVM_ARCH_FP_D_F11(a0)
	fsd f12, KVM_ARCH_FP_D_F12(a0)
	fsd f13, KVM_ARCH_FP_D_F13(a0)
	fsd f14, KVM_ARCH_FP_D_F14(a0)
	fsd f15, KVM_ARCH_FP_D_F15(a0)
	fsd f16, KVM_ARCH_FP_D_F16(a0)
	fsd f17, KVM_ARCH_FP_D_F17(a0)
	fsd f18, KVM_ARCH_FP_D_F18(a0)
	fsd f19, KVM_ARCH_FP_D_F19(a0)
	fsd f20, KVM_ARCH_FP_D_F20(a0)
	fsd f21, KVM_ARCH_FP_D_F21(a0)
	fsd f22, KVM_ARCH_FP_D_F22(a0)
	fsd f23, KVM_ARCH_FP_D_F23(a0)
	fsd f24, KVM_ARCH_FP_D_F24(a0)
	fsd f25, KVM_ARCH_FP_D_F25(a0)
	fsd f26, KVM_ARCH_FP_D_F26(a0)
	fsd f27, KVM_ARCH_FP_D_F27(a0)
	fsd f28, KVM_ARCH_FP_D_F28(a0)
	fsd f29, KVM_ARCH_FP_D_F29(a0)
	fsd f30, KVM_ARCH_FP_D_F30(a0)
	fsd f31, KVM_ARCH_FP_D_F31(a0)
	sw t0, KVM_ARCH_FP_D_FCSR(a0)
	csrw CSR_SSTATUS, t2
	ret
SYM_FUNC_END(__kvm_riscv_fp_d_save)

SYM_FUNC_START(__kvm_riscv_fp_f_restore)
	csrr t2, CSR_SSTATUS
	li t1, SR_FS
	lw t0, KVM_ARCH_FP_F_FCSR(a0)
	csrs CSR_SSTATUS, t1
	flw f0,  KVM_ARCH_FP_F_F0(a0)
	flw f1,  KVM_ARCH_FP_F_F1(a0)
	flw f2,  KVM_ARCH_FP_F_F2(a0)
	flw f3,  KVM_ARCH_FP_F_F3(a0)
	flw f4,  KVM_ARCH_FP_F_F4(a0)
	flw f5,  KVM_ARCH_FP_F_F5(a0)
	flw f6,  KVM_ARCH_FP_F_F6(a0)
	flw f7,  KVM_ARCH_FP_F_F7(a0)
	flw f8,  KVM_ARCH_FP_F_F8(a0)
	flw f9,  KVM_ARCH_FP_F_F9(a0)
	flw f10, KVM_ARCH_FP_F_F10(a0)
	flw f11, KVM_ARCH_FP_F_F11(a0)
	flw f12, KVM_ARCH_FP_F_F12(a0)
	flw f13, KVM_ARCH_FP_F_F13(a0)
	flw f14, KVM_ARCH_FP_F_F14(a0)
	flw f15, KVM_ARCH_FP_F_F15(a0)
	flw f16, KVM_ARCH_FP_F_F16(a0)
	flw f17, KVM_ARCH_FP_F_F17(a0)
	flw f18, KVM_ARCH_FP_F_F18(a0)
	flw f19, KVM_ARCH_FP_F_F19(a0)
	flw f20, KVM_ARCH_FP_F_F20(a0)
	flw f21, KVM_ARCH_FP_F_F21(a0)
	flw f22, KVM_ARCH_FP_F_F22(a0)
	flw f23, KVM_ARCH_FP_F_F23(a0)
	flw f24, KVM_ARCH_FP_F_F24(a0)
	flw f25, KVM_ARCH_FP_F_F25(a0)
	flw f26, KVM_ARCH_FP_F_F26(a0)
	flw f27, KVM_ARCH_FP_F_F27(a0)
	flw f28, KVM_ARCH_FP_F_F28(a0)
	flw f29, KVM_ARCH_FP_F_F29(a0)
	flw f30, KVM_ARCH_FP_F_F30(a0)
	flw f31, KVM_ARCH_FP_F_F31(a0)
	fscsr t0
	csrw CSR_SSTATUS, t2
	ret
SYM_FUNC_END(__kvm_riscv_fp_f_restore)

SYM_FUNC_START(__kvm_riscv_fp_d_restore)
	csrr t2, CSR_SSTATUS
	li t1, SR_FS
	lw t0, KVM_ARCH_FP_D_FCSR(a0)
	csrs CSR_SSTATUS, t1
	fld f0,  KVM_ARCH_FP_D_F0(a0)
	fld f1,  KVM_ARCH_FP_D_F1(a0)
	fld f2,  KVM_ARCH_FP_D_F2(a0)
	fld f3,  KVM_ARCH_FP_D_F3(a0)
	fld f4,  KVM_ARCH_FP_D_F4(a0)
	fld f5,  KVM_ARCH_FP_D_F5(a0)
	fld f6,  KVM_ARCH_FP_D_F6(a0)
	fld f7,  KVM_ARCH_FP_D_F7(a0)
	fld f8,  KVM_ARCH_FP_D_F8(a0)
	fld f9,  KVM_ARCH_FP_D_F9(a0)
	fld f10, KVM_ARCH_FP_D_F10(a0)
	fld f11, KVM_ARCH_FP_D_F11(a0)
	fld f12, KVM_ARCH_FP_D_F12(a0)
	fld f13, KVM_ARCH_FP_D_F13(a0)
	fld f14, KVM_ARCH_FP_D_F14(a0)
	fld f15, KVM_ARCH_FP_D_F15(a0)
	fld f16, KVM_ARCH_FP_D_F16(a0)
	fld f17, KVM_ARCH_FP_D_F17(a0)
	fld f18, KVM_ARCH_FP_D_F18(a0)
	fld f19, KVM_ARCH_FP_D_F19(a0)
	fld f20, KVM_ARCH_FP_D_F20(a0)
	fld f21, KVM_ARCH_FP_D_F21(a0)
	fld f22, KVM_ARCH_FP_D_F22(a0)
	fld f23, KVM_ARCH_FP_D_F23(a0)
	fld f24, KVM_ARCH_FP_D_F24(a0)
	fld f25, KVM_ARCH_FP_D_F25(a0)
	fld f26, KVM_ARCH_FP_D_F26(a0)
	fld f27, KVM_ARCH_FP_D_F27(a0)
	fld f28, KVM_ARCH_FP_D_F28(a0)
	fld f29, KVM_ARCH_FP_D_F29(a0)
	fld f30, KVM_ARCH_FP_D_F30(a0)
	fld f31, KVM_ARCH_FP_D_F31(a0)
	fscsr t0
	csrw CSR_SSTATUS, t2
	ret
SYM_FUNC_END(__kvm_riscv_fp_d_restore)
*/
