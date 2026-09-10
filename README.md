# doco

用文件管理项目当前文档和一次性变更工作包的 Rust CLI。使用 `clap` 解析命令，不包含模型运行时，也不自动提交、发布或回滚代码。

## 安装与使用

```sh
cargo install --path . --locked

# 在目标项目根目录运行；非交互环境必须明确选择 Agent。
doco init --agent codex --agent pi
doco new bounded-event-queue
```

`new` 只创建骨架。由开发者或 Agent 填写 `proposal.md`、`work/implement.md` 和 `work/tasks.md`，再实施和验证：

```sh
doco context bounded-event-queue
doco check bounded-event-queue

# 完成真实验收、更新当前文档与 proposal 结果后：
doco complete bounded-event-queue

# 预览后，明确授权删除 work/：
doco archive bounded-event-queue --dry-run
doco archive bounded-event-queue --yes
```

通过 `--root <目录>` 明确指定其他项目；默认仅使用当前目录，不向上猜测项目根。`doco --help` 和 `doco <命令> --help` 是完整命令参数入口。

初始化支持 Codex、Pi、Claude Code；Codex/Pi 默认共享项目 `.agents/skills/doco`。`--dry-run` 不写文件；`--refresh` 仅刷新识别出的受管接入内容，不覆盖项目事实。安装完成不代表已验证 Agent 会话加载。

## 开发

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
```

- [当前文档与实现边界](docs/README.md)
- [模块架构与失败恢复](docs/architecture.md)
- [机械检查支持的文档格式](docs/document-format.md)
- [原始设计案](doco-design-v2.md)：保留的产品设计输入，不代替当前实现说明。
