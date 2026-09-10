# CLI 视觉与交互优化

## 目的

doco 当前主要输出逐行纯文本，初始化需要手输 Agent 名称，操作变更需要反复复制 ID。状态、警告、删除预览和普通说明的视觉区分不足。改进终端阅读和选择体验，同时保留面向 Agent、脚本及 CI 的可预测行为，不改变文件安全或生命周期规则。

## 范围与完成标准

### 范围

- 为业务输出建立统一的语义渲染层：状态色、列表对齐、操作标签、警告/错误以及初始化 diff；不添加全屏 TUI。
- 新增全局 `--color auto|always|never`，默认 auto，尊重非空 NO_COLOR 和 TERM=dumb。颜色只作辅助，始终保留文字标签；重定向默认不含 ANSI 控制序列。
- 初始化缺省 Agent 时使用多选器；保持显式重复 --agent 参数及去重行为。
- 新增互斥的全局 `--interactive`、`--no-interactive`。只有显式 --interactive 才允许 context/check/complete/archive/reopen/cancel 在省略 ID 时选择已有变更；new 始终要求显式 ID。明确传入的 ID 不被菜单覆盖。
- 归档与取消保留完整删除预览、预检和执行前并发快照；不再要求破坏性文本确认。--dry-run 仍只预览，--yes 作为兼容参数保留但不再影响行为。取消原因和代码处置仍必须明确提供。
- 迁移分散的业务打印和终端选择，提供可注入的输出接口及可测试的选择规则。

### 可观察验收

1. TTY 的 list 显示 STATE / CHANGE / TASKS 标题、对齐列和总数，TASKS 为已完成/总任务数；已归档显示 -，任务文件缺失显示 ?，空文件显示 0/0。非 TTY 按原 ID 顺序输出 state、ID、任务数三列 TSV，空列表仍无 stdout 内容。计数仅反映已识别的任务行，不代替 check 或真实验收。
2. auto 在相应输出流不是 TTY、NO_COLOR 非空或 TERM=dumb 时不着色；always 可强制着色，never 无 SGR。颜色、布局、是否可交互独立判定。
3. 支持方向键选择、Space 多选、Enter 提交、Esc/Ctrl-C 退出；Agent 默认无选项勾选，变更只有一个候选时也不自动执行。菜单取消退出码 130，且未执行业务写操作。
4. new 缺少 ID，以及其他命令省略 ID 且未传 --interactive，仍按参数错误退出；不能交互时不等待输入，也不从重定向 stdin 读取回答。
5. archive/cancel 通过预检后无需确认即执行；--dry-run 不写入，兼容的 --yes 不选择目标、不填补参数、不绕过预检。菜单取消发生在预检前，不删除文件。
6. 原有初始化、检查、上下文、生命周期及安全回归继续通过；补充颜色策略、流分离、选择过滤、终端取消/恢复与执行前并发变化测试。
7. 更新用户指南及已实现架构说明，记录开关、键位、退出码和非交互兼容边界；设计阶段不把计划写成当前事实。

### 非目标与契约边界

不做仪表盘、无参数主菜单、主题配置文件、JSON 输出、国际化、动画、网络访问或新增生命周期状态；不自动 complete/archive，不弱化机械检查，不调整持久文档格式、锁或恢复算法。CLI 文案继续使用英文，设计文档使用中文。

普通非交互业务输出的行内容、顺序与原 stdout/stderr 归属保持兼容；新增参数错误和交互诊断除外。TTY list 布局及管道追加任务数第三列是有意变更。Clap 的帮助、版本与参数错误统一保留无色标准格式，--color 仅控制业务输出，不承诺美化 Clap 页面。控制字符将转为可见转义，不能借文档内容或路径注入终端指令。

## 结果

已交付。库侧新增 ui 模块的语义事件与 Reporter 端口（含 PlainReporter 兼容适配器），二进制侧新增 terminal 模块的能力探测、业务彩色渲染与 console 局部选择循环；新增全局 `--color` 与互斥 `--interactive`/`--no-interactive`；`init` 缺省 Agent 改为多选，除 `new` 外省略 ID 且显式 `--interactive` 时按各命令状态过滤选目标；`archive`/`cancel` 保留完整预览、锁内复核与 `--dry-run`，通过预检后无需确认即执行；`list` 增加已完成/总任务数（TTY 表头、对齐列与统计，管道三列 TSV，归档 `-`、缺失 `?`、空文件 0/0）。

实施中经用户批准的范围调整：不提供 `new` 文本交互，也不要求 `archive`/`cancel` 破坏性确认，`--yes` 仅作兼容参数保留且不补目标、不改变执行；为支持 Ctrl-C 菜单取消，改用 console 局部选择循环替代 dialoguer；`list` 任务数显示为追加任务 2.1–2.2。

验证结论：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`（62 项，含颜色矩阵、管道 TSV、选择过滤、无确认归档与并发复核回归）、`cargo build --release --locked`、`cargo +1.85 check --all-targets --locked` 全部通过；release 二进制在临时项目实测管道 TSV 与 ID 顺序、`--color` 的 always 与 never、`--dry-run` 无写入、无 `--yes` 归档、`new` 缺 ID 退出 2、`--interactive` 在重定向流下退出 1。经用户批准，原验收中的真实终端键位与光标恢复矩阵本阶段不可执行，仅由键位状态机与 CursorGuard 单测覆盖，属明确保留的未验证限制。

已同步当前文档：README.md、docs/README.md、docs/architecture.md、新增 docs/cli.md，并将 doco/architecture.md 从初始化占位替换为已实现边界摘要；无新增 spec 或 decision。
