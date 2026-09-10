pub const MARKER: &str = "<!-- doco:managed template=v1 -->";
pub const FILES: &[(&str, &str)] = &[
    ("SKILL.md", include_str!("../assets/skill/SKILL.md")),
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
];
pub fn change_file(name: &str, id: &str) -> String {
    FILES
        .iter()
        .find(|(path, _)| *path == format!("templates/{name}"))
        .expect("built-in template exists")
        .1
        .replace(&format!("{MARKER}\n"), "")
        .replace("{{id}}", id)
}
pub fn navigation(skill: &str) -> String {
    format!(
        "## Doco\n\nFor project documentation and managed changes, use the `doco` skill.\nRead `{skill}/SKILL.md` before creating, executing, completing,\nor archiving a change. Perform only the requested phase.\nStart from `doco/architecture.md` and relevant current specs and decisions.\nFor implementation, use the selected active change's proposal, design,\nand tasks. Treat completed changes, archived changes, and `doco/tmp/`\nas non-current material; consult them only when explicitly needed.\n"
    )
}
pub const ARCHITECTURE: &str = "# Current architecture\n\nNo architecture has been documented yet. Inspect the current source and record\nonly implemented module boundaries, key data flows and important constraints.\nDo not describe future change proposals as current facts.\n";
