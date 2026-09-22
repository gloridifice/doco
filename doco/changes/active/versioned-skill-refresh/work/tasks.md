# Execution tasks

- [x] 1.1 在根 SKILL.md 建立唯一 bundle 版本与升级分类
  - Design: [implementation](implement.md)
  - Acceptance: 只有内置根 `SKILL.md` 写入约定的 `doco:skill version=v1` HTML 标记；CLI 能严格验证内置版本，并把已安装的缺失或不可识别版本视为 `v0`，同时保持 doco 文件归属检查独立有效。

- [x] 1.2 为初始化计划增加可回滚文件组
  - Dependencies: 1.1
  - Design: [implementation](implement.md)
  - Acceptance: 文件组参与计划展示、全量预检、effective text 和锁内执行；支持按 expected snapshot 恢复原文件、删除本次创建文件及空目录，回滚不会覆盖外部并发修改，并能报告完整或失败的补偿结果。

- [x] 1.3 按“其他文件优先、根 skill 最后”接入 bundle 更新
  - Dependencies: 1.1, 1.2
  - Design: [implementation](implement.md)
  - Acceptance: 新安装、旧版自动升级和显式刷新都先处理全部其他已知文件，最后提交根 `SKILL.md`；任一步失败立即逆序回滚并停止后续初始化，未知文件保留，同版本与高版本规则符合设计。

- [x] 1.4 扩展 Pi 兼容目录的复用与升级
  - Dependencies: 1.3
  - Design: [implementation](implement.md)
  - Acceptance: Pi-only 初始化可复用受管旧版目录并通过同一文件组升级；无关根 skill、不受管的已知文件、同版本差异和高版本差异仍在写入前冲突。

- [x] 2.1 增加版本、顺序和事务回滚测试
  - Dependencies: 1.3, 1.4
  - Acceptance: 测试覆盖 `v0` 回退及版本边界、全 bundle 自动升级、根 skill 最后提交、每个写入点故障注入、回滚自身失败/并发保护、新安装清理、显式刷新、Pi 复用、BOM/CRLF 和未知文件保留。

- [x] 2.2 同步当前架构与 CLI 行为文档
  - Dependencies: 1.3, 1.4
  - Acceptance: `doco/architecture.md` 准确描述根 skill 唯一版本、组内提交顺序、补偿回滚及跨组非事务边界；`doco/specs/cli.md` 说明自动升级、`v0`、失败输出和 `--refresh` 关系，不修改无关文档格式规范。

- [x] 2.3 规范化内嵌 bundle 换行并修复跨平台字节比较
  - Dependencies: 1.3, 2.1
  - Design: [implementation](implement.md)
  - Acceptance: 内置 `templates::FILES` 以 LF 暴露规范内容；新安装、自动升级和模板生成不再继承构建检出换行；更新已有文件仍沿用目标文件换行风格，既有 LF/CRLF 支持不变。
  - Verification: 在全部 `assets/skill/**` 为 CRLF 的检出（等同 `windows-latest` CI）下 `cargo test --all-targets` 通过，专项 `v3_bundle_upgrade_installs_optional_spec_template_without_refresh` 通过；release 二进制对空项目 `init --agent most` 后 `SKILL.md`、`references/create.md`、`templates/spec.md` 均为 LF。

- [x] 3.1 执行最终验证并复核失败语义
  - Dependencies: 2.1, 2.2, 2.3
  - Acceptance: `cargo fmt --check`、故障注入定向测试、完整 `cargo test` 和 `doco check versioned-skill-refresh` 均通过；验证记录包含实际命令、结果与限制，并确认可捕获写入失败不会留下部分 skill 更新。

## Verification

- `cargo test init::plan::tests --lib`：4 个文件组顺序、逐写入点回滚、新建清理和并发保护测试通过。
- `cargo test --test init`：16 个 init 集成测试通过，包含仅根 skill 版本、`v0` 自动升级、最终提交顺序、高版本拒绝自动降级及 Pi 旧版目录复用。
- `cargo fmt --check`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过，无警告。
- `cargo test`：73 个测试通过，0 失败；doc tests 为 0。
- `git diff --check`：通过；仅提示工作区 Rust 文件未来由 Git 转为 CRLF，无空白错误。
- `target/debug/doco.exe check versioned-skill-refresh`：机械检查通过；仅有路径形文本的候选引用警告。
- `target/debug/doco.exe init --agent codex --dry-run --no-interactive --color never`：仓库自身安装副本和发布 bundle 全部为 SKIP，无写入。
- `grep -R "doco:skill version" assets/skill .agents/skills/doco`：版本标记只出现在两个根 `SKILL.md` 安装/发布副本中，均为 `v1`。
- 限制：故障注入覆盖可捕获的组内失败和外部并发改写；未模拟断电、进程强制终止或持续文件系统故障，设计与当前文档明确不对此承诺物理事务。
- 换行规范化回归：全部 `assets/skill/**` 转为 CRLF 后稳定复现原 Windows CI 失败（安装内容 LF vs 内嵌 CRLF），修复后同一检出下 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets` 与 `cargo build --release` 全部通过（12 个 test binary，0 失败）。
