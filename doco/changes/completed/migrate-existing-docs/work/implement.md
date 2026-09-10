<!-- doco:managed template=v1 -->
# 实施设计

## 1. 现状、目标与设计基线
设计基线为提交 `20134676d673d9afa44ce296307da65978e1df83`；开始调研时工作区干净，本工作包是当前新增内容。

`assets/skill/SKILL.md` 是发布态工作流入口，只路由到 create、execute、complete、archive 四份 reference；“documentation-only work”只有原则性一句话。`.agents/skills/doco/` 是本仓库安装态副本。`src/templates.rs::FILES` 通过 `include_str!` 明确枚举所有安装文件，因此仅增加磁盘文件不会让 `doco init` 发布它。`tests/init.rs` 会遍历该清单验证所选 Agent 的安装及幂等性，但目前没有对迁移 reference 的直接断言。

`src/init/mod.rs` 只创建 `doco/architecture.md`、`specs/`、`decisions/` 和生命周期目录并安装 skill；它保留已有项目事实，不扫描、分类或搬运 legacy docs。当前架构和 `target/doco-design-v2.md` 已界定规范目标目录的职责，但没有迁移流程。目标是在 Agent 工作流层补齐该语义能力，不把不确定的内容分类和删除下沉到 CLI。

首次运行初始化回归时还确认了一个既有跨平台问题：Windows 的 `core.autocrlf` 会让 `include_str!` 取得 CRLF 发布资源，`src/init/skill.rs` 和 Claude 导入兼容性检查却只规范化磁盘旧内容、直接与未规范化的生成内容比较，因而把相同 bundle 误报为冲突。该问题阻塞新增 bundle 在当前支持环境中的可靠验证；按现有“支持 LF/CRLF”约束，将比较双方规范化属于保持语义的机械修正。

## 2. 整体方案
新增 `assets/skill/references/migrate.md`，将迁移定义为“当前文档维护”的辅助动作，而不是第五种变更生命周期。指南内部设置三个授权边界：

1. **盘点**：只读发现已有文档、引用关系和实际源码，不改文件。
2. **设计**：按内容单元分类，输出目标树、逐源处置表、冲突和待确认项；默认在此停止等待批准。
3. **执行**：仅在用户明确批准后，先写目标再更新引用，最后处理旧文件并验证。

`assets/skill/SKILL.md` 的 YAML description 增加迁移触发语义，并在动作列表加入 migration reference；已有“只做请求阶段”和“讨论不创建 change”的边界继续适用。根 AGENTS/CLAUDE 受管导航不修改，因为它已经将所有 project documentation 工作导向 doco skill，且本次不改变四阶段生命周期入口。

将新 reference 加入 `src/templates.rs::FILES`，使共享 `.agents` 和 Claude 安装都获得完整 bundle；同步修改本仓库 `.agents/skills/doco/SKILL.md` 并增加其 `references/migrate.md`。不增加独立迁移模板，盘点表格式直接放在 reference 中，避免把一次性清单误认为必须持久化的 doco 文档。

同时在 `src/init/skill.rs` 的受管文件更新、无标记模板识别和兼容安装检查，以及 `src/init/mod.rs` 的 Claude 导入 bundle 检查中，对生成内容和磁盘内容执行相同的 CRLF→LF 规范化。输出继续按旧文件换行风格渲染，不改变用户文件的保留策略。

## 3. 关键 API 与数据模型
不新增或修改 Rust 公共 API、CLI 参数、持久状态或机械文档格式。migration reference 的代码级集成点是：

```rust
("references/migrate.md", include_str!("../assets/skill/references/migrate.md")),
```

迁移指南要求 Agent 在设计阶段至少输出以下逐源记录，可在对话中呈现；只有用户要求追踪时才放入 active change 的 work：

| 字段 | 含义 |
|---|---|
| Source | 原文件及必要的章节范围 |
| Kind/status | architecture、contract、decision、active plan、history、user/ops、generated；并标注 current/planned/historical/unknown |
| Authority/evidence | 用于核实的源码、测试、配置、现有契约或负责人确认 |
| Target/action | 目标路径及 KEEP、MERGE、SPLIT、MOVE、CONDENSE、DEFER、REMOVE-AFTER-APPROVAL |
| Conflicts | 与代码、现有 doco 文档或其他来源的冲突 |
| Validation/disposition | 链接检查、内容覆盖和旧文件最终处理方式 |

内容映射固定为：

| 已核实内容 | 默认目标或处置 |
|---|---|
| 当前系统目标、技术栈、模块边界、依赖、控制流/数据流、状态和运行约束 | 合并到单一 `doco/architecture.md` |
| 外部协议、公开行为、持久格式、兼容性或必须精确一致的约束 | 按契约主题拆入 `doco/specs/<contract>.md` |
| 重要且不易从代码看出的长期选择依据 | `doco/decisions/NNNN-<decision>.md`；保留已替代状态，不重编号已有 ADR |
| 经确认仍要继续实施的未完成目标 | 经用户同意后使用 create 流程建立完整 active change，不机械复制旧状态或勾选任务 |
| 已完成、废弃或状态不明的计划/RFC | 不直接伪造成 completed/archived；提取已核实的当前事实或 durable decision 后默认保留、延后或按用户明确的历史导入要求单独处理 |
| README、教程、操作手册、贡献说明、许可证、changelog、生成或供应商文档 | 保留在适合其读者的原位置；必要时与 doco 双向链接，不为目录整齐而搬入 doco |

现有 doco 文件始终是待合并目标，不得覆盖。文件名冲突、change ID 冲突、相互矛盾的来源和无法核实的断言进入待确认项，不由 Agent静默裁决。

## 4. 核心算法与实现规则
`migrate.md` 按以下规则组织：

1. **确定范围与阶段**：确认是仅盘点、盘点加设计，还是已批准执行。没有执行授权时不得运行 `doco init`、创建 change、移动、重写或删除文档。迁移本身属于 documentation-only work，除非用户要求持久追踪，否则不自动创建 doco change。
2. **建立清单**：从根 README/索引、常见文档目录、Agent 入口和仓库内文档链接发现候选，不只按文件扩展名或固定目录匹配。跳过 `.git`、构建产物、依赖/供应商目录、`doco/tmp` 及默认隔离的 completed/archived；已有 `doco/architecture.md`、specs 和有效 ADR 作为目标与当前来源纳入比较。记录未找到入口的孤立文档和外部/Wiki 资料缺口，不宣称已完整扫描不可访问来源。
3. **按内容单元拆分**：一个文件可被 SPLIT 到多个职责，一个目录也可 MERGE；不得用 `docs/architecture`、`adr`、`spec` 等名称直接推断权威性。区分当前事实、未来计划、历史说明和受众型文档。
4. **核实真实性**：用当前代码、测试、配置和已批准契约核对待迁移事实。代码与 spec 不一致时标记冲突并判断实现缺陷或明确契约变更，不能修改 spec 掩盖问题；历史计划不能覆盖当前实现；不同来源不明确时保留原文并请求决定。
5. **输出迁移设计**：展示建议目标树、逐源处置表、需要合并/拆分的章节、命名和链接变化、准备删除或保留的文件、风险与未决项、验证方法。每个已盘点源都必须有 disposition；默认停下等待用户批准。
6. **按目标优先执行**：确认 doco 基础结构存在；若没有，先单独取得初始化授权。先合并 architecture，再写 specs 和 decisions；只把确认仍 active 的计划交给 create 流程。保留项目语言，避免复制实现细节，保留必要来源链接。写完目标并审核内容覆盖后再更新入口和反向链接，最后只处理获批的旧文件。默认不删除，遇到并发修改、路径/编码问题或新冲突立即停止受影响项。
7. **验证闭环**：检查目标树职责、重复权威来源、遗漏和计划/历史泄漏；搜索旧路径并修复有效引用；使用项目已有 Markdown/link 检查器；对导入的 active change 逐个运行 `doco check <id>`；查看 Git diff 确认无无关改动。未运行或无法运行的验证必须明确说明。

针对不同结构的适配不是单独硬编码路径，而是同一算法的几种组合：扁平文档通常 MOVE/MERGE；深层 docs 保留受众型目录并抽取当前技术事实；单体设计文档按章节 SPLIT；已有 ADR 保留编号与替代链后 MOVE/MERGE；其他变更管理体系先按真实状态分类，不能把其目录名直接映射成 doco 生命周期；重复或过期文档在权威目标验证后才 CONDENSE 或 REMOVE-AFTER-APPROVAL。

迁移必须满足两个不变量：任何删除都有明确授权且已验证目标承接或有意舍弃；任何写入 current doco 的断言都是已实现或已批准的当前契约，而不是未经核实的未来或历史内容。

## 5. 固定决策与可自主调整范围
固定决策：迁移是 skill 的语义指南而非 CLI 命令；新增一份按需读取的 reference，不增加模板；采用内容职责映射而非路径映射；设计先行且默认停在审批门；不自动创建 change；不强制搬迁非 doco 职责文档；不机械制造 completed/archived 历史；不经明确批准删除旧文档；不修改根受管导航和 `target/doco-design-v2.md`。发布 bundle 的等价性比较必须同时规范化生成内容和磁盘内容，并继续保留现有文件的换行风格。

实施者可调整英文措辞、清单示例和段落编排，只要上述分类、授权、冲突和验证规则完整且 `SKILL.md` 保持简短。测试可放在现有 init 测试或新增专门测试中，但必须直接证明 `references/migrate.md` 属于发布 bundle，并覆盖共享与 Claude 安装路径。未决问题：无。

## 6. 验证与长期文档影响
实施后运行：

- 针对 skill bundle 的测试，确认 `templates::FILES` 包含 migration reference，Codex/Pi 共享目录和 Claude 目录均安装该文件，重复 init 幂等，并覆盖生成内容与安装内容换行风格不同的等价比较。
- 检查 `assets/skill/` 与 `.agents/skills/doco/` 对应文件内容一致，且每个生成文件保留 managed marker。
- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `cargo build --release --locked`
- `git diff --check`
- `doco check migrate-existing-docs`

新增指南不改变 CLI、模块边界、持久格式或已批准契约，因此预计无需修改 `doco/architecture.md` 和 `doco/specs/`。它也没有引入需要长期解释的新技术取舍，不新增 ADR。发布态 skill 自身就是该工作流规则的当前来源；`target/doco-design-v2.md` 是非当前设计材料，不更新。
