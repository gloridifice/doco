# list 生命周期时间距今实施设计

## 1. 现状、目标与设计基线

当前 [列表查询](../../../../../src/lifecycle/list.rs)递归遍历工作包，以最新文件系统 mtime 生成 `ChangeRow.updated`。这在 Git checkout 后失真，且当前代码使用 `modified.duration_since(now)`，对过去时间回退为零。状态仍由 active、completed、archived 三个变更目录唯一决定；归档只保留 proposal；本机索引不跨机器共享。

目标是在不改变状态事实来源和索引格式的前提下，把创建、完成、归档事件时间作为 proposal 内的机器可读元数据持久化，使 clone/pull 后的 list 结果稳定。原设计中“不持久化时间戳、取整个树 mtime”的固定决策由本次用户批准替代。

## 2. 整体方案

在 proposal 非代码区使用唯一单行 HTML 注释：

```md
<!-- doco:lifecycle v=1 created-at=2026-09-14T10:00:00Z completed-at=- archived-at=- -->
```

时间采用 RFC3339、UTC、秒精度；`-` 表示该事件尚未记录。标记由 package 模块解析和更新，新增 proposal 将标记放在标题前，proposal-only 保持 mode 标记为首行。旧 proposal 可以没有标记；首次后续生命周期写入时插入标记，但未知旧事件保持 `-`。

`new` 在临时骨架提交前记录 created-at。`complete` 在机械检查、并发复核和索引撤销后原子更新 proposal，再移动到 completed。`archive`/`cancel` 在正式执行、锁内复核和索引撤销后把 archived-at 与可能的取消结果一起原子写入，再按现有流程清理和移动。dry-run 不写入时间。

`list` 只读取 proposal 元数据并按目录状态选择字段，不再遍历树或读取 mtime。所有行共享一次查询时刻。状态过滤仍在任何 proposal/task 摘要读取前发生。

## 3. 关键 API 与数据模型

在 [package 模块](../../../../../src/package.rs)增加内部生命周期模型：

```rust
pub(crate) struct LifecycleTimes {
    pub created_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub archived_at: Option<OffsetDateTime>,
}

pub(crate) enum LifecycleEvent {
    Created,
    Completed,
    Archived,
}
```

解析函数区分“无标记”的兼容旧包与“标记存在但非法”的格式错误。更新函数保留 BOM、换行风格、mode 标记和正文，只替换唯一合法标记；无标记时在前导 BOM及 proposal-only mode 标记之后插入。

UI 模型改为 `LifecycleAge::{Known(Duration), Unknown}`，`ChangeRow.updated` 改名为 `age`。若需要保留公开源码兼容，可为旧 `ModifiedAge` 提供类型别名；文本协议第四列是有意的语义变更。

时间生成与解析复用 `time` 依赖。写入始终格式化为 `YYYY-MM-DDTHH:MM:SSZ`；解析接受 RFC3339 秒精度并按绝对时刻计算。索引的 `updated_at` 可复用公共时间辅助函数，但索引 CSV 不保存生命周期字段。

## 4. 核心算法与实现规则

状态到事件字段的映射固定为：active → created-at，completed → completed-at，archived → archived-at。缺少当前字段输出 `?`；不得回退 mtime。未来时间按零时长处理。格式规则保持：小于一小时为整分钟，小于一天为整小时，一天及以上为整天并追加非零剩余整小时。

metadata 解析只识别代码围栏外、完整独立行的 `doco:lifecycle` 标记。版本、字段集合和顺序必须符合 v1；重复标记、未知版本、非法时间、分数秒或重复字段报错。工具生成 UTC `Z` 时间；不以 created ≤ completed ≤ archived 为硬约束，避免时钟回拨和恢复状态使包不可操作。

完整包和 proposal-only 均使用同一元数据。检查器应把非法标记报告为格式错误，但缺失标记不警告，以保持旧包兼容。用户删除新包标记后视为旧格式，list 显示 `?`，后续命令不会猜测已丢失时间。

complete 在移动前写 completed-at。若移动失败，active 包可能已有完成尝试时间，但 list 仍读取 created-at；重试覆盖 completed-at。archive/cancel 同理，源状态不读取 archived-at，重试覆盖它。此顺序保持失败后可用原命令重试，错误信息应说明 proposal 可能已更新。

归档预览增加将更新 archived-at 的范围说明，但不生成或持久化预览时刻。归档最终仍仅保留 proposal。reopen 保留所有已有字段，再次 complete 覆盖 completed-at，因此只表示最近一次完成而不是事件历史。

## 5. 固定决策与可自主调整范围

固定：时间存储在 proposal HTML 注释；v1 三字段；UTC RFC3339 秒精度；旧包缺失显示 `?`；不回退 mtime；状态选择对应事件；AGE 替代 UPDATED；reopen 保留 created-at，再完成覆盖 completed-at；cancel 记录 archived-at；索引不保存事件时间。

可自主调整内部函数、类型和模块名、标记插入辅助实现、错误措辞及公开旧类型别名。不能改为 sidecar、Git 推导、文件系统时间、缓存时间或把标记当作状态来源。

Blocked: none
Open questions: none

## 6. 验证与长期文档影响

单测覆盖 metadata 空缺、合法 round-trip、三事件更新、重复/非法/围栏标记、BOM、LF/CRLF、proposal-only 插入位置、时间格式和年龄边界。集成测试覆盖 new、complete、reopen/re-complete、archive、cancel、dry-run、旧包 `?`、mtime 不参与结果、隐藏状态不读取以及归档只保留 proposal。

运行 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`、`cargo build --release --locked` 和 `cargo +1.85 check --all-targets --locked`。同步 [CLI 当前契约](../../../../specs/cli.md)、[文档格式](../../../../specs/document-format.md)和[当前架构](../../../../architecture.md)。若 skill 工作流文案无需理解该内部标记，则不升级 skill bundle。
