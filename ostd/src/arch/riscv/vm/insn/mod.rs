// SPDX-License-Identifier: MPL-2.0

mod csr;
mod wfi;
mod wrs;

use wrs::WRS;
use wfi::WFI;
use csr::{CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI};

/// The shift of insns
pub const INSN_OPCODE_SHIFT: usize = 2;
/// The mask of insns
pub const INSN_OPCODE_MASK: usize = 0x007c;

/// Return values used by function emulating a particular instruction
pub enum KvmInsnReturn {
	/// Need exit to user-space
	ExitToUserSpace = 0,
	/// Continue with next sepc
	ContinueNextSepc,
	/// Continue with same sepc
	ContinueSameSepc,
	/// Inject illegal instruction trap and continue
	IllegalTrap,
	/// Inject virtual instruction trap and continue
	VirtualTrap
}

type InsnFunction = dyn Fn(usize) -> KvmInsnReturn + Sync + Send + 'static;

struct InsnFunc {
	mask: usize,
	should_match: usize,
	func: &'static InsnFunction,
}

const SYSTEM_OPCODE_FUNCS: [InsnFunc; 8] = [
    CSRRW,
    CSRRS,
	CSRRC,
    CSRRWI,
	CSRRSI,
    CSRRCI,
	WFI,
    WRS,
];

/// Get the length of the insn
pub fn insn_len(insn: usize) -> usize {
	if (insn & 0x3) != 0x3 {
        return 2;
    } else {
        return 4;
    }
}

/// Handle the system insns
pub fn handle_system_opcode_insn(insn: usize) -> KvmInsnReturn {
	let mut ret = KvmInsnReturn::ExitToUserSpace;
    for i in 0..8 {
        let func = &SYSTEM_OPCODE_FUNCS[i];
        if insn & func.mask == func.should_match {
            let func = func.func;
            ret = func(insn);
        }
    }
    ret
}
