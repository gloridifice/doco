# 实施任务

本清单记录已获准的实施。任务只在实际验收后勾选；追加 list 任务数显示见 2.1–2.2。

- [x] 1.1 建立输出与交互端口及终端能力策略
  - Design: [模块、接口与颜色策略](implement.md)
  - Acceptance: 新增 Reporter、语义事件、纯文本兼容适配器、类型化取消及逐流能力策略；console 依赖满足 Rust 1.85；策略单测覆盖颜色、重定向、禁用交互的独立性，无进程全局样式开关。

- [x] 1.2 实现业务输出渲染与列表视觉样式
  - Dependencies: 1.1
  - Design: [视觉、列表及文本安全](implement.md)
  - Acceptance: 实现状态标签、diff 着色、TTY 表头/计数/空态/窄屏布局和控制字符转义；管道 list 保持 TSV、ID 顺序和空输出；内存 writer 测试覆盖有色/无色与失败传播。

- [x] 1.3 将现有业务打印迁移到统一端口
  - Dependencies: 1.1, 1.2
  - Design: [模块职责与公开 API 兼容](implement.md)
  - Acceptance: init、Plan、Report、lifecycle、context 及顶层错误使用统一 Renderer；CLI 走新入口，旧公开签名委托纯文本共享实现；原业务消息文本、顺序、stdout/stderr 归属和逐项部分执行记录不丢失。

- [x] 1.4 接入参数开关、Agent 多选和变更目标选择
  - Dependencies: 1.1, 1.2
  - Design: [参数与目标选择矩阵](implement.md)
  - Acceptance: 新增 --color、互斥交互开关及有条件必填 ID；init 空 Agent 多选和各命令候选过滤符合设计；new 始终要求显式 ID，显式已有 ID 不弹菜单，单候选不自动执行，零候选有诊断，非交互不等待输入，cancel 原有必填字段保留。
  - Blocked: none
  - Decision: 用户已批准 terminal/prompt.rs 的局部 console 选择循环以支持 Ctrl-C。

- [x] 1.5 接入无确认的破坏性执行并统一菜单取消行为
  - Dependencies: 1.3, 1.4
  - Design: [破坏性操作执行与状态资源](implement.md)
  - Acceptance: archive/cancel 先完整预览，非 dry-run 无文本确认并立即进入锁内复核与执行；--yes 为兼容参数且不补目标、不改变结果，dry-run 无写入；目标菜单取消退出 130 且无业务写入，终端状态在菜单各退出路径恢复。
  - Blocked: none
  - Decision: 无确认的 archive/cancel 执行路径已实现，目标菜单按用户决定改用支持 Ctrl-C 的局部选择器。

- [x] 1.6 补充自动化兼容及安全回归测试
  - Dependencies: 1.3, 1.4, 1.5
  - Design: [自动验证](implement.md)
  - Acceptance: Sandbox 可分别断言 stdout/stderr/退出码并隔离子进程环境；覆盖颜色矩阵、Clap 缺参/帮助、纯文本兼容、选择取消和候选过期；预览至锁定间修改文件、预览输出失败、--yes 有无等价及 dry-run 无写入均有断言；原有生命周期与文件安全测试继续通过。

- [x] 1.7 更新已实现行为的用户与架构文档
  - Dependencies: 1.3, 1.4, 1.5
  - Design: [长期文档影响](implement.md)
  - Acceptance: README.md、docs/README.md、docs/architecture.md 和新增 docs/cli.md 准确说明实际接口、颜色优先级、交互矩阵与兼容边界；doco/architecture.md 用已实现摘要替换占位入口并链接详细架构；不提前宣称能力，不修改未受影响的文档格式或 Agent skill。

- [x] 1.8 执行最终验证并记录交付证据
  - Dependencies: 1.6, 1.7
  - Design: [自动与真实终端验证](implement.md)
  - Acceptance: 执行 fmt、clippy、全部测试、locked release build，核验 Rust 1.85 兼容；在临时项目用 release 二进制核对重定向输出、颜色模式、dry-run 无写入、无确认归档、参数错误退出码与交互拒绝路径；如实记录实际命令、平台、结果以及本环境未能执行的真实 TTY 键位验收。
  - Decision: 用户要求标记任务完成，并批准将原验收中的“Windows/Unix 真实终端键位与光标恢复矩阵”按本节验证记录收窄为本阶段不可执行并明确记录的限制；该矩阵仍属未验证项，不得当作已通过。
  - Blocked: none

- [x] 2.1 在 list 中展示已完成/总任务数
  - Dependencies: 1.8
  - Design: [任务数增量设计](implement.md)
  - Acceptance: TTY 宽/窄布局与管道第三列均显示 done/total；归档为 -、缺失为 ?、空文件为 0/0；计数复用任务解析器，渲染层不读文件，不改变 Change 状态模型与菜单。

- [x] 2.2 验证任务数显示并同步文档
  - Dependencies: 2.1
  - Acceptance: 覆盖混合勾选、代码围栏/注释、CRLF/BOM、空文件、缺失、归档、不安全路径和渲染布局；fmt、clippy、完整测试、release build 及 MSRV 检查通过，更新 CLI 文档并记录实际验证限制。

## 验证记录

- 2.1–2.2：cargo test --all-targets 共 62 项通过；新增 tests/list_tasks.rs 覆盖混合勾选、CRLF/BOM、围栏/注释、空文件、缺失、归档、硬链接、非法编码和目录冒充文件。宽/窄 TTY 渲染用内存 writer 测试，PlainReporter 与 CLI 三列 TSV 一致。
- 2.1–2.2：cargo fmt --all --check、cargo clippy --all-targets -- -D warnings、cargo build --release --locked、cargo +1.85 check --all-targets --locked 均通过；release list 在勾选本次任务前实际输出 active、cli-visual-interaction、8/10。未安装或覆盖全局 doco，未执行 complete/archive。真实 TTY 视觉验收仍未执行。

- 1.1：cargo +1.85 check --all-targets --locked 通过；锁定 console 0.16.4。最初尝试的 dialoguer 0.12 已在改用局部 console 选择循环后移除。rustup toolchain list 起初仅显示 stable，执行 +1.85 命令时 rustup 自动同步/下载 1.85.1 组件后完成检查。
- 1.2：TerminalCapabilities 与 TerminalReporter 单测覆盖逐流颜色、NO_COLOR/TERM、TTY/管道列表、空态、窄屏、diff 和控制字符；cargo test --all-targets 中相关测试通过。
- 1.3：init、check、lifecycle、context 和顶层错误均已接入 Reporter；旧公开 API 保留纯文本包装。cargo test --all-targets 首轮共 51 个测试通过，包含全部原有初始化、生命周期和安全回归。
- cargo fmt --all 与 cargo check --all-targets 已通过；git diff --check 无空白错误，仅报告仓库既有 Windows autocrlf 转换提示。
- 1.4：解析/候选过滤单测覆盖缺失 ID、--interactive/--no-interactive 冲突、参数位置、显式 ID 不扫描项目和状态矩阵；局部选择器单测覆盖方向键、Space、Enter、Esc/q/Ctrl-C。`new --interactive` 缺 ID 的子进程回归返回 2，缺 ID 的重定向输入不会被读取。
- 1.5：archive/cancel 不带 --yes 与带 --yes 的子进程回归均执行；dry-run、预览输出失败和预览后并发修改都保留未删除的工作包。archive 的锁内复核和部分恢复既有测试继续通过。
- 1.6：新增 tests/cli_visual.rs，并扩展 Sandbox 分离 stdout/stderr、退出码和子进程环境；颜色矩阵、管道 TSV、错误流、无确认归档、参数错误及安全回归均通过。完整 cargo test --all-targets 共 60 个测试通过。
- 1.7：已更新 README.md、docs/README.md、docs/architecture.md，新增 docs/cli.md，并将 doco/architecture.md 从占位替换为当前边界摘要。
- 本次实施中设计负责人明确取消 new 文本交互与 archive/cancel 破坏性确认；并明确要求菜单支持 Ctrl-C，批准以 terminal/prompt.rs 局部 console 选择循环替换 dialoguer。原 Input/Esc 与菜单 Ctrl-C 阻塞均已解除。
- 1.8：cargo fmt --all --check、cargo clippy --all-targets -- -D warnings、cargo test --all-targets（60 项）、cargo build --release --locked、cargo +1.85 check --all-targets --locked 全部通过。
- 1.8 补充 release 二进制冒烟（临时项目，Windows）：管道 list 输出 `active\tdemo`；`--color always` 管道得到 `ESC[36mactiveESC[0m\tdemo` 且无表头；`archive --dry-run` 预览后 exit 0 且 work 仍在；`archive` 不带 --yes、stdin 关闭时无提示直接归档且仅保留 proposal；`new` 缺 ID 退出码 2；`--interactive check` 在重定向流下退出码 1 且 stderr 为 `--interactive requires an attached terminal`；`--version` 退出 0。
- 未验证项：本 agent 会话 stdin 不是 TTY，winpty 亦因 stdin 不是 TTY 拒绝启动，因此未在 Windows Terminal/PowerShell 或 Unix TTY 实测方向键、Space、Enter、Esc/Ctrl-C、短屏分页、长 ID/中文路径及光标回显恢复；这些路径目前只有键位状态机与 CursorGuard 的单元测试覆盖，不能声称已通过真实终端验收。
