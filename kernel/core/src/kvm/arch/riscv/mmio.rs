use ostd::arch::vm::insn_len;
use ostd::arch::vm::VcpuContext;
use crate::kvm::run_page::KvmRun;
use crate::kvm::ioctl::KVM_RUN_MMIO_DATA_OFFSET;
use crate::prelude::*;

const INSN_MATCH_LB: usize = 0x3;
const INSN_MASK_LB: usize = 0x707f;
const INSN_MATCH_LH: usize = 0x1003;
const INSN_MASK_LH: usize = 0x707f;
const INSN_MATCH_LW: usize = 0x2003;
const INSN_MASK_LW: usize = 0x707f;
const INSN_MATCH_LD: usize = 0x3003;
const INSN_MASK_LD: usize = 0x707f;
const INSN_MATCH_LBU: usize = 0x4003;
const INSN_MASK_LBU: usize = 0x707f;
const INSN_MATCH_LHU: usize = 0x5003;
const INSN_MASK_LHU: usize = 0x707f;
const INSN_MATCH_LWU: usize = 0x6003;
const INSN_MASK_LWU: usize = 0x707f;
const INSN_MATCH_SB: usize = 0x23;
const INSN_MASK_SB: usize = 0x707f;
const INSN_MATCH_SH: usize = 0x1023;
const INSN_MASK_SH: usize = 0x707f;
const INSN_MATCH_SW: usize = 0x2023;
const INSN_MASK_SW: usize = 0x707f;
const INSN_MATCH_SD: usize = 0x3023;
const INSN_MASK_SD: usize = 0x707f;
const INSN_MATCH_C_LD: usize = 0x6000;
const INSN_MASK_C_LD: usize = 0xe003;
const INSN_MATCH_C_SD: usize = 0xe000;
const INSN_MASK_C_SD: usize = 0xe003;
const INSN_MATCH_C_LW: usize = 0x4000;
const INSN_MASK_C_LW: usize = 0xe003;
const INSN_MATCH_C_SW: usize = 0xc000;
const INSN_MASK_C_SW: usize = 0xe003;
const INSN_MATCH_C_LDSP: usize = 0x6002;
const INSN_MASK_C_LDSP: usize = 0xe003;
const INSN_MATCH_C_SDSP: usize = 0xe002;
const INSN_MASK_C_SDSP: usize = 0xe003;
const INSN_MATCH_C_LWSP: usize = 0x4002;
const INSN_MASK_C_LWSP: usize = 0xe003;
const INSN_MATCH_C_SWSP: usize = 0xc002;
const INSN_MASK_C_SWSP: usize = 0xe003;

const SH_RD: usize = 7;
const SH_RS2C: usize = 2;

const INSN_16BIT_MASK: usize = 0x3;
const LOG_REGBYTES: usize = 3;
const REG_MASK: usize = (1 << (5 + LOG_REGBYTES)) - (1 << LOG_REGBYTES);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub(super) struct MmioDecode {
	pub(super) insn: usize,
	pub(super) insn_len: i32,
	pub(super) len: i32,
	pub(super) shift: i32,
	pub(super) return_handled: i32,
}

impl MmioDecode {
    pub(super) fn reset_with_insn(&mut self, trap_insn: usize) {
        self.insn = 0;
        self.insn_len = 0;
        self.len = 0;
        self.shift = 0;

        // Determine trapped instruction
        if trap_insn & 0x1 != 0 {
		    // Bit[0] == 1 implies trapped instruction value is
		    // transformed instruction or custom instruction.
		    self.insn = trap_insn | INSN_16BIT_MASK;
            if trap_insn & 0x1 << 1 != 0 {
                self.insn_len = insn_len(self.insn) as i32;
            } else {
                self.insn_len = 2;
            }
	    }
    }

    /// Decode length of MMIO and shift
    pub(super) fn load_decode(&mut self) {
        if self.insn & INSN_MASK_LW == INSN_MATCH_LW {
            self.len = 4;
            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
        } else if self.insn & INSN_MASK_LB == INSN_MATCH_LB {
            self.len = 1;
            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
        } else if self.insn & INSN_MASK_LBU == INSN_MATCH_LBU {
            self.len = 1;
        } else if self.insn & INSN_MASK_LD == INSN_MATCH_LD {
            self.len = 8;
            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
        } else if self.insn & INSN_MASK_LWU == INSN_MATCH_LWU {
            self.len = 4;
        } else if self.insn & INSN_MASK_LH == INSN_MATCH_LH {
            self.len = 2;
            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
        } else if self.insn & INSN_MASK_LHU == INSN_MATCH_LHU {
            self.len = 2;
        } else if self.insn & INSN_MASK_C_LD == INSN_MATCH_C_LD {
            self.len = 8;
            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
            self.insn = self.rvc_rs2s() << SH_RD;
        } else if self.insn & INSN_MASK_C_LDSP == INSN_MATCH_C_LDSP && (self.insn >> SH_RD) & 0x1f != 0 {
            self.len = 8;
            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
        } else if self.insn & INSN_MASK_C_LW == INSN_MATCH_C_LW {
            self.len = 4;
            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
            self.insn = self.rvc_rs2s() << SH_RD;
        } else if self.insn & INSN_MASK_C_LWSP == INSN_MATCH_C_LWSP && (self.insn >> SH_RD) & 0x1f != 0 {
            self.len = 4;
            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
        }

        self.return_handled = 0;
    }

    /// Decode length of MMIO and data
    pub(super) fn store_decode(&mut self, ctx: &VcpuContext) -> Result<usize> {
        let mut data = self.get_rs2(ctx)?;

        if self.insn & INSN_MASK_SW == INSN_MATCH_SW {
            self.len = 4;
        } else if self.insn & INSN_MASK_SB == INSN_MATCH_SB {
            self.len = 1;
        } else if self.insn & INSN_MASK_SD == INSN_MATCH_SD {
            self.len = 8;
        } else if self.insn & INSN_MASK_SH == INSN_MATCH_SH {
            self.len = 2;
        } else if self.insn & INSN_MASK_C_SD == INSN_MATCH_C_SD {
            self.len = 8;
            data = self.get_rs2s(ctx)?;
        } else if self.insn & INSN_MASK_C_SDSP == INSN_MATCH_C_SDSP && (self.insn >> SH_RD) & 0x1f != 0 {
            self.len = 8;
            data = self.get_rs2s(ctx)?;
        } else if self.insn & INSN_MASK_C_SW == INSN_MATCH_C_SW {
            self.len = 4;
            data = self.get_rs2s(ctx)?;
        } else if self.insn & INSN_MASK_C_SWSP == INSN_MATCH_C_SWSP && (self.insn >> SH_RD) & 0x1f != 0 {
            self.len = 4;
            data = self.get_rs2s(ctx)?;
        }
        self.return_handled = 0;

        Ok(data)
    }

    /// Handle the data from user
    pub(super) fn handle_return(&self, run: Arc<KvmRun>, ctx: &mut VcpuContext) -> Result<()> {
        let index = (self.insn >> SH_RD) & 0x1F;
        match self.len {
            1 => {
                let data = run.read_run_val::<u8>(KVM_RUN_MMIO_DATA_OFFSET)?;
                let wdata = ((((data as usize) << self.shift) as isize) >> self.shift) as usize;
                ctx.set_reg(index, wdata)?;
            }
            2 => {
                let data = run.read_run_val::<u16>(KVM_RUN_MMIO_DATA_OFFSET)?;
                let wdata = ((((data as usize) << self.shift) as isize) >> self.shift) as usize;
                ctx.set_reg(index, wdata)?;
            }
            4 => {
                let data = run.read_run_val::<u32>(KVM_RUN_MMIO_DATA_OFFSET)?;
                let wdata = ((((data as usize) << self.shift) as isize) >> self.shift) as usize;
                ctx.set_reg(index, wdata)?;
            }
            8 => {
                let data = run.read_run_val::<u64>(KVM_RUN_MMIO_DATA_OFFSET)?;
                let wdata = ((((data as usize) << self.shift) as isize) >> self.shift) as usize;
                ctx.set_reg(index, wdata)?;
            }
            _ => return Err(Error::new(Errno::EOPNOTSUPP)),
        }
        Ok(())
    }

    /// Get the rs2 reg_num of RVC insn
    fn rvc_rs2s(&self) -> usize {
        8 + self.rv_x(SH_RS2C, 3)
    }

    fn rv_x(&self, s: usize, n: usize) -> usize {
        self.rv_x_mask(s, (1 << (n)) - 1)
    }

    fn rv_x_mask(&self, s: usize, mask: usize) -> usize {
        ((self.insn) >> (s)) & (mask)
    }

    /// Get the rs2 reg_val of insn
    fn get_rs2(&self, ctx: &VcpuContext) -> Result<usize> {
        Ok(ctx.get_reg((((self.insn) >> 17) & REG_MASK) / size_of::<usize>())?)
    }

    /// Get the rs2 reg_val of RVC insn
    fn get_rs2s(&self, ctx: &VcpuContext) -> Result<usize> {
        Ok(ctx.get_reg((((self.rvc_rs2s() << 7) << 3) & REG_MASK) / size_of::<usize>())?)
    }
}
