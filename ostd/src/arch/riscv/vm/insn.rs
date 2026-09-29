/// 1



#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub struct KvmMmioDecode {
	pub insn: usize,
	pub insn_len: i32,
	pub len: i32,
	pub shift: i32,
	pub return_handled: i32,
}

struct KvmCsrDecode {
	insn: usize,
	return_handled: i32,
}

/* Return values used by function emulating a particular instruction */
enum KvmInsnReturn {
	KVM_INSN_EXIT_TO_USER_SPACE = 0,
	KVM_INSN_CONTINUE_NEXT_SEPC,
	KVM_INSN_CONTINUE_SAME_SEPC,
	KVM_INSN_ILLEGAL_TRAP,
	KVM_INSN_VIRTUAL_TRAP
}

pub type InsnFunction = dyn Fn(usize) -> usize + Sync + Send + 'static;

pub struct InsnFunc {
	pub mask: usize,
	pub should_match: usize,
	/*
	 * Possible return values are as follows:
	 * 1) Returns < 0 for error case
	 * 2) Returns 0 for exit to user-space
	 * 3) Returns 1 to continue with next sepc
	 * 4) Returns 2 to continue with same sepc
	 * 5) Returns 3 to inject illegal instruction trap and continue
	 * 6) Returns 4 to inject virtual instruction trap and continue
	 *
	 * Use enum kvm_insn_return for return values
	 */
	pub func: &'static InsnFunction,
}

pub const INSN_MATCH_CSRRW: usize = 0x1073;
pub const INSN_MASK_CSRRW: usize = 0x707f;
pub const INSN_MATCH_CSRRS: usize = 0x2073;
pub const INSN_MASK_CSRRS: usize = 0x707f;
pub const INSN_MATCH_CSRRC: usize = 0x3073;
pub const INSN_MASK_CSRRC: usize = 0x707f;
pub const INSN_MATCH_CSRRWI: usize = 0x5073;
pub const INSN_MASK_CSRRWI: usize = 0x707f;
pub const INSN_MATCH_CSRRSI: usize = 0x6073;
pub const INSN_MASK_CSRRSI: usize = 0x707f;
pub const INSN_MATCH_CSRRCI: usize = 0x7073;
pub const INSN_MASK_CSRRCI: usize = 0x707f;
pub const INSN_MASK_WFI: usize = 0xffffffff;
pub const INSN_MATCH_WFI: usize = 0x10500073;
pub const INSN_MASK_WRS: usize = 0xffffffff;
pub const INSN_MATCH_WRS: usize = 0x00d00073;


pub const INSN_MATCH_LB: usize = 0x3;
pub const INSN_MASK_LB: usize = 0x707f;
pub const INSN_MATCH_LH: usize = 0x1003;
pub const INSN_MASK_LH: usize = 0x707f;
pub const INSN_MATCH_LW: usize = 0x2003;
pub const INSN_MASK_LW: usize = 0x707f;
pub const INSN_MATCH_LD: usize = 0x3003;
pub const INSN_MASK_LD: usize = 0x707f;
pub const INSN_MATCH_LBU: usize = 0x4003;
pub const INSN_MASK_LBU: usize = 0x707f;
pub const INSN_MATCH_LHU: usize = 0x5003;
pub const INSN_MASK_LHU: usize = 0x707f;
pub const INSN_MATCH_LWU: usize = 0x6003;
pub const INSN_MASK_LWU: usize = 0x707f;
pub const INSN_MATCH_SB: usize = 0x23;
pub const INSN_MASK_SB: usize = 0x707f;
pub const INSN_MATCH_SH: usize = 0x1023;
pub const INSN_MASK_SH: usize = 0x707f;
pub const INSN_MATCH_SW: usize = 0x2023;
pub const INSN_MASK_SW: usize = 0x707f;
pub const INSN_MATCH_SD: usize = 0x3023;
pub const INSN_MASK_SD: usize = 0x707f;

pub const INSN_MATCH_C_LD: usize = 0x6000;
pub const INSN_MASK_C_LD: usize = 0xe003;
pub const INSN_MATCH_C_SD: usize = 0xe000;
pub const INSN_MASK_C_SD: usize = 0xe003;
pub const INSN_MATCH_C_LW: usize = 0x4000;
pub const INSN_MASK_C_LW: usize = 0xe003;
pub const INSN_MATCH_C_SW: usize = 0xc000;
pub const INSN_MASK_C_SW: usize = 0xe003;
pub const INSN_MATCH_C_LDSP: usize = 0x6002;
pub const INSN_MASK_C_LDSP: usize = 0xe003;
pub const INSN_MATCH_C_SDSP: usize = 0xe002;
pub const INSN_MASK_C_SDSP: usize = 0xe003;
pub const INSN_MATCH_C_LWSP: usize = 0x4002;
pub const INSN_MASK_C_LWSP: usize = 0xe003;
pub const INSN_MATCH_C_SWSP: usize = 0xc002;
pub const INSN_MASK_C_SWSP: usize = 0xe003;


pub const SYSTEM_OPCODE_FUNCS: [InsnFunc; 8] = [
    InsnFunc {
        mask: INSN_MASK_CSRRW,
		should_match: INSN_MATCH_CSRRW,
		func: &csr_insn,
    },
    InsnFunc {
        mask: INSN_MASK_CSRRS,
		should_match: INSN_MATCH_CSRRS,
		func: &csr_insn,
    },
	InsnFunc {
        mask: INSN_MASK_CSRRC,
		should_match: INSN_MATCH_CSRRC,
		func: &csr_insn,
    },
    InsnFunc {
        mask: INSN_MASK_CSRRWI,
		should_match: INSN_MATCH_CSRRWI,
		func: &csr_insn,
    },
	InsnFunc {
        mask: INSN_MASK_CSRRSI,
		should_match: INSN_MATCH_CSRRSI,
		func: &csr_insn,
    },
    InsnFunc {
        mask: INSN_MASK_CSRRCI,
		should_match: INSN_MATCH_CSRRCI,
		func: &csr_insn,
    },
	InsnFunc {
        mask: INSN_MASK_WFI,
		should_match: INSN_MATCH_WFI,
		func: &wfi_insn,
    },
    InsnFunc {
        mask: INSN_MASK_WRS,
		should_match: INSN_MATCH_WRS,
		func: &wrs_insn,
    },
];



pub fn csr_insn(insn: usize) -> usize {
    0
}

pub fn wfi_insn(insn: usize) -> usize {
	loop{}
    0
}

pub fn wrs_insn(insn: usize) -> usize {
    0
}


pub fn insn_len(insn: usize) -> usize {
	if (insn & 0x3) != 0x3 {
        return 2;
    } else {
        return 4;
    }
}
