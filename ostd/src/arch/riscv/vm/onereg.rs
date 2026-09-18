///1

pub const KVM_REG_ARCH_MASK:u64 = 0xff00000000000000;
pub const KVM_REG_SIZE_MASK:u64 = 0x00f0000000000000;

pub const KVM_REG_RISCV_TYPE_MASK: u64 = 0x00000000FF000000;
pub const KVM_REG_RISCV_TYPE_SHIFT: u64 = 24;
pub const KVM_REG_RISCV_SUBTYPE_MASK: u64 = 0x0000000000FF0000;
pub const KVM_REG_RISCV_SUBTYPE_SHIFT: u64 = 16;

pub const KVM_REG_RISCV_CONFIG: u64 = 0x01 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_CORE: u64 = 0x02 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_CSR: u64 = 0x03 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_TIMER: u64 = 0x04 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_FP_F: u64 = 0x05 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_FP_D: u64 = 0x06 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_ISA_EXT: u64 = 0x07 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_SBI_EXT: u64 = 0x08 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_VECTOR: u64 = 0x09 << KVM_REG_RISCV_TYPE_SHIFT;
pub const KVM_REG_RISCV_SBI_STATE: u64 = 0x0A << KVM_REG_RISCV_TYPE_SHIFT;

pub const KVM_RISCV_BASE_ISA_MASK: u64 = 0x0000000003FFFFFF;

pub const KVM_REG_RISCV_CSR_GENERAL: u64 = 	0x0 << KVM_REG_RISCV_SUBTYPE_SHIFT;
pub const KVM_REG_RISCV_CSR_AIA: u64 = 0x1 << KVM_REG_RISCV_SUBTYPE_SHIFT;
pub const KVM_REG_RISCV_CSR_SMSTATEEN: u64 = 0x2 << KVM_REG_RISCV_SUBTYPE_SHIFT;

pub const KVM_REG_RISCV_ISA_SINGLE: u64 =	0x0 << KVM_REG_RISCV_SUBTYPE_SHIFT;
pub const KVM_REG_RISCV_ISA_MULTI_EN: u64 =	0x1 << KVM_REG_RISCV_SUBTYPE_SHIFT;
pub const KVM_REG_RISCV_ISA_MULTI_DIS: u64 =	0x2 << KVM_REG_RISCV_SUBTYPE_SHIFT;

pub const KVM_REG_RISCV_SBI_SINGLE: u64 = 0x0 << KVM_REG_RISCV_SUBTYPE_SHIFT;
pub const KVM_REG_RISCV_SBI_MULTI_EN: u64 =	0x1 << KVM_REG_RISCV_SUBTYPE_SHIFT;
pub const KVM_REG_RISCV_SBI_MULTI_DIS: u64 = 0x2 << KVM_REG_RISCV_SUBTYPE_SHIFT;

pub const KVM_REG_RISCV_SBI_STA: usize = 0x0 << KVM_REG_RISCV_SUBTYPE_SHIFT;
pub const KVM_REG_RISCV_SBI_FWFT: usize = 0x1 << KVM_REG_RISCV_SUBTYPE_SHIFT;

pub const KVM_REG_RISCV:u64 = 0x8000000000000000;
pub const KVM_REG_SIZE_U32:u64 = 0x0020000000000000;
pub const KVM_REG_SIZE_U64:u64 = 0x0030000000000000;

/// 1
#[inline(always)]
pub fn kvm_reg_size(id: u64) -> u32 {
    const SHIFT: u64 = 52;
    const MASK: u64 = 0x00f0000000000000;
    
    1u32 << (((id & MASK) >> SHIFT) as u32)
}

/// 2
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub struct OneReg {
    /// regid
	pub id: u64,
    /// the addr we should read from / write to
	pub addr: u64,
}

/// CONFIG registers for KVM_GET_ONE_REG and KVM_SET_ONE_REG
pub struct KvmConfig {
    isa: usize,
    zicbom_block_size: usize,
    mvendorid: usize,
    marchid: usize,
    mimpid: usize,
    zicboz_block_size: usize,
    satp_mode: usize,
    zicbop_block_size: usize,
}

impl KvmConfig {
    pub const ISA: u64 = 0;
    pub const ZICBOM_BLOCK_SIZE: u64 = Self::ISA + 1;
    pub const MVENDORID: u64 = Self::ZICBOM_BLOCK_SIZE + 1;
    pub const MARCHID: u64 = Self::MVENDORID + 1;
    pub const MIMPID: u64 = Self::MARCHID + 1;
    pub const ZICBOZ_BLOCK_SIZE: u64 = Self::MIMPID + 1;
    pub const SATP_MODE: u64 = Self::ZICBOZ_BLOCK_SIZE + 1;
    pub const ZICBOP_BLOCK_SIZE: u64 = Self::SATP_MODE + 1;
}

pub struct UserRegs {
    // different to GeneralRegs, first is 'pc', not 'zero'
    pub pc: usize,
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
}

/// CORE registers for KVM_GET_ONE_REG and KVM_SET_ONE_REG
pub struct KvmCore {
	regs: UserRegs,
	mode: u64,
}

impl KvmCore {
    pub const PC: u64 = 0;
    pub const T6: u64 = Self::PC + 31;
    pub const MODE: u64 = Self::PC + 32;
}

#[derive(Clone, Copy, Debug, Default, Pod)]
pub struct KvmVcpuCsr {
	pub vsstatus: usize,
	pub vsie: usize,
	pub vstvec: usize,
	pub vsscratch: usize,
	pub vsepc: usize,
	pub vscause: usize,
	pub vstval: usize,
	pub hvip: usize,
	pub vsatp: usize,
	pub scounteren: usize,
	pub senvcfg: usize,
}

impl KvmVcpuCsr {
    pub const SIP: u64 = 7;
}

pub const IDX_VSSTATUS: usize = 0;
pub const IDX_VSIE: usize = 1;
pub const IDX_VSTVEC: usize = 2;
pub const IDX_VSSCRATCH: usize = 3;
pub const IDX_VSEPC: usize = 4;
pub const IDX_VSCAUSE: usize = 5;
pub const IDX_VSTVAL: usize = 6;
pub const IDX_HVIP: usize = 7;
pub const IDX_VSATP: usize = 8;
pub const IDX_SCOUNTEREN: usize = 9;
pub const IDX_SENVCFG: usize = 10;
pub const CSR_COUNT: usize = 11;

impl KvmVcpuCsr {
    /// 通过索引获取 CSR 值
    pub fn get_csr(&self, index: u64) -> Option<usize> {
        match index {
            0 => Some(self.vsstatus),
            1 => Some(self.vsie),
            2 => Some(self.vstvec),
            3 => Some(self.vsscratch),
            4 => Some(self.vsepc),
            5 => Some(self.vscause),
            6 => Some(self.vstval),
            7 => Some(self.hvip),
            8 => Some(self.vsatp),
            9 => Some(self.scounteren),
            10 => Some(self.senvcfg),
            _ => None,
        }
    }

    /// 通过索引设置 CSR 值
    pub fn set_csr(&mut self, index: usize, value: usize) -> bool {
        match index {
            0 => { self.vsstatus = value; true }
            1 => { self.vsie = value; true }
            2 => { self.vstvec = value; true }
            3 => { self.vsscratch = value; true }
            4 => { self.vsepc = value; true }
            5 => { self.vscause = value; true }
            6 => { self.vstval = value; true }
            7 => { self.hvip = value; true }
            8 => { self.vsatp = value; true }
            9 => { self.scounteren = value; true }
            10 => { self.senvcfg = value; true }
            _ => false,
        }
    }
}

/// TIMER registers for KVM_GET_ONE_REG and KVM_SET_ONE_REG
pub struct KvmRiscvTimer {
	pub frequency: u64,
	pub time: u64,
	pub compare: u64,
	pub state: u64,
}

impl KvmRiscvTimer {
    pub const FREQUENCY: u64 = 0;
    pub const TIME: u64 = Self::FREQUENCY + 1;
    pub const COMPARE: u64 = Self::TIME + 1;
    pub const STATE: u64 = Self::COMPARE + 1;
}

/// SBI STA extension registers for KVM_GET_ONE_REG and KVM_SET_ONE_REG */
pub struct KvmRiscvSbiSta {
	shmem_lo: usize,
	shmem_hi: usize,
}
