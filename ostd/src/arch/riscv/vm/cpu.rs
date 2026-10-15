use super::csr::{Hedeleg, Hideleg, Hcounteren, Hvip, Vsie};

/// Enable the virtualization for cpu
pub fn enable_virtualization_cpu(){
    Hedeleg::write(0);
    Hideleg::write(0);

    // VS should access only the time counter directly. Everything else should trap
    Hcounteren::write(0x02);

    Hvip::write(0);
}

/// Disable the virtualization for cpu
pub fn disable_virtualization_cpu(){
    Vsie::write(0);
    Hvip::write(0);
    Hedeleg::write(0);
    Hideleg::write(0);
}
