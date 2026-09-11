# Implementation design

## 1. Baseline and goals

当前内置 bundle 由 [skill 资源目录](../../../../../assets/skill/) 提供，[模板注册模块](../../../../../src/templates.rs) 通过 `include_str!` 编译 `SKILL.md`、references 和 change skeleton。所有源文件都带 `<!-- doco:managed template=v1 -->`，该字段是文件归属/模板标记，不是根 skill bundle 版本。

[skill 初始化模块](../../../../../src/init/skill.rs) 目前逐文件调用 `update`：内容相同则跳过，受管内容不同时只有 `--refresh` 可以整体替换。[初始化计划模块](../../../../../src/init/plan.rs) 在全量预检后按扁平 `FileOp` 顺序逐个原子写入；一旦中途失败，已成功文件保留，因此当前架构明确不提供跨文件事务。[文件安全模块](../../../../../src/safety.rs) 已提供 snapshot、并发复核、同目录临时文件原子替换和项目写锁，但没有按 expected snapshot 删除文件或回滚文件组的能力。

Pi-only 初始化还会以全 bundle 内容完全相等作为兼容目录的复用条件。相关集成测试位于 [init 测试](../../../../../tests/init.rs)、[边界测试](../../../../../tests/edge_cases.rs) 和 [安全测试](../../../../../tests/safety.rs)。本工作包之外没有与这些入口相关的未提交实现修改。

目标是以根 `SKILL.md` 作为唯一版本源和最后提交标记，在保留既有归属检查、预检、锁与并发保护的同时，为每个 skill bundle 增加可回滚的有序更新。

## 2. Overall approach

只在内置 `SKILL.md` 的受管标记之后增加独立版本行：

```text
<!-- doco:managed template=v1 -->
<!-- doco:skill version=v1 -->
```

其他内置文件继续只带原来的 `template=v1` 受管标记。CLI 从编译进二进制的根 `SKILL.md` 解析当前版本，不在其他资源文件中复制版本值，也不从 crate semver 推导。

`init::skill` 先读取目标根 `SKILL.md`，分别判断 doco 归属和已安装版本。目标不存在时走新安装；目标已受管且 `bundled_version > installed_version` 时开启 bundle 自动刷新；同版本或更高版本沿用现有内容差异规则。自动刷新一旦开启，就针对整个已知 bundle 使用新版内容，而不是试图从其他文件推断各自版本。

初始化计划增加“可回滚文件组”概念。每个选中的 skill 安装目录形成一个组，文件顺序固定为：

1. `SKILL.md` 以外的全部 `templates::FILES` 项，保持稳定注册顺序；
2. 根 `SKILL.md`，最后写入。

组内仍以单文件原子替换为基本写入操作，但保存每个文件的原 snapshot 和成功写入后的 snapshot。任一步失败时，对已经改变的路径逆序执行补偿操作。只有整个组成功后才报告组内文件已应用。普通项目目录、架构文件及 Agent 导航仍使用现有扁平计划；本变更不把不同 skill 目录或整个 init 合并为一个事务。

## 3. APIs and data model

### 版本和 bundle 决策

[内置根 skill](../../../../../assets/skill/SKILL.md) 中新增唯一的 `<!-- doco:skill version=v1 -->`。建议在 `init::skill` 增加以下内部能力，具体名称可调整：

- `skill_version(text: &str) -> u64`：只检查代码围栏外的独立版本行。恰好一个合法版本时返回整数；缺失、格式错误、重复、前导零或整数溢出均返回 `0`。
- `bundled_version() -> Result<u64>`：从编译内置的根 `SKILL.md` 严格读取版本。内置资源必须有且只有一个合法的非零版本；失败属于 CLI 构建资源错误并在 init 预检中报告，不得把内置版本静默视为 `v0`。
- `BundleMode::{Install, Current, Upgrade, Refresh}` 或等价分类：`Upgrade` 仅在内置版本严格高于已安装版本时产生；`Refresh` 来自显式 `--refresh`。分类不持久化。

版本格式只接受小写 `v` 后的规范非负 ASCII 十进制整数。“单个数字”表示单一整数维度，而不是 semver 多段数字，也不限制只能有一位。规范格式允许 `v0` 和 `v10`，不允许 `v01`。

归属与版本分离。已有根文件必须先通过现有 doco-managed 标记或现有精确无标记模板收养规则，才能使用其版本触发更新；无归属同名文件即使版本解析结果为 `0` 仍然冲突。bundle 中其他已存在的已知文件也继续逐个满足现有归属规则，`Upgrade`/`Refresh` 不允许覆盖不受管文件。缺失的已知文件可以创建，未知文件不进入计划。

### 可回滚文件组

[初始化计划模块](../../../../../src/init/plan.rs) 增加与普通 `files` 并列的组结构，建议形状：

```text
FileGroup
  label
  files: ordered Vec<FileOp>
  initially_missing_directories: deepest-first rollback list
```

`FileOp.before` 已保存原始 snapshot；组执行时另行记录实际成功写入后的 snapshot，用于确认回滚时路径仍是本进程刚写入的内容。`Plan::show`、`ensure_valid`、全量 snapshot 复核和 `effective_text` 必须同时覆盖普通文件与组内文件。这样 Claude import 检查能看到同一次计划中尚未落盘的新版 skill 内容。

[文件安全模块](../../../../../src/safety.rs) 增加受 snapshot 保护的补偿操作：

- 恢复原有文件：以成功写入后的 snapshot 为 expected，调用原子替换写回原始 bytes，并恢复平台支持的原权限。
- 删除本次新建文件：仅当当前 snapshot 与成功写入后的 snapshot 完全相等时删除；不得删除外部进程替换过的路径。
- 删除本次新建目录：仅处理组开始前不存在、由本组创建且回滚时仍为空的目录，按最深路径优先；不得递归删除或处理未知内容。

可以通过受控 helper、测试注入点或更小的 apply executor 实现，不要求公开稳定库 API。snapshot 仍保存在内存，不新增持久数据库。原始创建时间和修改时间不承诺恢复；“恢复原状态”指恢复原字节、存在性和当前实现可保留的权限，不包括文件系统时间戳。

## 4. Algorithms and rules

### 规划

对每个目标 skill 目录执行：

1. 读取并安全检查根 `SKILL.md`。目录非空但缺少根文件时继续按现有规则冲突；不存在的目标目录属于新安装。
2. 严格读取 CLI 内置根版本；解析已安装根版本，无法识别时取 `0`。
3. 已有根文件先确认 doco 归属。无归属时立即形成 conflict，`--refresh` 也不能绕过。
4. 选择模式：不存在为 `Install`；内置版本更高为 `Upgrade`；否则显式刷新为 `Refresh`；其余为 `Current`。
5. 对所有已知 bundle 文件生成目标内容和原 snapshot。`Upgrade` 与 `Refresh` 允许替换内容不同的受管文件；`Current` 保持现有“不同则冲突”；所有模式都拒绝覆盖不受管的既有文件。
6. 把除根 `SKILL.md` 外的 FileOp 依注册顺序加入组，根文件最后加入。记录写入所需且初始不存在的父目录。
7. 任一读取、归属、编码或路径检查失败都在 `Plan::ensure_valid` 前形成 conflict，因此不会开始任何初始化写入。

`Upgrade` 必须刷新整个已知 bundle：某个其他文件即使正文恰好已是新版，也会形成 SKIP；缺失文件形成 CREATE；旧受管内容形成 UPDATE。未知自定义文件不删除。

### 执行和提交

获得现有项目写锁后，仍先对所有普通文件、组内文件、只读依赖和目录执行全量 snapshot/路径复核。每个 skill 组按以下步骤执行：

```text
applySkillGroup(group)
  applied = []
  for op in group.files                 # SKILL.md guaranteed last
    skip if bytes unchanged
    atomicWrite(op.before, op.after)
    written = snapshot(op.path)
    require written.bytes == op.after
    applied.push(op, written)
  report APPLIED for committed group
```

最终根 `SKILL.md` 写入成功是该 bundle 的提交点。因为它最后执行，所以任何可捕获的早期失败都不会留下新版本标记；进程意外退出时，旧版本也能让下次 init 重新执行整个升级。

### 失败和回滚

若 `atomic_write` 或写后 snapshot 校验失败，先检查失败路径当前状态：仍等于 before 表示该步未生效；等于目标 bytes 表示该步可能已生效，也加入回滚集合；两者都不等表示出现并发/不确定状态，不得盲目覆盖。随后逆序补偿：

```text
rollback(applied)
  for op in reverse(applied)
    if op.before existed
      atomicWrite(expected = written, bytes = before.bytes)
      restore supported original permissions
    else
      removeFile(expected = written)
  remove newly-created empty directories deepest-first
```

所有补偿成功时，输出 `ROLLED BACK` 摘要并返回原始写入错误，明确该 skill bundle 没有保留本次更新。组内 `APPLIED` 事件延迟到提交成功后输出，避免把随后撤销的文件报告为最终成功。

任一补偿失败时继续尝试其余互不依赖的补偿，最终输出 `ROLLBACK FAILED`、原始失败、每个未恢复路径及原因。若当前 snapshot 不等于本进程记录的 written snapshot，视为外部并发修改，绝不覆盖。命令返回错误且不得声称恢复完成。

回滚完成后不继续执行后续组或普通文件。此前已经提交的独立 bundle 或普通文件仍遵循 init 既有“跨组非事务”边界。进程被强制结束、断电或持续 IO 故障无法依靠内存补偿保证回滚；下次 init 可凭仍旧的根版本重新升级，但这不等同于恢复原人工修改，错误文档必须如实说明该限制。

### Pi 兼容目录

Pi-only 复用检查改为调用同一 bundle 规划规则的只读分类：

- 当前内容完全一致可以复用；
- 根 skill 受管且内置版本更高时，可以复用并生成 `Upgrade` 文件组；
- 已知文件缺失可由该组创建；
- 无关根 skill、任一不受管的既有已知文件、同版本差异或高版本差异不可由普通 init 自动复用。

复用检查不写文件；真正更新仍由统一组执行和回滚。Codex 对替代 `.pi` 目录的冲突、Claude import 规则和未知文件保留规则不变。

## 5. Fixed decisions and discretion

固定决策：

- 版本只写在根 `SKILL.md`，使用独立行 `<!-- doco:skill version=v1 -->`；其他 bundle 文件不复制版本。
- 当前 bundle 版本为 `v1`，唯一真源是编译内置的根 skill；版本与 crate semver 无关。
- 缺失或不可识别的已安装版本按 `v0`；版本回退不授予文件覆盖归属。
- 旧版本触发整个已知 bundle 刷新，其他文件先写，根 `SKILL.md` 最后写。
- 新安装、自动升级和显式刷新都使用可回滚文件组；普通写入失败时必须补偿已完成步骤。
- 同版本差异继续要求 `--refresh`；高版本不自动降级；显式 `--refresh` 仍可覆盖受管文件并强制降级。
- 回滚以 snapshot 保护，不能为了恢复 doco 写入而覆盖外部并发修改。回滚失败必须显式暴露，不能隐藏在原始错误后。
- 事务边界是单个 skill bundle，不扩展到多个 Agent 目录、项目文档和导航文件。
- 不新增 ADR；交付后更新当前架构和 CLI 规范，明确新的局部事务边界和失败限制。

实现者可自主决定文件组类型和 helper 的具体名称、版本解析 regex 的组织方式、故障注入设施及 reporter 文案细节。版本必须从代码围栏外的独立行读取；根 skill 必须最后提交；回滚顺序、snapshot 保护和失败报告不可调整为“下次重试即可”。

没有阻塞性未决问题。

## 6. Verification and documentation impact

至少验证：

- 只有根 `SKILL.md` 含 skill 版本；其他内置文件不含该字段。内置版本必须唯一、合法、非零。
- 已安装版本的 `v0`、`v1`、多位整数、缺失、格式错误、重复、前导零和溢出按约定解析。
- 受管 `v0` bundle 无需 `--refresh` 即整体升级；测试观察到其他文件先于根 skill，提交后再次 init 为 SKIP。
- 对组内每个写入位置逐一故障注入，包括最终根 skill：回滚后原有文件字节和存在性恢复，本次新文件/空目录移除，未知文件保留，根版本不前移。
- 回滚步骤自身失败或路径被外部替换时，命令报告具体未恢复路径且不覆盖外部内容。
- 同版本人工修改在预检阶段零写入冲突；显式刷新走同一回滚组；高版本普通初始化不降级。
- 新安装失败不会留下半个 bundle；成功安装仍保证导航文件写入前 skill 已完整提交。
- Pi-only 旧版目录被复用并事务性升级，无关或不可安全升级的替代 skill 继续冲突。
- BOM、CRLF、dry-run、幂等性、Claude/Codex/Pi 选择、链接/硬链接和并发复核测试无回归。

执行 `cargo fmt --check`、相关故障注入定向测试和完整 `cargo test`。交付后更新 [当前架构](../../../../architecture.md) 中 init、skill bundle、写入顺序和失败恢复边界，并在 [CLI 规范](../../../../specs/cli.md) 记录自动升级、`v0`、`SKILL.md` 最后提交、回滚结果和 `--refresh` 关系。[文档格式规范](../../../../specs/document-format.md) 只约束项目工作包，无需修改。
