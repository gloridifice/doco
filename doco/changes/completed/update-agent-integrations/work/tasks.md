# Execution tasks

- [x] 1.1 合并集成选项并调整 Enter 交互
  - Design: [implementation](implement.md)
  - Acceptance: CLI 与菜单只暴露 Most agents、Claude；磁盘路径符合约定；多选 Enter 纳入当前项后提交且保留已有选择。

- [x] 1.2 实现入口块版本和共享集成规划
  - Dependencies: 1.1
  - Acceptance: 旧入口自动升级，同版本修改需 refresh，高版本不自动降级，块外字节和既有 bundle 安全语义保持不变。

- [x] 2.1 实现 `doco update`
  - Dependencies: 1.2
  - Acceptance: 自动检测两个完整目标、更新已安装 skill/入口、拒绝缺失半边和旧路径，支持 dry-run/refresh，零安装时明确失败。

- [x] 2.2 增加集成、交互与安全回归测试
  - Dependencies: 2.1
  - Acceptance: 覆盖选项组合、目标检测、入口版本、Claude 导入冲突、旧路径、dry-run、幂等、失败零写入/回滚和 Enter 快捷确认。

- [x] 3.1 同步当前文档并完成总体验证
  - Dependencies: 2.2
  - Acceptance: README、架构和 CLI 契约与实现一致；格式、clippy、全测试及 debug/release help 冒烟通过。

## Verification

- `cargo fmt --all --check`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过，无警告。
- `cargo test --all-targets`：9 个测试目标共 88 项通过。
- `cargo build --release --locked`：通过。
- debug 与 release 的 `doco --help`、`doco init --help`、`doco update --help`：确认 update 可见，init 只列出 `most`、`claude`，update 暴露 dry-run/refresh。
- `target/release/doco.exe update --dry-run --no-interactive --color never`：仓库自身 skill 和 AGENTS 入口全部 SKIP，零写入。
- tui-test 真实终端：初始项直接 Enter 后仅安装 Most agents；先 Space 选 Most、Down 移到 Claude、Enter 后两项均安装，两个进程均退出 0。
- `git diff --check`：通过；仅报告现有 Windows checkout 的 LF/CRLF 提示。
- `target/debug/doco.exe check update-agent-integrations`：机械检查通过；仅有路径形文本的候选引用警告。
