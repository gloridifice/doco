# 执行任务

- [x] 1.1 增加工作包最近修改时间查询与紧凑时长模型
  - Design: [实施设计](implement.md)
  - Acceptance: `list_changes` 安全遍历每个工作包，以统一查询时刻计算最新 mtime 的距今时长；边界格式符合整数分钟、小时、天/小时规则，未知和未来时间按设计处理。

- [x] 1.2 在所有 list 输出布局中增加修改时间
  - Dependencies: 1.1
  - Design: [实施设计](implement.md)
  - Acceptance: 宽 TTY 显示 UPDATED 列，窄 TTY 和管道输出追加同一字段；原有前三列顺序、颜色、排序、空态和统计行为不变。

- [x] 1.3 补充自动测试和当前文档
  - Dependencies: 1.1, 1.2
  - Acceptance: 格式边界、四列 TSV、TTY 布局、任务计数与安全失败有自动化覆盖；CLI 和架构当前文档准确描述已实现契约。

- [x] 1.4 执行整体质量验证
  - Dependencies: 1.3
  - Acceptance: fmt、clippy、全部目标测试、release locked 构建和 Rust 1.85 locked 检查实际运行并记录结果；未运行或受限项如实说明。

- [x] 2.1 增加 list 历史状态开关和读取前过滤
  - Dependencies: 1.4
  - Design: [实施设计](implement.md)
  - Acceptance: 默认只查询 active；`--completed` 和 `--archived` 分别追加对应状态且可组合；隐藏状态不会被读取，已有库级全量列表入口保持兼容。

- [x] 2.2 验证过滤矩阵并同步文档
  - Dependencies: 2.1
  - Acceptance: 自动测试覆盖默认、单开关、双开关和隐藏状态错误隔离；帮助、README、CLI 及架构文档准确；fmt、clippy、全部测试、release 和 MSRV 检查实际通过并记录。

## Verification

- 1.1–1.3：`cargo test --all-targets` 通过，共 63 项；新增紧凑时长边界单测，并更新宽/窄 TTY、彩色管道、四列 TSV、任务计数和不安全路径回归。`cargo run --quiet -- list` 实际输出第四列 `0m`。
- 1.4：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo build --release --locked`、`cargo +1.85 check --all-targets --locked` 均通过。
- Release 二进制冒烟成功输出按 ID 排序的四列 TSV，当前两项的 UPDATED 均为 `0m`。执行命令：
  ```text
  ./target/release/doco.exe --color never list
  ```
  未执行真实 TTY 视觉检查；宽/窄布局由内存 Reporter 单测覆盖。
- 2.1–2.2：`cargo test --all-targets` 通过，共 64 项；集成测试覆盖默认仅 active、单独 `--completed`、单独 `--archived`、双开关全量以及隐藏 completed 中不安全任务文件不影响默认列表。`cargo run --quiet -- list --help` 显示两个新开关；debug 和 release 二进制冒烟确认默认只输出 active，`--completed` 追加 completed，两开关可组合。
- 2.2：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo build --release --locked`、`cargo +1.85 check --all-targets --locked` 均通过。当前项目没有 archived 条目，因此 release 冒烟的 `--archived` 行为由临时沙箱集成测试覆盖。
