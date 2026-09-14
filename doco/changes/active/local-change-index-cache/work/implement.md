# 本地变更目录索引缓存实施设计

## 1. 现状、目标与设计基线

### 已核对的实现

- 设计起点为 Git `8db370f6e3f40164ac64a5fa95a342c80681e6bb`，创建前 `git status --short` 为空；`doco list` 中没有同目标 active 变更。当前 decisions 目录无决策文件。
- [当前架构](../../../../architecture.md) 明确目录是唯一生命周期状态来源，没有中央索引；本方案新增的是可丢弃派生缓存，不改变这一事实来源。
- [Project](../../../../../src/lib.rs) 的 `changes()` 先检查初始化，遍历三个状态目录，对每个条目调用 `safety::inspect`，检查合法 ID、目录类型和跨状态唯一性，以 BTreeMap 排序输出。`resolve()` 目前每次先收集全部变更。
- [生命周期](../../../../../src/lifecycle/mod.rs) 的 `new_change_in_mode_with_ui` 在写锁内调用 `changes()` 检查 ID，然后在 tmp 暂存并移动工作包。complete/reopen 在锁内解析目标和迁移；模板、完整包与 proposal-only 模式不应受缓存影响。
- [归档与取消](../../../../../src/lifecycle/archive.rs) 先预览，在非 dry-run 分支获取写锁后复核，执行取消结果写入、work 清理和最终移动。`retained_links()` 会遍历 doco 下保留的 Markdown，且执行两轮引用预检；ID 索引不能消除这项全文 IO。
- [列表](../../../../../src/lifecycle/list.rs) 先取 changes，再按状态过滤，然后才读取可见工作包的 proposal、任务及文件树 mtime。缓存只替换候选来源，不缓存或改变摘要。
- [安全层](../../../../../src/safety.rs) 提供逐级路径检查、不可覆盖持久化、带快照的删除、`sync_all`、目录移动和非阻塞 OS 排他锁。doco/tmp/.doco.lock 永不删除。缓存不复用或删除锁文件。
- [初始化](../../../../../src/init/mod.rs) 在 dry-run 返回之前不应用计划；`Plan::apply` 在 [plan.rs](../../../../../src/init/plan.rs) 内部获取并释放项目锁。因此 init 的缓存阶段可以在 apply 成功返回后单独加锁，不得递归获取同一把锁；[update](../../../../../src/update.rs) 复用计划但不应因此被附加缓存写入。
- [Reporter](../../../../../src/ui.rs) 已有 `flush(Stream)`，终端与 plain 实现均支持。开始扫描的提示使用既有端口显式刷新，不新增进度框架。
- [CLI](../../../../../src/cli.rs) 目前没有 fix；init 非交互仍要求显式 agent。[CLI 契约](../../../../specs/cli.md) 和 [文档格式](../../../../specs/document-format.md) 的交互、包格式和完成检查继续适用。
- [测试夹具](../../../../../tests/common/mod.rs) 的 `files()` 包含 tmp；[init 测试](../../../../../tests/init.rs) 和 [边界测试](../../../../../tests/edge_cases.rs) 有重复 init 后全文件字节相同的断言。引入每次重建日期后，需要只对成功 init 的幂等断言排除精确缓存文件，并单独验证缓存；不能全局忽略 tmp 而掩盖 dry-run/失败路径写入。

### 目标

保留 CSV 单文件方案，避免每次 new 枚举所有历史变更目录。首版不引入数据库、监听器、文件身份指纹、dirty 文件或第二个元数据文件。此前规模测试见 [proposal](../proposal.md)，只用于说明瓶颈，不作跨机器性能保证。

## 2. 整体方案

### 模块职责

新增 [src/index.rs](../../../../../src/index.rs)（库内部模块）：固定 CSV 编解码、目录时间戳、权威扫描、缓存加载、指定 ID 核对、缓存撤销与发布。新增 [src/fix.rs](../../../../../src/fix.rs)（库入口）：强制重建的锁、进度与错误编排。CLI 只分发参数，领域代码不依赖 clap。

```text
Project::changes / resolve ───→ index::load / probe_id
                                   ├─ 有效 CSV → 内存快照
                                   └─ 失效 → 权威目录扫描

new / complete / reopen / archive / cancel
  └─ 已持有项目锁 → load → 预检 → invalidate → 业务操作 → publish

init → Plan::apply 完成且释放锁 → 重建阶段重新加锁
fix ────────────────────────→ 强制扫描与发布
```

- 文件系统目录始终是事实来源。CSV 只含 ID 和状态，路径由已验证 ID、State 和 Project::path 重建，不接受 CSV 提供路径。
- 索引快照只在本次调用内存活，不在 Project 中引入跨命令、跨操作的长寿命可变缓存。生命周期入口复用同一份锁内快照进行一次更新。
- `changes()`、`resolve()` 等现有公开签名保持兼容；内部权威扫描必须独立于 `changes()`，避免缓存回退递归。
- 首版只读路径不写 CSV，不创建 tmp/lock，不输出命中或失效日志，保持 list/context 等机器输出不受污染。
- 正常写命令只逻辑更新一个记录，但整体原子重写文件；缓存命中后不做二次全量目录扫描。

## 3. 关键 API 与数据模型

以下是计划新增的接口，不是现有实现；实现时可调整私有类型名称，但必须保留职责和调用约束。

```rust
pub(crate) struct DirectoryMtimes([i128; 3]); // active, completed, archived
pub(crate) struct IndexSnapshot {
    entries: BTreeMap<String, State>,
    mtimes: Option<DirectoryMtimes>,
}

pub(crate) fn load(project: &Project) -> Result<IndexSnapshot>;
pub(crate) fn scan(project: &Project) -> Result<IndexSnapshot>;
pub(crate) fn probe_id(project: &Project, id: &str) -> Result<Option<Change>>;
pub(crate) fn invalidate(project: &Project, lock: &safety::Lock) -> Result<()>;
pub(crate) fn publish(
    project: &Project,
    lock: &safety::Lock,
    snapshot: &IndexSnapshot,
) -> Result<()>;

// 强制重建的公开库包装与 Reporter 入口。
pub fn run(project: &Project, dry_run: bool) -> Result<()>;
pub fn run_with_ui(
    project: &Project,
    dry_run: bool,
    reporter: &mut dyn Reporter,
) -> Result<()>;
```

invalidate 与 publish 是 crate 内接口，传入已有 Lock 表达必须在持锁期间调用；不得内部再加锁。快照由调用者拥有。实现可增加内部 `rebuild_locked` 和目标核对后重载帮助函数，供 fix/init 共享，而不是复制扫描算法。

### 固定 CSV v1

路径固定为 doco/tmp/changes-index.csv，UTF-8、三列 `type,key,value`。写 LF 并保留末尾换行；读接受 LF/CRLF，不接受 BOM、引号、转义、注释、空白行或额外列。此格式是受限 CSV，不承诺读任意电子表格导出的 CSV。

```csv
type,key,value
meta,version,1
meta,updated_at,2026-09-14T10:00:00Z
directory,active,1789380000123456789
directory,completed,1789380000234567890
directory,archived,1789380000345678901
change,feature-a,active
change,feature-b,archived
```

- 固定元数据行恰好各一条；三个 directory 行恰好各一条；change 行允许零条，ID 不能重复，即使重复行状态相同也作废。
- Writer 按示例头部顺序，change 按 ASCII ID 排序。Reader 可接受数据行换序，但要求首行为固定表头，所有字段及重复键检查通过。
- `updated_at` 为本次发布的 UTC RFC3339 日期时间，固定 `YYYY-MM-DDTHH:MM:SSZ`；只供排查，不参与有效性或 TTL 判断。日期格式化使用兼容项目声明 MSRV 的轻量时间库，不手写公历算法；版本锁定和依赖功能裁剪由实现者选择。
- directory 值是 UNIX epoch 起的有符号十进制纳秒数，以 i128 编解码，支持 epoch 之前的时刻；不经浮点、不截成秒。数值必须可表示为 SystemTime，溢出或非法格式使缓存作废。
- State 只允许 active/completed/archived，ID 复用 `validate_id`。无未知 type/key、无额外字段。未知版本、非法 UTF-8、截断、不完整元数据均回退扫描。
- CSV 只含已限制字符的固定字段，不需要通用 CSV 引用/转义框架。不要添加自由文本或把路径塞入记录。

### 错误分类

- 缓存不存在、可安全读取但内容损坏/版本不匹配：cache miss，不让派生数据损坏阻塞正常查询。
- 缓存路径或其父目录是链接、reparse point、硬链接文件、目录冒充文件，或者路径检查/实际 IO 权限失败：明确错误，不覆盖、不绕过安全检查。
- 实际状态目录缺失仍遵循 initialized 的错误，提示 init；fix 不创建项目骨架。新 clone 缺少 Git 不保存的空目录时由 init 创建。
- 获取 mtime 不受平台支持时可返回内存扫描结果（mtimes 为 None），但不能发布或信任缓存；init/写命令提示缓存不可用，fix 返回失败。普通目录读取或安全检查错误不能伪装为“不支持 mtime”。

## 4. 核心算法与实现规则

### 4.1 缓存读取和权威扫描

1. 保留初始化和根目录安全检查；安全读取三个状态目录的精确 mtime 为 before。
2. 安全读取 CSV 并校验完整格式；再次读取三个目录 mtime 为 after。before、after、CSV 快照全部相同才命中。缓存自己的写入不会修改这些状态目录。
3. 其他可恢复失效走 `scan`：扫描前取目录 mtime，复用原 changes 的三目录枚举、安全检查、合法 ID/目录类型和全局唯一性检查，最后再取目录 mtime。
4. 可用的前后 mtime 不同则报“目录在扫描期间发生变化，请重试”，不自动无限重试、不发布缓存；前后相同返回排序后的内存快照。只读失败不删除旧缓存。
5. 无缓存或 mtime 不可用的只读查询不持久化快照；连续只读调用可能重复扫描。成功的 init、fix 或生命周期操作才写文件。

mtime 相等只是失效启发式：无法完整识别保持时间戳的恢复、低精度时间戳碰撞、目录身份替换或任意外部并发写入。不得宣传缓存证明所有目录完全一致。其他用户手动编辑、Git pull/checkout 不受 doco 锁控制；迁移期间禁止并行改工作包的现有边界继续适用。

### 4.2 指定目标与缓存信任边界

- `probe_id` 先 validate_id，再分别 inspect 三个状态目录下该 ID 的路径及存在类型；不能仅用 exists 隐藏坏链接。零处为 None，一处返回实际 Change，多处为重复 ID 错误。
- `new` 不论缓存是否含该 ID，都调用 probe_id；任何真实同名目录拒绝创建。缓存记录与 probe 结果不一致时丢弃内存缓存并权威扫描，再完成预检；不得只修正单行后声称全局索引已校准。
- `resolve` 在缓存定位或缓存 miss 后同样 probe；目标不存在/状态不一致时进行一次权威扫描，再返回实际结果或 unknown ID。查找不存在 ID 且有效缓存也无该 ID 时，三路径核对为 None 即可报告 unknown，不为该负查找扫描全部目录。
- complete/reopen 在锁内检查实际源状态和其他状态同名路径；archive/cancel 在预览后获得锁时重新 load/probe，沿用已有内容与链接快照复核。目标包内容读取继续走 safety。
- list 等批量查询依赖 mtime 缓存列举，因此时间戳被保持时可能漏列未缓存的无关目录；可见包的内容读取仍安全，fix 强制校准。缓存命中不再附带检查所有无关目录的非法 ID、重复 ID 或新出现链接；这些全局异常在扫描/fix 时检查。必须在文档明确这项兼容边界。

### 4.3 生命周期写入协议

1. 获取现有项目锁；load 快照并 probe 目标；完成原有状态、内容、目标路径和快照预检。归档预览仍在锁外，锁内重新取得当前索引。
2. 在首次创建暂存包、修改 proposal、删除 work 或移动工作包之前调用 invalidate。它对缓存文件执行 inspect/snapshot/verify 后只删除这一个文件；不存在则成功，不创建标记。删除失败立即结束，不开始业务修改。
3. 执行既有业务算法。遇到原有失败继续按原错误/部分清理规则返回，不尝试用旧内存快照恢复缓存，缓存保持不存在。
4. 业务最终状态验证通过后在内存增改目标记录：new 加 active、complete 改 completed、reopen 改 active、archive/cancel 改 archived。此时再读取三个目录 mtime，建立待发布快照。
5. publish 在同一锁内复核待发布的目录 mtime，在 tmp 同目录临时文件中写完整 CSV、sync_all、以 expected=None 不覆盖式原子持久化；发布前最后核对目录 mtime。不能在发布后重新采样 mtime 来掩盖序列化期间的目录变化。
6. 业务成功但发布失败：不回滚业务，stderr 发 WARNING，说明目标真实状态已提交、缓存未写入、可运行 doco fix，正常业务成功退出码为 0。如果出现外部文件抢占缓存目标，不能覆盖它，也不能承诺该路径仍不存在；明确提示外部冲突。

撤销、临时文件和文件发布继续采用现有安全操作，清理只涉及本次拥有的临时文件。不可删除 .doco.lock 或递归清理 tmp。协议处理普通失败和进程中断，不额外承诺父目录元数据落盘、断电事务或外部恶意替换下的强一致性。业务成功后输出流写入失败仍遵循现有 IO 错误行为，不伪装为缓存可恢复警告。

### 4.4 init 与 fix

- init 的原计划/apply 完全成功后，输出 stdout `INDEX Building change index cache...`，立即 `Reporter::flush(Stdout)`，随后重新获取项目锁执行强制重建。apply 的锁已释放，缓存从该时刻实际目录重新扫描，不使用计划执行前状态；不扩大现有 integration 事务。
- init 每次都重建，包括纯 SKIP 和 --refresh；`update` 不增加索引动作。重复 init 可改变缓存日期，但受管 skill、入口和项目事实的幂等约束不变。
- init 缓存阶段失败时保留安装成果，stderr WARNING 含“Disk installation complete; change index cache was not rebuilt”及原因、fix 恢复提示；缓存辅助步骤失败不把已成功的安装改报未执行，正常输出通道可写时退出 0。冲突或安装失败不进入缓存阶段。
- `fix` 不接受变更 ID、agent 或交互选择；服从全局 root/color/no-interactive 参数。普通执行输出并刷新开始信息，加锁、先 invalidate、强制 scan、publish，成功后 stdout `INDEX Cache built: <n> active, <n> completed, <n> archived.`。强制扫描失败时不能留下旧文件供未来命中；重复 ID 等结构问题只报告，不替用户修复。
- init 成功使用同样完成信息；CLI 现有正文为英文，因此固定英文标签/文本，中文说明写在项目文档中，不新增语言选项。
- fix 是显式修复命令，锁冲突、扫描或发布失败退出 1；参数错误仍为 2。init 缓存步骤采用上述可恢复警告语义，生命周期提交后的缓存错误同理；不同命令不混淆“业务提交”和“缓存提交”。
- `init --dry-run` 在原计划中补充 `INDEX Would rebuild change index cache.`，不扫描尚不存在的目录、不写文件。`fix --dry-run` 校验已初始化项目，忽略缓存做权威扫描与计数，报告 `INDEX Would rebuild cache: ...`，不获取写锁、不撤销缓存、不创建 tmp。既有 archive/cancel dry-run 不增加写入。
- 所有进度只在 init/fix 等显式重建入口输出；list/context 的 stdout 不插入缓存诊断。警告经现有语义 Reporter 写 stderr、过滤控制字符，不直接 println 绕过端口。

### 4.5 远端拉取、忽略规则和性能

- Git pull/checkout 通常改变直接发生新增/删除的状态目录 mtime。下一命令发现不同则扫描，获得真实 active/completed/archived；不读取 Git 索引或远端信息，不执行 Git hooks。
- 缓存仅本机生成，位于已有默认 tmp 目录忽略范围。新 init 继续生成现有默认规则；既有用户 .gitignore 保持原样，由 README 明确自定义规则时需确保本地缓存被忽略。init/fix 不擅自 git rm、stage 或 untrack 用户已提交的文件，不承诺第三方打包器自动排除 tmp。
- 热路径需要固定数量的目录 mtime、安全父链检查、指定 ID 的三路径检查，以及单个 CSV 的顺序 IO；不枚举历史 ID 目录。CSV 解析、BTreeMap 构造和全量重写仍随 N 增长，CPU 可为 O(N log N)，文件读写为 O(N)，不声称严格常数时间。
- 全量扫描 O(N × 路径深度) 的文件系统成本保留给冷缓存和 fix。不在本变更顺便削弱 scan 的安全检查，也不引入无效性不清晰的父目录检查长期缓存。

## 5. 固定决策与可自主调整范围

### 固定

单个本地 CSV；格式 v1；无 dirty/sidecar/数据库；mtime 精确相等而非生成日期大小比较；目录唯一权威；读取目标三路径实际核对；写前撤销、写后原子发布；五种生命周期操作全部接入；init 强制重建并先输出/刷新进度；fix 只重建索引；只读与 dry-run 零持久写入；默认不入 Git；安装/业务提交后的缓存失败警告退出 0，显式 fix 失败退出 1。

已明确不采纳只读命令自动持久化重建的可选建议；后续若要改变需另行确认。本方案不支持外部并发修改期间的强一致性，不把 mtime 相等当安全证明；全局无关目录异常允许延迟至重建发现。

### 可自主调整

私有帮助函数与测试 hook 的名字、缓存结构内部组织、兼容当前 MSRV 的日期格式库和必要依赖版本、基准脚本所在测试目录。不得改变 CSV 字段、输出流/退出码、自动恢复边界或把所有 tests::Sandbox 快照整体忽略 tmp。没有阻塞设计问题。

## 6. 验证与长期文档影响

### 自动验证

- 单元测试：空索引、UTF-8/LF/CRLF、固定字段、未知版本、缺少/重复元数据、重复 ID、未知状态、非法 ID、非法日期、整数溢出、截断与非法列；目录时间戳纳秒差别和 epoch 之前数值；确定性记录排序。
- 缓存集成：缺失/损坏/过期回退、warm changes/resolve 不枚举历史目录、mtime before/after 变化拒绝发布、mtime 不支持时只使用内存。利用注入时间/扫描 hook，不能依赖短 sleep 假设文件系统精度。
- 目标安全：伪造合法 CSV 漏掉目标或写错状态并恢复父目录 mtime，new 仍拒绝真实同名目录、resolve/迁移仍采用实际状态；真实跨状态重复 ID 拒绝操作，fix 扫描报错且不发布。
- 外部变化：通过测试手动新增、删除、跨状态移动或真实 Git checkout 改变目录 mtime，过期缓存不覆盖磁盘状态。保持 mtime 的无关目录变化不作为必须自动识别的承诺，强制 fix 必须发现。
- 生命周期：完整包和 proposal-only 的 new/complete/reopen/archive/cancel 后缓存与全量扫描一致，包内容/结果/任务/链接预检仍生效。初始缓存缺失也能完成写操作并建立缓存。
- 失败注入：撤销缓存失败不改业务；撤销后进程退出/业务失败不留旧缓存；archive 部分清理失败可重试；业务成功但 publish 失败保留目标状态、stderr 警告/退出 0；显式 fix 同类失败退出 1；外部抢占缓存路径不覆盖；缓存目录/文件链接与硬链接拒绝。
- init/fix：空项目、重复 init、全 SKIP、远端克隆式无 tmp 夹具、锁冲突、非法目录、重建失败保留安装；用记录 Reporter 和扫描 hook 断言开始事件与 flush 发生在扫描之前，完成事件发生在 publish 之后。
- 零写入：list/context/check 及所有 dry-run 对缓存和锁文件均无新增/修改/删除。只对成功重复 init 的幂等比较排除精确 CSV，并单独校验新日期和记录；失败/dry-run 仍比较全部文件和目录。
- 回归：列表过滤先于摘要 IO、实时任务和 UPDATED、包模式、归档恢复、安全和 skill 安装/update 既有测试继续通过。

### 性能与交付验证

加入显式运行而非普通 CI 时长门槛的规模基准：0、100、1,000、10,000 条，release 构建，记录冷 new、热 new、fix 的中位耗时及环境。历史目录可只放状态枚举所需条目，报告中注明不是完整工作包校验基准。通过可计数 hook 自动断言热 new 不调用权威扫描、不逐项 inspect 历史目录；同时报告 CSV 整体写入成本，不以不可靠毫秒阈值替代结构验收。

实施后执行 `cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets` 和 `cargo build --release --locked`。核对新依赖未无意抬升声明 MSRV；测试环境若无法覆盖某平台权限/链接行为，明确记录限制。

实际实现完成时更新 architecture 的模块/派生状态/恢复边界，更新 specs/cli.md 的 init/fix/只读/失败契约，并新增 [doco/specs/change-index-cache.md](../../../../specs/change-index-cache.md) 作为受限 CSV 格式和失效语义的权威说明。更新 README 的克隆后 init、fix 和忽略规则使用说明及 doco/README.md 导航。既有包文档格式、根 Agent 入口和 skill 工作流无需修改，不因内部缓存变更递增 skill bundle 版本。本阶段不写未来实现为当前架构，不完成或归档该变更。
