# 本地变更目录索引缓存任务

本工作包已实施并验证。提案见 [proposal](../proposal.md)，设计与可自主调整范围见 [实施设计](implement.md)。

- [x] 1.1 实现固定 CSV v1 与目录时间戳模型
  - 完成条件：新增内部 index 模块；实现 type/key/value、唯一元数据、UTC 写入日期、精确有符号纳秒 mtime、合法 ID/状态和确定性排序；格式损坏/未知版本完整回退，固定 CSV 不引入自由文本或通用转义；单元测试覆盖空索引、LF/CRLF、重复字段、溢出和截断。
  - Verification: cargo test --lib index:: 通过 5 项：`cache_round_trips_and_stays_deterministic`（固定逐行输出与记录排序）、`corrupt_or_unknown_caches_are_rejected`（21 种损坏输入、CRLF、缺末行换行、重复/未知/超长/非法字段）、`timestamps_keep_nanoseconds_and_support_the_epoch`（秒级往返、epoch 之前、i128 溢出、加号/小数/空值拒绝、小数秒时间戳拒绝）、`counts_are_reported_per_state` 及后续两项缓存信任测试；`cargo clippy --all-targets -- -D warnings` 无告警。

- [x] 1.2 实现权威扫描、缓存加载、撤销和原子发布
  - 依赖：1.1
  - 完成条件：抽取不依赖缓存查询的三状态扫描，保留安全检查与全局 ID 唯一性；前后 mtime 一致才复用或发布；缺缓存/坏格式为 miss，不安全路径和真实 IO 错误仍报错；mtime 不支持时仅返回内存结果；写接口复用已有 Lock、安全快照删除和原子持久化，不增加 dirty 文件。
  - Verification: 新增 load、scan、probe、invalidate、publish 五个接口于 src/index.rs；`scan_initialized` 先取 mtime、再 `collect`、再取 mtime，不一致即报错；cargo test --lib index::tests::a_cached_snapshot_is_only_trusted_while_the_directory_stamps_match 验证时间戳匹配才命中、目录变化后 `cached` 返回 None；`an_unavailable_cache_file_warns_after_commit_without_failing_it` 验证发布失败只警告；tests/index.rs 的 hard_linked_cache_files_are_rejected、`invalidate_failure_stops_before_any_business_write` 验证不安全缓存对象报错且不覆盖。

- [x] 2.1 接入 Project 查询并核对指定目标的真实路径
  - 依赖：1.2
  - 完成条件：保持 changes/resolve 公开签名；缓存命中不枚举全部历史目录；指定 ID 核对三个状态路径，错误状态或记录与磁盘不符时权威重载，重复 ID/坏链接拒绝；list 的状态过滤、实时任务及 UPDATED 保持原语义；只读命令不写缓存、不创建锁、不污染 stdout。
  - Verification: `Project::changes` 与 `resolve` 改为调用 index 的 entries/resolve，公开签名未变；tests/index.rs 的 `a_stale_cache_never_authorizes_duplicates_or_wrong_states`（漏项与错状态伪造缓存后 `new` 仍拒绝、迁移仍按磁盘状态执行）、`read_only_commands_and_dry_runs_never_create_the_cache`、`a_project_without_a_cache_still_works_and_builds_it_on_the_next_write` 全部通过；原有 tests/list_tasks.rs、tests/lifecycle.rs（含 `ids_are_safe_unique_and_state_errors_do_not_move_anything` 的重复 ID 用例）与 terminal::output 列表输出测试未改动且通过。

- [x] 2.2 为五种生命周期操作维护索引
  - 依赖：2.1
  - 完成条件：new、complete、reopen、archive、cancel 在锁内加载、预检、撤销缓存、执行原业务、更新内存并发布；new 无论缓存如何都检查真实同名目录；撤销失败零业务修改，业务失败不恢复旧缓存，业务成功但缓存失败保留结果并以 stderr 警告/退出 0 提示 fix；完整包、proposal-only 和归档重试安全不变。
  - Verification: 五个入口均按“预检 → index 的 invalidate → 业务 → `publish_after_commit`”接入；tests/index.rs 的 lifecycle_writes_track_every_state_transition 覆盖 active→completed→active→completed→archived 与 cancel，且被拒绝的 `complete` 保持缓存字节不变；`invalidate_failure_stops_before_any_business_write` 验证撤销失败时 active/second 不存在；`a_committed_operation_survives_a_failed_archive_cleanup`（Windows）验证部分清理失败后无缓存、重试成功后缓存为 `goal=archived`；原有完整包、proposal-only、归档恢复和取消测试全部通过。

- [x] 3.1 新增 doco fix 强制重建入口
  - 依赖：1.2
  - 完成条件：新增公开库包装和 CLI fix/--dry-run；普通执行在写锁内先撤销旧缓存再扫描/发布，失败退出 1，不修正文档或自动解决重复 ID；未初始化项目提示 init；dry-run 仅扫描报告、不创建或改变任何缓存、锁文件或目录；开始信息及 flush 位于扫描前，完成信息显示三状态数量。
  - Verification: 新增 src/fix.rs 与 Command::Fix；tests/index.rs 的 fix_rebuilds_a_missing_or_corrupt_cache_and_dry_run_never_writes、`fix_requires_an_initialized_project_without_creating_one`、`an_uninitialized_project_is_never_touched_by_fix`、`rebuild_announces_and_flushes_before_it_scans`（记录 Reporter 断言事件为 `[INDEX, INDEX]`、flush 发生在首个事件之后）通过；`a_failing_cache_rebuild_keeps_a_successful_init` 验证非法目录使 `fix` 失败退出 1 且不发布缓存。

- [x] 3.2 在 init 成功后建立缓存并明确展示阶段
  - 依赖：3.1
  - 完成条件：init 计划应用成功并释放锁后，init 复用强制重建逻辑重新加锁；包括全 SKIP 在内每次 init 都先输出并刷新 INDEX Building change index cache...；缓存失败保留安装成果，明确警告/正常输出可写时退出 0；init --dry-run 只展示计划；update 不新增缓存操作，无嵌套加锁。
  - Verification: tests/index.rs 的 init_announces_and_builds_a_documented_cache 断言 Building 先于 Cache built 且 CSV 六行结构与纳秒时间戳合法；`repeated_init_rebuilds_the_cache_with_current_records` 验证重复 init 仍重建并刷新记录；`a_failing_cache_rebuild_keeps_a_successful_init` 验证安装完成、stderr 警告、退出 0、skill 已安装；tests/init.rs（含仅排除精确缓存文件的幂等比较与 `s.index_changes()` 校验）与 tests/update.rs 全部通过。

- [x] 4.1 补充失败恢复、安全、并发和兼容回归测试
  - 依赖：2.2, 3.2
  - 完成条件：覆盖缓存缺失/损坏/过期、目录扫描期间变化、外部移动、保持 mtime 的目标漏项/错状态、锁冲突、链接/硬链接、撤销和发布失败、进程中断、归档部分失败与修复重试；断言 init/fix 输出与 flush 顺序；仅成功 init 的幂等比较排除精确缓存文件并单测缓存，所有只读/dry-run/失败路径保留完整快照断言，未广泛忽略 tmp。
  - Verification: 新增 tests/index.rs 14 项与 src/index.rs 内 3 项缓存单元测试，覆盖缺缓存、损坏/未知版本、伪造时间戳不符的漏项与错状态、外部移动后重新列举、硬链接与目录冒充缓存、撤销失败、发布失败警告、init 缓存阶段失败、只读与 dry-run 零写入；tests/common/mod.rs 新增 `files_without_index`、`index_text`、`index_changes` 三个断言入口，仅成功 init 的幂等断言改用排除式比较，tests/safety.rs、tests/lifecycle.rs、tests/list_tasks.rs 仍使用完整快照；`cargo test` 全绿。

- [x] 4.2 建立可复现的规模基准与热路径计数断言
  - 依赖：2.2, 3.1
  - 完成条件：用 0、100、1,000、10,000 条临时目录夹具记录 release 冷 new、热 new、fix 中位耗时、机器环境和夹具范围；自动断言热 new 无权威扫描、无历史目录逐项安全检查；不把整体 CSV IO 说成 O(1)，不使用易抖动的毫秒 CI 门槛，不改真实项目工作包作为基准。
  - Verification: 新增 tests/scale.rs，默认 `#[ignore]`，用空历史目录夹具隔离索引成本；显式运行 cargo test --release --test scale -- --ignored --nocapture 输出：100 条 cold 301ms / warm 163ms，1,000 条 cold 1.90s / warm 145ms，10,000 条 cold 14.62s / warm 154ms（沙箱位于 C 盘临时目录，冷扫描受该文件系统延迟影响；target 盘的对照测量为 cold 8.00s / warm 0.09s）。结构性断言：篡改 history-00000 的状态后一次热写仍保留该错误记录，证明热路径未重新枚举目录；另断言最大规模下热耗时乘以 2 仍小于冷耗时。

- [x] 5.1 同步实际交付后的当前文档与使用说明
  - 依赖：4.1, 4.2
  - 完成条件：更新 architecture、CLI 契约、README 与 doco 导航，新增 change-index-cache 规范；明确 CSV 字段、init/fix 进度与退出码、缓存不共享、Git 拉取失效、只读不持久化、mtime 局限、外部并发边界、写前撤销恢复及归档全文扫描非目标；保留原包格式与 skill 工作流，不把缓存打包为资源，不无故升级 skill bundle。
  - Verification: 新增 [doco/specs/change-index-cache.md](../../../../specs/change-index-cache.md)（CSV 字段、失效与信任边界、维护协议），在 [doco/specs/cli.md](../../../../specs/cli.md) 增加“变更索引缓存”章节并在归档章节说明部分清理后缓存已撤销，更新 [doco/architecture.md](../../../../architecture.md) 的依赖图、模块职责、派生状态与失败恢复，更新 [README.md](../../../../../README.md) 与 [doco/README.md](../../../../README.md) 导航；assets/skill/ 未改动，skill bundle 版本未递增，`doco check` 通过。

- [x] 5.2 执行最终验收并记录真实验证证据
  - 依赖：5.1
  - 完成条件：运行 fmt、Clippy、all-targets 测试和 locked release 构建，核对新增依赖与声明 MSRV；检查实际目录和缓存、init/fix 帮助与输出、两种包模式和只读/dry-run 契约；记录规模基准、失败注入结果及平台限制，更新 proposal 交付摘要和任务证据，最后运行 doco check。
  - Verification: `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（全部套件通过，含 1 项显式忽略的规模基准）、`cargo build --release --locked` 均通过；新增依赖 `time 0.3.45` 自身声明 `rust-version = 1.83.0`，低于项目声明的 1.85，且 `cargo +1.85 build --locked` 已成功编译整个依赖树（随后仅因既有 src/check/tasks.rs:46、src/init/entry.rs:104 的 let-chains 报错，属本次变更前的既有 MSRV 偏差，未在本变更范围内修改）；手工冒烟验证 init/fix/init --dry-run 输出、克隆后无缓存仍可 list、外部移动被重新扫描、重复 ID 被拒绝；归档全文引用扫描未加速，已在设计非目标中说明。
