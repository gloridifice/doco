# versioned-skill-refresh

## Purpose

当前 CLI 编译进二进制的 doco skill 没有独立的 bundle 版本。内置 references 或模板变化后，`doco init` 只能按单文件内容差异报冲突，用户必须显式传入 `--refresh`，CLI 无法判断已安装 skill 是否过旧。

本变更只在根 `SKILL.md` 中记录单整数版本。若 CLI 内置版本高于已安装版本，普通 `doco init` 自动把整个受管 skill bundle 更新为内置内容：先更新所有其他文件，最后更新 `SKILL.md` 作为提交标记。更新过程中出现可处理的写入失败时，撤销本次已经写入的 bundle 文件，恢复更新前状态。

## Scope and acceptance

范围：

- 仅在内置 `SKILL.md` 写入 skill bundle 版本，格式固定为 `v` 加一个非负十进制整数，例如 `v0`、`v1`、`v2`；references 和模板文件不写 skill 版本。
- 首个版本为 `v1`。已安装 `SKILL.md` 中缺失、格式错误、重复或无法解析的版本统一按 `v0` 比较。
- 版本识别与文件归属识别分离：版本为 `v0` 不会使无 doco 受管标记的同名自定义 skill 获得覆盖权限。
- CLI 内置版本严格高于已安装版本时，无需 `--refresh`，将 `templates::FILES` 中除 `SKILL.md` 外的全部已知文件更新到内置内容，再将 `SKILL.md` 更新为新内容和新版本。未知自定义文件保留。
- skill bundle 作为独立原子回滚组执行。任一 bundle 文件写入失败时，按相反顺序恢复本次已替换文件的原始字节与支持的权限、删除本次新建文件，并清理本次新建且仍为空的目录；根 `SKILL.md` 不得提前保留新版本。
- 回滚成功后命令返回失败并明确说明 skill 更新未生效；回滚自身失败或发现外部并发修改时，不覆盖无法确认归属的当前内容，并报告未恢复路径及人工恢复要求。
- 新安装和显式 `--refresh` 也使用相同的 bundle 写入顺序与回滚机制，避免这些路径继续产生半个 skill bundle。
- 同版本但内容不同的受管文件仍要求 `--refresh`；已安装版本高于 CLI 内置版本时不自动降级。`--refresh` 的显式覆盖能力保持不变。
- Pi-only 初始化可以复用并事务性升级旧版 `.pi/skills/doco`；无关同名 skill 仍然冲突。
- 增加版本、更新顺序、故障注入回滚、恢复失败诊断、Pi 复用和既有安全边界测试，并同步当前架构与 CLI 行为文档。

验收标准：

- 内置 `v1` CLI 对一个有 doco 归属但没有可识别版本的 `SKILL.md` 按 `v0` 处理；普通 `doco init` 计划更新整个已知 bundle，不需要 `--refresh`。
- 成功升级时，所有其他已知文件先达到内置内容，根 `SKILL.md` 最后写入 `v1`；完成后再次初始化保持幂等。
- 在任意非 `SKILL.md` 文件或最终 `SKILL.md` 写入处注入失败，命令退出后所有已存在文件恢复原始内容，本次新建文件消失，本次新建的空目录被清理，`SKILL.md` 保持原版本。
- 同版本人工修改仍在写入前冲突，传入 `--refresh` 后才覆盖；高于内置版本的 skill 不会被普通初始化降级。
- 无 doco 归属的同名文件不会因版本回退为 `v0` 而被覆盖；未知自定义文件始终保留。
- dry-run、UTF-8 BOM、LF/CRLF、预检零写入、链接/硬链接拒绝和并发复核等既有契约继续通过测试。

非目标：

- 不在 references、skill 自带的 change skeleton、`AGENTS.md`、`CLAUDE.md` 或项目文档中复制 skill 版本。
- 不把 skill 版本绑定到 `Cargo.toml` 的 crate semver，也不引入多段版本、版本区间或迁移脚本。
- 不做三方合并；旧版受管文件中的人工修改会随 bundle 自动升级被内置新版替换，并在计划 diff 中展示。
- 不把整个 `doco init` 变成跨目录事务。每个 skill bundle 独立回滚；一个 bundle 提交后，后续 Agent bundle 或导航文件失败仍沿用现有部分初始化报告。
- 不承诺在进程被强制终止、断电、文件系统故障持续存在或外部进程并发改写时实现物理事务。可捕获的普通写入失败必须执行回滚；无法安全完成回滚时必须显式报告，不能声称已经恢复。

## Result

Pending — not completed.
