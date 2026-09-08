// riscv crate does not support hypervisor csrs(hgatp...)
// so we impl this by ourself

use core::arch::asm;

macro_rules! define_csr {
    ($name:ident, $addr:expr) => {
        #[derive(Clone, Copy)]
        pub struct $name;
        
        impl $name {
            #[inline]
            pub unsafe fn read() -> usize {
                match $addr {
                    0x600 => {
                        let r: usize;
                        unsafe { asm!("csrr {0}, 0x600", out(reg) r) };
                        r
                    }
                    0x680 => {
                        let r: usize;
                        unsafe { asm!("csrr {0}, 0x680", out(reg) r) };
                        r
                    }
                    0x645 => {
                        let r: usize;
                        unsafe { asm!("csrr {0}, 0x645", out(reg) r) };
                        r
                    }
                    0x602 => {
                        let r: usize;
                        unsafe { asm!("csrr {0}, 0x602", out(reg) r) };
                        r
                    }
                    0x603 => {
                        let r: usize;
                        unsafe { asm!("csrr {0}, 0x603", out(reg) r) };
                        r
                    }
                    0x606 => {
                        let r: usize;
                        unsafe { asm!("csrr {0}, 0x606", out(reg) r) };
                        r
                    }
                    0x204 => {
                        let r: usize;
                        unsafe { asm!("csrr {0}, 0x204", out(reg) r) };
                        r
                    }
                    // 添加更多地址...
                    _ => unimplemented!("CSR address {:#x} not supported", $addr),
                }
            }
            
            #[inline]
            pub unsafe fn write(value: usize) {
                match $addr {
                    0x600 => unsafe { asm!("csrw 0x600, {0}", in(reg) value) },
                    0x680 => unsafe { asm!("csrw 0x680, {0}", in(reg) value) },
                    0x645 => unsafe { asm!("csrw 0x645, {0}", in(reg) value) },
                    0x602 => unsafe { asm!("csrw 0x602, {0}", in(reg) value) },
                    0x603 => unsafe { asm!("csrw 0x603, {0}", in(reg) value) },
                    0x606 => unsafe { asm!("csrw 0x606, {0}", in(reg) value) },
                    0x204 => unsafe { asm!("csrw 0x204, {0}", in(reg) value) },
                    // 添加更多地址...
                    _ => unimplemented!("CSR address {:#x} not supported", $addr),
                }
            }
            
            #[inline]
            pub unsafe fn set_bits(mask: usize) {
                unsafe {
                    let current = Self::read();
                    Self::write(current | mask);
                }

            }
            
            #[inline]
            pub unsafe fn clear_bits(mask: usize) {
                unsafe {
                    let current = Self::read();
                    Self::write(current & !mask);
                }
            }
        }
    };
}

define_csr!(Hstatus, 0x600);
define_csr!(Hgatp, 0x680);
define_csr!(Hvip, 0x645);
define_csr!(Hedeleg, 0x602);
define_csr!(Hideleg, 0x603);
define_csr!(Hcounteren, 0x606);


define_csr!(Vsie, 0x204);


pub const CSR_HIE: u16 = 0x604;
pub const CSR_HTIMEDELTA: u16 = 0x605;
pub const CSR_HGEIE: u16 = 0x607;
pub const CSR_HENVCFG: u16 = 0x60a;
pub const CSR_HTIMEDELTAH: u16 = 0x615;
pub const CSR_HENVCFGH: u16 = 0x61a;
pub const CSR_HTVAL: u16 = 0x643;
pub const CSR_HIP: u16 = 0x644;
pub const CSR_HVIP: u16 = 0x645;
pub const CSR_HTINST: u16 = 0x64a;
pub const CSR_HGEIP: u16 = 0xe12;
