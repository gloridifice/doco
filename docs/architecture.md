# 当前架构

## 职责与依赖

```text
main → cli (clap) → terminal（console）
         ├─ init → plan / entry / skill
         └─ lifecycle → context / archive
                  ↓
             check → tasks
                  ↓
        ui::Reporter + Project + markdown + safety + templates
```

- `src/main.rs` 只退出 `cli::run` 返回的状态码；`src/cli.rs` 定义参数、候选状态过滤、交互 Agent/变更选择、命令分发和顶层语义错误渲染。
- `src/terminal/` 是二进制侧终端适配层：一次性探测各流 TTY、TERM、NO_COLOR 和尺寸，使用 console 渲染业务颜色、TTY list 和局部选择循环。选择器支持方向键、Space、Enter、Esc 和 Ctrl-C，以 RAII 恢复光标；它不保存业务状态。
- `src/ui.rs` 是库侧输出端口，定义语义事件、流、色调和控制字符过滤。init、check、lifecycle 通过 Reporter 输出；旧公开入口使用 PlainReporter 保持非终端调用。Clap 不进入领域层。
- `src/lib.rs` 提供项目根、稳定 ID 和从目录推导的状态模型。
- `src/init/` 将全部目标规划为 CREATE/UPDATE/SKIP 或 CONFLICT，处理受管区块、原生 skill 安装和有限的导入去重。写入前发现任何冲突就拒绝整个安装计划。
- `src/check/` 只检查文档结构、模板残留、任务格式、依赖和明确的阻塞/证据字段，不评价设计质量或测试真实性。
- `src/lifecycle/` 执行创建、上下文选择和状态迁移；归档清理与一般迁移分开实现。CLI list 默认请求 active，并用独立开关追加 completed/archived；`list_changes_filtered` 在摘要 IO 前过滤状态，再通过 safety 读取 active/completed 任务文件、复用 check 的任务解析器生成已完成/总数，并安全遍历可见工作包、以统一查询时刻计算最新 mtime 的紧凑距今时长。Reporter 只渲染已收集的行，兼容的 `list_changes` 库入口仍返回全部状态；Project 状态模型与菜单候选查询不读取这些摘要。
- `src/markdown.rs` 使用 CommonMark 解析器定位代码区和链接，辅以保守的标记/章节解析，不整体格式化用户文件。
- `src/safety.rs` 集中文件归属检查、写锁、并发复核、同目录临时文件原子替换和目录操作。
- `assets/skill/` 是唯一工作流模板来源，通过 `include_str!` 编译进二进制。不同 Agent 安装普通文件副本，不依赖运行时源码目录或符号链接。

## 持久化与生命周期

没有数据库、中央索引或复制状态的 front matter。状态仅来自 `doco/changes/{active,completed,archived}/<id>`，三个目录之间 ID 必须唯一。ID 使用小写 ASCII 字母、数字和单个内部连字符，长度不超过 80；拒绝 Windows 保留设备名。

`new` 在 `doco/tmp/` 暂存完整骨架后移动到 active；骨架故意不能通过 check。`complete` 检查任务、结果和证据字段后移动整个目录，但真实验收和当前事实同步仍由用户/Agent 完成。`reopen` 保留工作包，不擅自重置任务。

`archive` 仅接受 completed，先显示删除范围；`--dry-run` 在预览后停止，普通执行不再要求交互确认。它随后加锁并复核预览快照，保留完整 proposal，只删除 work 内预检过的文件，最后移动到 archived。`--yes` 仅为兼容保留，不改变流程。工作包根目录有其他文件时拒绝归档，要求用户先明确整理。`cancel` 仅接受 active，先将明确的取消原因和已实施代码处理方式写入 proposal 结果，再按相同清理路径归档；不回滚代码。

## 文件安全与失败恢复

所有路径检查包含父目录，拒绝符号链接、Windows reparse point（包括 junction）、硬链接文件和特殊文件。检查用户给定根目录后才规范化路径；仅在该根下操作。

写操作使用 `doco/tmp/.doco.lock` 的操作系统排他锁，进程退出自动释放，锁文件不删除，避免旧 inode 上持锁而新文件又获得第二把锁。`doco/.gitignore` 默认仅写 `/tmp/`。不要在写操作运行时自行删除 tmp 或锁文件。

初始化先预检，再加锁并复核读取快照，写入每个文件前再次核对内容和元数据。新文件使用不覆盖式持久化，已有文件使用同目录临时文件原子替换，保留可支持的权限和换行。**这不是跨文件事务**：失败时打印已应用项，保留成功部分，修复原因后幂等重试。skill 先于引用它的入口写入。UTF-8（可含 BOM）、LF/CRLF 受到支持；不能安全解码、未闭合结构或歧义导入会报冲突。

归档按预览清单自底向上删除，不沿链接递归。删除失败时保留原状态目录和 proposal，明确报告部分清理；修复文件占用/权限后重试。若 work 已删除但移动失败，同一归档命令允许缺少 work，以完成剩余移动。取消失败后可能已写入取消结果；用相同原因和处理方式重试。进程意外退出不一定来得及打印最终诊断，可用 `list` 和实际目录检查状态。

锁协调 doco 进程，不控制外部编辑器。快照和路径复核能拒绝检测到的并发修改，但不构成抵御恶意进程反复替换目录的操作系统沙箱，也不承诺多文件断电事务。不要在迁移或清理时并行修改同一工作包。

## 上下文与 Agent 接入边界

默认 context 从 architecture、指定 active 工作包及其显式引用输出路径候选，排除其他变更、tmp 和能识别的被替代 ADR。`--history` 只显式纳入指定历史工作包并标注快照。`doco:<id>` 引用解析当前位置，但不会因此自动把其他变更加入阅读范围。CLI 不约束外部搜索工具，也不自动判断所有语义相关资料。

根入口使用独立 DOCO 标记行，保留区块外字节。完整相同模板可幂等识别；任意自然语言改写不能保证去重。只支持安全、独立的 `@AGENTS.md` / `@./AGENTS.md` Claude 导入，复杂关系要求人工处理。Pi 的兼容 `.pi/skills/doco` 可以在仅选择 Pi 时复用；与共享安装冲突时要求用户显式迁移，不自动移动或删除旧 skill。

仅检查已知项目级发现位置和可见 override/config 提示，不审计用户全局插件、模型设置、权限、可信状态或所有加载开关。不自动登录或调用模型。Agent 的实际技能发现与规则遵循需要在客户端会话中另行验证。
