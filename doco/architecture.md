# 当前架构

本文档描述已实现的模块边界、持久状态、安全约束与恢复方式。CLI 的公开输出和交互契约见 [CLI 输出与交互](specs/cli.md)，CLI 可机械识别的持久文档格式见 [文档格式](specs/document-format.md)。精确命令参数以 `doco --help`、源码及集成测试为准；工作流规则由 [随工具发布的 skill](../assets/skill/SKILL.md) 提供。

## 职责与依赖

```text
main → cli (clap) → terminal（console）
         ├─ init ───┐
         ├─ update ─┴→ plan / entry / skill
         ├─ fix ──────→ index
         └─ lifecycle → context / archive
                  ↓
             check → tasks
                  ↓
        ui::Reporter + Project + markdown + safety + templates
```

- `src/main.rs` 只退出 `cli::run` 返回的状态码；`src/cli.rs` 定义参数、候选状态过滤、交互 Agent/变更选择、命令分发和顶层语义错误渲染。
- `src/terminal/` 是二进制侧终端适配层：一次性探测各流 TTY、TERM、NO_COLOR 和尺寸，使用 console 渲染业务颜色、TTY list 和局部选择循环。选择器支持方向键、Space、Enter、Esc 和 Ctrl-C，以 RAII 恢复光标；它不保存业务状态。
- `src/ui.rs` 是库侧输出端口，定义语义事件、流、色调和控制字符过滤。init、check、lifecycle 通过 Reporter 输出；旧公开入口使用 PlainReporter 保持非终端调用。Clap 不进入领域层。
- `src/lib.rs` 提供项目根、稳定 ID 和从目录推导的状态模型，`Project::changes` 与 `Project::resolve` 通过 index 查询；`src/index.rs` 维护可丢弃的本机 CSV 索引 `doco/tmp/changes-index.csv`，只缓存 `doco/changes` 直接子目录的 ID 与状态，读取时要求三个状态目录修改时间与缓存逐项相等，否则回退权威扫描；指定 ID 始终核对真实路径；写命令在业务提交前撤销缓存、提交后原子重写。`src/fix.rs` 是显式强制重建入口。`src/package.rs` 定义共享的工作包模式、显式标记和兼容解析，供模板、检查及各生命周期读取复用。
- `src/init/` 将全部目标规划为 CREATE/UPDATE/SKIP 或 CONFLICT，处理脚手架、受管入口、原生 skill 安装和有限的导入去重。安装目标只有 Most agents（`.agents` / `AGENTS.md`）与 Claude（`.claude` / `CLAUDE.md`）。根 `SKILL.md` 是 bundle 唯一版本源，DOCO 入口块有独立模板版本；内置版本较高时自动刷新，同版本差异或强制降级要求 `--refresh`。每个 skill 目录使用独立的可回滚文件组，其他文件先写、根 skill 最后提交。
- `src/update.rs` 检测上述两个既有完整集成并复用 init 的 integration/plan/entry/skill 路径，只更新 skill bundle 和入口文件，不创建缺失集成或项目文档。旧 `.pi`、`.codex` skill 路径是迁移冲突；Claude 只导入 `AGENTS.md` 时复用 Most agents，不视为独立 Claude 集成。
- `src/check/` 解析 proposal 中显式的工作包模式，检查文档结构、模板残留、任务格式、依赖和明确的阻塞/证据字段，不评价设计质量或测试真实性。无模式标记兼容为完整包；proposal-only 必须完全没有 work，严格完成检查从 proposal 读取验证证据。
- `src/lifecycle/` 执行创建、上下文选择和状态迁移；归档清理与一般迁移分开实现。CLI list 默认请求 active，并用独立开关追加 completed/archived；`list_changes_filtered` 在摘要 IO 前过滤状态，再通过 safety 读取 active/completed proposal 模式，完整包复用 check 的任务解析器生成已完成/总数，proposal-only 显示不适用，并安全遍历可见工作包、以统一查询时刻计算最新 mtime 的紧凑距今时长。Reporter 只渲染已收集的行，兼容的 `list_changes` 库入口仍返回全部状态；Project 状态模型与菜单候选查询不读取这些摘要。五种状态迁移在写锁内按“预检 → 撤销缓存 → 业务提交 → 发布缓存”维护索引，业务提交后的缓存失败只警告。
- `src/markdown.rs` 使用 CommonMark 解析器定位代码区和链接，辅以保守的标记/章节解析，不整体格式化用户文件。
- `src/safety.rs` 集中文件归属检查、写锁、并发复核、同目录临时文件原子替换和目录操作。
- `assets/skill/` 是唯一工作流模板来源，通过 `include_str!` 编译进二进制。不同 Agent 安装普通文件副本，不依赖运行时源码目录或符号链接。

## 持久化与生命周期

没有数据库、中央索引或复制状态的 front matter。状态仅来自 `doco/changes/{active,completed,archived}/<id>`，三个目录之间 ID 必须唯一。ID 使用小写 ASCII 字母、数字和单个内部连字符，长度不超过 80；拒绝 Windows 保留设备名。

`doco/tmp/changes-index.csv` 是上述目录的派生缓存，不是第二个事实来源：它只保存 ID、状态和三个状态目录的修改时间快照，被忽略、可删除、不跨机器共享。缓存命中时批量查询跳过目录枚举，但指定 ID 的创建和迁移仍核对真实路径；缓存的格式、失效与信任边界见 [变更索引缓存](specs/change-index-cache.md)。

active/completed 有两种合法形态。完整包包含 `proposal.md`、`work/implement.md` 和 `work/tasks.md`，没有模式标记的既有包均按完整包解释。proposal-only 只包含带唯一 `<!-- doco:change mode=proposal-only -->` 标记的 proposal，禁止存在 `work/`；模式不能从缺失文件推断，避免完整包意外丢失 work 后绕过任务检查。archived 对两种来源都只保留 proposal 及其模式标记。

`new` 在 `doco/tmp/` 暂存骨架后移动到 active；默认生成完整包，`--proposal-only` 只生成 proposal，二者的模板骨架都故意不能通过 check。`complete` 对完整包检查任务、结果和任务证据，对 proposal-only 检查结果、proposal 验证证据和 blocker，然后移动整个目录；真实验收和当前事实同步仍由用户/Agent 完成。`reopen` 按原模式保留包内容，不擅自重置任务。

`archive` 仅接受 completed，先显示删除范围；`--dry-run` 在预览后停止，普通执行不再要求交互确认。它随后加锁并复核预览快照，保留完整 proposal，只删除完整包 work 内预检过的文件，最后移动到 archived。proposal-only 没有 work 是正常状态；完整包在部分清理后缺少 work 仍可重试完成移动。`--yes` 仅为兼容保留，不改变流程。工作包根目录有其他文件时拒绝归档，要求用户先明确整理。`cancel` 仅接受 active，先将明确的取消原因和已实施代码处理方式写入 proposal 结果，再按相同清理路径归档；不回滚代码。archive 只保留目标和交付摘要，不承担实现历史，后者由 Git、PR 或项目发布记录保存。

## 文件安全与失败恢复

所有路径检查包含父目录，拒绝符号链接、Windows reparse point（包括 junction）、硬链接文件和特殊文件。检查用户给定根目录后才规范化路径；仅在该根下操作。

写操作使用 `doco/tmp/.doco.lock` 的操作系统排他锁，进程退出自动释放，锁文件不删除，避免旧 inode 上持锁而新文件又获得第二把锁。`doco/.gitignore` 默认仅写 `/tmp/`。不要在写操作运行时自行删除 tmp 或锁文件。

索引缓存的维护与业务写入同锁：预检通过后在首次业务修改前删除缓存，业务提交并通过最终校验后按实际状态原子重写；业务失败或进程中断留下无缓存状态，下次命令重新扫描。删除缓存失败时不修改业务文件；业务已提交而缓存写入失败时不回滚业务，改为警告并提示 `doco fix`。缓存发布前会再次核对三个状态目录的修改时间，不一致则拒绝写入。只读命令和所有 dry-run 不写缓存，也不创建 tmp。

init 和 update 先预检，再加锁并复核读取快照，写入每个文件前再次核对内容和元数据。新文件使用不覆盖式持久化，已有文件使用同目录临时文件原子替换，保留可支持的权限和换行。每个 skill bundle 是局部补偿事务：references 和模板先写，根 `SKILL.md` 最后作为版本提交点；可捕获的组内失败会按快照逆序恢复原字节和存在性，并清理本组新建的空目录。回滚不会覆盖外部并发修改，失败时会列出未恢复路径。skill 组仍先于引用它的入口写入。

不同 skill 目录、项目文档和导航文件之间**不是跨文件事务**：后续失败时打印已应用项，保留此前已提交的独立组或文件，修复原因后幂等重试。进程强制终止、断电或持续文件系统故障也不承诺物理事务；由于根 skill 最后写入，提交前退出时旧版本会使下次 init 重新刷新整个 bundle。UTF-8（可含 BOM）、LF/CRLF 受到支持；不能安全解码、未闭合结构或歧义导入会报冲突。

归档按预览清单自底向上删除，不沿链接递归。删除失败时保留原状态目录和 proposal，明确报告部分清理；修复文件占用/权限后重试。若 work 已删除但移动失败，同一归档命令允许缺少 work，以完成剩余移动。取消失败后可能已写入取消结果；用相同原因和处理方式重试。进程意外退出不一定来得及打印最终诊断，可用 `list` 和实际目录检查状态。

锁协调 doco 进程，不控制外部编辑器。快照和路径复核能拒绝检测到的并发修改，但不构成抵御恶意进程反复替换目录的操作系统沙箱，也不承诺多文件断电事务。不要在迁移或清理时并行修改同一工作包。

## 上下文与 Agent 接入边界

默认 context 从 architecture、指定 active 工作包及其显式引用输出路径候选；完整包加入 proposal、implement 和 tasks，proposal-only 只加入 proposal。它排除其他变更、tmp 和能识别的被替代 ADR。`--history` 只显式纳入指定历史工作包并标注快照。`doco:<id>` 引用解析当前位置，但不会因此自动把其他变更加入阅读范围。CLI 不约束外部搜索工具，也不自动判断所有语义相关资料。

根入口使用独立 DOCO 标记行及唯一入口模板版本，保留区块外字节。入口明确 doco 变更只用于显式跟踪和设计/协调，不是所有行为或实现细节修改的前置条件；未创建变更也不免除同步受影响当前文档的责任。完整相同模板可幂等识别；任意自然语言改写不能保证去重。Claude 的安全、独立 `@AGENTS.md` / `@./AGENTS.md` 导入表示复用 Most agents；复杂导入或与独立 Claude 入口并存要求人工处理。`.pi/skills/doco`、`.codex/skills/doco` 不再复用，也不会自动移动或删除。

仅检查已知项目级发现位置和可见 override/config 提示，不审计用户全局插件、模型设置、权限、可信状态或所有加载开关。不自动登录或调用模型。Agent 的实际技能发现与规则遵循需要在客户端会话中另行验证。
