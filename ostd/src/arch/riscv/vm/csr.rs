// riscv crate does not support hypervisor csrs(hgatp...)
// so we impl this by ourself

use core::arch::asm;


// 定义宏来简化自定义CSR的声明
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
                    // 添加更多地址...
                    _ => unimplemented!("CSR address {:#x} not supported", $addr),
                }
            }
            
            #[inline]
            pub unsafe fn write(value: usize) {
                match $addr {
                    0x600 => unsafe { asm!("csrw 0x600, {0}", in(reg) value) },
                    0x680 => unsafe { asm!("csrw 0x680, {0}", in(reg) value) },
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

pub const CSR_HEDELEG: u16 = 0x602;
pub const CSR_HIDELEG: u16 = 0x603;
pub const CSR_HIE: u16 = 0x604;
pub const CSR_HTIMEDELTA: u16 = 0x605;
pub const CSR_HCOUNTEREN: u16 = 0x606;
pub const CSR_HGEIE: u16 = 0x607;
pub const CSR_HENVCFG: u16 = 0x60a;
pub const CSR_HTIMEDELTAH: u16 = 0x615;
pub const CSR_HENVCFGH: u16 = 0x61a;
pub const CSR_HTVAL: u16 = 0x643;
pub const CSR_HIP: u16 = 0x644;
pub const CSR_HVIP: u16 = 0x645;
pub const CSR_HTINST: u16 = 0x64a;
pub const CSR_HGEIP: u16 = 0xe12;
