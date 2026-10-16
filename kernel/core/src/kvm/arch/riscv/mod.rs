// SPDX-License-Identifier: MPL-2.0
//! Specific implementation of KVM (architecture-specific components)

pub(super) mod vcpu;
pub(super) mod vm;
pub(super) mod ioctl;
pub(super) mod mmio;

#[derive(PartialEq)]
enum KvmExitReturn {
    ExitToUserSpace,
	ContinueRun,
}

impl From<usize> for KvmExitReturn {
    fn from(data: usize) -> Self {
        match data {
            0 => Self::ExitToUserSpace,
            _ => Self::ContinueRun,
        }
    }
}
