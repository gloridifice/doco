# Current architecture

doco 是单进程、文件驱动的 Rust CLI。`src/main.rs` 只退出 `src/cli.rs` 返回的状态码；CLI 使用 clap 解析命令、选择目标并渲染顶层错误，`src/terminal/` 负责 TTY 能力、彩色业务输出和局部选择器；库内 init、check、lifecycle 通过 `src/ui.rs` 的 Reporter 端口输出，不依赖终端实现。

项目状态只来自 `doco/changes/{active,completed,archived}` 目录。`list` 默认在摘要读取前过滤为 active，`--completed` / `--archived` 分别追加历史状态；它通过安全读取即时汇总任务数，并从可见工作包树的最新 mtime 计算无小数的距今时长，不持久化摘要或改变排序。写操作经过 `src/safety.rs` 的路径归属检查、项目锁、并发快照复核、原子文件替换或受控目录移动。archive/cancel 先输出预检范围；dry-run 停止于预览，普通执行无需确认但在删除前仍加锁并复核。

详细模块职责、生命周期和失败恢复边界见 [实现架构](../docs/architecture.md)，CLI 的颜色、交互与自动化契约见 [CLI 输出与交互](../docs/cli.md)，持久文档格式见 [文档格式](../docs/document-format.md)。
