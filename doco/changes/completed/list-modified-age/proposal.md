<!-- doco:lifecycle v=1 created-at=- completed-at=2026-09-20T08:02:30Z archived-at=- -->
# list 生命周期时间距今显示

## 目的

当前 `doco list` 使用工作包文件系统修改时间表示最近活动时间。Git 拉取或 checkout 会重建文件并改变 mtime，导致显示时间不代表变更真实的生命周期事件；现有距今计算还存在方向错误，使过去的 mtime 通常显示为 `0m`。改为在 proposal 中持久化创建、完成和归档时间，并按当前状态显示对应事件距今时长。

## 范围与完成标准

- `doco new` 在 `proposal.md` 的机器可读生命周期标记中写入 RFC3339 UTC 创建时间；`complete` 写入最近一次完成时间；`archive` 和 `cancel` 写入归档时间。
- 生命周期时间随 proposal 被 Git 共享并在归档后继续保留，不写入本机索引，也不改变目录仍是生命周期状态唯一事实来源的约束。
- `list` 按状态选择时间：active 使用创建时间，completed 使用完成时间，archived 使用归档时间；显示当前查询时刻与该时间的紧凑差值。
- TTY 宽表使用 `AGE` 列；窄表在每行末尾追加时长；重定向输出保持四个 TSV 字段，前三列顺序和含义不变，第四列改为生命周期年龄。
- 时长不带小数：不足一小时显示整分钟，不足一天显示整小时，一天及以上显示整天和非零剩余整小时；不足一分钟或事件时间晚于当前时钟时显示 `0m`。
- 旧工作包缺少生命周期标记时保持合法并显示 `?`，不得回退文件系统 mtime 或伪造历史时间；后续生命周期命令只写入实际发生的事件。格式错误或重复标记明确失败。
- reopen 不改创建时间；再次 complete 覆盖为最近一次完成时间。归档失败后允许按现有恢复流程重试，当前目录状态决定 list 读取哪个字段。
- 增加格式解析、生命周期写入、兼容、失败恢复、时间边界和所有 list 布局测试，并同步 CLI、文档格式和架构说明。

非目标：不改变 ID 排序、任务计数、安全锁、状态目录、索引缓存格式或 list 状态过滤；不从 Git 历史推导旧时间；不记录完整生命周期事件历史；不承诺秒级显示精度。

## 结果

已交付基于 proposal 持久元数据的生命周期年龄：new 写入 created-at，complete 写入最近 completed-at，archive/cancel 写入 archived-at；reopen 保留创建时间并在再次完成时覆盖完成时间。list 按真实目录状态选择对应事件，TTY 使用 AGE，TSV 保持四列；旧包缺少时间时显示 `?`，不再依赖文件系统 mtime。格式解析、失败重试、proposal-only、BOM/CRLF、旧包兼容、状态过滤和归档保留均有自动测试，当前架构、CLI 契约及文档格式已同步。

Verification: `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`（112 项通过，1 项显式 benchmark ignored）、`cargo build --release --locked`、`git diff --check`、临时项目 new/list 冒烟及 `doco check list-modified-age` 通过。`cargo +1.85 check --all-targets --locked` 已运行但被基线中未改动的两个 let-chain（`src/check/tasks.rs:46`、`src/init/entry.rs:104`）以 E0658 阻塞；该限制已向用户说明，用户随后明确要求执行 complete。
