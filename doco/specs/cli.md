# CLI 输出与交互

## 输出模式

业务输出通过语义渲染层写入原有流：计划、报告、上下文和成功信息使用 stdout，顶层运行错误使用 stderr。标签始终保留文字，颜色不承载唯一信息。来自文件、路径和错误链的控制字符会转换为可见转义，避免向终端注入控制序列。

全局 `--color auto|always|never` 只控制业务输出：

- `auto` 是默认值。对应输出流必须是 TTY，且 `NO_COLOR` 不为非空、`TERM` 不为 `dumb`，才输出 ANSI 样式。
- `always` 即使重定向也强制样式，适合明确需要 ANSI 的调用方。
- `never` 不输出业务 SGR。

stdout 和 stderr 分别判断。颜色与布局独立：TTY 中的 `list` 提供 STATE / CHANGE / TASKS / UPDATED 表头、对齐列、空态和状态统计；TASKS 显示已完成/总任务数，例如 `8/10`，UPDATED 显示工作包最近修改距今时长。重定向时按 ID 排序输出 `state<TAB>id<TAB>done/total<TAB>updated`，空列表无 stdout。`--color always` 不会把管道列表切换成 TTY 布局。Clap 的帮助、版本和参数错误保持标准无色格式。

`list` 默认只查询并显示 active。`--completed` 在 active 之外追加 completed，`--archived` 追加 archived；两项可以组合以显示全部状态。active 始终包含，首版不提供排除 active 或只显示历史状态的模式。状态过滤在读取任务和遍历工作包修改时间之前完成，因此未启用状态中的任务或工作包内容错误不影响结果；排序及 TTY 底部统计只基于可见行。

任务数先安全读取 active/completed 的 proposal 模式。完整变更读取 `work/tasks.md`，只统计任务解析器识别的 `[ ]` / `[x]` 任务行，忽略围栏示例和注释；任务文件为空显示 `0/0`，缺失显示 `?`。仅 proposal 变更和已归档变更显示 `-`，不读取不存在或已丢弃的工作材料。无模式标记的包仍是完整变更，不能因 tasks 或整个 work 缺失而显示 `-`。不安全路径、读取失败、非法 UTF-8 或非法模式仍报错，不伪装成零任务。计数是即时只读摘要，不等同于 check 通过或真实验收。

UPDATED 取工作包根目录及全部后代中最新的文件系统修改时间，并以一次列表查询的统一时刻计算。时长向下取整且不带小数：不足一小时显示分钟（`10m`），不足一天显示小时（`20h`），一天及以上显示天和非零剩余小时（`1d2h`、`20d5h`）；不足一分钟或时间戳略晚于当前时钟时显示 `0m`，无法取得修改时间时显示 `?`。该值不持久化、不改变列表排序，也不等同于生命周期事件时间。

## 变更包创建

`doco new <id>` 默认创建 proposal、实现设计和任务列表的完整骨架。`doco new <id> --proposal-only` 只创建带显式模式标记的 `proposal.md`，不创建 `work/`；它适用于范围、预期行为和验收都能在 proposal 中完整表达，且不需要独立设计选择或依赖任务拆分的受跟踪变更。CLI 不按行数、文件数或风险自动判断变更规模。

仅 proposal 模式是持久格式选择，不表示所有小型代码修改都必须创建变更。doco 归档不保存实现历史；普通行为修复和实现细节修改可以直接进行，Git、PR 或项目发布记录负责实现历史。无论是否创建变更，已记录的当前架构或规范事实发生变化时仍需同步当前文档。

## 初始化与 skill 更新

根 `SKILL.md` 以独立的 `v<非负整数>` 标记记录整个 doco skill bundle 的版本，当前版本为 `v2`；references 和模板文件不重复记录该版本。已安装版本缺失或无法唯一识别时按 `v0` 比较，但版本判断不授予文件归属：没有 doco 受管标记的同名文件仍然冲突，即使传入 `--refresh` 也不覆盖。

CLI 内置版本严格高于已安装版本时，普通 `init` 无需 `--refresh` 即刷新该目录下全部已知 skill 文件，保留未知文件。相同版本的受管内容差异仍要求 `--refresh`；更高的已安装版本不会被普通 init 自动降级。显式 `--refresh` 保留覆盖受管内容及强制降级的能力。

每个 skill 目录独立提交：先写 references 和模板，最后写根 `SKILL.md`。可捕获的组内写入失败会逆序恢复本次已改文件，并删除本次新建的文件和空目录；成功回滚输出 `ROLLED BACK` 并以失败状态退出。回滚步骤失败或检测到外部并发修改时输出 `ROLLBACK FAILED` 和未恢复路径，不覆盖无法确认的当前内容。该保证不跨不同 Agent skill 目录、项目文档或导航文件，也不覆盖强制终止、断电和持续文件系统故障。

## 交互范围

`--interactive` 和 `--no-interactive` 互斥。只有以下情况会打开菜单：

- `init` 未提供 `--agent`：在可交互终端多选 Codex、Claude Code、Pi。
- `context`、`check`、`complete`、`archive`、`reopen`、`cancel` 省略变更 ID 且显式提供 `--interactive`：按命令所需生命周期状态选择已有变更。

`new` 始终要求显式 ID，不提供交互输入。明确传入已有变更 ID 时不打开菜单，即使同时存在 `--interactive`。`--yes` 不选择目标或补充参数。

菜单使用方向键移动、Space 切换多选、Enter 提交、Esc 或 Ctrl-C 取消。单一候选也必须 Enter 提交，不会自动执行。每页最多显示 10 项，并根据终端高度缩小。菜单写 stderr，不从重定向 stdin 读取；交互要求 stdin、stdout、stderr 都是 TTY 且 `TERM` 不为 `dumb`。菜单取消退出 130，业务或 IO 错误退出 1，Clap 参数错误退出 2。

建议 CI 和 Agent 自动化传入所有参数并使用 `--no-interactive --color never`。

## 归档与取消

`archive` 和 `cancel` 依次执行状态、proposal、链接、目录内容及目标路径预检，输出 RETAIN / UPDATE / DELETE / MOVE 范围。`--dry-run` 在预览后返回，不写文件。

非 dry-run 在预览成功后不再询问破坏性确认，而是获取项目写锁、重新解析状态并复核工作包快照，再按既有恢复规则删除和移动。`--yes` 为旧脚本保留，但不改变执行行为。选择命令本身即授权执行；需要人工检查范围时应先单独运行 `--dry-run`。

仅 proposal 变更不存在 work 是正常状态，预览显示说明而非 `RECOVERY`；完整变更若已在先前失败中删除 work，仍显示 `RECOVERY` 并允许完成剩余移动。模式标记与 work 同时存在时拒绝归档。

删除仍不递归跟随链接。部分清理或移动失败时，按输出的 APPLIED / DELETED 路径和错误说明修复后重试；命令不承诺代码回滚、Git 操作或跨文件事务。archive 最终只保留 proposal，不是实现历史存储。
