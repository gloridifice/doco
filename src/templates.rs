use std::sync::LazyLock;

pub const MARKER: &str = "<!-- doco:managed template=v1 -->";

// Raw `include_str!` bytes follow whatever the build checkout contains, so a
// Windows `core.autocrlf` checkout would embed CRLF. `FILES` normalizes once to
// LF: new installs and marker parsing stay deterministic across platforms,
// while updates to existing files still render with the destination file's
// newline style via `markdown::styled`.
static RAW_FILES: &[(&str, &str)] = &[
    ("SKILL.md", include_str!("../assets/skill/SKILL.md")),
    (
        "references/migrate.md",
        include_str!("../assets/skill/references/migrate.md"),
    ),
    (
        "references/create.md",
        include_str!("../assets/skill/references/create.md"),
    ),
    (
        "references/execute.md",
        include_str!("../assets/skill/references/execute.md"),
    ),
    (
        "references/complete.md",
        include_str!("../assets/skill/references/complete.md"),
    ),
    (
        "references/archive.md",
        include_str!("../assets/skill/references/archive.md"),
    ),
    (
        "templates/proposal.md",
        include_str!("../assets/skill/templates/proposal.md"),
    ),
    (
        "templates/implement.md",
        include_str!("../assets/skill/templates/implement.md"),
    ),
    (
        "templates/tasks.md",
        include_str!("../assets/skill/templates/tasks.md"),
    ),
    (
        "templates/spec.md",
        include_str!("../assets/skill/templates/spec.md"),
    ),
];
pub static FILES: LazyLock<Vec<(&'static str, String)>> = LazyLock::new(|| {
    RAW_FILES
        .iter()
        .map(|(path, content)| (*path, crate::markdown::normalize(content)))
        .collect()
});
pub fn change_file(name: &str, id: &str) -> String {
    // `FILES` is already LF-normalized, so the exact-byte marker strip below
    // matches on every platform instead of depending on the build checkout.
    FILES
        .iter()
        .find(|(path, _)| *path == format!("templates/{name}"))
        .expect("built-in template exists")
        .1
        .replace(&format!("{MARKER}\n"), "")
        .replace("{{id}}", id)
}

pub fn proposal_only_file(id: &str) -> String {
    format!(
        "{}\n{}",
        crate::PROPOSAL_ONLY_MARKER,
        change_file("proposal.md", id)
    )
}
pub fn navigation(skill: &str) -> String {
    format!(
        "## Doco\n\nFor current project documentation and explicitly tracked changes, use the\n`doco` skill. Read `{skill}/SKILL.md` before creating, executing, completing,\nor archiving a change. Perform only the requested phase.\nA doco change package is optional for implementation and is not an\nimplementation-history mechanism: archiving retains only proposal.md. Routine\nbehavior fixes and implementation-detail changes may proceed without creating\na doco change unless the user explicitly requests tracking. Use Git, pull\nrequests, or release notes for implementation history. Regardless of tracking,\nupdate current architecture and specs when their documented facts or contracts\nchange.\nStart from `doco/architecture.md` and relevant current specs and decisions.\nWhen implementing a selected tracked change, use its proposal and any present\ndesign and task files. Treat completed changes, archived changes, and\n`doco/tmp/` as non-current material; consult them only when explicitly needed.\n"
    )
}
pub const ARCHITECTURE: &str = "# Current architecture\n\nNo architecture has been documented yet. Inspect the current source and record\nonly implemented module boundaries, key data flows and important constraints.\nDo not describe future change proposals as current facts.\n";
