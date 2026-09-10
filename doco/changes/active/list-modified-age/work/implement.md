# list 修改时间距今实施设计

## 1. 现状、目标与设计基线

当前工作树包含尚未提交但已完成的 `cli-visual-interaction` 交付，[列表查询](../../../../../src/lifecycle/list.rs)生成 `ChangeRow { id, state, tasks }`，[UI 端口](../../../../../src/ui.rs)的 `PlainReporter` 输出三列 TSV，[终端渲染](../../../../../src/terminal/output.rs)负责 TTY 三列表格或窄行。`Project::changes` 只从目录推导状态；`safety::tree` 已能递归检查工作包并采集每个节点的可选修改时间，但 `TreeEntry` 未暴露该值。[CLI 当前文档](../../../../../docs/cli.md)明确三列契约。

本变更只在上述列表查询和渲染链路中增加相对修改时间及显式历史状态过滤，不重写现有未提交实现，不读取文档正文推断时间，也不引入依赖或持久状态。

## 2. 整体方案

`lifecycle::list_changes` 在一次调用开始时捕获一个 `SystemTime::now()`，对每个 `Change` 调用 `safety::tree`，取工作包根目录及全部后代节点修改时间的最大值，再转换为 UI 层的紧凑时长值。所有行共享同一查询时刻，避免长列表因逐行取时造成边界不一致。

`safety` 继续负责路径和节点安全检查，只为 `TreeEntry` 提供只读修改时间访问器。`lifecycle` 负责文件系统时间到“距今时长”的转换；Reporter 只通过 `Display` 渲染该值。CLI 在读取摘要前把 `--completed`、`--archived` 转成查询过滤条件；active 始终包含，未启用的状态不会读取任务或遍历工作包。任务计数算法、状态推导和 ID 排序不变。

## 3. 关键 API 与数据模型

在 [UI 端口](../../../../../src/ui.rs)增加可复制比较的 `ModifiedAge`：

```rust
pub enum ModifiedAge {
    Known(std::time::Duration),
    Unknown,
}
```

它实现 `Display`，不暴露墙钟时间或路径。`ChangeRow` 增加 `updated: ModifiedAge`。这是公开 UI 行模型和非 TTY 文本协议的有意兼容变更；现有构造方和测试需补字段。

`TreeEntry` 增加 `pub fn modified(&self) -> Option<SystemTime>`，不开放其他快照内部字段。`list_changes` 保持现有“全部状态”公开行为以兼容库调用；新增 `list_changes_filtered(project, include_completed, include_archived)` 供 CLI 使用，内部再调用接收固定 `now` 的私有辅助函数。修改时间早于或等于查询时刻时用 `duration_since`；未来时间按零时长处理。若安全遍历失败，错误附带工作包路径并向上传播；若整棵树没有可用修改时间则生成 `Unknown`。

读取保持只读、不加项目写锁、不缓存。并发外部编辑可能使结果反映遍历期间的即时状态，与现有任务摘要的一致性级别相同；不据此认证完成状态。

## 4. 核心算法与实现规则

格式化只使用向下取整的整数单位：

1. 小于 3600 秒：输出完整分钟数加 `m`，因此 0–59 秒为 `0m`，3599 秒为 `59m`。
2. 小于 86400 秒：输出完整小时数加 `h`，忽略剩余分钟，因此 3600 秒为 `1h`，86399 秒为 `23h`。
3. 至少一天：输出完整天数加 `d`；剩余完整小时非零时紧接 `Nh`，为零则省略。示例为 `1d`、`1d2h`、`20d5h`。
4. `Unknown` 输出 `?`；所有格式均无空格和小数点。

TTY 宽表列顺序为 `STATE CHANGE TASKS UPDATED`，CHANGE 继续按最长 ID 对齐；窄表同序使用两个空格分隔。重定向输出为 `state<TAB>id<TAB>tasks<TAB>updated`。空列表行为、颜色、统计尾行及控制字符过滤不变。宽表阈值可在保持可读且测试固定的前提下从当前 40 调整；不得截断字段。

`List` 子命令增加两个布尔参数。无参数时过滤为 active；`--completed` 为 active + completed；`--archived` 为 active + archived；两项同时提供为全部状态。过滤发生在任务快照和 mtime 树遍历前，避免隐藏状态的无关 IO 或安全错误影响结果。Reporter 的状态计数仅统计实际收到的可见行。

递归时间包含目录和普通文件，以便子项创建、删除等目录内容变化也能成为最近修改；符号链接、reparse point、硬链接文件和特殊节点继续由 `safety::tree` 拒绝。

## 5. 固定决策与可自主调整范围

固定：最新时间取整个工作包树的最大 mtime；所有行使用同一 now；未来时间显示 `0m`；日级格式保留非零剩余小时，小时级不保留分钟；脚本输出追加第四列；active 始终显示，两个历史状态分别由 `--completed`、`--archived` 追加且可组合；不新增排序、配置、依赖或持久字段。

可自主调整内部辅助函数名、测试夹具及错误上下文措辞。不能改为 proposal 单文件时间、目录根单点时间、带小数时间或当前日期字符串。

Blocked: none
Open questions: none

## 6. 验证与长期文档影响

单测覆盖 0 秒、59 秒、10 分钟、59 分钟、1 小时、20 小时、整天、`1d2h`、`20d5h` 和未知值；Reporter 测试覆盖四列 TSV、宽 TTY 表头/对齐及窄输出。集成测试验证默认仅 active、两个历史开关的单独及组合结果、隐藏状态不触发读取、稳定的 `0m` 第四列、原有 ID 排序、任务计数和安全错误回归。

运行 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`，并运行项目现有 release/MSRV 检查。同步 [CLI 当前文档](../../../../../docs/cli.md)的列表协议以及[详细架构](../../../../../docs/architecture.md)、[当前架构入口](../../../../architecture.md)的列表查询职责；持久文档格式和 ADR 无影响。
