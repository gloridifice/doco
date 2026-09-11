# 实现设计

## 1. 现状、目标与设计基线

`src/cli.rs` 的 `New` 仅接收 ID，`src/lifecycle/mod.rs::new_change_with_ui` 总是创建 `work/` 和三个模板文件。`src/check/mod.rs::inspect` 对所有非 archived 变更无条件读取 implement/tasks；list、context、complete 和 reopen 同样假设 work 存在。archive 已能在部分清理后接受缺失 work，但会把它标为恢复状态。

`assets/skill/` 是发布 skill 的唯一模板源，`src/templates.rs::navigation` 生成项目根 Agent 入口；本仓库的 `AGENTS.md` 和 `.agents/skills/doco/` 是已安装副本。当前工作区已有其他 active 变更造成的未提交修改，涉及 `assets/skill/SKILL.md`、`.agents/skills/doco/SKILL.md`、`doco/architecture.md`、`doco/specs/cli.md`、init/safety 源码及测试；实现必须保留并合并这些修改，不得覆盖或依赖其他 active 工作包内容。

目标是在不改变旧包解释的前提下增加显式 proposal-only 格式，并让所有生命周期读取同一个模式判断。行为/细节修改是否创建 doco 由用户和 Agent 工作流决定，不由 CLI 强制。

## 2. 整体方案

在 check 层定义共享的变更包模式解析：proposal 中唯一的 `<!-- doco:change mode=proposal-only -->` 表示仅 proposal；无标记表示兼容的完整格式；未知或重复的 `doco:change mode=` 标记报错。模式由已安全读取的 proposal 文本决定，不能根据 work 是否存在推断。

CLI 为 `new` 增加 `--proposal-only`。生命周期创建函数保留现有公开入口并默认完整格式，新增显式格式入口供 CLI 调用。proposal-only 创建时不建立 `work/`，并向普通 proposal 模板插入模式标记。

check 根据共享模式分支。完整格式保持既有 implement/tasks 检查；proposal-only 要求 `work/` 不存在，只检查 proposal、链接和 blocker，并在严格完成检查中要求 proposal 验证证据。context、list、reopen、complete 和 archive 复用同一模式，不各自猜测。

Agent 导航和 skill 文本把“维护当前文档”“显式受跟踪变更”和“直接代码修改”分开：直接行为/细节修改允许不建变更，Git/PR 保存实现历史；选中受跟踪变更后才使用其 proposal 和可选 work。

## 3. 关键 API 与数据模型

新增类似以下领域类型和解析入口，名称可按现有模块风格微调：

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageMode {
    Full,
    ProposalOnly,
}

pub fn package_mode(proposal: &str) -> Result<PackageMode>;
```

模式标记固定为：

```md
<!-- doco:change mode=proposal-only -->
```

解析只识别 HTML 注释中的精确键和值。无标记返回 `Full`；重复标记或任何未知 mode 返回错误，避免含糊解释。archived 也保留并校验 proposal 中的标记，但 archived 的目录仍只允许 proposal。

创建 API 保留 `new_change` 和 `new_change_with_ui` 的现有签名并默认 `Full`。新增接受 `PackageMode` 的入口，避免破坏库调用方。CLI `New` 增加布尔 `proposal_only` 并映射到领域枚举。

验证证据识别复用 tasks 中现有 `Verification:` / `验证：` / `验证记录：` 字段及验证章节规则，但抽成可用于任意 Markdown 文本的帮助函数。proposal-only 仅在 complete 或 completed check 的严格模式要求证据；active 普通 check 不因尚未验证失败。

`TaskCount` 增加通用“不适用”语义，或复用统一的 `-` 渲染分支；proposal-only 与 archived 都显示 `-`，完整包任务文件缺失仍是 `Missing` 并显示 `?`。

## 4. 核心算法与实现规则

1. 安全读取 proposal，完成既有必写章节、placeholder 和 final Result 检查。
2. 扫描内容区外不做模式推断；从原始 Markdown 中匹配受控 HTML 注释标记，统计 `doco:change mode=` 声明：零个为 Full，一个合法值为 ProposalOnly，其余报错。
3. archived：仍只允许 proposal，完成 proposal 链接和模式校验后返回。
4. Full：安全读取 `work/implement.md` 与 `work/tasks.md`，执行现有章节、任务、依赖、blocker、验证和链接检查。整个 `work/` 缺失仍失败。
5. ProposalOnly：安全检查 `work` 路径；只要存在文件、目录或不安全对象就报冲突。检查 proposal blocker。严格模式要求 proposal 中有实际验证字段/章节。
6. list 先安全读取 proposal 并解析模式；ProposalOnly 输出不适用，Full 才读取 tasks。非法 proposal/路径错误继续使 list 失败，不伪装成 `-`。
7. context 始终加入 architecture 和 proposal；仅 Full 将两个 work 文件入队。
8. reopen 校验 proposal 与模式；Full 读取两个 work 文件，ProposalOnly 确认 work 不存在，再原样移动。
9. archive 根据模式解释 work 缺失：ProposalOnly 正常无删除项；Full 缺失时保留现有部分清理重试语义。两者都继续检查 retained links、并发快照和目标目录。
10. proposal-only 执行中如果出现重要设计选择、依赖任务或风险扩张，工作流要求先转换为完整格式：移除模式标记并同时创建两个模板文件，填完后 check；CLI 本次不新增自动转换命令。

路径安全、锁、原子写、目录移动和归档并发复核保持现有实现。模式不持久化到数据库或额外 sidecar，proposal 是唯一来源。

## 5. 固定决策与可自主调整范围

固定决策：模式必须显式标记；无标记永远解释为 Full；proposal-only 不允许 `work/`；CLI 参数名为 `--proposal-only`；完成必须有 proposal 验证证据；不按规模自动分类；doco 不作为实现历史；默认创建和旧公开 API 保持完整格式。

固定兼容约束：不能因 work 缺失自动降级旧包；完整包的任务计数、检查和归档恢复行为保持；archived 仍只有 proposal；未选择 doco 跟踪的代码修改仍应按影响更新当前文档。

可自主调整：共享枚举和帮助函数的具体模块位置、错误文本细节、模式 marker 插入 proposal 的具体标题前空行、`TaskCount` 内部枚举命名以及测试夹具提取方式，只要公开输出契约和安全语义一致。

阻塞：无。

## 6. 验证与长期文档影响

新增集成测试覆盖 proposal-only 创建、check、完成证据、list、context、reopen、archive、cancel，以及标记与 work 冲突、未知/重复标记和完整包缺 work 不降级。保留并运行现有完整生命周期、任务、输出和 init 测试。执行 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`、`cargo build --release --locked`。

更新 `doco/architecture.md` 的持久格式和生命周期分支，更新 `doco/specs/document-format.md` 的两种合法形态及严格完成规则，更新 `doco/specs/cli.md` 的 new 参数、TASKS 显示和 archive 输出，更新 README 快速说明。更新 `src/templates.rs::navigation`、根 `AGENTS.md`、发布 skill 及本仓库安装副本；skill bundle 版本按实现时已合并基线递增，以便 init 刷新安装内容。
