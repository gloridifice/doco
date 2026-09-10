# 执行任务

- [x] 1.1 切换任务编号解析与诊断
  - Design: [实施设计](implement.md)
  - Acceptance: 任务行和依赖仅接受两级正整数编号；旧格式及零、前导零、层级错误被拒绝，既有唯一性和依赖语义不变。

- [x] 1.2 更新自动化测试与夹具
  - Dependencies: 1.1
  - Acceptance: 生命周期和列表测试使用新编号，并覆盖合法编号、旧格式拒绝及关键非法边界。

- [x] 2.1 同步格式文档、skill 和仓库工作包
  - Dependencies: 1.1
  - Acceptance: 当前格式文档、发布与安装态 skill、模板以及仓库 active/completed 工作包全部使用新编号，任务引用保持一致且不残留旧格式。

- [x] 2.2 执行整体质量验证
  - Dependencies: 1.2, 2.1
  - Acceptance: fmt、clippy、全部目标测试、release locked 构建和 Rust 1.85 locked 检查通过；项目扫描确认旧编号已清除并记录实际结果。

## Verification

- 1.1：任务与依赖正则已切换为两级正整数格式，诊断示例同步更新；旧格式、零、前导零、单级和三级编号拒绝测试通过。
- 1.2：`cargo test --all-targets` 通过，共 64 项；生命周期测试覆盖模板编号、重复、未知/非法依赖、环、未完成依赖及非法编号边界，列表测试继续覆盖围栏、注释和任务计数。
- 2.1：格式文档、发布与安装态 skill、模板、测试夹具及仓库 active/completed 工作包已迁移；脚本扫描项目源码和文档，未发现旧式连续任务编号；发布与安装态 skill 目录逐文件比较无差异。
- 2.2：`cargo fmt --all --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets`、`cargo build --release --locked`、`cargo +1.85 check --all-targets --locked` 和 `git diff --check` 均通过；`doco check` 对本变更、`list-modified-age` 和已迁移的 `cli-visual-interaction` 均通过。
