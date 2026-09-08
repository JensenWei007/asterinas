/// 1

use crate::sync::SpinLock;
use alloc::vec::Vec;
use alloc::vec;
use crate::cpu::num_cpus;

pub struct VPlic {
    lock: SpinLock<usize>,
    cntxt_num: usize,
    hw: Vec<u32>,
    pend: Vec<u32>,
    act: Vec<u32>,
    prio: Vec<u32>,
    enbl: Vec<Vec<u32>>,
    threshold: Vec<u32>,
}

impl VPlic {
    /// 1
    pub fn new() -> Self {
        Self {
            lock: SpinLock::new(0),
            cntxt_num: 0,
            hw: vec![0; 32],
            pend: vec![0; 32],
            act: vec![0; 32],
            prio: vec![0; 32],
            enbl: vec![vec![0; 32]; num_cpus()*2],
            threshold: vec![0; num_cpus()*2],
        }
    }
}



pub fn vplic_init(){
    // Nothing
}
