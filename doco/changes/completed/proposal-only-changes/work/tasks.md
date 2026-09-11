# 执行任务

- [x] 1.1 增加显式变更包模式与 proposal-only 创建入口
  - 完成条件：`new --proposal-only` 只创建带合法模式标记的 proposal；默认和旧公开创建入口仍生成完整工作包；重复或未知模式标记有明确错误。

- [x] 1.2 按模式调整机械检查和验证证据识别
  - 依赖：1.1
  - 完成条件：完整包维持现有检查；proposal-only 拒绝任何 work，严格完成时要求 proposal 验证证据且阻塞项仍生效；完整包缺 work 不会自动降级。

- [x] 2.1 调整 list、context、complete、reopen、archive 和 cancel
  - 依赖：1.2
  - 完成条件：全部生命周期按共享模式工作；proposal-only 的任务列显示 `-`、context 不加入 work、归档不误报恢复；完整包部分归档恢复保持可重试。

- [x] 3.1 更新 Agent 工作流入口与发布 skill
  - 依赖：1.1
  - 完成条件：生成导航、根 AGENTS、发布 skill 和安装副本明确 doco 变更可选且不保存实现历史，并说明仅 proposal 的适用条件和执行/完成/归档规则；bundle 版本按当前基线递增。

- [x] 3.2 更新当前文档和 README
  - 依赖：1.2, 2.1, 3.1
  - 完成条件：架构、文档格式、CLI 契约和 README 准确描述两种格式、兼容规则、任务显示、验证要求及实现历史边界。

- [x] 4.1 增加兼容、冲突和完整生命周期测试
  - 依赖：2.1
  - 完成条件：测试覆盖 proposal-only 的创建、检查、完成证据、列表、上下文、重开、归档、取消，覆盖模式冲突和完整包缺 work，并保持旧包行为。

- [x] 4.2 运行整体验证并复核当前文档
  - 依赖：3.2, 4.1
  - 完成条件：格式、Clippy、全部测试和 release 构建通过；记录实际命令、结果和限制，确认未覆盖其他 active 变更。

## 验证记录

- `cargo fmt --all --check`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo test --all-targets`：通过，共 77 个测试（10 个 lib 单元测试、14 个 binary 单元测试、53 个集成测试）。
- `cargo build --release --locked`：通过。
- `cargo run --quiet -- init --agent codex --dry-run`：全部现有目录、skill 文件和 AGENTS 受管区块均为 SKIP，无写入或冲突。
- `cargo run --quiet -- new --help`：确认公开帮助包含 `--proposal-only` 及用途说明。
- `git diff --check`：通过；仅输出工作区既有的 LF/CRLF 转换提示，无空白错误。
- 保留了执行前已存在的其他 active 变更和未提交实现；本变更在其 skill bundle 版本机制上将当前版本递增为 v2，未修改其他 active 工作包内容，也未执行 Git、complete 或 archive。
