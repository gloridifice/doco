# CLI 视觉与交互实施设计

## 1. 现状、目标与设计基线

### 已核对的基线

- 工作树起点为 Git `04dfc22`，创建前 git status --short 为空；doco list 未发现已有变更，无需复用其他目标。
- [当前架构入口](../../../../architecture.md) 仍为初始化占位说明；doco/specs 与 doco/decisions 没有当前文档。实际已实现职责见 [架构说明](../../../../architecture.md)、[当前文档导航](../../../../README.md)，不能将空入口误当作没有实现。
- [CLI](../../../../../src/cli.rs) 使用 clap derive；所有变更 ID 都是必填 String。init 在 stdin/stdout 均为终端时手输空白分隔 Agent 名称，否则要求 --agent。list 直接输出按 ID 排序的两列 TSV。
- [入口](../../../../../src/main.rs) 将 anyhow 错误链写到 stderr，退出 1；Clap 管理参数错误和帮助退出。
- [生命周期](../../../../../src/lifecycle/mod.rs)、[上下文](../../../../../src/lifecycle/context.rs)、[归档](../../../../../src/lifecycle/archive.rs)、[初始化](../../../../../src/init/mod.rs)、[计划](../../../../../src/init/plan.rs)、[检查报告](../../../../../src/check/mod.rs) 各自 println。归档模块还直接读取确认。
- 归档先预检和快照，输出完整预览，确认后才加锁并复核，然后逐项删除；取消先记录结果再清理。初始化逐文件原子写入，不是跨文件事务。这些顺序必须保留。
- [集成夹具](../../../../../tests/common/mod.rs) 使用 null stdin、捕获 stdout/stderr 后拼接；[初始化测试](../../../../../tests/init.rs) 和 [生命周期测试](../../../../../tests/lifecycle.rs) 依赖现有标签、缺参拒绝和非交互无写入行为。
- [Cargo 配置](../../../../../Cargo.toml) 要求 Rust 1.85，目前没有交互/渲染依赖。下文新增模块、接口、参数均是计划，不是现有 API。

目标是轻量命令行增强，保持单次命令、短生命周期、文件驱动的结构。先明确接口和兼容规则，再迁移打印，最后接入菜单；不借此重写领域模型。

## 2. 整体方案

### 模块职责

```text
main / cli
  ├─ 参数解析、终端能力快照、目标解析
  ├─ terminal::TerminalReporter  → console（颜色与宽度）
  ├─ terminal::TerminalPrompter  → console::Term（局部选择循环）
  └─ init / lifecycle / check
       └─ ui 的 Reporter 接口
            └─ PlainReporter 兼容适配器

Project / markdown / safety / templates：不依赖 terminal 或交互库
```

- 新增库模块 src/ui.rs：语义消息类型、输出端口、确认端口，以及保留旧 API 的纯文本适配器。不得访问全局终端状态或引入生命周期写入逻辑。
- 新增二进制模块 src/terminal/mod.rs、output.rs、prompt.rs：能力判定、颜色/布局、已有项选择实现。初始化和生命周期模块只调用输出端口，不得自行选择颜色、检查 TTY 或读取 stdin。
- CLI 负责 Agent 多选和已有变更候选选择；new 始终要求显式 ID。archive/cancel 不读取确认输入，继续由领域层保有唯一的安全预检、复核与执行算法。
- 不缓冲整次执行。预检信息、APPLIED、DELETED、部分失败按原时机流式输出，保留故障恢复线索。

### 依赖选择

新增 console 0.16 系列，并锁定与 Rust 1.85 兼容的补丁版本及 Cargo.lock；若该系列无法满足 MSRV，先报告阻塞，不擅自提高 Rust 版本。使用 console 的逐样式强制开关，不调用进程全局 set_colors_enabled。局部选择器直接使用 console::Term，不引入文本/密码输入、模糊搜索、编辑器或外部事件框架。

不引入 ratatui/crossterm 应用框架，不自建按键事件循环，不增加常驻线程或异步运行时。交互库通过明确 stderr 终端运行，不暗中回退到系统控制终端绕过重定向检查。

### 输出流与视觉

- 业务计划、报告、list、context 以及成功信息保留现有 stdout；顶层运行错误仍在 stderr。不要顺手把现有 WARNING/ERROR 报告搬到 stderr，避免破坏调用方。
- 菜单、输入框、键位说明和取消信息仅写 stderr。所有破坏性预览在 stdout flush 成功后才能显示确认框。
- 标签保持可读英文：CREATE/APPLIED/成功为绿，UPDATE/WARNING 为黄，DELETE/CONFLICT/ERROR 为红，MOVE/READ/active 为青，completed 为绿，archived/SKIP 为普通或弱化色。正文和路径不整行染色；无色时不能丢失状态信息。
- 不依赖 emoji、Nerd Font、背景色或复杂边框。标题可加粗，提示使用 ASCII，例如 `[x]`、`[ ]`、`>`。不显示 spinner，不清除业务日志，不使用备用屏幕。
- 初始化 diff 按行渲染：+ 绿、- 红、@@ 青；保留 existing/planned 文件头和原上下文、换行，不能误将文件头当作业务成功/失败。

## 3. 关键 API 与数据模型

以下签名是拟新增的接口边界；字段内部命名可小幅调整，职责不可合并回全局打印。

```rust
pub enum Stream { Stdout, Stderr }
pub enum Tone { Plain, Info, Success, Warning, Danger, Muted }
pub enum Event<'a> {
    Line { stream: Stream, tone: Tone, label: &'a str, body: &'a str },
    Text { stream: Stream, text: &'a str },
    Diff { text: &'a str },
    Changes { changes: &'a [ChangeRow] },
}
pub trait Reporter {
    fn emit(&mut self, event: Event<'_>) -> std::io::Result<()>;
    fn flush(&mut self, stream: Stream) -> std::io::Result<()>;
}
```

Line 渲染约定为 label 与 body 间一个空格，label 为空时不额外加空格，行末一个换行；Text 原样保留已组织的换行。调用方保留原有消息文本及分段，不能通过“统一美化”删除风险提示。Event 借用仅持续到 emit 返回，不能持有 Project/Change 引用跨越一次命令或异步存储。

二进制内部增加：

- `ColorMode { Auto, Always, Never }`、`InteractionMode { Auto, Always, Never }`。
- `TerminalCapabilities`：stdin/stdout/stderr 是否终端、TERM、非空 NO_COLOR、列数；每次命令只读取一次。测试用构造值，不在并行测试中修改进程环境。
- `TerminalPrompter::select_agents()` 返回 Agent 集合；`select_change(&[Change])` 返回所选 ID，均采用内部 PromptOutcome。这些选择 API 无需暴露为领域库 API。
- `resolve_target(project, kind, supplied_id, history, mode, prompter)`：显式 ID 直接返回；只在允许时查询和展示已有候选，返回目标 String 或类型化取消错误。new 不调用此函数。
- 用可 downcast 的 `UserCancelled` 标记用户取消；main 将它转换为 130，业务/IO 错误仍为 1，参数使用错误为 2，帮助/版本为 0。不靠字符串匹配错误判断取消。

### 现有 API 兼容

保留现有公开函数签名：init::run、lifecycle 的 new_change/check/complete/context/reopen/archive、Report::show。新增对应 `_with_ui` 入口并使用 `&mut dyn Reporter`；CLI 仅调用新入口。旧函数委托给共享实现与纯文本适配器。archive 的 yes 参数为源码和脚本兼容继续接受，但不再触发或跳过确认；这是本次实施中由用户明确批准的安全交互契约变更。旧 Report::show 维持无返回值兼容，新业务路径改用可返回 IO 错误的 show_with_ui。私有 Plan::show/apply 可以直接增加 Reporter 参数，无需保留私有包装函数。

兼容适配器不是第二套安全算法；只有一份归档执行路径。Clap 和 terminal 不进入 Project/check/safety 的数据模型。外部库调用不启用彩色或新选择菜单。

### 状态与资源

Reporter、终端选择器和能力快照由 CLI 栈上拥有，每次命令销毁；不持久化偏好、最近选择或颜色设置。选择列表使用 Project::changes 的一次快照，O(n) 空间，不读取 proposal 正文来计算标题或进度。选中只得到稳定 ID，真正执行时依然由原生命周期入口重新 resolve 和检查状态。候选不是授权凭证。

选择菜单的终端 raw mode/光标状态依赖库的作用域恢复，并通过真实终端验收；所有成功、Esc、Ctrl-C 和 IO 错误路径都必须恢复。强制杀进程不承诺清理。输出写入失败退出 1：若预览输出失败则不开始删除；若已开始写入则不声称回滚，保留原恢复方式，尝试向可用 stderr 报告已发生的部分操作。

## 4. 核心算法与实现规则

### 4.1 参数、颜色、能力判定

新增全局参数：

| 参数 | 默认及规则 |
| --- | --- |
| --color auto\|always\|never | 默认 auto；仅控制业务 Renderer 与 prompt 样式 |
| --interactive | 显式允许补输 ID/选择目标，并要求可交互终端 |
| --no-interactive | 禁止所有提问；与 --interactive 互斥 |

每个输出流单独判定颜色：显式 never 为关，always 为开；auto 仅当该流是 TTY、NO_COLOR 不存在或为空、TERM 不为 dumb 才开启。显式 auto 仍尊重 NO_COLOR。always 可覆盖 NO_COLOR、TERM=dumb 和重定向。首版不读取 CLICOLOR/CLICOLOR_FORCE，不增加隐藏的环境优先级。

布局与颜色分离：仅 stdout 为 TTY 且 TERM 不为 dumb 时启用 list 美化；--color never 仍可有对齐列。--color always 加管道仅增加 SGR，不引入表头/统计或菜单。显式 --no-interactive 不禁用颜色；--color never 只关闭 SGR，不禁止交互菜单必需的光标控制。

可交互条件为 stdin、stdout、stderr 均为 TTY 且 TERM 不为 dumb。比原有条件增加 stderr，是因为菜单改在该流呈现；任一流被重定向即不打开终端控件。--interactive 不可突破这个条件，能力不满足时在业务写入前失败，提示提供显式参数并移除 --interactive；--no-interactive 则永不读取输入。

使用 Clap 标准解析器，禁用其颜色输出，帮助/版本/参数错误保留标准格式和 stream/退出码。不另写 argv 预扫描或全套自定义帮助渲染。除 new 外需要目标的 ID 字段改为 Option<String>，解析后在没有 --interactive 时构造 Clap MissingRequiredArgument；new 的 ID 保持必填。对每个子命令及全局参数前后位置作解析测试。cancel 的 reason/disposition 继续是必填，不能由 --interactive/--yes 豁免。

### 4.2 初始化与新建

- init 有 --agent：验证、排序、去重后直接运行，不再提问。
- init 无 --agent：Auto 或 Always 且能力满足时展示 MultiSelect，固定顺序 Codex / Claude Code / Pi，初始全部未勾选；Space 切换，Enter 提交。空选显示内联提示并留在选择器，不创建目录。Never 或能力不足时保持 requires --agent 错误。
- new 始终要求 ID并使用现有 validate_id；即使传 --interactive，缺失或非法值也直接报参数/业务错误，不进入文本输入，不自动生成或改写 ID。new_change 继续在锁内检查唯一性。
- dry-run 可为缺失 Agent或已有目标做选择，但自身不执行写入。

### 4.3 目标选择矩阵

只在缺少 ID 且显式 --interactive 时启动 Select；展示 `state  id`，按 ID 字典序，与 Project::changes 一致。

| 命令 | 候选状态 |
| --- | --- |
| context | active；带 --history 时为全部状态 |
| check | active / completed / archived |
| complete | active |
| reopen、archive | completed |
| cancel | active |

一个候选也必须按 Enter 确认，不自动执行；初始高亮第一项不是授权。零候选退出 1 并说明所需状态，不写入，不提出替代生命周期操作。多页列表每页最多 10 行，并按可用终端高度缩小，长 ID 可以视觉换行，不能截断传给业务层的真实 ID。首版不做模糊搜索。

--yes 不代替目标选择。显式提供不存在/状态错误的 ID 时保留原报错，不弹菜单“纠正”。选择后其他进程迁移目标，由原业务入口重新 resolve 拒绝无效状态；读命令也遵循现有权限和历史范围检查。

### 4.4 破坏性操作执行

1. 继续在 archive 内按原顺序验证状态、proposal、工作包根额外文件、保留链接、目标路径和快照；通过 Reporter 显示每一条 RETAIN/UPDATE/DELETE/RECOVERY/MOVE 及风险说明。
2. dry-run 在预览后返回 0。非 dry-run 不读取确认输入；--yes 仅作为向后兼容参数接受，不改变执行分支，也不代替交互目标选择。
3. 预览完整写出后立即获取原有锁，重新 resolve 并复核 tree/snapshot/保留链接。预览至加锁间的任何已检测修改继续拒绝，不能根据菜单缓存重建计划绕过复核。
4. 继续逐项记录结果写入、删除与移动，失败保持现有部分恢复语义。命令不在持有写锁时等待人输入。菜单取消只可能发生在选定目标之前，因此没有预览或删除。
5. 这是明确的产品调整：用户选择执行 archive/cancel 即授权通过预检后的删除；需要只查看范围时必须使用 --dry-run。

### 4.5 列表及文本安全

美化 list 示例（颜色仅加在状态，不依赖颜色识别）：

```text
STATE      CHANGE                  TASKS
active     cli-visual-interaction  8/10
completed  another-change          2/2

2 changes (1 active, 1 completed, 0 archived)
```

非空列表状态列宽固定 9，CHANGE 列按最长 ID 对齐，末列 TASKS 显示已完成/总数；不改变 ID 排序。终端不足 40 列时改为每项 state、id、done/total（两个空格分隔），保留空态/总数，不横向填充。管道追加第三列，保持 state 和 ID 两列的原有顺序。不截断路径/ID。空态使用 `No changes found. Create one with: doco new <id>`；该示例只属于输出文案，不是模板残留。

所有来自路径、文档、错误链的文本在最终渲染边界过滤终端控制字符：保留普通文本换行和制表符，其余 C0、DEL、C1 转为可见转义，不执行 ESC/OSC 等序列。单行菜单标签还将换行/制表符显示为转义。过滤发生在添加可信样式之前；纯文本兼容适配器也执行过滤，正常现有文本不变。不要在文件存储或业务校验中改写原始文本；显式强制颜色也不能让不可信 ANSI 穿透。

### 4.6 任务数增量设计

用户追加要求 list 显示任务数，复用当前 active 变更，不影响已交付菜单。新增 `lifecycle::list_changes(&Project) -> Result<Vec<ChangeRow>>`（ChangeRow 定义于 ui 模块）；ChangeRow 持有 id、state、TaskCount（Known { done, total } / Missing / Archived），只用于输出，不修改 Project::changes 或 Change 模型。

对 active/completed，逐项通过 safety::snapshot 读取 work/tasks.md，缺失显示 ?；其他 IO/安全/UTF-8 错误保留路径并返回错误，不能将不安全读取伪装成零任务。复用 check::tasks::parse 计数其识别的 items，done 来自 [x]，总数为所有合法任务行，排除代码围栏和注释。计数不是 check：不因验收字段/依赖等语义诊断失败，不修改任务，不据此认证交付；空文件显示 0/0。archived 显示 - 且不访问已删除的 work 或从 proposal 推算。

所有文件读取在生命周期查询层，纯文本和终端 Reporter 仅渲染已收集的行。读取是只读即时快照，不加写锁、不持久缓存；每次仅持有一个文档的解析结果，总体额外保留 O(变更数) 的统计行。脚本有意变更为 state<TAB>id<TAB>done/total，空列表仍无输出。使用现有 safety 规则拒绝硬链接、符号链接及特殊文件。

## 5. 固定决策与可自主调整范围

### 固定决策

- 不做全屏应用或无参数主菜单；除 init 原有缺省交互外，补参/选目标必须显式 --interactive。
- 非交互 stdout 的现有文本协议和流归属优先于全局改版；list 的 TTY 布局和交互 prompt 流为明确例外。
- 颜色采用逐流策略、ASCII 标签、无持久偏好；帮助/版本/参数错误无色，不重写 Clap。
- 采用 console，领域层通过可注入输出端口协作，旧公开函数保留薄包装；局部选择循环只在二进制 terminal 层。
- 不合并安全预检与 CLI 选择逻辑；归档/取消无需确认，但保留预览、锁内复核和 --dry-run。
- 取消不被当作成功；130 表示交互取消，不改变已有业务错误码。没有新持久格式、后台任务或外部命令执行。

### 可自主调整

内部辅助函数和文件拆分、提示短句、同一语义内的色调、测试 fixture 命名可调整。可优化窄终端换行，但不能隐藏待删除条目或缩写真正的目标值。若底层 prompt 默认样式不满足 --color never，应实现自定义 theme，而不是改变颜色契约。

Blocked: none
Open questions: none

本次实施中设计负责人已调整范围：不提供 new --interactive 文本输入，archive/cancel 不做破坏性确认；--yes 作为兼容参数保留但不影响行为。为满足用户明确要求的 Ctrl-C 菜单取消，允许 terminal/prompt.rs 在 console::Term 上实现局部选择循环，替换 dialoguer 的 Select/MultiSelect。选择器只处理方向键、j/k、Space（多选）、Enter、Esc、q 和 Ctrl-C，采用 RAII 恢复光标；不扩展为全屏 TUI、通用事件框架或领域层依赖。dialoguer 依赖随实现移除。

## 6. 验证与长期文档影响

### 自动验证

- 能力表驱动单测覆盖两个输出流 TTY 的不同组合、NO_COLOR 空/非空、TERM=dumb、三种颜色模式，以及 --no-interactive 与颜色独立；不使用全局环境开关污染其他测试。
- 对 Reporter 注入内存 writer：比较纯文本与去 SGR 后的彩色业务文本；测试标签、diff 文件头、空列表、窄宽布局、Unicode 路径和控制字符转义。错误 writer 验证 IO 失败传播且预览失败不启动删除。
- 解析测试覆盖各命令缺 ID、显式 --interactive、冲突参数、全局参数前后位置、cancel 必填字段、help/version/错误码；无参数仍是帮助提示/参数错误而不是菜单。
- 扩展 Sandbox 提供分离的 stdout/stderr、退出码、子进程环境配置和 stdin 设置；保持现有便利方法。子进程显式清理相关继承环境，测试 auto 管道无 ANSI、always 管道有 SGR、never 无 SGR、TSV 字节与顺序稳定、非交互缺参数立即退出。
- 对候选过滤与选择边界验证状态矩阵、零/单/多候选、显式 ID 不提问、空 Agent 重试和菜单取消不调用业务入口。new 缺 ID始终为参数错误。真实菜单键位不能仅靠规则测试证明。
- archive_with_ui 验证 dry-run 不写入，非 dry-run 不等待输入，--yes 有无结果一致，锁内并发复核仍拒绝且保留文件，输出失败不启动删除。原有初始化、链接/硬链接/junction、安全与部分恢复测试全部保留。

执行命令为 cargo fmt --all --check、cargo clippy --all-targets -- -D warnings、cargo test --all-targets、cargo build --release --locked，并在可用 Rust 1.85 工具链验证 cargo +1.85 check --all-targets --locked；缺失工具链如实记录，不默认下载或声称已验证。

### 真实终端验收

在 Windows Terminal/PowerShell 和一个 Unix TTY 使用临时项目检查：方向键、Space、Enter、Esc/Ctrl-C、短窗口滚动、长 ID/中文路径；菜单关闭后光标与回显正常。测试正常终端、--color never、NO_COLOR、TERM=dumb、stdout/stderr 分别重定向，并验证重定向不偷偷打开控制终端。以项目树快照确认菜单取消前后无业务写入。危险操作在临时项目单独验证无 --yes、兼容 --yes、dry-run 和并发变更。记录平台、命令及限制；缺少平台不能写成通过。

实施记录：以上真实终端矩阵未执行。重定向、颜色模式、dry-run、无确认归档、参数错误码和交互拒绝路径已用 release 二进制实测；键位与光标恢复只有状态机/CursorGuard 单测。用户已批准将该项作为明确保留的未验证限制，不得声称已通过。

### 长期文档

实施交付时更新 README.md 的交互示例和自动化建议；更新 docs/architecture.md 的端口边界、终端状态与错误语义；新增 docs/cli.md 记录矩阵、颜色优先级、键位和兼容例外，并在 docs/README.md 导航。将 doco/architecture.md 从占位说明补成已实现边界摘要并链接既有详细架构，不复制维护两套细节。本次只记录这些计划，不提前改写当前事实。

文档格式及 Agent 工作流未改变，docs/document-format.md、assets/skill 和安装副本无需变更；不新增持久格式 spec，也不为选型另建 ADR。最终执行证据记录在 [任务](tasks.md)，交付概要写入 proposal 的结果章节；不得因设计机械检查通过就标记功能已完成。
