use super::gstage::*;

pub fn local_hfence_gvma_all()
{
    unsafe {
        hfence_gvma_asm(0,0);
    }
}