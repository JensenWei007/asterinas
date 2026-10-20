// SPDX-License-Identifier: MPL-2.0

//! VCPU file descriptor implementation

use ostd::mm::VmIo;
use crate::{
    fs::{
        file::{AccessMode, FileCommon, FileLike, MappableObject, StatusFlags, file_table::FdFlags},
        pseudofs::AnonInodeFs,
    },
    prelude::*,
    util::ioctl::RawIoctl,
    context::current_userspace,
    dispatch_ioctl,
    vm::vmar::FileMmapRequest,
    kvm::{Vm, Vcpu, KvmRun, KvmRegSize},
};

use super::ioctl::{KVM_RUN_MMAP_SIZE, Run, GetStatsFd, GetOneReg, SetOneReg};

/// VCPU file descriptor
pub(super) struct VcpuFile {
    #[expect(dead_code, reason = "Acts as a placeholder; will be used when other KVM implementations are added in the future.")]
    vm: Arc<Vm>,
    vcpu: Arc<Vcpu>,
    run: Arc<KvmRun>,
    common: FileCommon,
}

impl VcpuFile {
    /// Creates a new VCPU file
    pub(super) fn new(vm: Arc<Vm>, vcpu_id: u32) -> Result<Self> {
        let vcpu = vm.create_vcpu(vcpu_id)?;
        vcpu.init()?;
        let pseudo_path = AnonInodeFs::new_path(|_| "anon_inode:[kvm-vcpu]".to_string());
        Ok(Self {
            vm,
            vcpu,
            run: Arc::new(KvmRun::new(KVM_RUN_MMAP_SIZE)?),
            common: FileCommon::new(pseudo_path, AccessMode::O_RDWR, StatusFlags::empty()),
        })
    }
}

impl FileLike for VcpuFile {
    fn read(&self, _writer: &mut VmWriter) -> Result<usize> {
        return_errno_with_message!(Errno::EINVAL, "cannot read from VCPU file");
    }

    fn write(&self, _reader: &mut VmReader) -> Result<usize> {
        return_errno_with_message!(Errno::EINVAL, "cannot write to VCPU file");
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        dispatch_ioctl!(match raw_ioctl {
            Run => {
                self.vcpu.run(self.run.clone())?;
                Ok(0)
            }
            GetStatsFd => {
                return_errno_with_message!(Errno::ENOTTY, "KVM_GET_STATS_FD is not supported")
            }
            cmd @ GetOneReg => {
                let reg = cmd.read()?;
                let reg_val = self.vcpu.get_onereg(reg.id)?;
                // The size must be verified here;
                // otherwise, a buffer overflow will occur.
                match KvmRegSize::from_id(reg.id)? {
                    KvmRegSize::U32 => {
                        let reg_val = reg_val as u32;
                        current_userspace!().write_val(reg.addr, &reg_val)?;
                    }
                    KvmRegSize::U64 => {
                        let reg_val = reg_val as u64;
                        current_userspace!().write_val(reg.addr, &reg_val)?;
                    }
                }
                Ok(0)
            }
            cmd @ SetOneReg => {
                let reg = cmd.read()?;
                // The size must be verified here;
                // otherwise, a buffer overflow will occur.
                let reg_val = match KvmRegSize::from_id(reg.id)? {
                    KvmRegSize::U32 => {
                        let reg_val:u32 = current_userspace!().read_val(reg.addr)?;
                        reg_val as usize
                    }
                    KvmRegSize::U64 => {
                        let reg_val:u64 = current_userspace!().read_val(reg.addr)?;
                        reg_val as usize
                    }
                };
                self.vcpu.set_onereg(reg.id, reg_val)?;
                Ok(0)
            }
            _ => {
                // Handle architecture-specific ioctls.
                self.vcpu.ioctl(raw_ioctl)
            }
        })
    }

    fn mappable(&self, _request: FileMmapRequest) -> Result<MappableObject<'_>> {
        self.run.mappable()
    }

    fn common(&self) -> &FileCommon {
        &self.common
    }

    fn dump_proc_fdinfo(self: Arc<Self>, _fd_flags: FdFlags) -> Box<dyn core::fmt::Display> {
        Box::new("kvm_vcpu\n")
    }
}

impl crate::process::signal::Pollable for VcpuFile {
    fn poll(
        &self,
        _mask: crate::events::IoEvents,
        _poller: Option<&mut crate::process::signal::PollHandle>,
    ) -> crate::events::IoEvents {
        // VCPUs don't support polling
        crate::events::IoEvents::empty()
    }
}
