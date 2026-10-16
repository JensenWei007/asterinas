---
date: 2026-10-15
mode: diff
base: 10a241870
head: a1a440a07
branch: main
title: "kvm for riscv"
---

# 摘要（Summary）

> 译者注：本文件是 [review.md](review.md) 的简体中文版，逐节翻译而成。文件路径、`line N` 锚点、代码块与 `diff` 引用与英文版逐字一致，便于两版对照；严重度沿用英文标记（`critical` 致命 / `major` 严重 / `minor` 次要 / `nit` 细节），规范短名（如 `atomic-commits`）与代码标识符保持原样。

本提交为 Asterinas 引入了 RISC-V KVM 实现：一个 `/dev/kvm` 设备前端（`kernel/core/src/device/kvm/`）、
一层与体系结构无关的 VM/vCPU 层（`kernel/core/src/kvm/`），
以及它在 OSTD 中依赖的 RISC-V H 扩展机制
（G-stage 页表、`switch.S` 的 guest 进入/退出路径、每 vCPU 的 CSR 上下文、
SBI 模拟，以及 one-reg/MMIO/timer/CSR 模拟）。
五个 persona 对 70 个改动文件做了评审，共得出 **65 条评论——3 条 `critical`、24 条 `major`、31 条 `minor`、7 条 `nit`**；
每一条都回到了代码上核对，没有一条被撤回。

## 做得好的地方

- **framekernel 的分层守住了。** 所有必须触碰 `unsafe` 的部分——页表遍历、CSR 访问、
  guest 进出的汇编——都留在 `ostd/` 内，kernel 一侧保持为安全 Rust。
  一个约 5 200 行的改动很容易侵蚀掉这条边界，这个改动没有。
- **Linux ABI 被忠实对齐。** KVM 的 ioctl 编号、`KVM_REG_*` 寄存器 id 编码、
  `kvm_run` 的偏移、以及 ISA/SBI 扩展列表都逐字段跟随 Linux，
  这正是将来能让未经修改的 VMM 跑起来的前提。
- **guest 进入路径紧凑且可审计**——单个 `switch.S` 例程配合显式的保存/恢复宏——
  并且 G-stage 代码在自己的文件头里声明了它的临时状态，
  这种坦率正是让一个过渡方案最终能被移除的关键。

## 按优先级排列的主要问题

1. **`unmap_page` 把物理地址当指针来遍历页表**
   （`ostd/src/arch/riscv/vm/gstage.rs:202`，`critical`，Security 与 Hardware 独立报出）。
   `map_page` 会用 `paddr_to_vaddr` 转换；unmap 的遍历不做转换，
   因此每一次删除或替换 slot 的 `KVM_SET_USER_MEMORY_REGION`
   都会通过一个伪造的内核指针写入 PTE，*并且*在后台帧已被释放之后
   仍让 guest 的映射保持有效。这与前面第 199 行缺失 `PTE_V` 检查是同一个函数、同一处修复。
2. **运行路径上 guest 可触发的 panic**（6 条评论，`major`/`critical`）：
   `exit`/`gstage_page_fault`/`csr_insn` 里的 `todo!()`、
   对用户可写的 `kvm_run` 字段做分发的 `KVM_GET_ONE_REG`，以及
   在不支持 SSTC 的主机上 timer 的 `unwrap()`。guest 或者 VMM 都能 panic 掉宿主内核；
   修复方式是确立一条统一约定——未实现的路径要么向 VMM 返回 `errno`，要么向 guest 注入异常。
3. **来自用户空间的寄存器编号未做校验**（`onereg/csr.rs`，`critical`）。
   `KVM_GET_ONE_REG`/`KVM_SET_ONE_REG` 传入 general 子类型、编号 ≥ 11 的 CSR id 时，
   会越界访问 `[usize; 11]`；`get_csr` 本就返回 `Option`，
   说明当初就打算校验，只是从未实现（Linux 在此返回 `EINVAL`）。
4. **G-stage 分配器静默吞掉了分配失败**
   （`gstage.rs:94` 与 `:144`）：`alloc_page` 把 OOM 报告成 `(0, 0)`，
   最终变成对空指针的写入和一个物理地址为 0 的 `hgatp` 根，而不是让 ioctl 失败。
5. **两个解码缺陷让整类指令的行为都是错的**：压缩指令的 MMIO 存储永远送 `x0`
   （`mmio.rs:201`，`major`），core one-reg 组的 `MODE` 永远报告为 0
   （`onereg/core.rs:60`，`major`）——后者还会在写 `MODE` 时清掉 `sstatus` 的其他所有字段
   （`onereg/core.rs:76`）。
6. **这个提交本身无法作为一个单元来评审**（`major`，Maintainability）：
   `kvm for riscv` 既不是命令式、也没有正文，还把该特性与项目级的 `TARGET_ARCH` 翻转、
   `.vscode` 目标切换以及无关的重构捆绑在一起；68 个被触及的文件中还有 50 个
   缺少 SPDX 头，且新子系统没有通过 `cargo fmt`。

## 结构性建议

- **每张查找表只保留一种身份标识。** 这里最常见的一类缺陷，
  是手工维护的并行结构已经彼此漂移：`KvmRiscvIsaExtId` 与
  `KVM_ISA_EXT_ARR`（从索引 7 起就对不上）、`SBI_EXT_TABLE` 的 `ext_idx` 与真实
  扩展 id（BASE 被挂在 `Max` 哨兵值上，而 guest 的 `a7` 却拿去和内部索引比较）、
  以及 `context.rs` 里的扁平寄存器视图与它所覆盖的 slice 布局。
  让每一者都从枚举派生，并在编译期断言布局，能消除这**一整类**问题而不是三个实例。
- **为 KVM 路径定下 panic 策略**，并把它写在模拟代码所在之处：
  凡是由 guest 或 VMM 选择的值，都不得走到 `todo!()`/`unwrap()`。
  这比看起来要小——只有六处——却能消除 guest 可触发的宿主 DoS。
- **让 G-stage 端到端可失败**（`alloc_page` → `GStagePageTable::new` →
  `GuestPhysMemSpace::new` → `VmMemory::new`），并正确地刷新它：
  map/unmap 之后缺失的远端 `hfence.gvma` 是正确性问题而非仅仅是性能问题，
  因为帧在其他 hart 的翻译失效之前就被释放了。
- **拆分提交**、补上 SPDX 头、对新文件跑 `cargo fmt`——
  这样对特性本身的评审就不会和对脚手架的评审混在一起。
- **在兼容性文档里覆盖新的用户空间接口。** `/dev/kvm` 是一条新的用户可见 ABI；
  `ioctl.scml` 及其 README 目前仍只列出 tty/pty/DRM/evdev/block/TDX/socket，
  而 `KVM_GET_STATS_FD` 桩返回的是一个从未被分配的 fd 编号——
  在文档里注明"已接受但未实现"能让两边都诚实。

## Maintainability（可维护性）

### `Makefile` line 6-不用管

> ```diff
> -TARGET_ARCH ?= x86_64
> +TARGET_ARCH ?= riscv64
> ```

`atomic-commits`（major）：把项目级的默认 `TARGET_ARCH` 从 `x86_64` 翻转为 `riscv64`，改变了所有裸 `make`（以及任何未显式传 `TARGET_ARCH` 的 CI 任务）的构建结果，而这件事与新增 KVM 无关；同一个提交还把共享的编辑器配置改了向（`.vscode/settings.json` 中的 `rust-analyzer.cargo.target`）。

**修复。** 保留 `x86_64` 作为默认值，需要时用 `make TARGET_ARCH=riscv64` 构建；或者把默认值的改动移到一个范围清晰的独立提交里；同时把个人化的 rust-analyzer 覆盖项从仓库内的设置中去掉。

**与文档章节同一行的评论共享同一修法**（`Docs still name x86_64 as the default target architecture`）：无论这个默认值改动是保留还是撤回，文档更新与提交拆分都必须一并决定。

### `commit a1a440a07 message`-不用管

> ```diff
> Makefile | 2 +-
> .vscode/settings.json | 2 +-
> ostd/src/arch/riscv/mm/mod.rs | 38 +--
> kernel/core/src/vm/vmar/vmar_impls/map.rs | 6 +-
> ... new KVM subsystem (47 files)
> ```

`atomic-commits`（major）：一个提交把 KVM 特性（约 5000 行新增代码）与本应作为更早的独立提交的准备性重构（`ostd/src/arch/riscv/mm/mod.rs` 中抽取的 `cmo_block_size`、`kernel/core/src/vm/vmar/vmar_impls/map.rs` 中重写的 `debug_assert`），以及无关的构建配置捆在一起，因此无法作为一个逻辑变更来评审或二分。

**修复。** 拆开它：(1) 一个 `Refactor` 提交，包含 `cmo_block_size` 与 `VmarMapOptions` 断言改动；(2) KVM 特性本身；(3) 默认目标架构的改动单独成提交；让每个提交都能独立评审。

### `commit a1a440a07 message`-不用管

> ```diff
> kvm for riscv
> 
> Signed-off-by: JensenWei007 <jensenwei007@gmail.com>
> ```

`imperative-subject`（major）：标题 `kvm for riscv` 全小写、没有动词、也不是命令式；提交信息也没有正文来说明改动，而日志惯例是 `Add`/`Implement`/`Fix` 这类动词开头的标题。

**修复。** 改成例如 `Implement KVM support for riscv64`，并补一段正文，说明设计（device/VM/VCPU 的划分、G-stage 页表这一过渡方案）以及尚未实现的部分。

### `kernel/core/src/device/kvm/device.rs` line 10-跑fmt，这个我在评审前给忘了，我的我的

> ```diff
> use super::{KVM_MAJOR, KVM_MINOR, vm_file::VmFile, ioctl::{GetApiVersion, CreateVm,CheckExtension,GetVcpuMmapSize,KVM_API_VERSION,KVM_RUN_MMAP_SIZE}};
> ```

`consistency`（minor）：新子系统的格式与仓库其余部分不一致：这行 import 在逗号后不留空格（`CreateVm,CheckExtension,GetVcpuMmapSize`），而且同样的问题反复出现（`kernel/core/src/device/kvm/ioctl.rs` 里的 `pub(super)const KVM_RUN_MMAP_SIZE`、`kernel/core/src/kvm/arch/riscv/vm.rs` 里的 `pub(in crate::kvm)  fn`、`ostd/src/arch/riscv/vm/` 下成片的制表符与空格混用）。

**修复。** 对这些新文件跑一遍 `cargo fmt`，让它们读起来和周边代码一致。

### `kernel/core/src/device/kvm/ioctl.rs` line 1-已修复

> ```diff
> +use crate::{
> +    prelude::*,
> +    util::ioctl::{InData, NoData},
> ```

`consistency`（minor）：本文件直接从 `use ...` 开始，缺少 `// SPDX-License-Identifier: MPL-2.0` 头，而 `kernel/core/src` 下 772 个 Rust 文件中的 762 个（以及同级的 `kernel/core/src/device/kvm/mod.rs`）都带有该头；另有约 46 个新文件也缺，例如 `ostd/src/arch/riscv/vm/` 下的全部文件。

**修复。** 给每个新文件加上首行 `// SPDX-License-Identifier: MPL-2.0`，让许可证头保持统一。

### `kernel/core/src/device/kvm/vcpu_file.rs` line 25-已修复

> ```diff
> +pub(super) struct VcpuFile {
> +    #[allow(unused)]
> +    vm: Arc<Vm>,
> ```

`narrow-lint-suppression`（minor）：`#[allow(unused)]` 掩盖了 `vm: Arc<Vm>` 为何存在（配对的 `Vcpu` 只持有 `Weak<Vm>`，因此这个字段负责保持 VM 存活）；该抑制既没有给出理由，用的也是 `allow`，而不是规范所指的 `#[expect(..., reason = ...)]` 形式，导致后来的读者无法判断这个字段是否仍然需要。

**修复。** 换成 `#[expect(dead_code, reason = "keeps the Vm alive while the VCPU file is open")]`，或者重构到"强引用显然由 `VcpuFile` 持有"的形式。

### `kernel/core/src/kvm/arch/riscv/mmio.rs` line 84-已修复

> ```diff
> +    pub(super) fn load_decode(&mut self) {
> +        if self.insn & INSN_MASK_LW == INSN_MATCH_LW {
> +            self.len = 4;
> +            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
> +        } else if self.insn & INSN_MASK_LB == INSN_MATCH_LB {
> +            self.len = 1;
> +            self.shift = 8 * (size_of::<usize>() as i32 - self.len);
> ```

`dry`（minor）：`load_decode`、`store_decode` 和 `handle_return` 重复了同样的按编码分支的代码体：八处 `self.len = N; self.shift = 8 * (size_of::<usize>() as i32 - self.len);` 分支、四处完全相同的 `read_run_val`/`set_reg` 分支，以及每条指令一对 `INSN_MATCH_*`/`INSN_MASK_*`（共 16 对，其中 12 对共用掩码 `0x707f`）。

**修复。** 用一张 (match, mask, length, sign-extend) 表驱动解码，并让三个函数共用 load/store 的管道代码，这样新增一条指令只需新增一行。

### `kernel/core/src/kvm/arch/riscv/vcpu.rs` line 97-已修复

> ```diff
> +        // If the vCPU has already run at least once,
> +        // then apply the configuration.
> +        let at_least_once = self.inner.lock().is_ran_at_least_once();
> +        if !at_least_once {
> +            self.inner.lock().config_ran_once();
> +        }
> ```

`explain-why`（minor）：注释说"如果 vCPU 已经至少运行过一次"就应用配置，但代码恰恰是在**尚未运行过**时才应用（`if !at_least_once { self.inner.lock().config_ran_once(); }`），因此注释描述的意图与行为正好相反。

**修复。** 重写注释以陈述真实规则，例如 `// On the first run, apply the static VCPU configuration before entering the guest.`，并考虑把额外的 `ran_at_least_once()` 调用合并进一个 helper，让"只运行一次"的标志只在一处维护。

### `kernel/core/src/kvm/ioctl.rs` line 11-已修复

> ```diff
> +use super::arch::ioctl::check_arch_extension;
> ```

`qualified-fn-imports`（minor）：自由函数被按裸名引入（此处的 `check_arch_extension`、`kernel/core/src/device/kvm/device.rs` 中的 `check_extension`），整个模块则被 glob 导入（`kernel/core/src/kvm/vm.rs` 中的 `use super::ioctl::*;`、`use super::arch::vm::*;`），于是调用点无法体现函数的来源。

**修复。** 导入父模块并限定调用，例如 `use super::arch;` 然后 `arch::ioctl::check_arch_extension(raw_ioctl)`；同时把 glob 导入替换为实际用到的具体条目。

### `kernel/core/src/kvm/ioctl.rs` line 89-不用管，参照linux设计，定义了全部常量，如果实在需要删干净也行，反正不影响，然后KVM_EXIT相关是KVM_RUN用的，因为KVM_RUN本质是一块union内存，所以值一样是正常的

> ```diff
> +pub const KVM_EXIT_IO: u32 = 2;
> +pub const KVM_EXIT_DEBUG: u32 = 2;
> ```

`expect-dead-code`（minor）：这个新常量块大部分从未被读取，且描述的是 riscv64 专用模块不会实现的 x86 专有特性（`KVM_RUN_CR8_OFFSET`、`KVM_RUN_APIC_BASE_OFFSET`、`KVM_RUN_IO_*`、`IA32_TSC_DEADLINE`、`KVM_IRQCHIP_*`、`KVM_EXIT_IO`/`KVM_EXIT_HLT`/`KVM_EXIT_SHUTDOWN`、大多数 `KVM_CAP_*`）；复制粘贴还造出了 `KVM_EXIT_DEBUG = 2`，与 `KVM_EXIT_IO` 取值相同。

**修复。** 只保留有具体用途的常量（加上该特性将来需要的 RISC-V 常量），删掉 x86 ABI 的遗留项；确需为计划中的用途保留某一项时，注明它对应的 KVM uapi 头文件。

### `kernel/core/src/kvm/mod.rs` line 1-已修复

> ```diff
> +pub mod vm;
> +
> +pub mod vcpu;
> ```

`module-docs`（minor）：新 KVM 子系统的入口直接以 `pub mod` 声明开场：没有 `//!` 头说明该子系统做什么、哪些类型（`Vm`、`Vcpu`、`VmMemory`、`KvmRun`）是关键类型，也没有说明本模块与 `crate::device::kvm` 的关系。

**修复。** 加上模块文档，例如 `//! KVM (virtualization) support for riscv64.`，随后列出关键类型，并说明本模块（VM/VCPU 状态）与 `device::kvm`（`/dev/kvm` 文件接口）的分工。

### `kernel/core/src/kvm/mod.rs` line 4-已修复

> ```diff
> +pub mod vcpu;
> +pub mod vm_memory;
> +pub mod ioctl;
> +pub mod run_page;
> ```

`narrow-visibility`（minor）：每个子模块都被声明为 `pub`，于是整套 hypervisor 实现（`vcpu`、`vm_memory`、`run_page`、`arch`……）都成了 `aster-core` 公开 API 的一部分，尽管其唯一消费者都在本 crate 内（`device::kvm::*`）。

**修复。** 从 `pub(crate)` 起步（在 `device::kvm` 使用 `crate::kvm::...` 路径的地方直接用 `mod`），把 `pub` 留给确实被别的 crate 需要的条目。

### `kernel/core/src/kvm/vcpu.rs` line 11-已修复

> ```diff
> +pub struct Vcpu {
> +    id: u32,
> +    pub vm: Weak<Vm>,
> ```

`getter-encapsulation`（minor）：`pub vm: Weak<Vm>` 把内部的弱引用暴露出来，而同一 impl 已经提供了 `Vcpu::vm()`（第 30 行），于是调用方可以绕过"upgrade 失败即报错"的路径，该字段今后也无法再改换表示。

**修复。** 把字段改为私有，让 `vm()` 保持为唯一访问入口。

### `kernel/core/src/kvm/vm.rs` line 17-已修复

> ```diff
> +pub struct Vm {
> +    pub id: u32,
> +    memory: VmMemory,
> ```

`getter-encapsulation`（minor）：`pub id: u32` 把 VM 标识符暴露为 `Vm` 表示的一部分；唯一的外部使用者是 `kernel/core/src/device/kvm/vm_file.rs` 中的 `dump_proc_fdinfo`，改为调用 getter 同样可行。

**修复。** 把 `id` 改为私有，并新增 `pub fn id(&self) -> u32`，与现有的 `memory()` 访问器并列。

### `kernel/core/src/kvm/vm.rs` line 98-已修复

> ```diff
> +fn default_guest_mem_prop(is_readonly: bool) -> PageProperty {
> +    let guest_page_flags = if is_readonly {
> +        PageFlags::RX
> +    } else {
> +        PageFlags::RWX
> +    };
> ```

`no-bool-args`（nit）：`default_guest_mem_prop(is_readonly: bool)` 用一个布尔值在两种行为之间做选择（`PageFlags::RX` 与 `PageFlags::RWX`），而调用点传的要么含义不明的 `false`，要么是一个复合表达式。

**修复。** 拆成 `readonly_guest_mem_prop()` 与 `guest_mem_prop()`，或者改用一个小的枚举，例如 `GuestMemAccess::{ReadOnly, ReadWrite}`。

### `ostd/src/arch/riscv/vm/context.rs` line 90-已修复

> ```diff
> +            match index {
> +                0..35 => Ok(self.slice.regs_1[index]),
> +                35..68 => Ok(self.slice.regs_2[index] as usize),
> +                68..100 => Ok(self.slice.regs_3[index] as usize),
> +                100 => Ok(self.slice.regs_4 as usize),
> +                101..164 => Ok(self.slice.regs_5[index] as usize),
> ```

`no-magic-number`（minor）：寄存器布局被编码成无名的索引区间（`0..35`、`35..68`、`68..100`、`100`、`101..164`、`164`）并在 `get_reg` 与 `set_reg` 中重复，而且这些边界与它们所索引的 slice 对不上（`regs_2` 是 33 个元素、`regs_3` 是 32 个、`regs_5` 是 64 个），后来的作者无法判断某个索引属于哪个子数组。

**修复。** 把布局命名一次（例如 `const CORE_REGS_LEN: usize = 35;`、`const FP_F_BASE: usize = ...;`），并在索引每个子数组时使用各自的 `index - base` 偏移，由 `get_reg` 与 `set_reg` 共用。

**与同一函数上的越界评论共享同一修法**（Security 与 Hardware，`ostd/src/arch/riscv/vm/context.rs` 第 91 行）：命名布局并为各子数组做偏移，是同一次改动，也能一并消除那些 panic。

### `ostd/src/arch/riscv/vm/gstage.rs` line 53-已修复

> ```diff
> +const PTE_U: u64 = 1 << 4;  // User (G-stage中未使用)
> ```

`consistency`（nit）：在一个其余部分均为英文的代码库中混入了中文片段：`const PTE_U: u64 = 1 << 4;  // User (G-stage中未使用)`。

**修复。** 改为英文，例如 `// User; unused in G-stage`。

### `ostd/src/arch/riscv/vm/insn/mod.rs` line 15-已修复

> ```diff
> +pub enum KvmInsnReturn {
> +    /// Need exit to user-space
> +    KvmInsnExitToUserSpace = 0,
> +    /// Continue with next sepc
> +    KvmInsnContinueNextSepc,
> ```

`rust-native`（minor）：该枚举在每个变体里重复了自身名称（`KvmInsnReturn::KvmInsnContinueNextSepc`、`KvmInsnReturn::KvmInsnExitToUserSpace = 0`）——这种 C 风格的命名在 Rust 调用点读起来是噪声；`ostd/src/arch/riscv/vm/sbi/mod.rs` 中的 `KvmRiscvSbiExtStatus::KvmRiscvSbiExtStatusEnabled`、`KvmRiscvSbiExtId::KvmRiscvSbiExtV01`、`KvmVcpuSbiReturnError::SbiErrNotSupported` 也有同样问题。

**修复。** 去掉前缀，让枚举名承载上下文：`KvmInsnReturn::{ExitToUserSpace, ContinueNextSepc, ...}`、`KvmRiscvSbiExtStatus::{Enabled, Disabled, ...}`、`KvmRiscvSbiExtId::{V01, Time, ...}`、`KvmVcpuSbiReturnError::{NotSupported, ...}`。

### `ostd/src/arch/riscv/vm/isa.rs` line 93-已修复

> ```diff
> +static KVM_ISA_EXT_ARR: [IsaExtensions; KvmRiscvIsaExtId::MAX as usize] = [
> +    // single
> +    IsaExtensions::A,          // KVM_RISCV_ISA_EXT_A
> +    ...
> +    IsaExtensions::V,          // KVM_RISCV_ISA_EXT_V
> ```

`dry`（major）：id 到扩展的映射被写了两遍，而且顺序不同：`KvmRiscvIsaExtId` 枚举把 `SVPBMT` 放在 7、`V` 放在 15，而 `KVM_ISA_EXT_ARR` 把 `V` 放在索引 7（它自己的行尾注释还写着 `KVM_RISCV_ISA_EXT_V`）、把 `SVADU` 放在 15，因此 `KVM_ISA_EXT_ARR[kvm_ext]` 从索引 7 起就不可能与该枚举一致。

**修复。** 只保留一种有序表示——例如一张 `[(KvmRiscvIsaExtId, IsaExtensions)]` 表，或对 id 做 `match`——这样两种顺序就无法悄悄漂移。

**与正确性章节同一行的评论共享同一修法**（`Wrong table mapping`）：这种漂移已经发生了——`KVM_ISA_EXT_ARR[kvm_ext]` 从索引 7 起就返回错误的扩展，所以这是一处修复，而不是整理。

### `ostd/src/arch/riscv/vm/isa.rs` line 199-已修复

> ```diff
> +fn kvm_riscv_isa_disable_allowed(_ext: usize) -> bool {
> +    false
> +}
> ```

`expect-dead-code`（nit）：`kvm_riscv_isa_disable_allowed` 在此定义却从未被调用，邻近的 `kvm_riscv_isa_check_host` 返回的 `Err(1)` 其值也从未被读取（`kvm_riscv_vcpu_setup_isa` 中是 `if let Ok(ext)`），于是这条错误通道既未被使用也没有解释。

**修复。** 在有调用方之前删除 `kvm_riscv_isa_disable_allowed`（或随禁用路径一起加入，并附 `#[expect(dead_code)]` 与理由）；另外让 `kvm_riscv_isa_check_host` 返回 `bool`，或者注明 `1` 的含义。

### `ostd/src/arch/riscv/vm/onereg/mod.rs` line 96-已修复

> ```diff
> +        self.copy_config_reg_vec(vec.as_deref_mut());
> +        self.copy_core_reg_vec(vec.as_deref_mut());
> +        ...
> +        self.copy_sbi_state_reg_vec(vec.as_deref_mut())?;
> ```

`consistency`（minor）：`copy_reg_vec` 驱动的各个按类型 helper 形态并不统一：`copy_config_reg_vec` 返回 `usize`，`copy_sbi_state_reg_vec` 返回 `Result<usize>`，`copy_core_reg_vec`/`copy_csr_reg_vec`/`copy_fp_*_reg_vec` 返回 `()`，`copy_timer_reg_vec`/`copy_vector_reg_vec`/`copy_isa_ext_reg_vec` 返回硬编码的 `0`，而 `num_*_regs` 有的是 `copy_*(None)`、有的则是单独的 `size_of` 计算。

**修复。** 给每个子类型统一契约，例如 `fn copy_regs(&self, out: &mut Vec<usize>) -> Result<()>` 配合 `fn num_regs(&self) -> usize`，再由 `copy_reg_vec`/`num_regs` 统一驱动它们。

### `ostd/src/arch/riscv/vm/sbi/base.rs` line 15-已修复

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

`dry`（minor）：15 个 SBI 扩展各自重复了整个 13 字段的 `KvmVcpuSbiExtension` 字面量，尽管其中几乎每个字段都是 `None`/`0`/`false`（`base`、`hsm`、`ipi`、`rfence`、`srst`、`susp`、`time`、`v01`、`pmu`、`sta`、`fwft`，以及 `forward.rs` 中的四个），于是新增一个字段就要改所有这些地方。

**修复。** 增加构造函数或"可选字段取默认值"的 helper（例如 `KvmVcpuSbiExtension::new(SBI_EXT_BASE, &sbi_ext_base_handler)`），对少数额外设置字段的扩展使用 struct update 语法。

### `ostd/src/arch/riscv/vm/sbi/mod.rs` line 188

> ```diff
> +    KvmRiscvSbiExtensionEntry {
> +        ext_idx: KvmRiscvSbiExtId::KvmRiscvSbiExtMax,
> +        ext: &VCPU_SBI_EXT_BASE,
> +    },
> ```

`dry`（major）：同一个扩展被标识了三次——`KvmRiscvSbiExtId` 判别值、手写的 `ext_idx` 字段、以及 `SBI_EXT_TABLE_SIZE = 15`——而且这些副本已经出现分歧：`VCPU_SBI_EXT_BASE` 的条目被注册在 `KvmRiscvSbiExtId::KvmRiscvSbiExtMax` 哨兵值之下，导致 `ext_status` 永远没有 BASE 的槽位，而哨兵槽位被复用。

**修复。** 按枚举顺序存放扩展（或者通过枚举而非并行索引来查找），并让长度由 `SBI_EXT_TABLE.len()` 推导，这样条目就不会漂出自己的槽位。

**与正确性章节第 247 行的评论共享同一修法**（`Wrong lookup key`）：两者都源于"一个扩展被标识两次"——应该用它自身携带的身份（`extid_start..=extid_end`）去查找，并让 `ext_status` 使用同一个槽位。

### `ostd/src/arch/riscv/vm/timer.rs` line 55

> ```diff
> +    pub(super) fn call_next_event(&self, next_cycle: usize) {
> +        let func = self.timer_next_event.unwrap();
> +        func(next_cycle);
> +    }
> ```

`closure-fn-suffix`（minor）：`let func = self.timer_next_event.unwrap();` 给一个可调用对象起了个读起来像数据的名字；同样的命名还出现在 `ostd/src/arch/riscv/vm/sbi/mod.rs` 与 `ostd/src/arch/riscv/vm/insn/mod.rs` 的 `func`/`handler` 局部变量上。

**修复。** 改名为 `next_event_fn`（以及 `handler_fn`、`csr_insn_fn`……），让每个位置都表明这里是在调用一个函数指针。

### `ostd/src/arch/riscv/vm/vcpu.rs` line 354

> ```diff
> +        if irq == u32::MAX {
> +            self.irqs_pending |= 0x1<<10;
> +        } else {
> +            self.irqs_pending &= !(0x1<<10);
> +        }
> +        self.irqs_pending_mask |= 0x1 <<10;
> ```

`no-magic-number`（minor）：`set_interrupt` 用裸字面量 `0x1<<10` 在三处编码 HVIP 的 VS 外部中断位，而同一文件的 `sync_interrupts` 用的是具名形式 `0x1 << IRQ_VS_SOFT`；这个位号属于硬件契约，读者在调用点无从核实。

**修复。** 在 `ostd/src/arch/riscv/vm/types.rs` 中 `IRQ_VS_SOFT` 旁边加上 `const IRQ_VS_EXT: usize = 10;`，并在该处使用 `1 << IRQ_VS_EXT`。

### `test/initramfs/nix/initramfs.nix` line 17

> ```diff
> +  qemu = builtins.path { path = ./../src/qemu-system-riscv64; };
> +  kernel = builtins.path { path = ./../src/Image; };
> +  initramfs = builtins.path { path = ./../src/rv.cpio.gz; };
> ```

干净检出上构建会失败（major）：`qemu`、`kernel` 和 `initramfs` 被无条件地从 `./../src/qemu-system-riscv64`、`./../src/Image` 和 `./../src/rv.cpio.gz` 求值，但这些路径是被 `.gitignore` 忽略的构建产物，因此在任何尚未在本地构建过它们的干净检出上，共享的 initramfs derivation 都会失败。

**修复。** 用显式选项配合 `lib.optionalString` 把 KVM 测试负载关起来（就像紧随其后的 `benchmark`/`conformance` 那样处理），或者把它们做成正规的 Nix derivation，而不是要求 `test/initramfs/src/` 下存在未纳入版本控制的文件。

## Correctness（正确性）

### `kernel/core/src/device/kvm/vcpu_file.rs` line 64

> ```diff
>             GetStatsFd => {
>                 // Currently, this syscall is invoked, but the fd is not used.
>                 Ok(10)
>             }
> ```

凭空捏造的 fd（minor）：`KVM_GET_STATS_FD` 直接返回常量 `10`，仿佛它是一个新建的描述符，但实际上没有分配任何描述符。若 VMM 采信这个返回值（或者在其常规的 fd 记账中关闭这个收到的 fd），它操作的将是调用方 fd 表中恰好占据 10 号的那个无关文件。

**修复。** 要么在 stats 实现之前直接拒绝该 ioctl（`return_errno_with_message!(Errno::ENOTTY, "KVM_GET_STATS_FD is not supported")`），要么创建一个真正的匿名文件，像 `CreateVm`/`CreateVcpu` 那样插入调用方的文件表，并返回该 fd。

**与安全性章节同一行的评论共享同一修法**（`Returns an unallocated file descriptor`）：只需一处修复——经 `CreateVm`/`CreateVcpu` 的路径安装真实的 fd，或者直接让 ioctl 以 `ENOTTY`/`ENOSYS` 失败；绝不要交回一个常量槽位号。

### `kernel/core/src/kvm/arch/riscv/mmio.rs` line 201

> ```diff
>     /// Get the rs2 reg_val of RVC insn
>     fn get_rs2s(&self, ctx: &VcpuContext) -> Result<usize> {
>         Ok(ctx.get_reg((((self.rvc_rs2s() << 7) << 3) & REG_MASK) / size_of::<usize>())?)
>     }
> ```

寄存器索引错误（major）：在 `get_rs2s` 中，寄存器编号先左移 `7` 再 `<< 3`，因此 `(rvc_rs2s() << 7) << 3` 等于 `rvc_rs2s() << 10`；用它去与 `REG_MASK`（`0xF8`）相与永远是 `0`，于是每次压缩存储都读取寄存器 `x0`，把硬编码的 `0` 当作 MMIO 数据写出去。（`C.SD`/`C.SW` 的源寄存器在 `rs2' = 8 + bits[4:2]`，这正是 `rvc_rs2s()` 已经返回的值；而 `C.SDSP`/`C.SWSP` 也被送进了 `get_rs2s`，尽管它们的 `rs2` 是位于第 6:2 位的 5 位字段。）

**修复。** 让寄存器编号与 `get_rs2` 处于同一量纲（那里是对已经乘过 `REGBYTES` 的值套掩码）：使用 `((self.rvc_rs2s() << 3) & REG_MASK) / size_of::<usize>()`，或者干脆 `ctx.get_reg(self.rvc_rs2s())?`。对 SP 相对的形式，应解码 5 位字段（`(self.insn >> 2) & 0x1f`）而不是用 `rvc_rs2s()`。

**与硬件章节同一行的评论共享同一修法**（`Wrong register number for compressed stores`）：同一处缺陷，同一行修复。

### `kernel/core/src/kvm/arch/riscv/vcpu.rs` line 251

> ```diff
>         match trap.get_trap_type() {
>             EXC_VIRTUAL_INST_FAULT => { ... }
>             EXC_INST_GUEST_PAGE_FAULT | EXC_LOAD_GUEST_PAGE_FAULT | EXC_STORE_GUEST_PAGE_FAULT => { ... }
>             EXC_SUPERVISOR_SYSCALL => { ... }
>             _ => todo!("VcpuArch exit: Trap({:#x}) is not impled!", trap.get_trap_type()),
>         }
> ```

可达的 panic（major）：`exit` 对它处理的五种陷入原因之外的所有情况都会 panic，而 `hedeleg`（`KVM_HEDELEG_DEFAULT = 0xB30D`）只把原因 0、2、3、8、9、12、13、15 委托给 guest。guest 能触发却未被委托的原因——例如加载/存储地址未对齐（4/6）或加载/存储访问异常（5/7）——会被投递给 HS，从而命中这个 `todo!()`，于是 guest 执行一次未对齐访问就会杀掉宿主内核，而不是让异常反射回 guest 自身。

**修复。** 绝不在 guest 驱动的路径上 `todo!()`：把未处理的陷入反射回 guest（设置 `vscause`/`vstval`/`vsepc` 并在 `vstvec` 继续，就像 guest page fault 路径所做的那样），对真正不可达的状态使用 `Err(...)`，让调用方向 VMM 返回 `errno` 而不是 panic。

**与其他宿主 panic 类评论共享同一修法**（`kernel/core/src/kvm/arch/riscv/vcpu.rs` 第 110、223 行，`ostd/src/arch/riscv/vm/insn/csr.rs` 第 53 行，`ostd/src/arch/riscv/vm/timer.rs` 第 55 行）：这些路径都不得对 guest 或 VMM 选定的输入 panic——每一条都应转化为给 VMM 的 `Err(...)`/`errno`，或注入 guest 的异常。

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

资源永不释放（minor）：`create_vcpu` 把一个强引用 `Arc<Vcpu>` 存进 `Vm::vcpus`，而没有任何代码会移除它，因此关闭 VCPU 文件描述符只是丢掉了 `VcpuFile` 那个引用：`Vcpu`（连同它的 `VcpuArchInner`、timer 与 SBI 上下文）会一直活到整个 `Vm` 消失，`Drop for Vcpu`（那条 `release VCPU` 追踪）永远不会执行，而针对该 id 的 `KVM_CREATE_VCPU` 即便在用户空间关闭了先前的 VCPU 之后也仍然以 `EEXIST` 失败。

**修复。** 不要让这张表保持 vcpu 存活：改存 `Weak<Vcpu>`，并把无法 upgrade 的条目不视作占用；或者在 `Drop for Vcpu` 中移除该条目（在那里 upgrade 存下的 `Weak<Vm>` 并获取 `Vm::vcpus`），这样最后一次关闭 fd 才真正释放 VCPU 并释放它的 id。

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

被吞掉的分配失败（minor）：分配失败被报告成成功——`alloc_page` 返回 `(0, 0)`，而 `GStagePageTable::new` 回退到 root/root_paddr 全零的表。这些零随后被当作地址和指针使用：`map_page` 通过全零的 `new_table` 指针写入（对空指针调用 `write_bytes`），并为物理地址 0 安装一个叶子 PTE；`get_root()` 又给 `hgatp` 一个物理地址为 0 的 G-stage 根，于是 guest 在页表是伪造的情况下被进入，而不是让 `KVM_CREATE_VM`/`KVM_SET_USER_MEMORY_REGION` 以错误失败。

**修复。** 让这些路径可失败并让错误向上传播（`GuestPhysMemSpace::new` 与 `VmMemory::new` 本就返回 `Result`）：`fn alloc_page(&mut self) -> Result<(u64, u64)>` 返回例如 `Err(Error::NotEnoughResources)`，并让 `GStagePageTable::new() -> Result<Self>` 返回 `alloc_segment` 的错误而不是那张全零的表。

**与第 144 行的 `Null pointer dereference` 评论共享同一修法**（Security）：两个症状都是同一个 `(0, 0)` 哨兵值，因此一处改动——可失败的 `alloc_page`/`GStagePageTable::new`，经 `GuestPhysMemSpace::new` 传到 `VmMemory::new`——同时修复两者。

### `ostd/src/arch/riscv/vm/insn/csr.rs` line 53

> ```diff
> fn csr_insn(_insn: usize) -> KvmInsnReturn {
>     todo!("CSR insn emul is not impled!")
> }
> ```

可达的 panic（major）：`csr_insn` 是注册在 `SYSTEM_OPCODE_FUNCS` 中全部六种 `csr*` 形式上的处理函数，却是一个 `todo!()`，因此任何触发虚拟指令异常的 CSR 指令都会 panic 宿主。这是 guest 可达的：`enable_virtualization_cpu` 设置 `Hcounteren = 0x02`（只允许 `time`），所以 guest 执行 `rdcycle`/`rdinstret`（或访问任何未委托的 CSR 如 `hstatus`）都会以 `EXC_VIRTUAL_INST_FAULT` 陷入 HS，指令在 `stval` 中，随后由 `VcpuArch::virtual_insn` 走到 `handle_system_opcode_insn` → `csr_insn`。

**修复。** 实现该 CSR 访问的模拟（至少覆盖 `Hcounteren`/`Scounteren` 允许的计数器），或者返回 `KvmInsnReturn::KvmInsnVirtualTrap`/`KvmInsnIllegalTrap`，并让 `VcpuArch::handle_insn_return`（它目前对这些取值同样 `todo!()`）把异常注入 guest，从而使 guest 的 CSR 访问永远不会 panic 宿主。

**与其他宿主 panic 类评论共享同一修法**（`ostd/src/arch/riscv/vm/timer.rs` 第 55 行，`kernel/core/src/kvm/arch/riscv/vcpu.rs` 第 110、223、251 行）：整条模拟路径只遵循一条规则——每个未实现的分支都要有确定的、不 panic 的出口。

### `ostd/src/arch/riscv/vm/isa.rs` line 93

> ```diff
> static KVM_ISA_EXT_ARR: [IsaExtensions; KvmRiscvIsaExtId::MAX as usize] = [
>     // single
>     IsaExtensions::A,          // KVM_RISCV_ISA_EXT_A
>     ...
>     IsaExtensions::M,          // KVM_RISCV_ISA_EXT_M
>     IsaExtensions::V,          // KVM_RISCV_ISA_EXT_V   <-- index 7, but V is id 15
> ```

错误的表映射（major）：`KVM_ISA_EXT_ARR` 以 KVM 扩展 id 为索引（`kvm_riscv_isa_check_host` 中的 `KVM_ISA_EXT_ARR[kvm_ext]`，而 `kvm_riscv_vcpu_setup_isa` 把同一个 `i` 既当 id 又当 ISA 位图的位号），因此它的条目必须按 `KvmRiscvIsaExtId` 的顺序排列。可它们却按 Linux 源码顺序排列（Linux 用的是指定初始化器），于是每个 id 都错位了：`IsaExtensions::V` 落在索引 7，而 `V` 的 id 是 15；`SVPBMT`（id 7）在索引 18、`SSTC`（id 8）在索引 13，依此类推。因此每个 id ≥ 7 的扩展都在用错误的宿主扩展去测试和上报，例如 guest 的 ISA 第 8 位（SSTC）是依据 `SMNPM`/`SSNPM` 的存在情况来设置的，而第 7 位（SVPBMT）则依据 `V`。

**修复。** 把每个条目放到它对应的 `KvmRiscvIsaExtId` 位置（或者从 id 构建该表，例如在 `const`/`OnceCell` 中做 `match`，使 index == id），并删掉那些逐行的 `// KVM_RISCV_ISA_EXT_...` 注释，或让它们与实际位置一致。

**与可维护性章节同一行的 `dry` 评论共享同一修法**：重排序与"只保留一种有序表示"的重构是同一次改动。

### `ostd/src/arch/riscv/vm/onereg/core.rs` line 60

> ```diff
>         match reg_num {
>             KvmCore::PC => Ok(self.guest_context.get_sepc()),
>             KvmCore::MODE => Ok((self.guest_context.get_sstatus() & SR_SPP == 1) as usize),
>             KvmCore::PC..=KvmCore::T6 => Ok(self.guest_context.get_reg(reg_num)?),
> ```

错误的谓词（major）：`get_sstatus() & SR_SPP == 1` 先用 `SR_SPP`（`0x100`）取掩码，又把结果与 `1` 比较，因此该表达式恒为 `false`，core 的 `MODE` 寄存器永远读出 `0`（用户态），即使 VCPU 正处于特权态。任何通过 `KVM_GET_ONE_REG` 保存/恢复或检查 VCPU 状态的 VMM 都会拿到错误的特权级。

**修复。** 检测该位本身，而不是把掩码后的值与 `1` 比较（并保留括号，避免 `!= 0` 被 `&` 吞掉）：
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

错误的位掩码（major）：`sstatus & SR_SPP` 只保留 `SR_SPP` 这一位，清掉了 guest `sstatus` 的所有其他字段（`SIE`、`SPIE`、`FS`、`SUM`……），因此通过 `KVM_SET_ONE_REG` 写 `MODE = 0` 会悄悄摧毁 VCPU 已保存的状态，而不是只清除 `SR_SPP`。

**修复。** 只清这一位：
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

可达的 panic（critical）：`get_general_csr` 把用户提供的 CSR 编号直接转交给 `KvmVcpuCsr::get_csr`，而后者在固定的 11 元素 slice（`self.slice.slice[index]`）上索引且不做任何边界检查，并且永远返回 `Some(..)`，所以这里的 `unwrap` 什么也保护不了。一次 `KVM_GET_ONE_REG`（或 `KVM_SET_ONE_REG`），只要 `regs.id` 带有 `KVM_REG_RISCV_CSR` 类型字段、general 子类型且编号 `>= 11`，就会 panic 内核：这些掩码只清掉 arch/size 字段和第 24-25 位，因此像 `0x400`（子类型 0，`reg_num = 1024`）这样的 `reg_num` 会抵达 `get_csr`/`set_csr` 并越界索引。

**修复。** 对 CSR 编号做边界检查并传播真实错误，而不是 unwrap，例如：
```rust
fn get_general_csr(&self, reg_num: usize) -> Result<usize> {
    match reg_num {
        KvmVcpuCsr::SIP => Ok(sip_value),
        _ => self.guest_csr.get_csr(reg_num).ok_or(Error::NotFound),
    }
}
```
并让 `KvmVcpuCsr::get_csr`/`set_csr` 拒绝越界索引（`self.slice.slice.get(index)` 以及 `set_csr` 中的范围检查），使 `set_general_csr` 这条路径让 ioctl 失败而不是 panic。

**与同一对函数上的 `validate-at-boundaries` 评论共享同一修法**（Security，`ostd/src/arch/riscv/vm/onereg/csr.rs` 第 57 行）：一次校验同时覆盖两者——在 ioctl 边界（`Vcpu::get_onereg`/`set_onereg`）检查一次 `reg_num`，用 `Err(Error::NotFound)` 拒绝越界编号（Linux 返回 `EINVAL`），并让 `KvmVcpuCsr::get_csr`/`set_csr` 使用 `slice.get(index)`/返回错误而不是直接索引。

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

错误的查找键（major）：`vcpu_get_sbi_ext_idx` 把 guest 放进 `a7` 的值（`handle_sbi_call`）拿去与 `ext_idx` 比较，而 `ext_idx` 是内部 KVM 索引（`0..=14`），同时也被用来索引 `ext_status`；真正的扩展 id 存放在 `extid_start`/`extid_end` 中，而这两个字段在任何地方都没有被读取过。因此所有现代 SBI 扩展 id 都匹配不上（`SBI_EXT_TIME` = `0x54494D45`、`SBI_EXT_BASE` = `0x10`、`SBI_EXT_STA`、`SBI_EXT_FWFT`……），所有 guest SBI 调用都落入错误分支并以 `SBI_ERR_NOT_SUPPORTED` 作答；而旧式 id `0..=8` 则因巧合而匹配上并被路由到不相关的处理函数。

**修复。** 用条目自身携带的 id 区间来选取：
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

**与可维护性章节第 188 行的评论共享同一修法**（`dry`）：用真实的 id 区间来定位条目、并让 `ext_status` 使用同一个槽位，既能消除错路由，也能消除手工维护的 `ext_idx`——包括被挂在 `KvmRiscvSbiExtMax` 哨兵值上的 BASE 条目。

### `ostd/src/arch/riscv/vm/timer.rs` line 55

> ```diff
> 	pub(super) fn call_next_event(&self, next_cycle: usize) {
> 		let func = self.timer_next_event.unwrap();
>     	func(next_cycle);
> 	}
> ```

可达的 panic（major）：只有当宿主宣称 `Sstc` 时 `timer_next_event` 才会被赋值（见 `init`），因此在没有该扩展的 CPU 上该字段保持 `None`，这个 `unwrap` 就会 panic 内核。这条路径对 guest 可达：由 `handle_sbi_call` 处理的 ecall 会走到 `sbi_ext_time_handler`，后者用 guest 请求的到期时间调用 `call_next_event`，于是在这样的宿主上，guest 编程自己的 timer 就会使宿主崩溃，而不是得到确定的失败。

**修复。** 不要 unwrap：给 `call_next_event` 一个可失败或带标志返回的签名（或检查 `sstc_enabled`/`init_done`），在没有 timer 后端时让 `sbi_ext_time_handler` 返回 `SBI_ERR_NOT_SUPPORTED`；或者在调用之前补上非 Sstc 的回退路径（编程一个宿主 timer）。

**与其他宿主 panic 类评论共享同一修法**（`ostd/src/arch/riscv/vm/insn/csr.rs` 第 53 行，`kernel/core/src/kvm/arch/riscv/vcpu.rs` 第 110、223、251 行）：同一条规则——未实现的功能必须让调用失败，而不是让宿主崩溃。

## Security（安全性）

### `kernel/core/src/device/kvm/vcpu_file.rs` line 64

> ```diff
>             GetStatsFd => {
>                 // Currently, this syscall is invoked, but the fd is not used.
>                 Ok(10)
>             }
> ```

返回未分配的文件描述符（minor）：`KVM_GET_STATS_FD` 返回常量 `10` 却没有在调用方的文件表里安装任何东西，于是用户空间收到的这个描述符编号实际上指向恰好占据 10 号槽位的那个文件（或者根本没有文件）。采信该返回值的 VMM——正如它在 Linux 上所做的那样，那里这个 ioctl 会安装一个全新的 anon-inode fd——会去 `read`/`ioctl`/`close` 一个无关的已打开文件，而关闭它可能让进程中并发的 `open` 悄悄丢失自己的描述符。注释里"该 fd 未被使用"的说法并不能消除这个隐患。

**修复。** 要么安装一个真实的文件（照 `CreateVcpu`/`CreateVm` 的路径把 anon-inode 文件插入当前文件表并返回该 fd），要么在 stats 接口存在之前让该 ioctl 以 `Errno::ENOTTY`/`ENOSYS` 失败。

**与正确性章节同一行的评论共享同一修法**（`Fabricated file descriptor`）：只需一处修复——经 `CreateVm`/`CreateVcpu` 的路径安装真正的匿名文件，或者干脆让该 ioctl 失败。

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

越界写入用户缓冲区（minor）：这两个 one-register ioctl 总是向调用方地址搬运一个 `usize`（8 字节）——此处 `current_userspace!().write_val(regs.addr, &reg_val)`，第 74 行 `current_userspace!().read_val(regs.addr)`——完全忽略 VMM 编码在 `regs.id` 中的大小。对于一个 `KVM_REG_SIZE_U32` 寄存器（本内核自己就会宣告这类寄存器：任何 `KVM_REG_RISCV_FP_F` 寄存器，见 `copy_fp_f_reg_vec` 用 `KVM_REG_SIZE_U32` 构造其 id），Linux 的 ABI 允许调用方传入 4 字节缓冲区，而本内核会写到它末尾之后 4 字节（对称地，`KVM_SET_ONE_REG` 会多读 4 字节，把相邻的 VMM 内存当作寄存器值）。

**修复。** 遵守寄存器 id 中编码的大小：对 `KVM_REG_SIZE_U32` 截断为 `u32` 并且只 `write_val`/`read_val` 4 字节，对 `KVM_REG_SIZE_U64` 保持 8 字节，拒绝大小字段既非前者也非后者的 id。

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

`validate-at-boundaries`（major）：`run()` 依据从 vCPU 的 `kvm_run` 页读出的 `exit_reason` 做分发（`run.read_run_val::<u32>(KVM_RUN_EXIT_REASON_OFFSET)`，偏移 8，第 108 行），但该页正是 VMM 自己对 vCPU fd 的可写 mmap（`VcpuFile::mappable` 返回 `KvmRun` 的 VMO），所以这个值由用户空间提供且未经校验。一个在偏移 8 写入 `KVM_EXIT_RISCV_SBI`（35）或 `KVM_EXIT_RISCV_CSR`（36）再调用 `KVM_RUN` 的 VMM，就能把内核驱入 `todo!()` 并使其 panic。内核自身从不产生这两种 exit reason——写入该字段的只有 `KVM_EXIT_MMIO`（第 187、212 行）和 `KVM_EXIT_UNKNOWN`（第 120、232 行）——因此没有任何内部逻辑依赖该字段取 35/36。

**修复。** 把这个字段当作不可信输入：只接受内核可能产生的取值（目前是 `KVM_EXIT_MMIO`，即 MMIO 返回路径，以及 `KVM_EXIT_UNKNOWN`），对其余取值返回例如 `Errno::EINVAL` 的错误，直到 SBI/CSR 返回路径被实现，而不是对它 `todo!()`。

**与其他宿主 panic 类评论共享同一修法**（`ostd/src/arch/riscv/vm/insn/csr.rs` 第 53 行，`ostd/src/arch/riscv/vm/timer.rs` 第 55 行，`kernel/core/src/kvm/arch/riscv/vcpu.rs` 第 223、251 行）：把每个由 guest 或 VMM 控制的选择子都当作不可信输入，并给它确定的、不 panic 的出口。

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

可达的 panic（major）：`gstage_page_fault` 只实现了 load 与 store 两种 guest page fault，遇到其他情况就 `todo!()`，而它的调用方会把三种原因路由到这里：`EXC_INST_GUEST_PAGE_FAULT | EXC_LOAD_GUEST_PAGE_FAULT | EXC_STORE_GUEST_PAGE_FAULT => ... ret = self.gstage_page_fault(run, trap)`。guest 自己决定 PC，因此一次取指落在没有有效 G-stage 条目的 guest 物理地址上（例如跳转到已映射 RAM 区域之外——VMM 只映射内存 slot，而且每个被映射的页都被强制加上 `PTE_X`）时，会以 `EXC_INST_GUEST_PAGE_FAULT`（20）抵达 `_ => todo!(...)` 分支并 panic 宿主内核：guest、或者仅仅是一个 guest 的缺陷，就成了宿主的 DoS。相邻的 `exit` 兜底分支（`_ => todo!("VcpuArch exit: Trap({:#x}) is not impled!", ...)`，第 251 行）对于任何未被 `KVM_HEDELEG_DEFAULT` 委托给 VS 的陷入也是同样的形态。

**修复。** 显式处理 `EXC_INST_GUEST_PAGE_FAULT`（把取指当作一次 MMIO/取指 exit 交给 VMM 模拟，或注入相应的 guest 异常），并把 `gstage_page_fault`/`exit` 中的兜底 `todo!()` 换成真实处理或错误返回，使意外陷入无法 panic 宿主。

**与其他宿主 panic 类评论共享同一修法**（`kernel/core/src/kvm/arch/riscv/vcpu.rs` 第 110、251 行，`ostd/src/arch/riscv/vm/insn/csr.rs` 第 53 行，`ostd/src/arch/riscv/vm/timer.rs` 第 55 行）：整条运行路径只遵循一条约定——未实现的情况要么向 VMM 返回 `Err(...)`，要么向 guest 注入异常。

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

数组越界索引（minor）：扁平寄存器视图用绝对索引去索引各个 slice 数组，但这些数组只和自身字段一样长：`regs_1` 是 `[usize; 35]`，所以 `0..35 => regs_1[index]` 没问题；然而 `35..68 => regs_2[index]` 索引的是 `[u32; 33]`、`68..100 => regs_3[index]` 索引的是 `[u64; 32]`、`101..164 => regs_5[index]` 索引的是 `[u64; 64]`（`set_reg` 的第 107、108、110 行同样如此）。这些分支里的每个索引都超出了数组末尾，因此只要不减去偏移量，它们全都会 panic（永远无法返回寄存器值）；今天的调用方（`onereg/core.rs` 与 `mmio.rs`）只传 0..=31，所以这个缺陷目前是潜伏的，但对这些 `pub` API 的任何新调用方都是一颗地雷。与之相关的是，没有任何东西把这个视图钉死它所覆盖的布局（`VcpuContextReg` 与 `FFpuContext`/`DFpuContext`/`QFpuContext`，后者位于另一个模块且可能被独立修改），因此这个 union 的类型双关可能悄悄漂移。

**修复。** 用各自 slice 内的偏移来索引（`regs_2[index - 35]`、`regs_3[index - 68]`、`regs_5[index - 101]`，`set_reg` 中相应修改），或者把整个保存区建模成单个数组；并加入编译期检查确保两个视图一致，例如 `const _: () = assert!(size_of::<VcpuContextReg>() == size_of::<VcpuContextSlice>() && align_of::<VcpuContextReg>() == align_of::<VcpuContextSlice>());`。

**与硬件章节同一行的评论共享同一修法**，以及可维护性章节第 90 行的 `no-magic-number` 评论：按子数组做 `index - base` 偏移、把布局命名一次、再加上布局断言——一次编辑，覆盖三条评论。

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

`justify-unsafe-use`（minor）：本文件中的十个 `unsafe` 块没有一个带有 `// SAFETY:` 注释（第 74-76、136-137、143-148、148-150、160-161、164-166、196-197、206-211 行），尽管它们全都是从页表内容派生出的裸指针访问——恰恰是需要把可靠性论证（该指针是 `paddr_to_vaddr` 映射过的表页、`idx < 512`、PTE 有效、帧分配器成功）写下来的地方。文件头声称该代码不符合编码规范，并不豁免这一要求；而缺少这些论证，正是第 202 行的错误地址缺陷不易被读者发现的原因之一。

**修复。** 为每个 `unsafe` 块补上 `// SAFETY:` 注释，说明指针为何有效、为何在界内（涉及多个条件时使用编号列表形式）。

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

空指针解引用（minor）：`alloc_page` 用返回 `(0, 0)` 来表示失败，而 `map_page` 不加检查地消费它：`let new_table = new_table_va as *mut u64; core::ptr::write_bytes(new_table, 0, PAGE_SIZE / 8);`——一旦帧分配器耗尽，这就是对虚拟地址 0 的一次存储（内核 panic）。这并非纯理论问题：VMM 可以申请任意大的 guest 内存区域（`KVM_SET_USER_MEMORY_REGION` 没有大小上限），而 `map_guest_memory` 逐页遍历，每 512 页分配一个中间 G-stage 表并固定一个用户帧，因此 VMM 能把分配器逼到失败。`GStagePageTable::new` 也有同样的静默失败形态（第 83 行构出 `root_paddr: 0, root: 0`，随后 `get_root()` 把 0 喂给 `update_hgatp`），把一次 OOM 变成一张伪造的 guest 页表而不是一个错误。

**修复。** 传播失败：让 `alloc_page` 与 `map_page`/`new` 返回 `Result`/`Option`，以例如 `Err(Error::NotEnoughResources)` 失败，而不是通过空指针或零指针写入（并让 `GuestPhysMemSpace::new` 把该错误上抛给 `VmMemory::new`）。

**与第 94 行的 `Swallowed allocation failure` 评论共享同一修法**（Correctness）：修复就是那里描述的、唯一那条可失败的 `alloc_page`/`GStagePageTable::new` 路径。

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

把物理地址当指针使用（critical）：`GStagePageTable::unmap_page` 在遍历页表时没有把下一级表的地址转回虚拟地址：`let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT; ... table = next_table as *mut u64;`。PTE 里存的是**物理**地址，而 `map_page` 在使用前会做转换（`table = paddr_to_vaddr(next_table as usize) as *mut u64;`，第 155 行）；这条路径没有。由于 `paddr_to_vaddr` 会加上 `LINEAR_MAPPING_BASE_VADDR`（`ostd/src/mm/kspace/mod.rs`），根级之下的每一次访问——`table.add(idx)`、`read_volatile`，以及本应清除叶子 PTE 的最后那次 `write_volatile(pte_ptr, 0)`——都打在了一个无关的内核/用户虚拟地址上，而不是页表页。后果有二：(a) 一次指向错误地址的 8 字节零写入，即内核内存被破坏；若该地址未映射则是内核页错误（panic）；(b) guest 物理 PTE 实际上从未被清除，于是 `KVM_SET_USER_MEMORY_REGION` 删除或替换 slot 并不会吊销 guest 的映射——旧 slot 的 `UFrame` 紧接着就被释放，因此后台帧可能回到宿主分配器，而 guest 仍持有指向它的有效 G-stage 翻译，可以读写被回收的宿主内存。`unmap_page` 也从不检查 `PTE_V`，因此对一个没有有效条目的 GPA 做 unmap 会顺着零 PTE 取出的地址继续下降。

**修复。** 完全按照 map 路径做地址转换，并在 PTE 无效时拒绝下降：
```rust
let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT;
if pte & PTE_V == 0 {
    return Ok(());
}
table = paddr_to_vaddr(next_table as usize) as *mut u64;
```
（`tables_to_free` 在这里同样是死代码；要么用它释放已清空的表，要么删掉它。）

**与硬件章节关于同一函数的评论共享同一修法**（第 199 与 202 行）：有效性检查与 `paddr_to_vaddr` 转换都应属于对这条遍历的同一次重写。

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

guest-stage 翻译缺少远端失效（major）：清除叶子 PTE 之后代码只发出一条裸的 `hfence.gvma`，它仅在**当前 hart** 上（且仅针对 `hgatp` 中的 VMID；`ostd/src/arch/riscv/vm/vcpu.rs` 的 `update_hgatp` 始终装入 VMID 0）失效 G-stage 翻译。G-stage 表由该 VM 的所有 vCPU 共享，其他 hart 可能正在并发地为该 VM 执行 `KVM_RUN`，因此没有任何东西使它们缓存的翻译失效，而远端 fence 也没有实现（RFENCE/HVIP 的 IPI SBI 处理函数都是桩）。具体的交错时序：vCPU A 在 hart 1 上运行 guest 代码并访问 GPA X（G-stage TLB 条目 GPA X -> HPA Y）；hart 0 执行 `KVM_SET_USER_MEMORY_REGION` 替换或删除该 slot，于是 `unmap_guest_memory` 清除 PTE 并只刷新 hart 0，随后释放该 slot 的 `UFrame`，宿主分配器可能把 HPA Y 交给别人；hart 1 那条陈旧条目仍把 GPA X 翻译到被复用的帧上，于是 guest 可以读写宿主内存。`map`（同样会改变 GPA->HPA 绑定）甚至不刷新本地 hart。

**修复。** 每一次 guest 内存的 map/unmap 变更都要在该 VM 正在运行的各 hart 上传播远端 G-stage fence（例如经 SBI RFENCE 路径广播 IPI/`hfence.gvma`）——并且要在旧帧被释放之前完成；同时让 `map` 和 `unmap` 一样做刷新。

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

`validate-at-boundaries`（major）：用户空间通过 `KVM_GET_ONE_REG`/`KVM_SET_ONE_REG` 提供的寄存器 id（`VcpuFile::ioctl` -> `Vcpu::get_onereg`/`set_onereg` -> `VcpuArchInner::get_one_reg`/`set_one_reg`）一路到达 CSR 数组而没有任何边界检查。`VcpuArchInner::get_csr`/`set_csr` 剥掉 arch/size/type 字段后把 `reg_num` 交给 `get_general_csr`/`set_general_csr`，后者最终落到 `self.guest_csr.get_csr(reg_num).unwrap()`（第 181 行）与 `self.guest_csr.set_csr(reg_num, reg_val)`（第 198 行）；而它们在 `[usize; 11]` 数组上做 `self.slice.slice[index]`（第 57、62 行）。掩码保留了用户取值的第 0..15 位与第 32..51 位，因此 `KVM_REG_RISCV | KVM_REG_SIZE_U64 | KVM_REG_RISCV_CSR | 11`（索引 11 恰好是数组长度），或者任何更大的值如 `| 0x11`、或第 32..51 位中的某一位置位，都会越界索引并 panic 内核——任何持有 vCPU fd 的进程都能 DoS 宿主。注意 `KvmVcpuCsr::get_csr` 返回 `Option`，可见当初确实打算做边界检查，只是从未实现（它永远返回 `Some`）。同样未校验的输入还会命中针对 `KVM_REG_RISCV_CSR_AIA`/`_SMSTATEEN` 子类型的 `todo!()`（第 172-173、191-192 行），而按 Linux ABI 编写的 VMM 完全可能合法地发送它们。

**修复。** 在边界处校验索引并返回错误而不是 panic，例如
```rust
fn get_csr(&self, index: usize) -> Option<usize> {
    self.slice.slice.get(index).copied()
}
```
让 `set_csr` 对 `index >= 11` 跳过或返回错误，并让 `get_general_csr`/`set_general_csr` 把该情况与未实现的子类型一并映射为 `Err(Error::NotFound)`（Linux 对未知寄存器 id 回复 `-EINVAL`）。

**与同一对函数上的可达 panic 评论共享同一修法**（Correctness，`ostd/src/arch/riscv/vm/onereg/csr.rs` 第 181 行）：同一处边界检查同时服务两者——`get_csr` 用 `slice.get(index)`、`set_csr` 做范围检查、两个 `*_general_csr` 调用方以 `Err(Error::NotFound)` 上报。

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

可达的 panic（major）：`call_next_event` 里有 `let func = self.timer_next_event.unwrap();`，但只有当宿主 CPU 具备 `IsaExtensions::SSTC` 时 `VcpuTimer::init` 才会设置 `timer_next_event`；在任何没有 SSTC 的宿主上它保持 `None`（注释提到的高精度定时器回退路径目前并不存在）。这条路径可由 guest 选择输入触发：一条 ecall，其 `a7` 被 `SBI_EXT_TABLE` 分发到 `VCPU_SBI_EXT_TIME`，会进入 `sbi_ext_time_handler`（`ostd/src/arch/riscv/vm/sbi/time.rs`），后者调用 `arch.timer.call_next_event(arch.guest_context.get_a0())`——`a7` 和 `a0` 都由 guest 控制——因此 guest 能在任何没有 SSTC 的机器上 panic 宿主内核。

**修复。** 不要 unwrap：让 `call_next_event` 处理后端缺失的情况（例如 `let Some(func) = self.timer_next_event else { return; };` 外加一条显式的"不支持"路径），并且/或者在 `sstc_enabled` 为 false 时拒绝路由 SBI TIME 扩展（向 guest 返回 `SbiErrNotSupported`）。

**与其他宿主 panic 类评论共享同一修法**（`ostd/src/arch/riscv/vm/insn/csr.rs` 第 53 行，`kernel/core/src/kvm/arch/riscv/vcpu.rs` 第 110、223、251 行）：同一条规则适用于 guest 能触达的每一条未实现路径。

## Hardware（硬件）

### `kernel/core/src/kvm/arch/riscv/mmio.rs` line 201

> ```diff
>     /// Get the rs2 reg_val of RVC insn
>     fn get_rs2s(&self, ctx: &VcpuContext) -> Result<usize> {
>         Ok(ctx.get_reg((((self.rvc_rs2s() << 7) << 3) & REG_MASK) / size_of::<usize>())?)
>     }
> ```

压缩指令存储的寄存器编号错误（major）：`get_rs2s` 计算的是 `((self.rvc_rs2s() << 7) << 3) & REG_MASK`，即 `(reg << 10) & 0xF8`。任何值左移 10 位后，其第 7:3 位恒为零，因此该表达式对任何输入都求值为 0，该 helper 永远读取 `x0`（其值恒为 0）。于是 `store_decode` 的 `C.SW`、`C.SD`、`C.SWSP`、`C.SDSP` 分支会把 0 写进 `KVM_RUN_MMIO_DATA_OFFSET`，而不是源寄存器的值。对这些编码而言，寄存器编号恰好就是 `rvc_rs2s()`（第 4:2 位加 8）；只有非压缩路径才需要 `(insn >> 17) & REG_MASK` 这种提取方式，因为那里的 5 位字段位于第 24:20 位。

**修复。** 直接使用寄存器索引（这就是压缩形式的寄存器原语，即 `8 + rv_x(insn, SH_RS2C, 3)`）：
```rust
Ok(ctx.get_reg(self.rvc_rs2s())?)
```

**与正确性章节同一行的评论共享同一修法**（`Wrong register index`）：同一处缺陷，同一行修复。

### `ostd/src/arch/riscv/vm/context.rs` line 91

> ```diff
>                 0..35 => Ok(self.slice.regs_1[index]),
>                 35..68 => Ok(self.slice.regs_2[index] as usize),
>                 68..100 => Ok(self.slice.regs_3[index] as usize),
>                 100 => Ok(self.slice.regs_4 as usize),
>                 101..164 => Ok(self.slice.regs_5[index] as usize),
>                 164 => Ok(self.slice.regs_6 as usize),
> ```

数组索引越界（minor）：扁平索引的各个分支用扁平索引去索引寄存器保存区的子数组，而不是用该数组内的偏移：`35..68` 区间的 `regs_2[index]` 作用于 `[u32; 33]`、`68..100` 的 `regs_3[index]` 作用于 `[u64; 32]`、`101..164` 的 `regs_5[index]` 作用于 `[u64; 64]`。这些区间里的每个索引都越界，因此这些分支永远无法读出它们本应读取的 FP 字——第一次这样的调用就会在边界检查中 panic（`set_reg` 有同样的缺陷）。最后一个区间还少了一个：`101..164` 只覆盖 `fp_qctx.f` 的 64 个 `u64` 中的 63 个，而索引 164 被映射到 Q 的 `fcsr`，那其实是 101 之后的第 65 个字。

**修复。** 用各自子数组的偏移来索引，并补全最后一个区间（`set_reg` 中同样修改）：
```rust
35..68 => Ok(self.slice.regs_2[index - 35] as usize),
68..100 => Ok(self.slice.regs_3[index - 68] as usize),
101..165 => Ok(self.slice.regs_5[index - 101] as usize),
165 => Ok(self.slice.regs_6 as usize),
```

**与安全性章节同一行的评论共享同一修法**，以及可维护性章节第 90 行的 `no-magic-number` 评论：偏移修复与补全区间是同一次编辑；union 布局本身已核对过与 FFpu/DFpu/QFpu 上下文逐字节对齐，因此错的只是索引。

### `ostd/src/arch/riscv/vm/gstage.rs` line 199

> ```diff
>             let pte = unsafe { core::ptr::read_volatile(pte_ptr) };
> 
>             let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT;
>             tables_to_free[free_count] = table;
> ```

缺少页表有效性检查（major）：unmap 的遍历在没有检查 `PTE_V` 的情况下下降到下一级。对于一个不存在的条目，`pte & PTE_PPN_MASK` 为零，于是 `next_table` 为 0，下一轮迭代就解引用 `core::ptr::null_mut::<u64>().add(idx)`；任何 PPN 为零的条目（该级没有有效映射）都会得到同样的野指针，而不是干净的无操作或错误。`map_page` 在决定分配时是做了这个检查的（`if pte & PTE_V == 0`），因此 V=0 的条目恰恰是遍历可能遇到的状态，例如对一段并未完全填充的 GPA 区间做 unmap 时。

**修复。** 下降之前先确认条目有效，与 `map_page` 一致：
```rust
if pte & PTE_V == 0 {
    return Ok(()); // nothing mapped at this GPA
}
```

**与 `gstage.rs::unmap_page` 的其他评论共享同一修法**（本文件第 202 行，以及安全性章节同一行的评论）：对这条遍历只做一次重写——照 `map_page` 的做法，在 `pte & PTE_V == 0` 时提前返回，并在每次解引用之前用 `paddr_to_vaddr(...)` 转换。

### `ostd/src/arch/riscv/vm/gstage.rs` line 202

> ```diff
>             let next_table = ((pte & PTE_PPN_MASK) >> 10) << PAGE_SHIFT;
>             tables_to_free[free_count] = table;
>             free_count += 1;
>             table = next_table as *mut u64;
> ```

物理地址被当作虚拟指针（critical）：`unmap_page` 从 PTE 的 PPN 取出下一级表的地址——那是一个物理地址——并直接当作指针解引用（`table = next_table as *mut u64;`）。`map_page` 对同一个值用 `paddr_to_vaddr` 做了转换（第 155 行）；这里却把原始 PA（例如 `0x8020_1000`，远低于 `LINEAR_MAPPING_BASE_VADDR` = `0xffff_ffc0_0000_0000`）当内核 VA 使用，于是遍历会在调用进程恰好映射到那里的内容上读取页表节点，然后通过它写入 PTE。因此任何一次 `unmap`（例如对同一 slot 的第二次 `KVM_SET_USER_MEMORY_REGION`，或者经 `VmMemory::set_region` 以 `memory_size == 0` 删除区域）都会沿着伪造的指针走并破坏无关内存或触发异常。

**修复。** 完全照 `map_page` 的做法，把 PTE 中的物理地址转换为其线性映射 VA：
```rust
table = paddr_to_vaddr(next_table as usize) as *mut u64;
```

**与 `gstage.rs::unmap_page` 的其他评论共享同一修法**（上面第 199 行的 `Missing page-table validity check`，以及安全性章节关于同一行的评论）：一次遍历重写即可覆盖这三条。

### `ostd/src/arch/riscv/vm/switch.S` line 200

> ```diff
> 	/* Now we back to Host */
> 	.align 2
> 
> .Lkvm_switch_return:
> ```

`asm-prefer-balign`（nit）：这里用 `.align 2` 对齐 `.Lkvm_switch_return`，而该地址会在 `sret` 之前被写进 `stvec`，使下一次 guest 陷入在那里恢复执行。`.align` 的含义依赖于体系结构（在 RISC-V 上它表示 2 的幂），而硬件规范要求使用不含歧义的字节数指令。

**修复。** 改用字节数形式，表达同样的 4 字节对齐：
```asm
.balign 4
```

### `ostd/src/arch/riscv/vm/switch.S` line 211

> ```diff
> 	/* Return to rust code */
> 	ret
> ```

`asm-type-and-size`（minor）：`__kvm_riscv_switch_to` 带有 `.type __kvm_riscv_switch_to, @function` 却没有 `.size`。它是一个 Rust 可调用的函数（在 `ostd/src/arch/riscv/vm/vcpu.rs` 底部的 `extern "C"` 块中声明），因此规范里针对启动/陷入 trampoline 的豁免并不适用，该符号的范围对调试器而言是未定义的。

**修复。** 在最后的 `ret` 之后补上 size 指令（入口路径与 `.Lkvm_switch_return` 恢复路径是连续的）：
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

guest 的 FPU 状态从未被保存或恢复（major）：VM 进入/退出路径上没有任何代码触碰浮点状态：`swap_in_guest_state`/`swap_in_host_state`（以及 `enter_exit`）只交换 `scounteren`/`senvcfg`，而 `switch.S` 只保存通用寄存器。因此 `guest_context` 中由 `onereg/fp_f.rs`、`onereg/fp_d.rs` 读写的 `fp_fctx`/`fp_dctx` 保存区从未与硬件 FPU 之间做加载或存储（这条路径上不存在任何 `load_fpu_context_f`/`load_fpu_context_d` 调用）：对 F/D 寄存器的 `KVM_SET_ONE_REG` 永远到不了 guest，而 `KVM_GET_ONE_REG` 报告的是陈旧值。guest 真正的 FP 值只存在于 FPU 寄存器中，所以当 vCPU 线程被调度出去时（例如 `WFI` 模拟中的 `Task::yield_now()`，或者被宿主抢占），宿主的 FPU 上下文切换会覆盖它们，guest 恢复后拿到的是别的线程的 FP 值。

**修复。** 在切换前后针对该 guest 启用的扩展（`F`/`D`）保存/恢复 guest 的 FP 状态，方式与 `scounteren`/`senvcfg` 的交换相同。`FFpuContext`/`DFpuContext` 的 `save()`/`load()` helper 已存在于 `ostd/src/arch/riscv/cpu/context.rs`（它们包装了 `save_fpu_context_f/d`/`load_fpu_context_f/d`），但目前是私有的；把它们开放给 VM 模块并在此调用，例如：
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

每 hart 的 CSR 只在创建 VM 的那个 hart 上编程（major）：每次进入时的加载会重写每 hart 的 H 扩展 CSR `hedeleg`、`hideleg` 和 `henvcfg`，却漏掉了 `hcounteren`。`hcounteren` 只在 `enable_virtualization_cpu()`（`ostd/src/arch/riscv/vm/cpu.rs:9`）中被写入，而它只在执行 `CREATE_VM` 的那个 hart 上运行一次（`kernel/core/src/kvm/arch/riscv/vm.rs:11`）；vCPU 线程并未被固定到该 hart 上。在其他每个 hart 上 `hcounteren` 仍是 0，于是 guest 直接读取 `time`（VS 态 `rdtime`，或受 `hcounteren.TM` 门控的 S 态 `time` CSR）会触发异常而不是返回计数器值——尽管 `Hcounteren::write(0x02)` 的存在恰恰是为了让"VS 只应直接访问 time 计数器"。

**修复。** 在每一个可能运行 vCPU 的 hart 上编程 `hcounteren`，例如把它纳入每次进入的配置（添加 `Hcounteren` 的导入）：
```rust
fn config_load(&self) {
    Hdeleg::write(self.cfg.hedeleg);
    Hideleg::write(self.cfg.hideleg);
    Henvcfg::write(self.cfg.henvcfg);
    Hcounteren::write(0x02);
}
```
或者改为由"每 CPU 的硬件启用"钩子完成，而不是在 VM 创建时一次性调用。

## Documentation（文档）

### `Makefile` line 6

> ```diff
>  # Global build options.
> -TARGET_ARCH ?= x86_64
> +TARGET_ARCH ?= riscv64
>  BENCHMARK ?= none
> ```

文档仍把 x86_64 说成默认目标架构（minor）：`TARGET_ARCH` 现在默认是 `riscv64`，这使现有两处文档变得不实：`AGENTS.md` 第 42 行写着"将 `TARGET_ARCH` 设为 `x86_64`（默认）、`riscv64` 或 `loongarch64`"，`book/src/kernel/nix-development.md` 第 13 行说 ARM64 开发 shell"默认仍然构建 x86-64 内核"。现在一个普通的 `make kernel`/`make run_kernel` 会构建并运行 RISC-V 内核，因此照着这些文档操作的人会对自己构建出的东西产生误解。

**修复。** 如果新的默认值是有意为之，就在同一次改动里更新文档：把 `AGENTS.md` 第 42 行的默认值写成 `riscv64`，并相应改写 `book/src/kernel/nix-development.md` 的表述。如果这只是本地做 RISC-V 开发时的遗留（同一个提交还顺带把 `.vscode/settings.json` 切到了 RISC-V 目标），那就把它从该提交中移除。

**与可维护性章节同一行的 `atomic-commits` 评论共享同一修法**：同一个决定——要么保留 `riscv64` 默认值并修文档，要么把这个翻转从该提交中去掉。

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

`linux-compat-docs`（major）：本次改动在 `kernel/` 下新增了一个用户可见 API——`/dev/kvm` 字符设备及其 ioctl 族（`KVM_GET_API_VERSION`、`KVM_CREATE_VM`、`KVM_CHECK_EXTENSION`、`KVM_GET_VCPU_MMAP_SIZE`、`KVM_CREATE_VCPU`、`KVM_SET_USER_MEMORY_REGION`、`KVM_RUN`、`KVM_GET_ONE_REG`/`KVM_SET_ONE_REG`、`KVM_GET_REG_LIST`、`KVM_INTERRUPT`、`KVM_SET_MP_STATE`、`KVM_GET_STATS_FD`）——但 Linux 兼容性文档没有被触及。`book/src/kernel/linux-compatibility/syscall-flag-coverage/file-descriptor-and-io-control/ioctl.scml` 仍然只列举 tty/pty/DRM/evdev/block/TDX/socket 命令族，该页 `README.md` 的 `### ioctl` 小节也没有 KVM 条目，尽管 `book/src/kernel/linux-compatibility/README.md` 的 `ioctl` 一行正是把读者指向那份覆盖清单。于是文档中的 `ioctl` 覆盖度与用户空间实际可达的代码已经不一致。

**修复。** 在同一次改动中更新 `ioctl` 的 flag 覆盖文档。在 `book/src/kernel/linux-compatibility/syscall-flag-coverage/file-descriptor-and-io-control/ioctl.scml` 中增加一组命令与一条规则，参照现有的 `drm_ops` 与 TDX 条目：

```c
kvm_ops = KVM_GET_API_VERSION | KVM_CREATE_VM | KVM_CHECK_EXTENSION |
          KVM_GET_VCPU_MMAP_SIZE | KVM_CREATE_VCPU |
          KVM_SET_USER_MEMORY_REGION | KVM_GET_ONE_REG | KVM_SET_ONE_REG |
          KVM_GET_REG_LIST | KVM_INTERRUPT | KVM_SET_MP_STATE | KVM_RUN |
          KVM_GET_STATS_FD;

// Control Kernel-based Virtual Machine (KVM) devices
ioctl(fd, op = <kvm_ops>, ..);
```

然后在该页 `README.md` 的 `### ioctl` 小节中加入 KVM，并注明哪些部分目前只是被接受但尚未实现。

### `ostd/src/arch/riscv/vm/gstage.rs` line 34

> ```diff
> //       1.ostd/src/arch/riscv/vm/gstage.rs
> //       2.ostd/src/vm/mod.rs
> //       3.ostd/src/vm/gpm_space.rs
> //       4.ostd/src/vm/temp_riscv_gpm.rs
> //       5.kernel/src/kvm/vm_memory.rs(func map_guest_memory() & unmap_guest_memory())
> ```

删除清单中的文件路径已过时（minor）："若本文件将被删除需要检查的文件"这份清单指向 `kernel/src/kvm/vm_memory.rs`，但该路径并不存在：模块位于 `kernel/core/src/kvm/vm_memory.rs`（`kernel/src/` 下只有 `lib.rs`）。同一份清单还逐字重复出现在 `ostd/src/vm/temp_riscv_gpm.rs` 第 35 行，因此照着这份清单操作的维护者会被这两处一起指向一个不存在的文件。

**修复。** 把两份副本中的路径都改为 `kernel/core/src/kvm/vm_memory.rs`：

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

`semantic-line-breaks`（nit）：`KvmRiscvIsaExtId` 上的这段文档注释把一个句子拆到三行，同时又在行中间开启新句子：`... specific to KVM. This is not the same as the host ISA` / `extension IDs as that is internal to the host and should not be exposed` / `to the guest. This should always be contiguous ...`。

**修复。** 在句子边界处换行，例如：

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

`semantic-line-breaks`（nit）：这些文档行把两个句子挤在一行里——`/// This callback is optional. If provided, it will be called during`（第 87 行）与 `... any necessary initialization. Returns true ...`（第 89 行）——于是换行落在句子内部而不是句子边界上。紧邻的 `supported` 注释还漏了一个词：`the feature is assumed to` / `supported`（第 82-83 行）。

**修复。** 每行一句并修正措辞，例如：

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

`semantic-line-breaks`（nit）：本文件中有三段文档注释把句子边界放在行中间、又把句子其余部分折到下一行：`KvmRiscvSbiExtId`（第 46-48 行，`... specific to KVM. This is not the same as the SBI` / `extension IDs defined by ...`）、`handler`（第 157-159 行，``... a given extension or group of`` / ``extension. But it should always ...``）、以及 `init`（第 165-167 行，``... VCPU init/destroy. These`` / ``might be use if ...``）。

**修复。** 重新折行，使每行结束在句子或从句结束处，例如：

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

`semantic-line-breaks`（minor）：`irqs_pending`/`irqs_pending_mask` 的文档块（第 43-61 行）按固定列宽折行而不是按语义边界折行，于是句子的结尾落在行中间（`... represents bits changed in irqs_pending. Updates`、``... via `SET_INTERRUPT`. However, ...``、`... specific interrupt numbers,` / `however, ...`），而续写的词又从下一行开始。今后只要编辑其中一句，整段都会重排——这正是该规则想要避免的 diff 噪声。

**修复。** 按句子（及从句）边界折行，一行一个意思，例如：

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

新打包的测试资源没有任何文档（minor）：打包过程现在会把 `qemu-system-riscv64`、`Image` 和 `rv.cpio.gz` 从 `test/initramfs/src/` 复制到 `/kvm_test`（而且第 17-19 行的 `builtins.path` 使这些文件成为该 derivation 的必需输入），然而没有任何文档说明它们是什么、从哪里来。`test/initramfs/README.md` 仍在描述目录结构与添加配置文件的规定，却完全没提到它们，而 `.gitignore` 又把三者都隐藏了，因此一次全新的检出无法让开发者知道该往 `test/initramfs/src/` 里放什么。

**修复。** 在同一次改动中记录这个新负载：把 `kvm_test/` 与那三个 `src/` 输入加入 `test/initramfs/README.md` 的目录结构一节，并补一个简短小节说明 `qemu-system-riscv64`、`Image`、`rv.cpio.gz` 的来源，以及 RISC-V KVM 测试需要它们。
