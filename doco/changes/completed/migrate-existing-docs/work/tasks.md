<!-- doco:managed template=v1 -->
# 执行任务

- [x] 1.1 编写既有项目文档迁移指南
  - 设计：[实施设计](implement.md) 的“核心算法与实现规则”
  - 完成条件：`assets/skill/references/migrate.md` 覆盖只读盘点、语义分类、不同源结构映射、事实核实、逐源迁移设计、用户审批、目标优先执行、旧文件处置和验证闭环，并明确非 doco 文档与历史材料的边界。
  - 验证：已逐节复核英文 migration reference，并扫描发布态及安装态 skill，CJK 内容为 0 行。

- [x] 1.2 接入并同步 migration reference
  - 依赖：1.1
  - 设计：[实施设计](implement.md) 的“整体方案”和“关键 API 与数据模型”
  - 完成条件：发布态 `SKILL.md` 可触发并路由迁移动作，Rust 内嵌文件清单会安装新 reference，本仓库 `.agents/skills/doco/` 副本同步；bundle 等价比较同时规范化生成内容和磁盘内容的换行且保留旧文件风格；CLI、根受管导航和生命周期格式不变。
  - 验证：发布态与安装态 skill 的路径集合及文件字节逐项一致；换行回归单元测试通过。

- [x] 1.3 补充安装 bundle 回归测试
  - 依赖：1.2
  - 设计：[实施设计](implement.md) 的“验证与长期文档影响”
  - 完成条件：测试直接断言 migration reference 属于生成 bundle，并证明共享及 Claude skill 安装可获得该文件、不同换行风格不产生假冲突且重复初始化保持幂等。
  - 验证：`cargo test --test init` 通过 13 项；换行回归定向测试通过 1 项。

- [x] 2.1 完成整体质量检查与文档影响复核
  - 依赖：1.3
  - 完成条件：格式、Clippy、全量测试、release 构建、diff 检查和本变更 doco check 均有实际结果；发布态与安装态 skill 内容一致；确认无需修改当前 architecture、spec 或 ADR。
  - 验证：完整质量检查通过；发布态与安装态 skill 路径和字节一致，当前 architecture、spec 和 ADR 无需修改。

## 验证

- `cargo fmt --all --check`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo test --all-targets`：通过，共 65 项测试。
- `cargo build --release --locked`：通过。
- `git diff --check`：通过。
- `doco check migrate-existing-docs`：任务全部勾选后复查通过。
- 发布态及安装态 skill CJK 扫描为 0 行，路径集合和文件字节完全一致。
- 文档影响复核：未改变模块边界、持久格式或批准契约，无需更新当前 architecture、spec 或 ADR。
