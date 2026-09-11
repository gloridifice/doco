# update-agent-integrations

## Purpose

当前 `init` 把 Codex、Pi 和 Claude 当成三个 Agent 选项，但 Codex 与 Pi 已共享 `.agents/skills/doco` 和 `AGENTS.md`，选项与实际安装目标不一致；升级后也缺少一个只刷新既有集成、不会重跑项目脚手架安装的命令。

本变更将安装目标收敛为 Most agents 与 Claude，并增加 `doco update`，用于检查和更新已安装的 skill bundle 与对应入口文件。

## Scope and acceptance

- `doco init --agent` 只接受 `most` 与 `claude`：Most agents 写入 `.agents/skills/doco` 和 `AGENTS.md`，Claude 写入 `.claude/skills/doco` 和 `CLAUDE.md`；交互菜单只显示这两个目标。
- 多选菜单按 Enter 时把当前高亮项纳入选择并立即确认；Space 仍切换选择，先前已选择项保留，Esc/Ctrl-C/q 仍取消。
- `doco update [--dry-run] [--refresh]` 不接受 Agent 参数，自动检测两个固定安装目标，只检查和更新已存在且完整的 skill/入口组合；两者都存在时全部更新。
- update 不创建缺失集成。skill 与入口只存在一侧、入口引用路径不一致、同名内容不受管或两个目标均未安装时明确失败并提示使用 `doco init`。
- skill 沿用根 `SKILL.md` bundle 版本规则；AGENTS/CLAUDE 受管块增加独立模板版本，旧无版本块按 v0 自动升级，同版本内容差异仍要求 `--refresh`，更高版本普通运行不降级，`--refresh` 可显式覆盖受管内容或强制降级。受管块外内容保持不变。
- `.pi/skills/doco` 与 `.codex/skills/doco` 不再是安装或更新目标；发现时作为旧版/替代路径冲突，要求人工迁移到 `.agents/skills/doco`。该收敛取代 [versioned skill refresh](doco:versioned-skill-refresh) 中保留 Pi-only 目录复用的旧决定，但不改变其版本与回滚机制。
- `CLAUDE.md` 的 `@AGENTS.md` 仅表示 Claude 复用 Most agents，不构成独立 Claude 安装；独立 Claude skill 与导入/重复 doco 块并存时拒绝。
- update 复用 init 的预检、计划输出、写锁、快照复核、原子替换和 skill bundle 回滚；dry-run 零写入，重复运行幂等。
- 增加命令解析、目标检测、入口版本、冲突、回滚、Enter 快捷确认和现有生命周期菜单回归测试，并同步 README、当前架构和 CLI 契约。

非目标：不更新 `doco/` 当前文档或变更包，不创建目录、脚手架或缺失入口，不自动移动/删除旧 skill，不调用模型、不执行 Git，不保留 `codex`/`pi` 为公开或隐藏 `--agent` 别名。

## Result

已交付。

- `init --agent` 收敛为 `most`（`.agents/skills/doco` + `AGENTS.md`）与 `claude`（`.claude/skills/doco` + `CLAUDE.md`），移除 `codex`/`pi` 取值；`.pi/skills/doco`、`.codex/skills/doco` 只报迁移冲突，不再自动复用。
- 新增 `doco update [--dry-run] [--refresh]`：只检测并刷新已存在的完整集成，不创建缺失内容；集成不完整、同名内容不受管、发现旧路径或两个目标都不存在时拒绝并提示 `doco init`。
- DOCO 入口块新增唯一模板版本 `<!-- doco:entry template=v1 -->`：缺失、非法或重复按 `v0` 比较并自动升级；同版本内容差异需 `--refresh`；更高版本普通运行不降级。
- `CLAUDE.md` 的 `@AGENTS.md` 视为复用 Most agents，不算独立 Claude 安装；与独立 Claude 集成并存时拒绝。
- `init` 交互菜单只显示 Most agents 与 Claude；多选菜单 Enter 选中当前高亮项并立即确认，Space 选择的项保留。
- 同步 `README.md`、`doco/architecture.md` 与 `doco/specs/cli.md`。

验证结论：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`（9 个目标共 88 项）、`cargo build --release --locked`、`doco check update-agent-integrations` 与 `git diff --check` 全部通过；release 二进制的 `doco update --dry-run` 对仓库自身零写入。tui-test 真实终端确认直接 Enter 只装 Most agents，Space 选 Most 后移动到 Claude 再 Enter 会安装两个集成且退出码为 0。语义一致性与需求符合度仍由人工评审确认。
