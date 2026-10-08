<h2 align="center">
doco
</h2>

<p align="center">
<a href="../README.md">English</a>
</p>

`doco` 是一个面向 Agent 的文档 CLI 工具。灵感来自 Fission-AI 的 [OpenSpec](https://github.com/Fission-AI/openspec)。

## 理念

doco 将文档分为两类：

- `现状文档`只关心目前项目是什么样的，包括项目架构、技术决策、规格
  - doco 认为代码是良好的现状文档，会引导 Agent 创建代码索引，并只创建必要的现状文档
- `变更文档`只关心本次变更做什么，包括目前地、规格（可选）、设计（可选）、任务

doco 的一个特点是灵活，一次变更的大部分文档都是可选的，你可以让 ai 创建的很详细，以使用低上下文容量和切换 agent 的情况。
你也可以让 Agent 创建得很简单，快速实现这次任务，不在文档上浪费时间和 Token。

大部分时候，doco skill 会帮助 Agent 自己选择创建详细变更还是轻量化变更，你不需要关心这件事。

此外 doco 在意 CLI 工具得速度，你可以见 [命令性能](#命令性能) 一节了解性能表现。决不让维护文档成为 Agent 工作的累赘。

## 快速开始

使用 cargo 安装：

```
cargo install doco-cli
```

然后在工作区执行下列命令即可：

```
doco init
```

### 更新

```
cargo install doco-cli
```

然后在工作区执行  `doco update` 更新 Skill。

### 自行构建

也可以克隆本项目并从源码构建，需要 Rust 1.85 或更高版本：

```sh
cargo install --path . --locked
```

## 交互示例

```
你：我想优化 CLI 的 new/complete/archive/list 命令速度，觉得加一个热缓存比较合适。给我一份设计方案。
AI：[设计方案]
你：好的，创建 doco 变更。
AI：doco new optimize-cli-commands
    - proposal.md
    - work/
      - implement.md
      - tasks.md
      - specs/          # 可选的目标契约，按需编写
        - cache.md
> 切换到成本更低的模型
你：开始实现。
```

要查看目前的变更列表，使用 `doco list`。

## 子目录库

在已初始化项目的子目录中执行 `doco init`，只创建该目录的 `doco/` 库，不安装 skill、不创建 `AGENTS.md` 或 `CLAUDE.md`，也不修改既有入口。子库复用顶层库的工作流，无需选择 Agent。

普通命令向上找到最近的库；`--root <目录>` 精确选库，不回退父目录。`init` 始终在当前或指定目录初始化。各库的变更 ID、写操作、索引和锁相互独立。

`doco list` 默认列出整个项目父子库及兄弟库的 active 变更：当前库优先，其余由最顶层父库开始按父先于子的顺序展示，多库输出增加 `PROJECT` 列。`-c` 加入 completed，`-a` 显示全部状态。嵌套 Git 根和链接目录不纳入外层项目。在顶层库执行 `doco update` 更新共享工作流。

## 深入聊聊变更

doco 将变更分为需要在归档后保留的 `proposal.md`，和仅在本次工作中保留的 `work/` 目录。`work/` 目录包含可选的 `implement.md / specs / tasks.md` 来保证跨会话和模型的稳定性。

- `implement.md` 记录了关键代码实现，包括结构体设计、较难的算法等
- `tasks.md` 是一个可以跟踪的任务列表，记录详细的执行步骤
- `spcs` 记录了具体的设计规格，记录代码以外的约束，例如交互等

不过，大部分变更根本不需要全套的 `work/` 文件，一些小修小改往往只需要一个 `proposal.md + task.md`，甚至干脆不需要使用 doco 变更。所以，doco Skill 也会引导 Agent 不要每次任务都创建 spec，小任务直接执行即可。

## 命令性能

下面是 0~10000 个变更的实测表现（Intel i7-10700），单位为秒。

| 命令                     | 0 个变更 |   100 | 1,000 | 10,000 |
| ---------------------- | ----: | ----: | ----: | -----: |
| `doco new`             | 0.082 | 0.083 | 0.080 |  0.081 |
| `doco check`           | 0.058 | 0.054 | 0.062 |  0.059 |
| `doco complete`        | 0.100 | 0.097 | 0.123 |  0.100 |
| `doco list`            | 0.035 | 0.033 | 0.053 |  0.053 |
| `doco list --archived` | 0.032 | 0.215 | 1.874 | 17.351 |
| `doco archive`         | 0.117 | 0.830 | 8.404 | 69.865 |
| `doco fix`（重建缓存）       | 0.046 | 0.127 | 0.779 |  6.489 |
你也可以自己运行测试：

`cargo test --release --test scale -- --ignored --nocapture`。

## 文档索引

- [文档指南](../doco/README.md)
- [架构与故障恢复](../doco/architecture.md)
- [CLI 输出与交互契约](../doco/specs/cli.md)
- [机器可读的文档格式](../doco/specs/document-format.md)
- [本地变更索引缓存](../doco/specs/change-index-cache.md)
