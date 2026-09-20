# 机械检查支持的文档格式

active/completed 支持两种合法形态：

- 完整变更包含 `proposal.md`、`work/implement.md`、`work/tasks.md`。没有模式标记的既有变更均按完整变更解释；缺少 work 不能自动降级。
- 仅 proposal 变更只包含 `proposal.md`，且 proposal 中必须有唯一的 `<!-- doco:change mode=proposal-only -->` 标记，`work/` 不得存在。未知或重复的 `doco:change mode=` 标记是格式错误。

archived 始终只保留 proposal，并保留其模式标记和生命周期时间。标题与正文内容可使用项目约定语言；以下是检查器识别的章节名，不支持任意同义改写。

## 生命周期时间

CLI 新建的 proposal 在标题前包含唯一单行标记：

```md
<!-- doco:lifecycle v=1 created-at=2026-09-14T10:00:00Z completed-at=- archived-at=- -->
```

三个字段使用 RFC3339 时间或 `-`，时间精确到整秒；CLI 写出时统一使用 UTC `Z`。`new` 写 created-at；`complete` 写或覆盖 completed-at；`archive` 和 `cancel` 写 archived-at；`reopen` 不改时间。该标记只记录列表显示所需的事件时间，生命周期状态仍只由工作包所在目录决定。

没有标记的既有 proposal 合法，未知事件不会从文件系统或 Git 推断。后续生命周期命令插入标记时，只填写本次实际发生的事件，其他未知字段保持 `-`。代码围栏内的示例不是标记；代码围栏外重复标记、未知版本、字段缺失或顺序错误、非法 RFC3339 和分数秒均为格式错误。系统时钟可能回拨，因此检查器不强制三个时间单调递增。

完整包的标记通常为 proposal 第一行；proposal-only 先保留 mode 标记，生命周期标记紧随其后。工具更新标记时保留 proposal 的其余内容和换行风格。归档仍只保留一个 `proposal.md` 文件。

## 必写章节

使用二级 Markdown 标题 `## `。章节必须唯一、正文非空，且没有 `TODO`、`TBD`、`{{...}}`、`待填写`、`待补充` 等模板标记。

| proposal | 可用标题 |
|---|---|
| 目的 | `Purpose` / `目的` |
| 范围 | `Scope and acceptance` / `Scope and completion criteria` / `范围与完成标准` |
| 结果 | `Result` / `Results` / `Outcome` / `结果` |

active 的结果可以是 `Pending — not completed.`；完成与普通归档时不能仍标记为 pending/尚未完成。取消命令将结果正文更新为取消原因与代码处理方式，不以任务完成作为取消前提。

仅 proposal 变更在 complete 和 completed 快照检查时，还必须在 proposal 中包含非空的 `Verification:` / `验证：` / `验证记录：` 字段，或 `## Verification` / `## 验证` / `## 验证记录` / `## 执行证据` 章节。该证据应记录实际命令、结果和限制；CLI 只能确认字段存在，不能验证真实性。active 普通 check 不要求尚未执行的验证。

完整变更的 implement 必须覆盖以下六节，标题可以有数字前缀：

| 英文模板标题 | 中文标题 |
|---|---|
| Baseline and goals | 现状、目标与设计基线 |
| Overall approach | 整体方案 |
| APIs and data model | 关键 API 与数据模型 |
| Algorithms and rules | 核心算法与实现规则 |
| Fixed decisions and discretion | 固定决策与可自主调整范围 |
| Verification and documentation impact | 验证与长期文档影响 |

这是机械识别约定，不是设计完整性的证明。小变更可以每节只写简短的明确决定；不适用的算法/文档影响要明确说明，而不是保留空模板。

## 任务

以下任务格式只适用于完整变更；仅 proposal 变更不使用任务复选框。

```markdown
- [ ] 1.1 实现容量上限
  - 完成条件：越界入队返回明确错误，不改变已有顺序。

- [ ] 2.1 验证边界与回归
  - 依赖：1.1
  - 完成条件：容量、顺序及退出行为通过约定检查。
```

- 列表标记可用 `-`、`*`、`+`；仅支持 `[ ]` 和小写 `[x]`。
- 稳定任务 ID 为两个以点分隔的正整数，例如 `1.1`、`1.2`、`2.1`；每段不得为零或带前导零。ID 必须唯一，后面必须有动作文本。编号用于人工分组，不要求连续、递增或存在对应的父级任务。
- 每项必须有非空 `Acceptance:` 或 `完成条件：`。
- `Dependencies:` / `Depends on:` / `依赖：` 支持空格、逗号或顿号分隔的任务 ID，也可写 `none` / `无`。依赖必须存在且无环；被勾选任务不能依赖未完成任务。
- `Blocked:` / `Blocker:` / `阻塞：` / `阻塞原因：` 是显式阻塞；已解决后移除或写 `none` / `无` / `resolved` / `已解决`。设计中的 `Open questions:` / `未决问题：` 同样参与完成检查。
- 完成时，tasks 至少要有非空 `Verification:` / `验证：` / `验证记录：` 字段，或对应的 `## Verification` / `## 验证` / `## 验证记录` / `## 执行证据` 章节。记录实际命令、结果和限制，而非仅写“通过”。CLI 不能验证记录是否真实。
- 正常 check 将未完成任务和明确阻塞报告为 WARNING；complete 和 completed 快照检查将其视为 ERROR。
- 代码围栏里的示例不计为实际任务。不要用注释、任务勾选或完成命令代替实际验证。

## 引用和归档

工作包使用相对 Markdown 链接；稳定跨变更引用可用 `[历史目标](doco:change-id)`。context 解析其状态和位置，但不会自动读取其他变更。

context 跟随显式 Markdown 链接与形似路径的行内代码，从当前架构和所选工作包找候选资料。ADR 文首可用 `Status: Superseded`、`Superseded by:`、`已被 ADR-0002 替代` 或 `状态：已替代` 标注旧决策；其他表述需要 Agent 判断，不能假设工具全部识别。

归档检查 doco 下保留的 Markdown 文件，拒绝其中可识别的 Markdown 链接、HTML href/src 和行内路径指向本次将删除的 work（含 URL 百分号编码路径）。不会分析任意自然语言、动态脚本、所有 HTML 编码或仓库外引用；proposal 独立可读仍需人工确认。找不到的普通源码引用可能是计划新增文件，check 仅警告，不能当作已实现证据。
