use core::ops::Range;

use crate::mm::{
    AnyUFrameMeta, HasPaddr, Paddr, PageProperty, PagingConstsTrait, PagingLevel, PodOnce, UFrame,
    frame::{FrameRef, uframe_from_raw, uframe_ref_from_raw},
    page_prop::{CachePolicy, PageFlags, PageTableFlags, PrivilegedPageFlags as PrivFlags},
    page_table::{PageTableConfig, PteScalar, PteTrait},
};

#[derive(Clone, Debug)]
pub enum GstagePtConfig {}

// use sv39*4
pub const GSTAGE_MAX_PGD_LEVELS: usize = 3;
const NR_LEVELS: usize = 3;
const ADDRESS_WIDTH: usize = 39;

unsafe impl PageTableConfig for GstagePtConfig {
    // 1 for 512GB, 256 is enough.
    const TOP_LEVEL_INDEX_RANGE: Range<usize> = 0..256;

    type E = PageTableEntry;
    type C = PagingConsts;

    /// All mappings are tracked untyped frames.
    type Item = GstageItem;
    type ItemRef<'a> = GstageItemRef<'a>;

    fn item_raw_info(item: &Self::Item) -> (Paddr, PagingLevel, PageProperty) {
        let (frame, prop) = item;
        (frame.paddr(), frame.map_level(), *prop)
    }

    unsafe fn item_from_raw(paddr: Paddr, level: PagingLevel, prop: PageProperty) -> Self::Item {
        debug_assert_eq!(level, 1);
        // SAFETY: The caller ensures that the raw item was produced from a
        // `UFrame` previously consumed by this page table.
        let frame = unsafe { uframe_from_raw(paddr) };
        (frame, prop)
    }

    unsafe fn item_ref_from_raw<'a>(
        paddr: Paddr,
        level: PagingLevel,
        prop: PageProperty,
    ) -> Self::ItemRef<'a> {
        debug_assert_eq!(level, 1);
        // SAFETY: The caller ensures that the mapped frame outlives `'a`.
        let frame = unsafe { uframe_ref_from_raw(paddr) };
        (frame, prop)
    }
}

pub(crate) type GstageItem = (UFrame, PageProperty);
pub(crate) type GstageItemRef<'a> = (FrameRef<'a, dyn AnyUFrameMeta>, PageProperty);

#[derive(Clone, Debug, Default)]
pub(crate) struct PagingConsts {}

impl PagingConstsTrait for PagingConsts {
    const BASE_PAGE_SIZE: usize = 4096;
    const NR_LEVELS: PagingLevel = 3;
    const ADDRESS_WIDTH: usize = 39;
    const VA_SIGN_EXT: bool = true;
    const HIGHEST_TRANSLATION_LEVEL: PagingLevel = 2;
    const PTE_SIZE: usize = size_of::<PageTableEntry>();
}

bitflags::bitflags! {
    #[repr(C)]
    #[derive(Pod)]
    pub struct PteFlags: usize {
        const VALID =       1 << 0;
        const READABLE =    1 << 1;
        const WRITABLE =    1 << 2;
        const EXECUTABLE =  1 << 3;
        const USER =        1 << 4;
        const GLOBAL =      1 << 5;
        const ACCESSED =    1 << 6;
        const DIRTY =       1 << 7;
        
        const CUSTOM_CACHE = 1 << 8;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub struct PageTableEntry(usize);

impl PageTableEntry {
    const PHYS_MASK: usize = 0x00ff_ffff_ffff_f000;
    
    fn is_present(&self) -> bool {
        self.0 & PteFlags::VALID.bits() != 0
    }

    fn is_last(&self, level: PagingLevel) -> bool {
        level == 1 || (self.0 & (PteFlags::READABLE | PteFlags::WRITABLE | PteFlags::EXECUTABLE).bits()) != 0
    }

    fn prop(&self) -> PageProperty {
        let flags = 
            (if self.0 & PteFlags::READABLE.bits() != 0 { PageFlags::R.bits() } else { 0 }) |
            (if self.0 & PteFlags::WRITABLE.bits() != 0 { PageFlags::W.bits() } else { 0 }) |
            (if self.0 & PteFlags::EXECUTABLE.bits() != 0 { PageFlags::X.bits() } else { 0 });

        let cache = CachePolicy::Writeback;

        PageProperty {
            flags: PageFlags::from_bits(flags as u8).unwrap(),
            cache,
            priv_flags: PrivFlags::empty(),
        }
    }

    fn pt_flags(&self) -> PageTableFlags {
        PageTableFlags::empty()
    }

    fn new_page(paddr: Paddr, _level: PagingLevel, prop: PageProperty) -> Self {
        let mut entry = paddr & Self::PHYS_MASK;
        
        entry |= PteFlags::VALID.bits() | PteFlags::ACCESSED.bits() | PteFlags::DIRTY.bits();
        
        if prop.flags.contains(PageFlags::R) {
            entry |= PteFlags::READABLE.bits();
        }
        if prop.flags.contains(PageFlags::W) {
            entry |= PteFlags::WRITABLE.bits();
        }
        if prop.flags.contains(PageFlags::X) {
            entry |= PteFlags::EXECUTABLE.bits();
        }
        
        Self(entry)
    }

    fn new_pt(paddr: Paddr, _flags: PageTableFlags) -> Self {
        let entry = (paddr & Self::PHYS_MASK) | PteFlags::VALID.bits();
        Self(entry)
    }
}

impl PodOnce for PageTableEntry {}

/// SAFETY: The implementation is safe because:
///  -
unsafe impl PteTrait for PageTableEntry {
    fn from_repr(repr: &PteScalar, level: PagingLevel) -> Self {
        match repr {
            PteScalar::Absent => PageTableEntry(0),
            PteScalar::PageTable(paddr, flags) => Self::new_pt(*paddr, *flags),
            PteScalar::Mapped(paddr, prop) => Self::new_page(*paddr, level, *prop),
        }
    }

    fn to_repr(&self, level: PagingLevel) -> PteScalar {
        if !self.is_present() {
            return PteScalar::Absent;
        }

        let paddr = self.0 & Self::PHYS_MASK;
        if self.is_last(level) {
            PteScalar::Mapped(paddr, self.prop())
        } else {
            PteScalar::PageTable(paddr, self.pt_flags())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum HgatpMode {
    Off = 0,
    Sv32x4 = 1,
    Sv39x4 = 8,
    Sv48x4 = 9,
    Sv57x4 = 10,
}

impl HgatpMode {
    pub fn from_pgd_levels(levels: usize) -> Self {
        match levels {
            2 => HgatpMode::Sv32x4,
            3 => HgatpMode::Sv39x4,
            4 => HgatpMode::Sv48x4,
            5 => HgatpMode::Sv57x4,
            _ => HgatpMode::Off,
        }
    }
}

pub const HGATP_PPN:usize = 0x00000FFFFFFFFFFF;
pub const HGATP_VMID_SHIFT:usize = 44;
pub const HGATP_VMID:usize = 0x3FFF000000000000;
pub const HGATP_MODE_SHIFT:usize = 60;

/// 执行 HFENCE.GVMA，使用指定的寄存器（直接写入 .insn）
/// 
/// # 参数
/// - `rs1`: 寄存器编号 (0-31)
/// - `rs2`: 寄存器编号 (0-31)
/// 
/// # 格式
/// .insn r opcode, func3, func7, rd, rs1, rs2
/// HFENCE.GVMA: opcode=0x73, func3=0, func7=49, rd=x0
#[inline(always)]
pub unsafe fn hfence_gvma_asm(rs1: u8, rs2: u8) {
    let rs1_val: usize = rs1.into();
    let rs2_val: usize = rs2.into();
    
    unsafe {
        core::arch::asm!(
            // .insn r opcode, func3, func7, rd, rs1, rs2
            ".insn r 0x73, 0, 49, x0, {rs1}, {rs2}",
            rs1 = in(reg) rs1_val,
            rs2 = in(reg) rs2_val,
            options(nostack, preserves_flags)
        );
    }
}


/// HFENCE.VVMA - Hypervisor Virtual-Virtual-Memory Fence 指令编码
///
/// 用于 VS-stage 地址转换（guest virtual -> guest physical）的屏障指令
///
/// # 参数
/// - `rs1`: 源寄存器编号 (0-31)
///   - 若 rs1 = x0: fence 所有 guest 虚拟地址
///   - 若 rs1 ≠ x0: 仅 fence 指定的 guest 虚拟地址
/// - `rs2`: 源寄存器编号 (0-31)
///   - 若 rs2 = x0: fence 所有 ASID
///   - 若 rs2 ≠ x0: 仅 fence 指定的 ASID
///
/// # 注意
/// - 仅在 M-mode 或 HS-mode 时有效
/// - 应用到当前 VM (由 hgatp.VMID 标识)
/// - 在 V=1 时执行会触发 virtual-instruction exception
#[inline(always)]
pub fn hfence_vvma(rs1: u8, rs2: u8) -> u32 {
    const OPCODE_SYSTEM: u32 = 0x73;
    const FUNC3: u32 = 0;
    const FUNC7: u32 = 17;  // HFENCE.VVMA 的 FUNC7 是 17 (0b0010001)
    const RD: u32 = 0;
    
    OPCODE_SYSTEM
        | (FUNC3 << 12)
        | (FUNC7 << 25)
        | (RD << 7)
        | ((rs1 as u32) << 15)
        | ((rs2 as u32) << 20)
}
