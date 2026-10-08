# CLI 输出与交互

## 输出模式

业务输出通过语义渲染层写入原有流：计划、报告、上下文和成功信息使用 stdout，顶层运行错误使用 stderr。标签始终保留文字，颜色不承载唯一信息。来自文件、路径和错误链的控制字符会转换为可见转义，避免向终端注入控制序列。

全局 `--color auto|always|never` 只控制业务输出：

- `auto` 是默认值。对应输出流必须是 TTY，且 `NO_COLOR` 不为非空、`TERM` 不为 `dumb`，才输出 ANSI 样式。
- `always` 即使重定向也强制样式，适合明确需要 ANSI 的调用方。
- `never` 不输出业务 SGR。

stdout 和 stderr 分别判断。颜色与布局独立：单库 TTY `list` 提供 STATE / CHANGE / TASKS / AGE 表头、对齐列、空态和状态统计；TASKS 显示已完成/总任务数，例如 `8/10`，AGE 显示当前状态对应生命周期事件距今时长。单库重定向时按 ID 排序输出 `state<TAB>id<TAB>done/total<TAB>age`；项目含多个库时，TTY 和管道输出都在最前增加 PROJECT 列，管道为 `project<TAB>state<TAB>id<TAB>done/total<TAB>age`。列数由发现的库数量决定，不因状态过滤或某个库为空改变。project 是相对最顶层库的路径，分隔符统一为 `/`，顶层库为 `.`。路径中的控制字符可见转义；窄终端仍保留库路径。空列表无管道 stdout。`--color always` 不会把管道列表切换成 TTY 布局。Clap 的帮助、版本和参数错误保持标准无色格式。

`list` 默认只查询并显示 active。`--completed`（缩写 `-c`）在 active 之外追加 completed，`--archived`（缩写 `-a`）同时追加 completed 和 archived，显示全部状态；两项组合与单独使用 `--archived` 相同。active 始终包含，首版不提供排除 active 或只显示历史状态的模式。状态过滤在读取 proposal、任务和生命周期时间之前完成，因此未启用状态中的任务或工作包内容错误不影响结果；排序及 TTY 底部统计只基于可见行。

任务数先安全读取 active/completed 的 proposal 模式。完整变更读取 `work/tasks.md`，只统计任务解析器识别的 `[ ]` / `[x]` 任务行，忽略围栏示例和注释；任务文件为空显示 `0/0`，缺失显示 `?`。仅 proposal 变更和已归档变更显示 `-`，不读取不存在或已丢弃的工作材料。无模式标记的包仍是完整变更，不能因 tasks 或整个 work 缺失而显示 `-`。不安全路径、读取失败、非法 UTF-8 或非法模式仍报错，不伪装成零任务。计数是即时只读摘要，不等同于 check 通过或真实验收。

AGE 按目录状态选择 proposal 中持久化的事件时间：active 使用 created-at，completed 使用最近一次 completed-at，archived 使用 archived-at。一次列表查询使用统一当前时刻计算差值。时长向下取整且不带小数：不足一小时显示分钟（`10m`），不足一天显示小时（`20h`），一天及以上显示天和非零剩余小时（`1d2h`、`20d5h`）；不足一分钟或事件时间略晚于当前时钟时显示 `0m`。旧包缺少标记或当前状态对应字段时显示 `?`，不回退文件系统时间；非法或重复标记报错。AGE 不改变列表排序，reopen 后的 active 仍显示最初创建距今时长。

## 项目定位与父子库

显式 `--root <目录>` 精确选择库根，不回退父目录。未指定 `--root` 时，除 `init` 外的命令从当前目录向上寻找最近的 doco 库，先检查所在目录，再在 Git 根（`.git` 目录或文件）处停止。未找到库时提示初始化且不写文件；`init` 始终以当前目录为目标，不转而初始化父库。`Project::open` 库 API 仍保持精确打开，`Project::discover` 提供就近定位。

父子关系由目录位置推导，无注册表。最近的祖先库是父库；项目范围为当前库所在祖先链的最顶层库及其全部后代库，包含兄弟库。没有父库的子文件夹独立初始化为顶层库。嵌套 Git 仓库、worktree 和 submodule 是独立项目边界，不纳入外层项目，也不向外层查找父库。跨越普通目录寻找后代库，跳过符号链接和 Windows reparse point/junction，不进入 `doco`、`.git`、`.hg`、`.svn`、`.agents`、`.claude`、`.pi`、`.codex`、`.tgrep`、`node_modules`、`target`、`dist`、`build`、`vendor`、`.venv`、`venv`、`__pycache__`。不读取 Git ignore 规则作为库配置。

包含 `doco/architecture.md` 或 `doco/changes` 的目录视为库候选，并按初始化契约检查；普通同名目录不算库。发现的库缺少必需结构、存在不安全路径或无法读取时明确报错，收集完成前不输出部分变更，不能静默漏列或回退父库。不允许在父库的 `doco/` 存储内部初始化子库。

`doco list` 不填范围参数即列出整个项目的库：当前库全部变更优先，其余从最顶层父库开始深度优先，父先于子，同级目录按路径排序，当前库跳过不重复；每个库内按 ID 排序。例如根库下有 `api`、`api/auth` 和 `web`：

| 所选库 | 库顺序 |
| --- | --- |
| 根 | 根 → api → auth → web |
| api | api → 根 → auth → web |
| auth | auth → 根 → api → web |
| web | web → 根 → api → auth |

在 `api/auth/src` 执行时当前库为 auth。默认 active、`-c` 和 `-a` 的状态规则在每个库分别应用；一次聚合使用统一当前时刻计算 AGE，统计仅包含可见变更。只读查询不写注册表、索引或锁。兼容的 `list_changes`、`list_changes_filtered` 库 API 仍只查询传入的库；CLI 使用 `list_project_changes_filtered` 聚合。

父库描述整体架构和共享约束，子库描述局部事实，不自动复制、合并或覆盖文档。库各自拥有变更目录、索引和锁；ID 只在库内跨状态唯一，允许父子库同名。除 `list` 外，生命周期命令、context、check、fix 和交互候选仅使用所选库，不因聚合列表扩大范围，`doco:<id>` 引用也仅在所选库解析。操作其他库需明确运行 `doco --root <库根> <命令>`。

## 变更包创建

`doco new <id>` 默认创建 proposal、实现设计和任务列表的完整骨架。`doco new <id> --proposal-only` 只创建带显式模式标记的 `proposal.md`，不创建 `work/`；它适用于范围、预期行为和验收都能在 proposal 中完整表达，且不需要独立设计选择或依赖任务拆分的受跟踪变更。两种模式都会在 proposal 中写入 created-at；CLI 不按行数、文件数或风险自动判断变更规模。

完整包支持可选 `work/specs/**/*.md` 目标规格，目录和文件由用户/Agent 按需创建；`new` 不预建空目录、占位规格，也没有新增规格参数。skill 提供 `templates/spec.md`，用于复杂行为、接口或兼容性变更。proposal-only 仍禁止 work，需要独立规格时应先转换为完整包。

`context` 自动把所选完整包中的工作规格加入路径候选，排序、去重并继续跟随显式引用；completed 需要 `--history` 并使用快照标识。工作规格不是当前有效契约，未引用的其他 work 材料和其他变更不自动进入范围。`check` 对存在的规格检查正文、模板残留、引用及显式阻塞，阻塞在 active 普通检查中警告，在 complete/completed 检查中报错；缺失或空规格目录不警告。详细规则见[文档格式](document-format.md)。

仅 proposal 模式是持久格式选择，不表示所有小型代码修改都必须创建变更。doco 归档不保存实现历史；普通行为修复和实现细节修改可以直接进行，Git、PR 或项目发布记录负责实现历史。无论是否创建变更，已记录的当前架构或规范事实发生变化时仍需同步当前文档。

## 初始化与集成更新

检测到父库时，`init` 仅创建目标目录内的 `doco/` 骨架并重建本库索引，显示目标及最近父库，不修改父库。无需 `--agent`，不弹 Agent 选择；显式 `--agent` 和 `--refresh` 也不创建、刷新或删除子目录中的任何 skill、AGENTS/CLAUDE 入口或既有集成。已存在的当前文档保持不变，dry-run 零写入。子库复用顶层库的 skill 与入口；顶层入口说明 skill 路径相对入口目录，文档路径相对所选库。子库 `update` 成功返回且零写入，提示用 `--root` 在顶层库更新共享集成，不自动修改父库。根库的交互与集成规则如下。

`init --agent` 只有两个正式值：`most` 将 skill 安装到 `.agents/skills/doco` 并在 `AGENTS.md` 写受管入口，`claude` 将 skill 安装到 `.claude/skills/doco` 并在 `CLAUDE.md` 写受管入口；两项可组合且重复项会去重。旧 `codex`、`pi` 参数不再接受，`.pi/skills/doco` 和 `.codex/skills/doco` 也不再是安装目标；选择 Most agents 时发现这些旧版或替代路径会要求先人工迁移。

根 `SKILL.md` 以独立的 `v<非负整数>` 标记记录整个 doco skill bundle 的版本，当前版本为 `v5`；references 和模板文件不重复记录该版本。AGENTS/CLAUDE 的 DOCO 块在 START 后以唯一 `<!-- doco:entry template=v2 -->` 记录入口模板版本。已安装版本缺失、格式错误、重复或无法解析时按 `v0` 比较，但版本判断不授予文件归属：没有 doco 受管标记的同名 skill 仍然冲突，入口缺失或边界歧义也不能用 `--refresh` 强行取得归属。

CLI 内置版本严格高于已安装版本时，普通 `init` 或 `update` 无需 `--refresh` 即刷新对应受管内容。相同版本的内容差异仍要求 `--refresh`；更高的已安装版本不会被普通运行降级。显式 `--refresh` 保留覆盖受管内容及强制降级的能力。入口更新只替换 DOCO 块，块外字节保持不变；块外任何 doco 相关自然语言均不参与检测。入口归属只认代码围栏外唯一且按顺序闭合的 `<!-- DOCO:START -->` 与 `<!-- DOCO:END -->`。没有标记时保留原文并追加新受管块，不自动收编无标记模板；标记缺失、重复、嵌套或顺序错误时，即使使用 `--refresh` 也拒绝写入，并提示删除整个 DOCO 块后重试。

根库的 `doco update` 不接受 Agent 参数，也不创建集成；它要求项目已初始化，并检测 `.agents`/AGENTS 与 `.claude`/CLAUDE 两个固定组合。完整组合会全部更新；skill 与受管入口只存在一侧、入口引用目录不一致、同名内容不受管、发现旧路径或两个组合都不存在时失败并提示使用 `init` 或人工迁移。只有 `CLAUDE.md` 的 `@AGENTS.md` 导入时，Claude 复用 Most agents，不算独立 Claude 安装；独立 Claude skill 与该导入并存时拒绝。`update --dry-run` 只显示计划，零写入。

每个 skill 目录独立提交：先写 references 和模板，最后写根 `SKILL.md`。可捕获的组内写入失败会逆序恢复本次已改文件，并删除本次新建的文件和空目录；成功回滚输出 `ROLLED BACK` 并以失败状态退出。入口文件在 bundle 之后独立写入，不参与该回滚组。回滚失败或检测到外部并发修改时输出 `ROLLBACK FAILED` 和未恢复路径，不覆盖无法确认归属的当前内容。该保证不跨不同 skill 目录、入口文件或项目文档，也不覆盖强制终止、断电和持续文件系统故障。

## 变更索引缓存

`init` 在根库集成安装或子库文档初始化成功后重建本库的本机变更索引缓存，即使受管文件全部为 SKIP。扫描前在 stdout 输出 `INDEX Building change index cache...` 并刷新，完成后输出 `INDEX Cache built: <n> active, <n> completed, <n> archived.`。缓存步骤失败不撤销已成功的安装：stderr 输出 WARNING，说明安装已完成、缓存未重建及修复方式，命令仍以 0 退出。`init --dry-run` 只在计划中输出 `INDEX Would rebuild change index cache.`。

`doco fix` 只重建索引：在写锁内先撤销旧缓存，再权威扫描并原子写入，不修改文档、不移动或删除工作包、不自动解决重复 ID。未初始化项目提示先运行 `doco init`，且不创建任何文件。成功输出同样包含 Building 和 Cache built 两行。`fix --dry-run` 校验项目并扫描计数，输出 `INDEX Would rebuild cache: ...`，不获取写锁、不写缓存、不创建 tmp；重建失败退出 1。

`new`、`complete`、`reopen`、`archive`、`cancel` 在成功提交后更新缓存。缓存不可用不影响业务结果：命令仍成功退出 0，只在 stderr 警告并提示 `doco fix`。只读命令（`list`、`context`、`check`）缺少缓存时扫描到内存，不持久化也不加写锁；`list` 的 stdout 不包含缓存诊断。缓存格式、失效条件和信任边界见[变更索引缓存](change-index-cache.md)。

## 交互范围

`--interactive` 和 `--no-interactive` 互斥。只有以下情况会打开菜单：

- 根库 `init` 未提供 `--agent`：在可交互终端多选 Most agents（`.agents` / `AGENTS.md`）和 Claude（`.claude` / `CLAUDE.md`）。
- `context`、`check`、`complete`、`archive`、`reopen`、`cancel` 省略变更 ID 且显式提供 `--interactive`：按命令所需生命周期状态选择已有变更。

`update`、`fix`、`new` 始终不提供交互输入。明确传入已有变更 ID 时不打开菜单，即使同时存在 `--interactive`。`--yes` 不选择目标或补充参数。

菜单使用方向键移动、Space 切换多选、Esc、Ctrl-C 或 q 取消。多选菜单按 Enter 时先把当前高亮项纳入选择，再立即提交全部已选择项；已选择的当前项不会被反选，先前用 Space 选择的项会保留。单选菜单按 Enter 直接提交当前项。单一候选也必须 Enter 提交，不会自动执行。每页最多显示 10 项，并根据终端高度缩小。菜单写 stderr，不从重定向 stdin 读取；交互要求 stdin、stdout、stderr 都是 TTY 且 `TERM` 不为 `dumb`。菜单取消退出 130，业务或 IO 错误退出 1，Clap 参数错误退出 2。

建议 CI 和 Agent 自动化传入所有参数并使用 `--no-interactive --color never`。

## 归档与取消

`archive` 和 `cancel` 依次执行状态、proposal、链接、目录内容及目标路径预检，输出 RETAIN / UPDATE / DELETE / MOVE 范围。`--dry-run` 在预览后返回，不写文件。

非 dry-run 在预览成功后不再询问破坏性确认，而是获取项目写锁、重新解析状态并复核工作包快照，先将 archived-at 原子写入 proposal，再按既有恢复规则删除和移动。`cancel` 同时写入取消结果和 archived-at。`--yes` 为旧脚本保留，但不改变执行行为。选择命令本身即授权执行；需要人工检查范围时应先单独运行 `--dry-run`。

仅 proposal 变更不存在 work 是正常状态，预览显示说明而非 `RECOVERY`；完整变更若已在先前失败中删除 work，仍显示 `RECOVERY` 并允许完成剩余移动。模式标记与 work 同时存在时拒绝归档。完整包的工作规格与其他 work 材料一样出现在删除预览中，归档或取消后不保留；保留文档指向这些规格的引用仍会阻止归档。

删除仍不递归跟随链接。部分清理或移动失败时，按输出的 APPLIED / DELETED 路径和错误说明修复后重试；此时缓存已被撤销，重试会重新扫描目录。命令不承诺代码回滚、Git 操作或跨文件事务。时间写入后发生清理或移动失败时，源状态保持不变，重试会覆盖目标事件时间；list 始终按实际目录状态选择字段。archive 最终只保留含生命周期时间的 proposal，不是实现历史存储。
