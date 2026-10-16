---
date: 2026-10-15
mode: diff
base: 10a241870
head: a1a440a07
branch: main
title: "kvm for riscv"
---

# Summary

This commit adds a RISC-V KVM implementation: a `/dev/kvm` device front end
(`kernel/core/src/device/kvm/`), an architecture-neutral VM/vCPU layer
(`kernel/core/src/kvm/`), and the RISC-V H-extension machinery it rests on in OSTD
(G-stage page tables, the `switch.S` guest entry/exit path, the per-vCPU CSR context,
SBI emulation, and one-reg/MMIO/timer/CSR emulation). Five persona reviews over the
70 changed files found **65 comments — 3 `critical`, 24 `major`, 31 `minor`, 7 `nit`**;
every one of them was checked back against the code, and none was retracted.

## What the change does well

- **The framekernel split holds.** Everything that must touch `unsafe` — page-table
  walks, CSR access, the guest entry/exit assembly — lives in `ostd/`, and the kernel
  side stays safe Rust. A change of ~5 200 lines could easily have eroded that boundary;
  this one does not.
- **The Linux ABI is mirrored faithfully.** The KVM ioctl numbers, the `KVM_REG_*`
  register-id encoding, the `kvm_run` offsets and the ISA/SBI extension lists follow
  Linux field by field, which is what will let an unmodified VMM work later.
- **The guest entry path is compact and auditable** — a single `switch.S` routine with
  explicit save/restore macros — and the G-stage code states its own temporary status in
  its header, which is exactly the kind of honesty that makes a stopgap removable.

## Top issues, ranked

1. **`unmap_page` walks page tables using physical addresses as pointers**
   (`ostd/src/arch/riscv/vm/gstage.rs:202`, `critical`, reported independently by Security
   and Hardware). `map_page` converts with `paddr_to_vaddr`; the unmap walk does not, so
   every `KVM_SET_USER_MEMORY_REGION` that removes or replaces a slot writes a PTE
   through a bogus kernel pointer *and* leaves the guest's translation live while the
   backing frames are freed. Same function, same fix as the missing `PTE_V` check
   (line 199).
2. **Guest-reachable panics on the run path** (6 comments, `major`/`critical`):
   `todo!()` in `exit`/`gstage_page_fault`/`csr_insn`, the `KVM_GET_ONE_REG` dispatch on a
   userspace-writable `kvm_run` field, and the timer `unwrap()` on SSTC-less hosts. A
   guest, or a VMM, can panic the host kernel; the fix is one convention — unimplemented
   becomes an `errno` to the VMM or a fault injected into the guest.
3. **Unvalidated register numbers from userspace** (`onereg/csr.rs`, `critical`).
   `KVM_GET_ONE_REG`/`KVM_SET_ONE_REG` with a general-subtype CSR id ≥ 11 indexes a
   `[usize; 11]` out of bounds; `get_csr` already returns `Option`, so the check was
   intended and never written (Linux replies `EINVAL`).
4. **Silently swallowed allocation failure in the G-stage allocator**
   (`gstage.rs:94` and `:144`): `alloc_page` reports OOM as `(0, 0)`, which becomes a null
   pointer store and a `hgatp` root of physical 0 instead of a failed ioctl.
5. **Two decoding bugs that make whole instruction classes wrong**: compressed MMIO
   stores always send `x0` (`mmio.rs:201`, `major`), and `MODE` in the core one-reg group
   is always reported as 0 (`onereg/core.rs:60`, `major`) — the latter also clears every
   other `sstatus` field on a `MODE` write (`onereg/core.rs:76`).
6. **The commit itself is not reviewable as one unit** (`major`, Maintainability):
   `kvm for riscv` is non-imperative, has no body, and bundles the feature with a
   project-wide `TARGET_ARCH` flip, `.vscode` retargeting, and unrelated refactors; 50 of
   the 68 touched files also lack the SPDX header, and the new subsystem is not
   `cargo fmt`-clean.

## Structural recommendations

- **One identity per lookup table.** The single most common defect class here is
  hand-maintained parallel structures that have already drifted: `KvmRiscvIsaExtId` vs
  `KVM_ISA_EXT_ARR` (wrong from index 7 on), `SBI_EXT_TABLE`'s `ext_idx` vs the real
  extension ids (BASE parked on the `Max` sentinel, and the guest's `a7` compared against
  an internal index), and the flat register view in `context.rs` vs the slice layout it
  overlays. Deriving each from the enum, and asserting the layout at compile time, removes
  the whole class rather than three instances.
- **Decide a panic policy for the KVM path** and state it where the emulation is written:
  nothing the guest or the VMM selects may reach `todo!()`/`unwrap()`. This is a smaller
  change than it looks — six sites — and it removes the guest-triggerable host DoS.
- **Make the G-stage fallible end to end** (`alloc_page` → `GStagePageTable::new` →
  `GuestPhysMemSpace::new` → `VmMemory::new`) and flush it properly: the missing remote
  `hfence.gvma` after map/unmap is a correctness bug, not just a performance one, because
  frames are freed before other harts' translations are invalidated.
- **Split the commit**, add the SPDX headers, and run `cargo fmt` over the new files —
  then the review of the feature itself is not mixed with the review of the scaffolding.
- **Cover the new userspace surface in the compatibility docs.** `/dev/kvm` is a new
  user-facing ABI; `ioctl.scml` and its README still list only tty/pty/DRM/evdev/block/
  TDX/socket, and the `KVM_GET_STATS_FD` stub returns an fd number that was never
  allocated — a documented "accepted but not implemented" note would make both honest.

## Maintainability

### `Makefile` line 6

> ```diff
> -TARGET_ARCH ?= x86_64
> +TARGET_ARCH ?= riscv64
> ```

`atomic-commits` (major): Flipping the project-wide default `TARGET_ARCH` from `x86_64` to `riscv64` changes what every plain `make` (and any CI job that does not pass `TARGET_ARCH`) builds, and is unrelated to adding KVM; the same commit also retargets the shared editor config (`rust-analyzer.cargo.target` in `.vscode/settings.json`).

**Fix.** Keep `x86_64` as the default and build with `make TARGET_ARCH=riscv64`, or move the default change into a separate clearly-scoped commit; drop the personal rust-analyzer override from the checked-in settings.

**Shared with the Documentation section's comment on the same line** (`Docs still name x86_64 as the default target architecture`): whether the flip stays or goes, the docs and the commit split have to be decided together.

### `commit a1a440a07 message`

> ```diff
> Makefile | 2 +-
> .vscode/settings.json | 2 +-
> ostd/src/arch/riscv/mm/mod.rs | 38 +--
> kernel/core/src/vm/vmar/vmar_impls/map.rs | 6 +-
> ... new KVM subsystem (47 files)
> ```

`atomic-commits` (major): One commit bundles the KVM feature (about 5000 new lines) with preparatory refactoring that belongs in its own earlier commit (`cmo_block_size` extraction in `ostd/src/arch/riscv/mm/mod.rs`, the `debug_assert` rewrite in `kernel/core/src/vm/vmar/vmar_impls/map.rs`) and with unrelated build configuration, so it cannot be reviewed or bisected as one logical change.

**Fix.** Split it: (1) a `Refactor` commit for `cmo_block_size` and the `VmarMapOptions` assertion change, (2) the KVM feature proper, (3) any default-target change separately; keep each commit reviewable on its own.

### `commit a1a440a07 message`

> ```diff
> kvm for riscv
> 
> Signed-off-by: JensenWei007 <jensenwei007@gmail.com>
> ```

`imperative-subject` (major): The subject `kvm for riscv` is lowercase, has no verb, and is not in the imperative mood; the message also has no body explaining the change, while the log convention is verb-first subjects such as `Add`/`Implement`/`Fix`.

**Fix.** Retitle it e.g. `Implement KVM support for riscv64` and add a body paragraph covering the design (device/VM/VCPU split, the G-stage page-table stopgap) and what is still unimplemented.

### `kernel/core/src/device/kvm/device.rs` line 10

> ```diff
> use super::{KVM_MAJOR, KVM_MINOR, vm_file::VmFile, ioctl::{GetApiVersion, CreateVm,CheckExtension,GetVcpuMmapSize,KVM_API_VERSION,KVM_RUN_MMAP_SIZE}};
> ```

`consistency` (minor): The new subsystem is not formatted like the rest of the tree: this import packs items with no space after the commas (`CreateVm,CheckExtension,GetVcpuMmapSize`), and the same keeps appearing (`pub(super)const KVM_RUN_MMAP_SIZE` in `kernel/core/src/device/kvm/ioctl.rs`, `pub(in crate::kvm)  fn` in `kernel/core/src/kvm/arch/riscv/vm.rs`, tab/space mixes throughout `ostd/src/arch/riscv/vm/`).

**Fix.** Run `cargo fmt` over the new files so they read like the surrounding code.

### `kernel/core/src/device/kvm/ioctl.rs` line 1

> ```diff
> +use crate::{
> +    prelude::*,
> +    util::ioctl::{InData, NoData},
> ```

`consistency` (minor): This file starts straight at `use ...`, without the `// SPDX-License-Identifier: MPL-2.0` header that 762 of the 772 Rust files in `kernel/core/src` (and the sibling `kernel/core/src/device/kvm/mod.rs`) carry; roughly 46 more new files are missing it too, e.g. everything under `ostd/src/arch/riscv/vm/`.

**Fix.** Add the `// SPDX-License-Identifier: MPL-2.0` first line to every new file so license headers stay uniform.

### `kernel/core/src/device/kvm/vcpu_file.rs` line 25

> ```diff
> +pub(super) struct VcpuFile {
> +    #[allow(unused)]
> +    vm: Arc<Vm>,
> ```

`narrow-lint-suppression` (minor): `#[allow(unused)]` hides why `vm: Arc<Vm>` exists (the paired `Vcpu` only holds a `Weak<Vm>`, so this field keeps the VM alive); the suppression gives no reason and uses `allow` rather than the `#[expect(..., reason = ...)]` form the guideline points to, so a future reader cannot tell whether the field is still needed.

**Fix.** Replace it with `#[expect(dead_code, reason = "keeps the Vm alive while the VCPU file is open")]`, or restructure so the strong reference is obviously owned by `VcpuFile`.

### `kernel/core/src/kvm/arch/riscv/mmio.rs` line 84

> ```diff
> +    pub(super) fn load_decode(&mut self) {
> +        if self.insn & INSN_MASK_LW == INSN_MATCH_LW {
> +            self.len = 4;
> +            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
> +        } else if self.insn & INSN_MASK_LB == INSN_MATCH_LB {
> +            self.len = 1;
> +            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
> ```

`dry` (minor): `load_decode`, `store_decode` and `handle_return` repeat the same per-encoding body: eight `self.len = N; self.shift = 8 * (size_of::<usize>() as i32 - self.len);` arms, four identical `read_run_val`/`set_reg` arms, and an `INSN_MATCH_*`/`INSN_MASK_*` pair per instruction (16 pairs, of which 12 share the mask `0x707f`).

**Fix.** Drive the decode from one table of (match, mask, length, sign-extend) entries and share the load/store plumbing between the three functions, so adding an instruction means adding one row.

### `kernel/core/src/kvm/arch/riscv/vcpu.rs` line 97

> ```diff
> +        // If the vCPU has already run at least once,
> +        // then apply the configuration.
> +        let at_least_once = self.inner.lock().is_ran_at_least_once();
> +        if !at_least_once {
> +            self.inner.lock().config_ran_once();
> +        }
> ```

`explain-why` (minor): The comment says the configuration is applied "If the vCPU has already run at least once", but the code applies it exactly when it has not run yet (`if !at_least_once { self.inner.lock().config_ran_once(); }`), so the documented intent is the opposite of the behaviour.

**Fix.** Rewrite the comment to state the real rule, e.g. `// On the first run, apply the static VCPU configuration before entering the guest.`, and consider folding the extra `ran_at_least_once()` call into one helper so the run-once flag is maintained in a single place.

### `kernel/core/src/kvm/ioctl.rs` line 11

> ```diff
> +use super::arch::ioctl::check_arch_extension;
> ```

`qualified-fn-imports` (minor): Free functions are pulled in by bare name (`check_arch_extension` here, `check_extension` in `kernel/core/src/device/kvm/device.rs`) and whole modules are glob-imported (`use super::ioctl::*;`, `use super::arch::vm::*;` in `kernel/core/src/kvm/vm.rs`), so a call site does not reveal where the function comes from.

**Fix.** Import the parent module and qualify the call, e.g. `use super::arch;` then `arch::ioctl::check_arch_extension(raw_ioctl)`, and replace the glob imports with the explicit items that are actually used.

### `kernel/core/src/kvm/ioctl.rs` line 89

> ```diff
> +pub const KVM_EXIT_IO: u32 = 2;
> +pub const KVM_EXIT_DEBUG: u32 = 2;
> ```

`expect-dead-code` (minor): Much of this new constant block is never read and describes x86-only features a riscv64-only module will not implement (`KVM_RUN_CR8_OFFSET`, `KVM_RUN_APIC_BASE_OFFSET`, `KVM_RUN_IO_*`, `IA32_TSC_DEADLINE`, `KVM_IRQCHIP_*`, `KVM_EXIT_IO`/`KVM_EXIT_HLT`/`KVM_EXIT_SHUTDOWN`, most `KVM_CAP_*`); the copy-paste also produced `KVM_EXIT_DEBUG = 2`, the same value as `KVM_EXIT_IO`.

**Fix.** Keep only the constants with a concrete use (plus the RISC-V ones the feature will need) and drop the x86 ABI leftovers; where one must stay for a planned use, cite the KVM uapi header it mirrors.

### `kernel/core/src/kvm/mod.rs` line 1

> ```diff
> +pub mod vm;
> +
> +pub mod vcpu;
> ```

`module-docs` (minor): The entry point of the new KVM subsystem opens straight with `pub mod` declarations: there is no `//!` header saying what the subsystem does, which types (`Vm`, `Vcpu`, `VmMemory`, `KvmRun`) are the key ones, or how this module relates to `crate::device::kvm`.

**Fix.** Add a module doc such as `//! KVM (virtualization) support for riscv64.` followed by the key types and the split between this module (VM/VCPU state) and `device::kvm` (the `/dev/kvm` file interface).

### `kernel/core/src/kvm/mod.rs` line 4

> ```diff
> +pub mod vcpu;
> +pub mod vm_memory;
> +pub mod ioctl;
> +pub mod run_page;
> ```

`narrow-visibility` (minor): Every submodule is declared `pub`, so the whole hypervisor implementation (`vcpu`, `vm_memory`, `run_page`, `arch`, ...) becomes part of `aster-core`'s public API even though the only consumers are inside this crate (`device::kvm::*`).

**Fix.** Start from `pub(crate)` (or plain `mod` where `device::kvm` uses `crate::kvm::...` paths) and reserve `pub` for items another crate actually needs.

### `kernel/core/src/kvm/vcpu.rs` line 11

> ```diff
> +pub struct Vcpu {
> +    id: u32,
> +    pub vm: Weak<Vm>,
> ```

`getter-encapsulation` (minor): `pub vm: Weak<Vm>` exposes the internal weak reference even though the same impl already offers `Vcpu::vm()` (line 30), so callers can bypass the upgrade-with-error path and the field cannot change representation later.

**Fix.** Make the field private and keep `vm()` as the only accessor.

### `kernel/core/src/kvm/vm.rs` line 17

> ```diff
> +pub struct Vm {
> +    pub id: u32,
> +    memory: VmMemory,
> ```

`getter-encapsulation` (minor): `pub id: u32` exposes the VM identifier as part of `Vm`'s representation; the only external user is `dump_proc_fdinfo` in `kernel/core/src/device/kvm/vm_file.rs`, which could call a getter just as well.

**Fix.** Make `id` private and add `pub fn id(&self) -> u32`, next to the existing `memory()` accessor.

### `kernel/core/src/kvm/vm.rs` line 98

> ```diff
> +fn default_guest_mem_prop(is_readonly: bool) -> PageProperty {
> +    let guest_page_flags = if is_readonly {
> +        PageFlags::RX
> +    } else {
> +        PageFlags::RWX
> +    };
> ```

`no-bool-args` (nit): `default_guest_mem_prop(is_readonly: bool)` uses a boolean to select between two behaviours (`PageFlags::RX` vs `PageFlags::RWX`), and call sites pass either an opaque `false` or a compound expression.

**Fix.** Split it into `readonly_guest_mem_prop()` and `guest_mem_prop()`, or take a small enum such as `GuestMemAccess::{ReadOnly, ReadWrite}`.

### `ostd/src/arch/riscv/vm/context.rs` line 90

> ```diff
> +            match index {
> +                0..35 => Ok(self.slice.regs_1[index]),
> +                35..68 => Ok(self.slice.regs_2[index] as usize),
> +                68..100 => Ok(self.slice.regs_3[index] as usize),
> +                100 => Ok(self.slice.regs_4 as usize),
> +                101..164 => Ok(self.slice.regs_5[index] as usize),
> ```

`no-magic-number` (minor): The register layout is encoded as unnamed index ranges (`0..35`, `35..68`, `68..100`, `100`, `101..164`, `164`) duplicated in `get_reg` and `set_reg`, and the boundaries do not line up with the slices they index (`regs_2` is 33 elements, `regs_3` 32, `regs_5` 64), so a later writer cannot tell which sub-array an index belongs to.

**Fix.** Name the layout once (e.g. `const CORE_REGS_LEN: usize = 35;`, `const FP_F_BASE: usize = ...;`) and index each sub-array with its own `index - base` offset, shared by `get_reg` and `set_reg`.

**Shared with the out-of-bounds comments on the same function** (Security and Hardware, `ostd/src/arch/riscv/vm/context.rs` line 91): naming the layout and offsetting each sub-array index is one change that also removes those panics.

### `ostd/src/arch/riscv/vm/gstage.rs` line 53

> ```diff
> +const PTE_U: u64 = 1 << 4;  // User (G-stage中未使用)
> ```

`consistency` (nit): A Chinese fragment is mixed into an otherwise English codebase: `const PTE_U: u64 = 1 << 4;  // User (G-stage中未使用)`.

**Fix.** Write it in English, e.g. `// User; unused in G-stage`.

### `ostd/src/arch/riscv/vm/insn/mod.rs` line 15

> ```diff
> +pub enum KvmInsnReturn {
> +    /// Need exit to user-space
> +    KvmInsnExitToUserSpace = 0,
> +    /// Continue with next sepc
> +    KvmInsnContinueNextSepc,
> ```

`rust-native` (minor): The enum repeats its own name in every variant (`KvmInsnReturn::KvmInsnContinueNextSepc`, `KvmInsnReturn::KvmInsnExitToUserSpace = 0`) - C-style naming that Rust call sites read as noise; the same happens with `KvmRiscvSbiExtStatus::KvmRiscvSbiExtStatusEnabled`, `KvmRiscvSbiExtId::KvmRiscvSbiExtV01` and `KvmVcpuSbiReturnError::SbiErrNotSupported` in `ostd/src/arch/riscv/vm/sbi/mod.rs`.

**Fix.** Drop the prefix and let the enum name carry the context: `KvmInsnReturn::{ExitToUserSpace, ContinueNextSepc, ...}`, `KvmRiscvSbiExtStatus::{Enabled, Disabled, ...}`, `KvmRiscvSbiExtId::{V01, Time, ...}`, `KvmVcpuSbiReturnError::{NotSupported, ...}`.

### `ostd/src/arch/riscv/vm/isa.rs` line 93

> ```diff
> +static KVM_ISA_EXT_ARR: [IsaExtensions; KvmRiscvIsaExtId::MAX as usize] = [
> +    // single
> +    IsaExtensions::A,          // KVM_RISCV_ISA_EXT_A
> +    ...
> +    IsaExtensions::V,          // KVM_RISCV_ISA_EXT_V
> ```

`dry` (major): The id-to-extension mapping is written twice, in two different orders: the `KvmRiscvIsaExtId` enum puts `SVPBMT` at 7 and `V` at 15, while `KVM_ISA_EXT_ARR` puts `V` at index 7 (its own trailing comment claims `KVM_RISCV_ISA_EXT_V`) and `SVADU` at 15, so `KVM_ISA_EXT_ARR[kvm_ext]` cannot agree with the enum from index 7 onward.

**Fix.** Keep a single ordered representation - e.g. a `[(KvmRiscvIsaExtId, IsaExtensions)]` table or a `match` on the id - so the two orders cannot silently drift apart.

**Shared with the Correctness section's comment on the same line** (`Wrong table mapping`): the drift is already live — `KVM_ISA_EXT_ARR[kvm_ext]` returns the wrong extension from index 7 on, so this is a fix, not a tidy-up.

### `ostd/src/arch/riscv/vm/isa.rs` line 199

> ```diff
> +fn kvm_riscv_isa_disable_allowed(_ext: usize) -> bool {
> +    false
> +}
> ```

`expect-dead-code` (nit): `kvm_riscv_isa_disable_allowed` is defined here and never called, and its neighbour `kvm_riscv_isa_check_host` returns `Err(1)` whose value is never read (`if let Ok(ext)` in `kvm_riscv_vcpu_setup_isa`), so the error channel is both unused and unexplained.

**Fix.** Delete `kvm_riscv_isa_disable_allowed` until it has a caller (or add it with the disable path and an `#[expect(dead_code)]` plus reason), and either return `bool` from `kvm_riscv_isa_check_host` or document what `1` means.

### `ostd/src/arch/riscv/vm/onereg/mod.rs` line 96

> ```diff
> +        self.copy_config_reg_vec(vec.as_deref_mut());
> +        self.copy_core_reg_vec(vec.as_deref_mut());
> +        ...
> +        self.copy_sbi_state_reg_vec(vec.as_deref_mut())?;
> ```

`consistency` (minor): The per-type helpers that `copy_reg_vec` drives do not share one shape: `copy_config_reg_vec` returns `usize`, `copy_sbi_state_reg_vec` returns `Result<usize>`, `copy_core_reg_vec`/`copy_csr_reg_vec`/`copy_fp_*_reg_vec` return `()`, `copy_timer_reg_vec`/`copy_vector_reg_vec`/`copy_isa_ext_reg_vec` return a hardcoded `0`, and `num_*_regs` is sometimes `copy_*(None)` and sometimes a separate `size_of` computation.

**Fix.** Give every subtype the same contract, e.g. `fn copy_regs(&self, out: &mut Vec<usize>) -> Result<()>` plus `fn num_regs(&self) -> usize`, and let `copy_reg_vec`/`num_regs` drive them uniformly.

### `ostd/src/arch/riscv/vm/sbi/base.rs` line 15

> ```diff
> +pub(super) const VCPU_SBI_EXT_BASE: KvmVcpuSbiExtension = KvmVcpuSbiExtension {
> +    extid_start: SBI_EXT_BASE,
> +    extid_end: SBI_EXT_BASE,
> +    default_disabled: false,
> +    handler: &sbi_ext_base_handler,
> +    probe: None,
> +    init: None,
> +    ...
> +};
> ```

`dry` (minor): Each of the 15 SBI extensions repeats the full 13-field `KvmVcpuSbiExtension` literal even though almost every field is `None`/`0`/`false` (`base`, `hsm`, `ipi`, `rfence`, `srst`, `susp`, `time`, `v01`, `pmu`, `sta`, `fwft`, and the four in `forward.rs`), so adding a field means editing all of them.

**Fix.** Add a constructor or a `Default`-for-optionals helper (e.g. `KvmVcpuSbiExtension::new(SBI_EXT_BASE, &sbi_ext_base_handler)`) and use struct-update syntax for the few extensions that set more.

### `ostd/src/arch/riscv/vm/sbi/mod.rs` line 188

> ```diff
> +    KvmRiscvSbiExtensionEntry {
> +        ext_idx: KvmRiscvSbiExtId::KvmRiscvSbiExtMax,
> +        ext: &VCPU_SBI_EXT_BASE,
> +    },
> ```

`dry` (major): An extension is identified three times over - by the `KvmRiscvSbiExtId` discriminant, by the hand-written `ext_idx` field, and by `SBI_EXT_TABLE_SIZE = 15` - and the copies have already diverged: the entry for `VCPU_SBI_EXT_BASE` is registered under the `KvmRiscvSbiExtId::KvmRiscvSbiExtMax` sentinel, so `ext_status` never gets a BASE slot and the sentinel slot is reused.

**Fix.** Store extensions in enum order (or look them up through the enum rather than a parallel index) and derive the size from `SBI_EXT_TABLE.len()`, so an entry cannot drift out of its slot.

**Shared with the Correctness section's comment at line 247** (`Wrong lookup key`): both follow from identifying an extension twice — look it up through the identity it already carries (`extid_start..=extid_end`) and index `ext_status` off the same slot.

### `ostd/src/arch/riscv/vm/timer.rs` line 55

> ```diff
> +    pub(super) fn call_next_event(&self, next_cycle: usize) {
> +        let func = self.timer_next_event.unwrap();
> +        func(next_cycle);
> +    }
> ```

`closure-fn-suffix` (minor): `let func = self.timer_next_event.unwrap();` gives a callable a name that reads like data; the same naming appears as `func`/`handler` locals in `ostd/src/arch/riscv/vm/sbi/mod.rs` and `ostd/src/arch/riscv/vm/insn/mod.rs`.

**Fix.** Rename to `next_event_fn` (and `handler_fn`, `csr_insn_fn`, ...) so each site signals that a function pointer is being invoked.

### `ostd/src/arch/riscv/vm/vcpu.rs` line 354

> ```diff
> +        if irq == u32::MAX {
> +            self.irqs_pending |= 0x1<<10;
> +        } else {
> +            self.irqs_pending &= !(0x1<<10);
> +        }
> +        self.irqs_pending_mask |= 0x1 <<10;
> ```

`no-magic-number` (minor): `set_interrupt` encodes the HVIP VS-external-interrupt bit as the raw literal `0x1<<10` in three places, while `sync_interrupts` in the same file uses the named form `0x1 << IRQ_VS_SOFT`; the bit position is a hardware contract a reader cannot verify at the call site.

**Fix.** Add `const IRQ_VS_EXT: usize = 10;` beside `IRQ_VS_SOFT` in `ostd/src/arch/riscv/vm/types.rs` and use `1 << IRQ_VS_EXT` here.

### `test/initramfs/nix/initramfs.nix` line 17

> ```diff
> +  qemu = builtins.path { path = ./../src/qemu-system-riscv64; };
> +  kernel = builtins.path { path = ./../src/Image; };
> +  initramfs = builtins.path { path = ./../src/rv.cpio.gz; };
> ```

Build breaks on a clean checkout (major): `qemu`, `kernel` and `initramfs` are evaluated unconditionally from `./../src/qemu-system-riscv64`, `./../src/Image` and `./../src/rv.cpio.gz`, but those paths are `.gitignore`d build outputs, so the shared initramfs derivation fails on any clean checkout that has not built them locally.

**Fix.** Gate the KVM test payloads behind an explicit option with `lib.optionalString` (as `benchmark`/`conformance` are handled just below) or build them as proper Nix derivations, instead of requiring untracked files in `test/initramfs/src/`.

## Correctness

### `kernel/core/src/device/kvm/vcpu_file.rs` line 64

> ```diff
>             GetStatsFd => {
>                 // Currently, this syscall is invoked, but the fd is not used.
>                 Ok(10)
>             }
> ```

Fabricated file descriptor (minor): `KVM_GET_STATS_FD` returns the constant `10` as if it were a newly created descriptor, but no descriptor is allocated. A VMM that honours the return value (or that closes the fd it received, e.g. during its normal fd bookkeeping) ends up operating on whatever unrelated file occupies fd 10 in the caller's table.

**Fix.** Either reject the ioctl until stats are implemented (`return_errno_with_message!(Errno::ENOTTY, "KVM_GET_STATS_FD is not supported")`) or create a real anonymous file, insert it into the caller's file table the way `CreateVm`/`CreateVcpu` do, and return that fd.

**Shared with the Security section's comment on the same line** (`Returns an unallocated file descriptor`): one fix — install a real fd through the `CreateVm`/`CreateVcpu` path, or fail the ioctl with `ENOTTY`/`ENOSYS`; never hand back a constant slot number.

### `kernel/core/src/kvm/arch/riscv/mmio.rs` line 201

> ```diff
>     /// Get the rs2 reg_val of RVC insn
>     fn get_rs2s(&self, ctx: &VcpuContext) -> Result<usize> {
>         Ok(ctx.get_reg((((self.rvc_rs2s() << 7) << 3) & REG_MASK) / size_of::<usize>())?)
>     }
> ```

Wrong register index (major): In `get_rs2s` the register number is shifted left by `7` before the `<< 3`, so `(rvc_rs2s() << 7) << 3` is `rvc_rs2s() << 10`; masking that with `REG_MASK` (`0xF8`) is always `0`, hence every compressed store reads register `x0` and stores a hardwired `0` as the MMIO data. (`C.SD`/`C.SW` put the source register in `rs2' = 8 + bits[4:2]`, which `rvc_rs2s()` already returns; `C.SDSP`/`C.SWSP` are also routed through `get_rs2s` even though their `rs2` is the 5-bit field at bits 6:2.)

**Fix.** Keep the register number in the same domain as `get_rs2` (where the mask is applied to a value already scaled by `REGBYTES`): use `((self.rvc_rs2s() << 3) & REG_MASK) / size_of::<usize>()`, or simply `ctx.get_reg(self.rvc_rs2s())?`. For the SP-relative forms decode the 5-bit field (`(self.insn >> 2) & 0x1f`) instead of `rvc_rs2s()`.

**Shared with the Hardware section's comment on the same line** (`Wrong register number for compressed stores`): same defect, same one-line fix.

### `kernel/core/src/kvm/arch/riscv/vcpu.rs` line 251

> ```diff
>         match trap.get_trap_type() {
>             EXC_VIRTUAL_INST_FAULT => { ... }
>             EXC_INST_GUEST_PAGE_FAULT | EXC_LOAD_GUEST_PAGE_FAULT | EXC_STORE_GUEST_PAGE_FAULT => { ... }
>             EXC_SUPERVISOR_SYSCALL => { ... }
>             _ => todo!("VcpuArch exit: Trap({:#x}) is not impled!", trap.get_trap_type()),
>         }
> ```

Reachable panic (major): `exit` panics for every trap cause outside the five it handles, and `hedeleg` (`KVM_HEDELEG_DEFAULT = 0xB30D`) delegates only causes 0, 2, 3, 8, 9, 12, 13 and 15 to the guest. Causes the guest can raise but that are not delegated — e.g. load/store address misaligned (4/6) or load/store access fault (5/7) — are delivered to HS and hit this `todo!()`, so a guest performing an unaligned access kills the host kernel instead of having the trap reflected into itself.

**Fix.** Never `todo!()` on a path the guest drives: reflect the unhandled trap into the guest (set `vscause`/`vstval`/`vsepc` and resume at `vstvec`, as the guest-page-fault path does) and use `Err(...)` for genuinely unreachable states so the caller returns an `errno` to the VMM instead of panicking.

**Shared with the other host-panic comments** (`kernel/core/src/kvm/arch/riscv/vcpu.rs` lines 110 and 223, `ostd/src/arch/riscv/vm/insn/csr.rs` line 53, `ostd/src/arch/riscv/vm/timer.rs` line 55): none of these paths may panic on input the guest or the VMM selects — each turns into an `Err(...)`/`errno` for the VMM or into a fault injected into the guest.

### `kernel/core/src/kvm/vm.rs` line 48

> ```diff
>     pub fn create_vcpu(self: &Arc<Self>, vcpu_id: u32) -> Result<Arc<Vcpu>> {
>         let mut vcpus = self.vcpus.lock();
>         if vcpus.contains_key(&vcpu_id) {
>             return_errno_with_message!(Errno::EEXIST, "vCPU already exists");
>         }
>         let vcpu = Vcpu::new(vcpu_id, self)?;
>         vcpus.insert(vcpu_id, vcpu.clone());
>         drop(vcpus);
>         Ok(vcpu)
>     }
> ```

Resource never released (minor): `create_vcpu` stores a strong `Arc<Vcpu>` in `Vm::vcpus` and nothing ever removes it, so closing the VCPU file descriptor only drops the `VcpuFile` reference: the `Vcpu` (with its `VcpuArchInner`, timer and SBI context) lives until the whole `Vm` goes away, `Drop for Vcpu` (the `release VCPU` trace) never runs, and `KVM_CREATE_VCPU` for that id keeps failing with `EEXIST` even after userspace closed the previous VCPU.

**Fix.** Do not keep the vcpu alive from the map: store `Weak<Vcpu>` and treat an entry that no longer upgrades as free, or remove the entry in `Drop for Vcpu` (upgrading the stored `Weak<Vm>` and taking `Vm::vcpus` there), so the last fd close really releases the VCPU and frees its id.

### `ostd/src/arch/riscv/vm/gstage.rs` line 94

> ```diff
>     fn alloc_page(&mut self) -> (u64, u64) {
>         if let Ok(frame) = FrameAllocOptions::new().alloc_frame() {
>             let ret_va = paddr_to_vaddr(frame.paddr()) as u64;
>             let ret_pa = frame.paddr() as u64;
>             self.frames.push(frame);
>             return (ret_va, ret_pa);
>         }
>         (0,0)
>     }
> ```

Swallowed allocation failure (minor): Allocation failure is reported as success: `alloc_page` returns `(0, 0)` and `GStagePageTable::new` falls back to a table with `root`/`root_paddr` zeroed. The zeros are then used as an address and a pointer: `map_page` writes the zeroed `new_table` pointer (`write_bytes` on null) and installs a leaf PTE for physical address 0, and `get_root()` gives `hgatp` a G-stage root of physical 0, so the guest is entered with a bogus page table instead of `KVM_CREATE_VM`/`KVM_SET_USER_MEMORY_REGION` failing with an error.

**Fix.** Make these paths fallible and let the error propagate (both `GuestPhysMemSpace::new` and `VmMemory::new` already return `Result`): `fn alloc_page(&mut self) -> Result<(u64, u64)>` returning e.g. `Err(Error::NotEnoughResources)`, and `GStagePageTable::new() -> Result<Self>` returning the `alloc_segment` error instead of the zeroed table.

**Shared with the `Null pointer dereference` comment at line 144** (Security): both symptoms are the same `(0, 0)` sentinel, so one change — a fallible `alloc_page`/`GStagePageTable::new` propagated through `GuestPhysMemSpace::new` to `VmMemory::new` — fixes both.

### `ostd/src/arch/riscv/vm/insn/csr.rs` line 53

> ```diff
> fn csr_insn(_insn: usize) -> KvmInsnReturn {
>     todo!("CSR insn emul is not impled!")
> }
> ```

Reachable panic (major): `csr_insn` is the handler registered for all six `csr*` forms in `SYSTEM_OPCODE_FUNCS` and is a `todo!()`, so any CSR instruction that raises a virtual-instruction exception panics the host. This is guest-reachable: `enable_virtualization_cpu` sets `Hcounteren = 0x02` (only `time` is allowed), so a guest `rdcycle`/`rdinstret` (or any access to a non-delegated CSR such as `hstatus`) traps to HS with `EXC_VIRTUAL_INST_FAULT` and the instruction in `stval`, which `VcpuArch::virtual_insn` turns into `handle_system_opcode_insn` -> `csr_insn`.

**Fix.** Emulate the CSR access (at least the counters `Hcounteren`/`Scounteren` permit) or return `KvmInsnReturn::KvmInsnVirtualTrap`/`KvmInsnIllegalTrap` and make `VcpuArch::handle_insn_return` (which currently also `todo!()`s for those values) inject the trap into the guest, so a guest CSR access can never panic the host.

**Shared with the other host-panic comments** (`ostd/src/arch/riscv/vm/timer.rs` line 55, `kernel/core/src/kvm/arch/riscv/vcpu.rs` lines 110, 223 and 251): one rule for the whole emulation path — every unimplemented case gets a defined, non-panicking outcome.

### `ostd/src/arch/riscv/vm/isa.rs` line 93

> ```diff
> static KVM_ISA_EXT_ARR: [IsaExtensions; KvmRiscvIsaExtId::MAX as usize] = [
>     // single
>     IsaExtensions::A,          // KVM_RISCV_ISA_EXT_A
>     ...
>     IsaExtensions::M,          // KVM_RISCV_ISA_EXT_M
>     IsaExtensions::V,          // KVM_RISCV_ISA_EXT_V   <-- index 7, but V is id 15
> ```

Wrong table mapping (major): `KVM_ISA_EXT_ARR` is indexed by the KVM extension id (`KVM_ISA_EXT_ARR[kvm_ext]` in `kvm_riscv_isa_check_host`, and `kvm_riscv_vcpu_setup_isa` uses the same `i` both as the id and as the ISA bitmap bit), so its entries must be ordered like `KvmRiscvIsaExtId`. They are in Linux's source order instead (Linux used designated initializers), so every id is shifted: `IsaExtensions::V` sits at index 7 although `V` is id 15, `SVPBMT` (id 7) is at index 18, `SSTC` (id 8) at index 13, and so on. Each extension id >= 7 is therefore tested and reported for the wrong host extension, e.g. the guest's ISA bit 8 (SSTC) is set from `SMNPM`/`SSNPM` presence and bit 7 (SVPBMT) from `V`.

**Fix.** Place each entry at its `KvmRiscvIsaExtId` position (or build the table from the id, e.g. a `match` in a `const`/`OnceCell` so index == id), and drop the per-line `// KVM_RISCV_ISA_EXT_...` comments or make them match the actual position.

**Shared with the Maintainability section's `dry` comment on the same line**: the reordering and the "one ordered representation" refactor are the same change.

### `ostd/src/arch/riscv/vm/onereg/core.rs` line 60

> ```diff
>         match reg_num {
>             KvmCore::PC => Ok(self.guest_context.get_sepc()),
>             KvmCore::MODE => Ok((self.guest_context.get_sstatus() & SR_SPP == 1) as usize),
>             KvmCore::PC..=KvmCore::T6 => Ok(self.guest_context.get_reg(reg_num)?),
> ```

Wrong predicate (major): `get_sstatus() & SR_SPP == 1` masks with `SR_SPP` (`0x100`) and then compares the result against `1`, so the expression is always `false` and the core `MODE` register always reads `0` (user mode) even while the VCPU is in supervisor mode. Any VMM that saves/restores or inspects VCPU state through `KVM_GET_ONE_REG` gets a wrong privilege mode.

**Fix.** Test the bit rather than comparing the masked value with `1` (and keep the parentheses, so `!= 0` is not swallowed by the `&`):
```rust
KvmCore::MODE => Ok(((self.guest_context.get_sstatus() & SR_SPP) != 0) as usize),
```

### `ostd/src/arch/riscv/vm/onereg/core.rs` line 76

> ```diff
>                 } else {
>                     let sstatus = self.guest_context.get_sstatus();
>                     self.guest_context.set_sstatus(sstatus & SR_SPP);
>                 }
> ```

Wrong bit mask (major): `sstatus & SR_SPP` keeps only the `SR_SPP` bit and clears every other field of the guest `sstatus` (`SIE`, `SPIE`, `FS`, `SUM`, ...), so writing `MODE = 0` through `KVM_SET_ONE_REG` silently destroys the VCPU's saved status instead of only clearing `SR_SPP`.

**Fix.** Clear just the one bit:
```rust
let sstatus = self.guest_context.get_sstatus();
self.guest_context.set_sstatus(sstatus & !SR_SPP);
```

### `ostd/src/arch/riscv/vm/onereg/csr.rs` line 181

> ```diff
>     fn get_general_csr(&self, reg_num: usize) -> usize {
>         match reg_num {
>             KvmVcpuCsr::SIP => ((self.guest_csr.get_hvip() >> VSIP_TO_HVIP_SHIFT) & VSIP_VALID_MASK) & IRQ_LOCAL_MASK,
>             _ => self.guest_csr.get_csr(reg_num).unwrap()
>         }
>     }
> ```

Reachable panic (critical): `get_general_csr` forwards the user-supplied CSR number straight into `KvmVcpuCsr::get_csr`, which indexes the fixed 11-element slice (`self.slice.slice[index]`) with no bounds check and always returns `Some(..)`, so this `unwrap` protects nothing. A `KVM_GET_ONE_REG` (or `KVM_SET_ONE_REG`) whose `regs.id` carries the `KVM_REG_RISCV_CSR` type field with the general subtype and a number `>= 11` panics the kernel: the masks clear only the arch/size fields and bits 24-25, so a `reg_num` such as `0x400` (subtype 0, `reg_num = 1024`) reaches `get_csr`/`set_csr` and indexes out of bounds.

**Fix.** Bounds-check the CSR number and propagate a real error instead of unwrapping, e.g.:
```rust
fn get_general_csr(&self, reg_num: usize) -> Result<usize> {
    match reg_num {
        KvmVcpuCsr::SIP => Ok(sip_value),
        _ => self.guest_csr.get_csr(reg_num).ok_or(Error::NotFound),
    }
}
```
and make `KvmVcpuCsr::get_csr`/`set_csr` reject out-of-range indices (`self.slice.slice.get(index)` plus a range check in `set_csr`) so that the `set_general_csr` path fails the ioctl rather than panicking.

**Shared with the `validate-at-boundaries` comment on the same functions** (Security, `ostd/src/arch/riscv/vm/onereg/csr.rs` line 57): one validation covers both — check `reg_num` once at the ioctl boundary (`Vcpu::get_onereg`/`set_onereg`) and reject out-of-range numbers with `Err(Error::NotFound)` (Linux replies `EINVAL`), with `KvmVcpuCsr::get_csr`/`set_csr` using `slice.get(index)`/an error instead of direct indexing.

### `ostd/src/arch/riscv/vm/sbi/mod.rs` line 247

> ```diff
> fn vcpu_get_sbi_ext_idx(idx: usize) -> Result<usize> {
> 	for i in 0..SBI_EXT_TABLE_SIZE {
>         if SBI_EXT_TABLE[i].ext_idx as usize == idx {
>             return Ok(i);
>         }
>     }
> 	Err(Error::NotFound)
> }
> ```

Wrong lookup key (major): `vcpu_get_sbi_ext_idx` matches the value the guest puts in `a7` (`handle_sbi_call`) against `ext_idx`, which is the internal KVM index (`0..=14`) also used to index `ext_status`; the real extension ids are stored in `extid_start`/`extid_end`, and those fields are never read anywhere. Every modern SBI extension id therefore fails to match (`SBI_EXT_TIME` = `0x54494D45`, `SBI_EXT_BASE` = `0x10`, `SBI_EXT_STA`, `SBI_EXT_FWFT`, ...), so all guest SBI calls fall into the error branch and are answered with `SBI_ERR_NOT_SUPPORTED`, while the legacy ids `0..=8` match by accident and are routed to unrelated handlers (`a7 == 1` reaches the TIME handler, `a7 == 8` the vendor handler).

**Fix.** Select the entry by the id range the entries already carry:
```rust
fn vcpu_get_sbi_ext_idx(extid: usize) -> Result<usize> {
    for i in 0..SBI_EXT_TABLE_SIZE {
        let ext = SBI_EXT_TABLE[i].ext;
        if extid >= ext.extid_start && extid <= ext.extid_end {
            return Ok(i);
        }
    }
    Err(Error::NotFound)
}
```

**Shared with the Maintainability section's comment at line 188** (`dry`): keying the entry by its real id range and indexing `ext_status` off that same slot removes both the mis-routing and the hand-maintained `ext_idx` — including the BASE entry parked on the `KvmRiscvSbiExtMax` sentinel.

### `ostd/src/arch/riscv/vm/timer.rs` line 55

> ```diff
> 	pub(super) fn call_next_event(&self, next_cycle: usize) {
> 		let func = self.timer_next_event.unwrap();
>     	func(next_cycle);
> 	}
> ```

Reachable panic (major): `timer_next_event` is only assigned when the host advertises `Sstc` (see `init`), so on a CPU without that extension the field stays `None` and this `unwrap` panics the kernel. The call is reachable from the guest: an ecall handled by `handle_sbi_call` reaches `sbi_ext_time_handler`, which calls `call_next_event` with the guest's requested deadline, so a guest programming its timer on such a host crashes the host instead of getting a defined failure.

**Fix.** Do not unwrap: give `call_next_event` a fallible or flag-returning signature (or check `sstc_enabled`/`init_done`) and have `sbi_ext_time_handler` return `SBI_ERR_NOT_SUPPORTED` when no timer backend exists, or provide the non-Sstc fallback path (programming a host timer) before calling.

**Shared with the other host-panic comments** (`ostd/src/arch/riscv/vm/insn/csr.rs` line 53, `kernel/core/src/kvm/arch/riscv/vcpu.rs` lines 110, 223 and 251): the same rule — an unimplemented feature must fail the call, not the host.

## Security

### `kernel/core/src/device/kvm/vcpu_file.rs` line 64

> ```diff
>             GetStatsFd => {
>                 // Currently, this syscall is invoked, but the fd is not used.
>                 Ok(10)
>             }
> ```

Returns an unallocated file descriptor (minor): `KVM_GET_STATS_FD` returns the constant `10` without installing anything in the caller's file table, so userspace receives a descriptor number that in fact refers to whatever file occupies slot 10 (or to no file at all). A VMM that trusts the return value — as it does on Linux, where this ioctl installs a fresh anon-inode fd — will `read`/`ioctl`/`close` an unrelated open file, and closing it can make a concurrent `open` in the process silently lose its descriptor. The comment that the fd is not used does not remove the hazard.

**Fix.** Either install a real file (mirror the `CreateVcpu`/`CreateVm` path and insert an anon-inode file into the current file table, returning that fd) or fail the ioctl with `Errno::ENOTTY`/`ENOSYS` until the stats interface exists.

**Shared with the Correctness section's comment on the same line** (`Fabricated file descriptor`): one fix — a real anonymous file installed through the `CreateVm`/`CreateVcpu` path, or an outright ioctl failure.

### `kernel/core/src/device/kvm/vcpu_file.rs` line 69

> ```diff
>             cmd @ GetOneReg => {
>                 let regs = cmd.read()?;
>                 let reg_val = self.vcpu.get_onereg(regs.id)?;
>                 current_userspace!().write_val(regs.addr, &reg_val)?;
>                 Ok(0)
>             }
>             cmd @ SetOneReg => {
>                 let regs = cmd.read()?;
>                 let reg_val = current_userspace!().read_val(regs.addr)?;
>                 self.vcpu.set_onereg(regs.id, reg_val)?;
> ```

Writes past the user buffer (minor): Both one-register ioctls always move a `usize` (8 bytes) to or from the caller's address — `current_userspace!().write_val(regs.addr, &reg_val)` here and `current_userspace!().read_val(regs.addr)` at line 74 — ignoring the size the VMM encoded in `regs.id`. For a `KVM_REG_SIZE_U32` register, which this kernel itself advertises (any `KVM_REG_RISCV_FP_F` register, see `copy_fp_f_reg_vec` building its ids with `KVM_REG_SIZE_U32`), Linux's ABI lets the caller pass a 4-byte buffer, and the kernel then stores 4 bytes past the end of it (and symmetrically reads 4 bytes past it on `KVM_SET_ONE_REG`, taking adjacent VMM memory as the register value).

**Fix.** Honour the size encoded in the register id: for `KVM_REG_SIZE_U32` truncate to `u32` and `write_val`/`read_val` only 4 bytes, for `KVM_REG_SIZE_U64` keep 8, and reject ids whose size field is neither.

### `kernel/core/src/kvm/arch/riscv/vcpu.rs` line 110

> ```diff
>         // Process the data returned after being handled by the user.
>         match run.read_run_val::<u32>(KVM_RUN_EXIT_REASON_OFFSET)? {
>             KVM_EXIT_MMIO => self.mmio_return(run.clone())?,
>             KVM_EXIT_RISCV_SBI => todo!("EXIT_SBI_RETURN is not impled!"),
>             KVM_EXIT_RISCV_CSR => todo!("EXIT_CSR_RETURN is not impled!"),
>             _ => 0
>         };
> ```

`validate-at-boundaries` (major): `run()` dispatches on `exit_reason` read out of the vCPU's `kvm_run` page (`run.read_run_val::<u32>(KVM_RUN_EXIT_REASON_OFFSET)`, offset 8, line 108), but that page is the VMM's own writable mmap of the vCPU fd (`VcpuFile::mappable` returns the `KvmRun` VMO), so the value is supplied by userspace and is not validated. A VMM that writes `KVM_EXIT_RISCV_SBI` (35) or `KVM_EXIT_RISCV_CSR` (36) at offset 8 and calls `KVM_RUN` drives the kernel into `todo!()` and panics it. The kernel itself never produces those two exit reasons — the only values ever written to that field are `KVM_EXIT_MMIO` (lines 187, 212) and `KVM_EXIT_UNKNOWN` (lines 120, 232) — so nothing internal depends on the field holding 35/36.

**Fix.** Treat the field as untrusted input: accept only the values the kernel can have produced (at present `KVM_EXIT_MMIO`, which is the MMIO-return path, and `KVM_EXIT_UNKNOWN`), and return an error such as `Errno::EINVAL` for anything else until the SBI/CSR return paths are implemented, instead of `todo!()`-ing on it.

**Shared with the other host-panic comments** (`ostd/src/arch/riscv/vm/insn/csr.rs` line 53, `ostd/src/arch/riscv/vm/timer.rs` line 55, `kernel/core/src/kvm/arch/riscv/vcpu.rs` lines 223 and 251): treat every guest- or VMM-controlled selector as untrusted and give it a defined, non-panicking outcome.

### `kernel/core/src/kvm/arch/riscv/vcpu.rs` line 223

> ```diff
>     fn gstage_page_fault(&self, run: Arc<KvmRun>, trap: KvmCpuTrap) -> Result<KvmExitReturn> {
>         let fault_addr = trap.get_fault_addr();
> 
>         match trap.get_trap_type() {
>             EXC_LOAD_GUEST_PAGE_FAULT => self.mmio_load(run, fault_addr, trap.get_trap_insn()),
>             EXC_STORE_GUEST_PAGE_FAULT => self.mmio_store(run, fault_addr, trap.get_trap_insn()),
>             _ => todo!("gstage_page_fault: Other scause({:#x}) in addr({:#x}) is not impled!", trap.get_trap_type(), fault_addr),
>         }
>     }
> ```

Reachable panic (major): `gstage_page_fault` implements only load and store guest-page faults and `todo!()`s on everything else, but its caller routes three causes here: `EXC_INST_GUEST_PAGE_FAULT | EXC_LOAD_GUEST_PAGE_FAULT | EXC_STORE_GUEST_PAGE_FAULT => ... ret = self.gstage_page_fault(run, trap)`. The guest picks its own PC, so an instruction fetch from a guest-physical address that has no valid G-stage entry (e.g. a jump outside the mapped RAM regions — the VMM only maps the memory slots, and every mapped page has `PTE_X` forced on) arrives as `EXC_INST_GUEST_PAGE_FAULT` (20), takes the `_ => todo!(...)` arm and panics the host kernel: a guest, or simply a guest bug, becomes a host DoS. The neighbouring `exit` catch-all (`_ => todo!("VcpuArch exit: Trap({:#x}) is not impled!", ...)`, line 251) has the same shape for any trap that is not delegated to VS by `KVM_HEDELEG_DEFAULT`.

**Fix.** Handle `EXC_INST_GUEST_PAGE_FAULT` explicitly (emulate the fetch as an MMIO/instruction-fetch exit to the VMM, or inject the appropriate guest fault) and replace the catch-all `todo!()`s in `gstage_page_fault`/`exit` with real handling or an error return, so that an unexpected trap cannot panic the host.

**Shared with the other host-panic comments** (`kernel/core/src/kvm/arch/riscv/vcpu.rs` lines 110 and 251, `ostd/src/arch/riscv/vm/insn/csr.rs` line 53, `ostd/src/arch/riscv/vm/timer.rs` line 55): one convention for the whole run path — unimplemented becomes `Err(...)` to the VMM or a fault injected into the guest.

### `ostd/src/arch/riscv/vm/context.rs` line 91

> ```diff
>     pub fn get_reg(&self, index: usize) -> Result<usize> {
>         unsafe {
>             match index {
>                 0..35 => Ok(self.slice.regs_1[index]),
>                 35..68 => Ok(self.slice.regs_2[index] as usize),
>                 68..100 => Ok(self.slice.regs_3[index] as usize),
>                 100 => Ok(self.slice.regs_4 as usize),
>                 101..164 => Ok(self.slice.regs_5[index] as usize),
>                 164 => Ok(self.slice.regs_6 as usize),
>                 _ => Err(Error::NotFound)
>             }
>         }
>     }
> ```

Out-of-bounds array indexing (minor): The flat register view indexes the slice arrays with the absolute index, but the arrays are only as long as their own field: `regs_1` is `[usize; 35]` so `0..35 => regs_1[index]` is fine, yet `35..68 => regs_2[index]` indexes a `[u32; 33]`, `68..100 => regs_3[index]` indexes a `[u64; 32]`, and `101..164 => regs_5[index]` indexes a `[u64; 64]` (same in `set_reg`, lines 107, 108 and 110). Every index in those arms is past the end of the array, so all of them panic if the offset is not subtracted (they can never yield a register value); today's callers (`onereg/core.rs` and `mmio.rs`) only pass 0..=31, so the bug is latent, but it is a landmine for any new caller of these `pub` APIs. Relatedly, nothing pins this view to the layout it overlays (`VcpuContextReg` with `FFpuContext`/`DFpuContext`/`QFpuContext`, which live in another module and may be changed independently), so the union's type punning can silently drift.

**Fix.** Index with the offset inside each slice (`regs_2[index - 35]`, `regs_3[index - 68]`, `regs_5[index - 101]`, and correspondingly in `set_reg`), or model the whole saving area as a single array; and add a compile-time check that the two views agree, e.g. `const _: () = assert!(size_of::<VcpuContextReg>() == size_of::<VcpuContextSlice>() && align_of::<VcpuContextReg>() == align_of::<VcpuContextSlice>());`.

**Shared with the Hardware section's comment on the same lines** and the Maintainability section's `no-magic-number` comment at line 90: index with `index - base` per sub-array, name the layout once, and add the layout assertion — one edit, three comments.

### `ostd/src/arch/riscv/vm/gstage.rs` line 74

> ```diff
>             unsafe {
>                 core::ptr::write_bytes(root as *mut u64, 0, 4 * PAGE_SIZE / 8);
>             }
> ...
>                 unsafe {
>                     core::ptr::write_bytes(new_table, 0, PAGE_SIZE / 8);
>                 }
> ...
>         let pte_ptr = unsafe { table.add(idx) };
>         let pte = unsafe { core::ptr::read_volatile(pte_ptr) };
> ...
>         unsafe {
>             core::ptr::write_volatile(pte_ptr, 0);
>             asm!("hfence.gvma");
>         }
> ```

`justify-unsafe-use` (minor): Not one of the ten `unsafe` blocks in this file carries a `// SAFETY:` comment (lines 74-76, 136-137, 143-148, 148-150, 160-161, 164-166, 196-197, 206-211), although they are all raw-pointer accesses derived from page-table contents — exactly the places where the soundness argument (the pointer is a `paddr_to_vaddr`-mapped table page, `idx < 512`, the PTE is valid, the frame allocator succeeded) must be written down. The file header's note that the code does not comply with coding standards does not waive the requirement, and the absence of these arguments is part of why the wrong-address bug at line 202 is not obvious to a reader.

**Fix.** Add a `// SAFETY:` comment to every `unsafe` block stating why the pointer is valid and in bounds (use the numbered-list form when several conditions are involved).

### `ostd/src/arch/riscv/vm/gstage.rs` line 144

> ```diff
>             if pte & PTE_V == 0 {
>                 let (new_table_va, new_table_pa) = self.alloc_page();
>                 let new_table = new_table_va as *mut u64;
>                 
>                 unsafe {
>                     core::ptr::write_bytes(new_table, 0, PAGE_SIZE / 8);
>                 }
> ...
>     fn alloc_page(&mut self) -> (u64, u64) {
>         if let Ok(frame) = FrameAllocOptions::new().alloc_frame() { ... }
>         (0,0)
>     }
> ```

Null pointer dereference (minor): `alloc_page` signals failure by returning `(0, 0)` and `map_page` consumes it without checking: `let new_table = new_table_va as *mut u64; core::ptr::write_bytes(new_table, 0, PAGE_SIZE / 8);` — a store to virtual address 0 (a kernel panic) whenever the frame allocator runs dry. This is not purely theoretical: a VMM can ask for an arbitrarily large guest memory region (`KVM_SET_USER_MEMORY_REGION` has no size cap), and `map_guest_memory` walks page by page, allocating an intermediate G-stage table per 512 pages and pinning a user frame per page, so the VMM can drive the allocator to failure. `GStagePageTable::new` has the same silent-failure shape (line 83 builds `root_paddr: 0, root: 0`, and `get_root()` then feeds 0 into `update_hgatp`), turning an OOM into a bogus guest page table instead of an error.

**Fix.** Propagate the failure: return `Result`/`Option` from `alloc_page` and make `map_page`/`new` fail with e.g. `Err(Error::NotEnoughResources)` rather than writing through a null or zero pointer (and have `GuestPhysMemSpace::new` surface the error to `VmMemory::new`).

**Shared with the `Swallowed allocation failure` comment at line 94** (Correctness): the fix is the single fallible `alloc_page`/`GStagePageTable::new` path described there.

### `ostd/src/arch/riscv/vm/gstage.rs` line 202

> ```diff
>         for level in (1..GSTAGE_LEVELS).rev() {
>             let idx = vpn[level] as usize;
>             let pte_ptr = unsafe { table.add(idx) };
>             let pte = unsafe { core::ptr::read_volatile(pte_ptr) };
>                         
>             let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT;
>             tables_to_free[free_count] = table;
>             free_count += 1;
>             table = next_table as *mut u64;
>         }
> ```

Physical address used as a pointer (critical): `GStagePageTable::unmap_page` walks the page tables without converting the next-level table address back to a virtual address: `let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT; ... table = next_table as *mut u64;`. What the PTE holds is a *physical* address, and `map_page` converts it before use (`table = paddr_to_vaddr(next_table as usize) as *mut u64;`, line 155); this path does not. Since `paddr_to_vaddr` adds `LINEAR_MAPPING_BASE_VADDR` (`ostd/src/mm/kspace/mod.rs`), every access below the root level — `table.add(idx)`, `read_volatile`, and the final `write_volatile(pte_ptr, 0)` that is supposed to clear the leaf PTE — targets an unrelated kernel/user virtual address instead of the page-table page. Two consequences: (a) an 8-byte zero store to a wrong address, i.e. kernel memory corruption, or a kernel page fault (panic) if that address is not mapped; (b) the guest-physical PTE is never actually cleared, so `KVM_SET_USER_MEMORY_REGION` deleting or replacing a slot does not revoke the guest's mapping — the old slot's `UFrame`s are dropped immediately afterwards, so the backing frame can go back to the host allocator while the guest still has a live G-stage translation to it and can read/write recycled host memory. `unmap_page` also never checks `PTE_V`, so unmapping a GPA that has no valid entry descends into the address taken from a zero PTE.

**Fix.** Convert the address exactly as the map path does and refuse to descend on an invalid PTE:
```rust
let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT;
if pte & PTE_V == 0 {
    return Ok(());
}
table = paddr_to_vaddr(next_table as usize) as *mut u64;
```
(`tables_to_free` is dead here as well; either use it to free emptied tables or drop it.)

**Shared with the Hardware section's comments on the same function** (lines 199 and 202): the validity check and the `paddr_to_vaddr` conversion both belong in the same rewrite of the walk.

### `ostd/src/arch/riscv/vm/gstage.rs` line 210

> ```diff
>         let idx = vpn[0] as usize;
>         let pte_ptr = unsafe { table.add(idx) };
>                 
>         unsafe {
>             core::ptr::write_volatile(pte_ptr, 0);
>             asm!("hfence.gvma");
>         }
>         Ok(())
>     }
> ```

Missing remote invalidation of guest-stage translations (major): After clearing a leaf PTE the code issues a bare `hfence.gvma`, which only invalidates G-stage translations on the *current hart* (and only for the VMID in `hgatp`; `update_hgatp` in `ostd/src/arch/riscv/vm/vcpu.rs` always installs VMID 0). The G-stage tables are shared by all vCPUs of the VM and other harts can be running `KVM_RUN` for that VM concurrently, so nothing invalidates their cached translations, and no remote fence is implemented (the RFENCE/HVIP IPI SBI handlers are stubs). Concrete interleaving: vCPU A runs guest code on hart 1 and touches GPA X (G-stage TLB entry GPA X -> HPA Y); hart 0 executes `KVM_SET_USER_MEMORY_REGION` replacing or deleting that slot, so `unmap_guest_memory` clears the PTE and flushes hart 0 only, then drops the slot's `UFrame`s so the host allocator may hand HPA Y to someone else; hart 1's stale entry still translates GPA X to the reused frame, letting the guest read and write host memory. `map` (which also changes GPA->HPA bindings) does not flush even the local hart.

**Fix.** Propagate every guest-memory map/unmap change with a remote G-stage fence on the harts running this VM (e.g. an IPI/`hfence.gvma` broadcast via the SBI RFENCE path) before the old frames can be freed, and flush in `map` as well as `unmap`.

### `ostd/src/arch/riscv/vm/onereg/csr.rs` line 57

> ```diff
>     // SAFETY: Access to union members is safe.
>     fn get_csr(&self, index: usize) -> Option<usize> {
>         unsafe { Some(self.slice.slice[index]) }
>     }
> 
>     // SAFETY: Access to union members is safe.
>     fn set_csr(&mut self, index: usize, value: usize) {
>         unsafe { self.slice.slice[index] = value };
>     }
> ```

`validate-at-boundaries` (major): The register id supplied by userspace through `KVM_GET_ONE_REG`/`KVM_SET_ONE_REG` (`VcpuFile::ioctl` -> `Vcpu::get_onereg`/`set_onereg` -> `VcpuArchInner::get_one_reg`/`set_one_reg`) reaches the CSR array with no bound check. `VcpuArchInner::get_csr`/`set_csr` strip the arch/size/type fields and pass `reg_num` to `get_general_csr`/`set_general_csr`, which end in `self.guest_csr.get_csr(reg_num).unwrap()` (line 181) and `self.guest_csr.set_csr(reg_num, reg_val)` (line 198); those do `self.slice.slice[index]` on a `[usize; 11]` array (lines 57 and 62). The masking keeps bits 0..15 and 32..51 of the user value, so `KVM_REG_RISCV | KVM_REG_SIZE_U64 | KVM_REG_RISCV_CSR | 11` (index 11 is already the array length), or any larger value such as `| 0x11` or a bit in 32..51, indexes out of bounds and panics the kernel — any process holding a vCPU fd can DoS the host. Note that `KvmVcpuCsr::get_csr` returns `Option`, so a bounds check was evidently intended but never implemented (it always returns `Some`). The same unvalidated input also hits `todo!()` for the `KVM_REG_RISCV_CSR_AIA`/`_SMSTATEEN` subtypes (lines 172-173 and 191-192), which a VMM written against the Linux ABI may legitimately send.

**Fix.** Validate the index at the boundary and report an error instead of panicking, e.g.
```rust
fn get_csr(&self, index: usize) -> Option<usize> {
    self.slice.slice.get(index).copied()
}
```
make `set_csr` skip/return an error for `index >= 11`, and have `get_general_csr`/`set_general_csr` map both that and the unimplemented subtypes to `Err(Error::NotFound)` (Linux replies `-EINVAL` for an unknown register id).

**Shared with the reachable-panic comment on the same functions** (Correctness, `ostd/src/arch/riscv/vm/onereg/csr.rs` line 181): the same bounds check serves both — `slice.get(index)` in `get_csr`, a range check in `set_csr`, and `Err(Error::NotFound)` propagated by the two `*_general_csr` callers.

### `ostd/src/arch/riscv/vm/timer.rs` line 55

> ```diff
> 	pub(super) fn call_next_event(&self, next_cycle: usize) {
> 		let func = self.timer_next_event.unwrap();
>     	func(next_cycle);
> 	}
> 
> // timer_next_event is only Some() when has_extensions(IsaExtensions::SSTC):
> 	pub(super) fn init(&mut self) {
> 		self.init_done = true;
> 		self.next_set = false;
> 		if has_extensions(IsaExtensions::SSTC) {
> 			self.sstc_enabled = true;
> 			self.timer_next_event = Some(&kvm_riscv_vcpu_update_vstimecmp);
> 		}
> 	}
> ```

Reachable panic (major): `call_next_event` does `let func = self.timer_next_event.unwrap();`, but `timer_next_event` is only set by `VcpuTimer::init` when the host CPU has `IsaExtensions::SSTC`; on any host without SSTC it stays `None` (the hrtimer fallback the comment mentions does not exist yet). The path is reachable with guest-chosen input: a guest `ecall` whose `a7` is dispatched by `SBI_EXT_TABLE` to `VCPU_SBI_EXT_TIME` lands in `sbi_ext_time_handler` (`ostd/src/arch/riscv/vm/sbi/time.rs`), which calls `arch.timer.call_next_event(arch.guest_context.get_a0())` — the guest controls both `a7` and `a0` — so a guest can panic the host kernel on any SSTC-less machine.

**Fix.** Do not unwrap: make `call_next_event` handle the missing backend (e.g. `let Some(func) = self.timer_next_event else { return; };` plus an explicit unsupported path), and/or refuse to route the SBI TIME extension while `sstc_enabled` is false (return `SbiErrNotSupported` to the guest).

**Shared with the other host-panic comments** (`ostd/src/arch/riscv/vm/insn/csr.rs` line 53, `kernel/core/src/kvm/arch/riscv/vcpu.rs` lines 110, 223 and 251): the same rule applies to every unimplemented path the guest can reach.

## Hardware

### `kernel/core/src/kvm/arch/riscv/mmio.rs` line 201

> ```diff
>     /// Get the rs2 reg_val of RVC insn
>     fn get_rs2s(&self, ctx: &VcpuContext) -> Result<usize> {
>         Ok(ctx.get_reg((((self.rvc_rs2s() << 7) << 3) & REG_MASK) / size_of::<usize>())?)
>     }
> ```

Wrong register number for compressed stores (major): `get_rs2s` computes `((self.rvc_rs2s() << 7) << 3) & REG_MASK`, i.e. `(reg << 10) & 0xF8`. Bits 7:3 of any value shifted left by 10 are always zero, so the expression evaluates to 0 for every input and the helper always reads `x0` (whose value is always 0). The `C.SW`, `C.SD`, `C.SWSP` and `C.SDSP` branches of `store_decode` therefore write 0 into `KVM_RUN_MMIO_DATA_OFFSET` instead of the source register's value. For these encodings the register number is exactly `rvc_rs2s()` (bits 4:2 plus 8); only the uncompressed path needs the `(insn >> 17) & REG_MASK` extraction, because there the 5-bit field sits at bits 24:20.

**Fix.** Use the register index directly (this is the compressed-register primitive, `8 + rv_x(insn, SH_RS2C, 3)`):
```rust
Ok(ctx.get_reg(self.rvc_rs2s())?)
```

**Shared with the Correctness section's comment on the same line** (`Wrong register index`): same defect, same one-line fix.

### `ostd/src/arch/riscv/vm/context.rs` line 91

> ```diff
>                 0..35 => Ok(self.slice.regs_1[index]),
>                 35..68 => Ok(self.slice.regs_2[index] as usize),
>                 68..100 => Ok(self.slice.regs_3[index] as usize),
>                 100 => Ok(self.slice.regs_4 as usize),
>                 101..164 => Ok(self.slice.regs_5[index] as usize),
>                 164 => Ok(self.slice.regs_6 as usize),
> ```

Out-of-bounds array index (minor): The flat-index arms index each sub-array of the register save area with the flat index instead of the offset within that array: `regs_2[index]` for `index` in `35..68` on a `[u32; 33]`, `regs_3[index]` for `68..100` on a `[u64; 32]`, and `regs_5[index]` for `101..164` on a `[u64; 64]`. Every index in those ranges is out of bounds, so these arms can never read the FP words they were written for - the first such call panics in the bounds check (`set_reg` has the same defect). The last range is also one short: `101..164` covers only 63 of the 64 `u64` words of `fp_qctx.f`, and index 164 is mapped to the Q `fcsr`, which is the 65th word after `101`.

**Fix.** Index each sub-array with its own offset and close the last range (same change in `set_reg`):
```rust
35..68 => Ok(self.slice.regs_2[index - 35] as usize),
68..100 => Ok(self.slice.regs_3[index - 68] as usize),
101..165 => Ok(self.slice.regs_5[index - 101] as usize),
165 => Ok(self.slice.regs_6 as usize),
```

**Shared with the Security section's comment on the same lines** and the Maintainability `no-magic-number` comment at line 90: the offsetting fix and the closed last range are the same edit; the union layout itself was checked to line up byte-for-byte with the FFpu/DFpu/QFpu contexts, so only the indexing is wrong.

### `ostd/src/arch/riscv/vm/gstage.rs` line 199

> ```diff
>             let pte = unsafe { core::ptr::read_volatile(pte_ptr) };
> 
>             let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT;
>             tables_to_free[free_count] = table;
> ```

Missing page-table validity check (major): The unmap walk descends to the next level without checking `PTE_V`. For an entry that is not present, `pte & PTE_PPN_MASK` is zero, so `next_table` is 0 and the next iteration dereferences `core::ptr::null_mut::<u64>().add(idx)`; any entry with a zero PPN (no valid mapping at that level) gives the same wild pointer instead of a clean no-op or error. `map_page` performs this check (`if pte & PTE_V == 0`) when deciding to allocate, so a V=0 entry is exactly the state the walk can meet, e.g. when a GPA range that is not fully populated is unmapped.

**Fix.** Verify the entry before descending, mirroring `map_page`:
```rust
if pte & PTE_V == 0 {
    return Ok(()); // nothing mapped at this GPA
}
```

**Shared with the other `gstage.rs::unmap_page` comments** (this file, line 202, and the Security section's comment on the same line): the walk is rewritten once — mirror `map_page` by returning early on `pte & PTE_V == 0` and converting with `paddr_to_vaddr(...)` before every dereference.

### `ostd/src/arch/riscv/vm/gstage.rs` line 202

> ```diff
>             let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT;
>             tables_to_free[free_count] = table;
>             free_count += 1;
>             table = next_table as *mut u64;
> ```

Physical address used as a virtual pointer (critical): `unmap_page` takes the next-level table address from the PTE PPN, which is a physical address, and dereferences it directly as a pointer (`table = next_table as *mut u64;`). `map_page` converts the very same value with `paddr_to_vaddr` (line 155); here the raw PA (e.g. `0x8020_1000`, far below `LINEAR_MAPPING_BASE_VADDR` = `0xffff_ffc0_0000_0000`) is used as a kernel VA, so the walk reads page-table nodes at whatever the calling process happens to have mapped there, and then writes a PTE through it. Any `unmap` (for example a second `KVM_SET_USER_MEMORY_REGION` for the same slot, or a region deletion via `VmMemory::set_region` with `memory_size == 0`) therefore follows bogus pointers and corrupts unrelated memory or faults.

**Fix.** Convert the PTE's physical address to its linear-mapping VA exactly as `map_page` does:
```rust
table = paddr_to_vaddr(next_table as usize) as *mut u64;
```

**Shared with the other `gstage.rs::unmap_page` comments** (the `Missing page-table validity check` at line 199, and the Security section's comment on this same line): one rewrite of the walk covers all three.

### `ostd/src/arch/riscv/vm/switch.S` line 200

> ```diff
> 	/* Now we back to Host */
> 	.align 2
> 
> .Lkvm_switch_return:
> ```

`asm-prefer-balign` (nit): `.align 2` is used to align `.Lkvm_switch_return`, the address that is put into `stvec` before `sret` so that the next guest trap resumes there. `.align`'s meaning is architecture-dependent (on RISC-V it is a power of two), while the hardware guideline asks for the unambiguous byte-count directive.

**Fix.** Use the byte-count form, which expresses the same 4-byte alignment:
```asm
.balign 4
```

### `ostd/src/arch/riscv/vm/switch.S` line 211

> ```diff
> 	/* Return to rust code */
> 	ret
> ```

`asm-type-and-size` (minor): `__kvm_riscv_switch_to` carries `.type __kvm_riscv_switch_to, @function` but no `.size`. It is a Rust-callable function (declared in the `extern "C"` block at the bottom of `ostd/src/arch/riscv/vm/vcpu.rs`), so the guideline's boot/trap-trampoline exemption does not apply and the symbol's extent is left undefined for debuggers.

**Fix.** Add the size directive after the final `ret` (the entry path and the `.Lkvm_switch_return` resume path are contiguous):
```asm
.size __kvm_riscv_switch_to, .-__kvm_riscv_switch_to
```

### `ostd/src/arch/riscv/vm/vcpu.rs` line 197

> ```diff
>     pub fn swap_in_guest_state(&mut self){
>         let scounteren = self.guest_csr.get_scounteren();
>         self.host_scounteren = riscv::register::scounteren::read().bits();
>         ...
>         let senvcfg = self.guest_csr.get_senvcfg();
>         self.host_senvcfg = riscv::register::senvcfg::read().bits();
>         ...
>     }
> ```

Guest FPU state is never saved or restored (major): Nothing on the VM entry/exit path touches the floating-point state: `swap_in_guest_state`/`swap_in_host_state` (and `enter_exit`) only swap `scounteren`/`senvcfg`, and `switch.S` saves general registers only. The `fp_fctx`/`fp_dctx` save area of `guest_context`, which `onereg/fp_f.rs` and `onereg/fp_d.rs` read and write, is therefore never loaded from or stored into the hardware FPU (no `load_fpu_context_f`/`load_fpu_context_d` call exists on this path): `KVM_SET_ONE_REG` on an F/D register never reaches the guest, and `KVM_GET_ONE_REG` reports stale values. The real guest FP values live only in the FPU registers, so when the vCPU thread is descheduled (for example the `Task::yield_now()` in the `WFI` emulation, or host preemption) the host's FPU context switch overwrites them and the guest resumes with another thread's FP values.

**Fix.** Save/restore the guest FP state around the switch for the extension enabled for the guest (`F`/`D`), the same way `scounteren`/`senvcfg` are swapped. The `FFpuContext`/`DFpuContext` `save()`/`load()` helpers exist in `ostd/src/arch/riscv/cpu/context.rs` (they wrap `save_fpu_context_f/d`/`load_fpu_context_f/d`) but are private; expose them to the VM module and call them here, e.g.:
```rust
self.guest_context.fp_dctx.load();
// ... __kvm_riscv_switch_to ...
self.guest_context.fp_dctx.save();
```

### `ostd/src/arch/riscv/vm/vcpu.rs` line 270

> ```diff
>     fn config_load(&self) {
>         Hedeleg::write(self.cfg.hedeleg);
>         Hideleg::write(self.cfg.hideleg);
>         Henvcfg::write(self.cfg.henvcfg);
>     }
> ```

Per-hart CSR programmed on the VM-creating hart only (major): The per-entry load rewrites the per-hart H-extension CSRs `hedeleg`, `hideleg` and `henvcfg`, but not `hcounteren`. `hcounteren` is written only in `enable_virtualization_cpu()` (`ostd/src/arch/riscv/vm/cpu.rs:9`), which runs once, on the hart that executes `CREATE_VM` (`kernel/core/src/kvm/arch/riscv/vm.rs:11`); vCPU threads are not pinned to that hart. On every other hart `hcounteren` is still 0, so the guest's direct `time` reads (VS-mode `rdtime`, or the S-mode `time` CSR, gated by `hcounteren.TM`) raise an exception instead of returning the counter, even though `Hcounteren::write(0x02)` exists precisely so that "VS should access only the time counter directly".

**Fix.** Program `hcounteren` on every hart that can run a vCPU, e.g. as part of the per-entry configuration (add `Hcounteren` to the imports):
```rust
fn config_load(&self) {
    Hedeleg::write(self.cfg.hedeleg);
    Hideleg::write(self.cfg.hideleg);
    Henvcfg::write(self.cfg.henvcfg);
    Hcounteren::write(0x02);
}
```
or from a per-CPU "hardware enable" hook instead of the one-shot call at VM creation.

## Documentation

### `Makefile` line 6

> ```diff
>  # Global build options.
> -TARGET_ARCH ?= x86_64
> +TARGET_ARCH ?= riscv64
>  BENCHMARK ?= none
> ```

Docs still name x86_64 as the default target architecture (minor): `TARGET_ARCH` now defaults to `riscv64`, which makes two existing docs wrong: `AGENTS.md` line 42 says "Set `TARGET_ARCH` to `x86_64` (default), `riscv64`, or `loongarch64`", and `book/src/kernel/nix-development.md` line 13 says the ARM64 development shell "still builds the x86-64 kernel by default". A plain `make kernel`/`make run_kernel` now builds and runs the RISC-V kernel, so anyone following those docs is misled about what their build produces.

**Fix.** If the new default is intended, update the docs in this same change: change `AGENTS.md` line 42 to name `riscv64` as the default, and reword the `book/src/kernel/nix-development.md` sentence accordingly. If the change is a leftover from local RISC-V development (the same commit also switches `.vscode/settings.json` to the RISC-V target), drop it from this commit instead.

**Shared with the Maintainability section's `atomic-commits` comment on the same line**: the same decision — keep the `riscv64` default and fix the docs, or drop the flip from this commit.

### `kernel/core/src/device/kvm/ioctl.rs` line 17

> ```diff
> +// System ioctls(for device.rs)
> +pub(super) type GetApiVersion = ioc!(KVM_GET_API_VERSION, 0xAE, 0x00, NoData);
> +pub(super) type CreateVm = ioc!(KVM_CREATE_VM, 0xAE, 0x01, NoData);
> +pub(super) type CheckExtension = ioc!(KVM_CHECK_EXTENSION, 0xAE, 0x03, NoData);
> +...
> +pub(super) type Run = ioc!(KVM_RUN, 0xAE, 0x80, NoData);
> +pub(super) type GetStatsFd = ioc!(KVM_GET_STATS_FD, 0xAE, 0xCE, NoData);
> ```

`linux-compat-docs` (major): This change adds a new user-visible API under `kernel/` — the `/dev/kvm` character device and its ioctl family (`KVM_GET_API_VERSION`, `KVM_CREATE_VM`, `KVM_CHECK_EXTENSION`, `KVM_GET_VCPU_MMAP_SIZE`, `KVM_CREATE_VCPU`, `KVM_SET_USER_MEMORY_REGION`, `KVM_RUN`, `KVM_GET_ONE_REG`/`KVM_SET_ONE_REG`, `KVM_GET_REG_LIST`, `KVM_INTERRUPT`, `KVM_SET_MP_STATE`, `KVM_GET_STATS_FD`) — but the Linux Compatibility docs are not touched. `book/src/kernel/linux-compatibility/syscall-flag-coverage/file-descriptor-and-io-control/ioctl.scml` still enumerates only the tty/pty/DRM/evdev/block/TDX/socket command families, and the `### ioctl` section of that page's `README.md` has no KVM entry, even though the `ioctl` row of `book/src/kernel/linux-compatibility/README.md` points readers at exactly that coverage. The documented `ioctl` coverage therefore no longer matches the code that user space can reach.

**Fix.** Update the `ioctl` flag coverage in this same change. Add a command group and a rule to `book/src/kernel/linux-compatibility/syscall-flag-coverage/file-descriptor-and-io-control/ioctl.scml`, mirroring the existing `drm_ops` and TDX entries:

```c
kvm_ops = KVM_GET_API_VERSION | KVM_CREATE_VM | KVM_CHECK_EXTENSION |
          KVM_GET_VCPU_MMAP_SIZE | KVM_CREATE_VCPU |
          KVM_SET_USER_MEMORY_REGION | KVM_GET_ONE_REG | KVM_SET_ONE_REG |
          KVM_GET_REG_LIST | KVM_INTERRUPT | KVM_SET_MP_STATE | KVM_RUN |
          KVM_GET_STATS_FD;

// Control Kernel-based Virtual Machine (KVM) devices
ioctl(fd, op = <kvm_ops>, ..);
```

Then add KVM to the `### ioctl` section of `book/src/kernel/linux-compatibility/syscall-flag-coverage/file-descriptor-and-io-control/README.md`, noting the parts that are currently accepted but not implemented.

### `ostd/src/arch/riscv/vm/gstage.rs` line 34

> ```diff
> //       1.ostd/src/arch/riscv/vm/gstage.rs
> //       2.ostd/src/vm/mod.rs
> //       3.ostd/src/vm/gpm_space.rs
> //       4.ostd/src/vm/temp_riscv_gpm.rs
> //       5.kernel/src/kvm/vm_memory.rs(func map_guest_memory() & unmap_guest_memory())
> ```

Stale file path in the deletion checklist (minor): The "Files should be checked if this file is going to be deleted" list points at `kernel/src/kvm/vm_memory.rs`, but that path does not exist: the module lives at `kernel/core/src/kvm/vm_memory.rs` (`kernel/src/` contains only `lib.rs`). The same list is duplicated verbatim in `ostd/src/vm/temp_riscv_gpm.rs` line 35, so both copies would send a maintainer who follows this checklist to a file that is not there.

**Fix.** Correct the path in both copies to `kernel/core/src/kvm/vm_memory.rs`:

```diff
-//       5.kernel/src/kvm/vm_memory.rs(func map_guest_memory() & unmap_guest_memory())
+//       5.kernel/core/src/kvm/vm_memory.rs(func map_guest_memory() & unmap_guest_memory())
```

### `ostd/src/arch/riscv/vm/isa.rs` line 5

> ```diff
> /// ISA extension IDs specific to KVM. This is not the same as the host ISA
> /// extension IDs as that is internal to the host and should not be exposed
> /// to the guest. This should always be contiguous to keep the mapping simple
> /// in KVM implementation.
> ```

`semantic-line-breaks` (nit): This doc comment on `KvmRiscvIsaExtId` wraps a sentence across three lines while starting a new sentence in the middle of one: `... specific to KVM. This is not the same as the host ISA` / `extension IDs as that is internal to the host and should not be exposed` / `to the guest. This should always be contiguous ...`.

**Fix.** Break at the sentence boundaries, e.g.:

```rust
/// ISA extension IDs specific to KVM.
/// This is not the same as the host ISA extension IDs,
/// as that is internal to the host and should not be exposed to the guest.
/// This should always be contiguous
/// to keep the mapping simple in the KVM implementation.
```

### `ostd/src/arch/riscv/vm/sbi/fwft.rs` line 87

> ```diff
> 	/// Check if the feature is supported on the vcpu.
> 	/// This callback is optional, if not provided the feature is assumed to
> 	/// supported
> 	supported: &'static FwftSupportFunction,
> 
> 	/// Probe and initialize the feature on the vcpu.
> 	/// This callback is optional. If provided, it will be called during
> 	/// vcpu initialization to probe the feature availability and perform
> 	/// any necessary initialization. Returns true if the feature is supported
> 	/// and initialized successfully, false otherwise.
> ```

`semantic-line-breaks` (nit): These doc lines pack two sentences onto one line — `/// This callback is optional. If provided, it will be called during` (line 87) and `... any necessary initialization. Returns true ...` (line 89) — so breaks fall inside sentences instead of at the sentence boundaries. The `supported` comment just above also dropped a word: `the feature is assumed to` / `supported` (lines 82-83).

**Fix.** Keep one sentence per line and fix the wording, e.g.:

```rust
	/// Check if the feature is supported on the vcpu.
	/// This callback is optional;
	/// if it is not provided, the feature is assumed to be supported.
	supported: &'static FwftSupportFunction,

	/// Probe and initialize the feature on the vcpu.
	/// This callback is optional.
	/// If it is provided, it is called during vcpu initialization
	/// to probe feature availability and perform any necessary initialization.
	/// Returns `true` if the feature is supported and initialized successfully,
	/// `false` otherwise.
	init: Option<&'static FwftInitFunction>,
```

### `ostd/src/arch/riscv/vm/sbi/mod.rs` line 46

> ```diff
> /// SBI extension IDs specific to KVM. This is not the same as the SBI
> /// extension IDs defined by the RISC-V SBI specification.
> ...
> 	/// SBI extension handler. It can be defined for a given extension or group of
> 	/// extension. But it should always return linux error codes rather than SBI
> 	/// specific error codes.
> ...
> 	/// Init/deinit function called once during VCPU init/destroy. These
> 	/// might be use if the SBI extensions need to allocate or do specific
> 	/// init time only configuration.
> ```

`semantic-line-breaks` (nit): Three doc comments in this file put a sentence boundary in the middle of a line and scroll the rest of the sentence onto the next line: `KvmRiscvSbiExtId` (lines 46-48, `... specific to KVM. This is not the same as the SBI` / `extension IDs defined by ...`), `handler` (lines 157-159, ``... a given extension or group of`` / ``extension. But it should always ...``), and `init` (lines 165-167, ``... VCPU init/destroy. These`` / ``might be use if ...``).

**Fix.** Re-break each comment so a line ends where a sentence or clause ends, e.g.:

```rust
/// SBI extension IDs specific to KVM.
/// This is not the same as the SBI extension IDs
/// defined by the RISC-V SBI specification.
```

### `ostd/src/arch/riscv/vm/vcpu.rs` line 43

> ```diff
> 	/// ### VCPU interrupts
> 	/// The irqs_pending bitmap represents pending interrupts whereas
> 	/// irqs_pending_mask represents bits changed in irqs_pending. Updates
> 	/// to these bitmaps are serialized so vcpu interrupt sync/flush cannot
> 	/// drop a newly injected interrupt while syncing guest-visible HVIP.
> ```

`semantic-line-breaks` (minor): The doc block for `irqs_pending`/`irqs_pending_mask` (lines 43-61) is wrapped at a fixed column instead of at semantic boundaries, so sentence ends land in the middle of lines (`... represents bits changed in irqs_pending. Updates`, ``... via `SET_INTERRUPT`. However, ...``, `... specific interrupt numbers,` / `however, ...`) while continuation words start the next line. Any later edit to one sentence reflows the whole paragraph, which is exactly the diff noise this rule is meant to avoid.

**Fix.** Break the prose at sentence (and clause) boundaries, one idea per line, e.g.:

```rust
    /// ### VCPU interrupts
    /// The `irqs_pending` bitmap represents pending interrupts,
    /// whereas `irqs_pending_mask` represents bits changed in `irqs_pending`.
    /// Updates to these bitmaps are serialized,
    /// so interrupt sync/flush cannot drop a newly injected interrupt
    /// while syncing guest-visible HVIP.
    ///
    /// Note: If we do not use AIA to implement interrupt virtualization,
    /// all guest interrupts must originate from the user-space VMM (e.g., QEMU)
    /// via `SET_INTERRUPT`.
    /// However, the VMM's invocation of the `ioctl` requires CPU resources;
    /// this means that in a single-core environment,
    /// unless the `vcpu_run` thread yields control,
    /// the guest will never receive the interrupt.
    ///
    /// Note: Generally, interrupts are associated with specific interrupt numbers,
    /// however, we do not need to concern ourselves with the specific number;
    /// only with whether an interrupt has occurred.
    /// This is why `set_interrupt` does not distinguish between interrupt numbers.
    /// We only need to tell the guest that an interrupt is pending;
    /// the guest then queries the PLIC via MMIO to find the specific number.
    /// Since the PLIC is managed by the user-space VMM (e.g. QEMU),
    /// all the necessary setup has already been handled.
```

### `test/initramfs/nix/initramfs.nix` line 64

> ```diff
> +  qemu = builtins.path { path = ./../src/qemu-system-riscv64; };
> +  kernel = builtins.path { path = ./../src/Image; };
> +  initramfs = builtins.path { path = ./../src/rv.cpio.gz; };
> ...
> +    mkdir -p $out/kvm_test
> +    cp ${qemu} $out/kvm_test/qemu-system-riscv64
> +    cp ${kernel} $out/kvm_test/Image
> +    cp ${initramfs} $out/kvm_test/rv.cpio.gz
> ```

New packaged test assets are left undocumented (minor): The packaging now copies `qemu-system-riscv64`, `Image` and `rv.cpio.gz` from `test/initramfs/src/` into `/kvm_test` (and the `builtins.path` calls on lines 17-19 make those files mandatory for the derivation), yet no doc says what they are or where they come from. `test/initramfs/README.md` still describes the directory structure and the rule for adding configuration files without any mention of them, and `.gitignore` hides all three, so a fresh checkout gives a developer no clue what to place in `test/initramfs/src/`.

**Fix.** Document the new payload in the same change: add `kvm_test/` and the three `src/` inputs to the Directory Structure section of `test/initramfs/README.md`, and add a short subsection that says where `qemu-system-riscv64`, `Image` and `rv.cpio.gz` come from and that the RISC-V KVM test expects them.
