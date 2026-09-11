# Implementation design

## 1. Baseline and goals

`src/cli.rs` 当前定义 Codex/Claude/Pi 三个值并把 Codex、Pi 映射到共享安装；`src/init/mod.rs` 同时承担脚手架、skill、入口和 Claude 导入规划；`src/init/skill.rs` 负责版本化 bundle；`src/init/entry.rs` 负责无版本的 DOCO 入口块；`src/terminal/prompt.rs` 的多选 Enter 只提交已勾选项，空选择也会返回。

工作树已有未提交的 skill v3、complete/archive 工作流措辞、CLI 当前版本和测试调整，实施必须保留并在该基线上继续。

目标是增加只刷新既有集成的 update 路径，并让 CLI 选项、磁盘目标和交互行为一致；项目事实文档及生命周期不参与更新。

## 2. Overall approach

将集成建模为 `Most` 与 `Claude`。从 init 中抽出共享的集成规划函数：init 仍负责显式目标选择、目录/脚手架创建；update 负责检测完整的既有目标，随后复用相同 skill/入口生成器和 `Plan` 应用流程。计划顺序保持 skill bundle 在前、入口文件在后。

新增 `src/update.rs` 负责检测、update 专有冲突和 `run_with_ui`；不复制安全写入实现。入口模板版本解析留在 `init/entry.rs`，通用整数标记解析可提取小 helper，但不改变 skill 根版本格式。

## 3. APIs and data model

CLI 使用 `Integration::{Most, Claude}`，Clap 值为 `most`、`claude`。`Command::Update { dry_run: bool, refresh: bool }` 无 ID、无 Agent 参数、无交互选择。

内部以固定目标描述符表达路径：Most=`.agents/skills/doco` + `AGENTS.md`，Claude=`.claude/skills/doco` + `CLAUDE.md`。update 检测结果为有序目标集合，顺序 Most 后 Claude；每个目标要求 skill 根和入口文件同时存在。

入口块在 START 后写唯一 `<!-- doco:entry template=v1 -->`。缺失、非法、重复或溢出版本按 v0 比较，但 DOCO START/END 仍是归属边界；缺失/重复边界继续是不可覆盖冲突。`--refresh` 不授予未受管内容归属。

## 4. Algorithms and rules

init：Most 直接安装 `.agents`/AGENTS，Claude 直接安装 `.claude`/CLAUDE；两项可组合。`.pi`、`.codex` 发现即加入计划冲突。Claude 入口有独立块；若已有 `@AGENTS.md`，选择 Claude 时按重复工作流冲突处理。

update：先要求项目 initialized，再检查旧路径；随后对两个目标读取 skill 根和入口存在性。两者都不存在则失败；仅一侧存在则记录冲突且不为该目标创建另一侧；完整目标调用共享规划。Most 与 Claude 独立存在时可同时更新。CLAUDE 仅导入 AGENTS 且没有 `.claude` skill 时不是 Claude 目标。

skill 规则不变。入口生成内容相同则 SKIP；内置入口版本高于已安装时自动替换；相同或更高版本内容不同则无 refresh 冲突，有 refresh 才替换。入口外字节、BOM 与换行由现有 markdown helpers 保留。

多选 Enter 先令 `selected[cursor] = true`，再返回所有已选择索引；已选择项不被反选。单选行为不变。

## 5. Fixed decisions and discretion

固定：只有 `most`、`claude` 两个公开值，无旧别名；update 不创建缺失集成；入口独立版本；`.pi`/`.codex` 只报迁移冲突；Claude import-only 属于 Most；Enter 会加入当前项并确认。

可自主调整：共享规划 helper 的具体类型/文件位置、诊断措辞、测试辅助函数和内部版本解析 helper 名称，只要公开行为和安全边界不变。

Blocked: none.

## 6. Verification and documentation impact

运行 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets` 和 debug/release help 冒烟。测试覆盖选项、两个目标组合、旧路径、缺失半边、无安装、入口版本升级/刷新/降级、dry-run/幂等/回滚、BOM/CRLF/受管块外保留及 Enter 行为。

更新 `README.md`、`doco/architecture.md`、`doco/specs/cli.md`；仅在工作流说明需要暴露 update 时更新 bundled skill 并相应处理 skill 版本。`doco/specs/document-format.md` 不记录 Agent 集成文件格式，无需修改。
