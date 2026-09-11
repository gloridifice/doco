# 支持仅 proposal 的轻量变更

## 目的

当前 active/completed 变更必须同时保存 `proposal.md`、`work/implement.md` 和 `work/tasks.md`。目标明确、无需独立设计或任务拆分的小型受跟踪变更也必须维护两份 work 文件，成本与收益不匹配。同时，现有 Agent 入口容易让人误以为所有行为或实现细节修改都必须创建 doco 变更；但归档只保留 proposal，doco 本身并不是实现历史系统。

本变更让用户显式选择仅 proposal 的受跟踪格式，并明确普通行为修复和实现细节修改可以不创建 doco 变更。Git、PR 或项目原有发布记录继续承担实现历史职责。

## 范围与完成标准

- `doco new <id> --proposal-only` 创建只包含 `proposal.md` 的 active 变更，并在 proposal 中写入可机械识别的 `<!-- doco:change mode=proposal-only -->` 标记；默认 `new` 和没有模式标记的既有变更继续使用完整工作包。
- active/completed 支持完整工作包和仅 proposal 两种格式。仅 proposal 格式禁止存在 `work/`；完整格式缺少 work 文件时不得被自动降级，避免绕过任务检查。
- check、complete、reopen、context、list、archive 和 cancel 均正确处理两种格式。仅 proposal 完成时必须有非 pending 的 Result、无未解决 blocker，并在 proposal 中记录非空验证证据；不要求任务复选框。
- list 对仅 proposal 和 archived 的 TASKS 显示 `-`；完整格式任务缺失仍显示 `?`。archive 将仅 proposal 无 `work/` 视为正常状态，同时保留完整包部分清理后的重试能力。
- 根 Agent 入口和随工具发布的 skill 明确：doco 变更是可选的设计/协调记录，不是实现历史；普通行为修复和实现细节修改无需强制创建变更，但无论是否跟踪，只要当前架构或规范事实受影响就必须同步当前文档。
- 更新 README、当前架构、CLI 契约和文档格式规范；保持既有无标记完整变更及公开库创建入口兼容。
- 非目标：按代码行数或风险自动判断“小变更”；替代 Git/PR/发布记录；保存完整变更归档时的 work；引入数据库或新的生命周期状态。
- 验收：新增模式及冲突/兼容/生命周期测试通过，原有测试通过，格式、Clippy 和构建检查通过。

## 结果

已交付仅 proposal 的显式变更包模式：`doco new --proposal-only` 只创建带模式标记的 proposal，全部检查、列表、上下文和生命周期命令按共享模式处理；无标记旧包继续按完整包检查，不能因 work 缺失自动降级。proposal-only 完成必须在 proposal 中记录验证证据，归档时无 work 被视为正常状态。

Agent 入口、发布 skill 和当前文档已明确 doco 不是实现历史机制，普通行为修复和实现细节修改无需强制创建变更；Git、PR 或发布记录承担实现历史，受影响的当前架构和规范仍须同步。skill bundle 当前版本递增为 v2。

验证：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`（77 个测试）和 `cargo build --release --locked` 均通过；init dry-run 显示现有 skill 与 AGENTS 受管内容全部为 SKIP。
