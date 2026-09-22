mod common;
use common::Sandbox;
use doco::templates;
use std::fs;

#[test]
fn update_help_has_no_agent_selection() {
    let s = Sandbox::new();
    let help = s.ok(&["update", "--help"]);
    assert!(help.contains("--dry-run"));
    assert!(help.contains("--refresh"));
    assert!(!help.contains("--agent"));
}

#[test]
fn updates_all_installed_integrations_and_is_idempotent() {
    let s = Sandbox::new();
    s.ok(&["init", "--agent", "most", "--agent", "claude"]);
    for (directory, entry) in [
        (".agents/skills/doco", "AGENTS.md"),
        (".claude/skills/doco", "CLAUDE.md"),
    ] {
        let skill = format!("{directory}/SKILL.md");
        s.write(
            &skill,
            &s.read(&skill).replace("<!-- doco:skill version=v4 -->", ""),
        );
        let reference = format!("{directory}/references/create.md");
        s.write(
            &reference,
            &format!("{}\nOld managed edit\n", s.read(&reference)),
        );
        s.write(
            entry,
            &s.read(entry)
                .replace("<!-- doco:entry template=v1 -->\n", "")
                .replace("Perform only", "Perform exactly"),
        );
    }

    let output = s.ok(&["update"]);
    assert!(output.contains("APPLIED .agents"), "{output}");
    assert!(output.contains("APPLIED .claude"), "{output}");
    for (directory, entry) in [
        (".agents/skills/doco", "AGENTS.md"),
        (".claude/skills/doco", "CLAUDE.md"),
    ] {
        for (file, content) in templates::FILES.iter() {
            assert_eq!(s.read(&format!("{directory}/{file}")), *content);
        }
        let entry = s.read(entry);
        assert!(entry.contains("<!-- doco:entry template=v1 -->"));
        assert!(entry.contains("Perform only"));
        assert!(!entry.contains("Perform exactly"));
    }

    let before = s.files();
    let output = s.ok(&["update"]);
    assert!(!output.contains("APPLIED "), "{output}");
    assert_eq!(s.files(), before);
}

#[test]
fn v3_bundle_upgrade_installs_optional_spec_template_without_refresh() {
    for command in [
        vec!["init", "--agent", "most", "--agent", "claude"],
        vec!["update"],
    ] {
        let s = Sandbox::new();
        s.ok(&["init", "--agent", "most", "--agent", "claude"]);
        for directory in [".agents/skills/doco", ".claude/skills/doco"] {
            let skill = format!("{directory}/SKILL.md");
            s.write(&skill, &s.read(&skill).replace("version=v4", "version=v3"));
            fs::remove_file(s.path(&format!("{directory}/templates/spec.md"))).unwrap();
            s.write(
                &format!("{directory}/references/create.md"),
                &format!("{}\nOld workflow\n", templates::MARKER),
            );
        }
        let before = s.files();
        let mut preview = command.clone();
        preview.push("--dry-run");
        s.ok(&preview);
        assert_eq!(s.files(), before);
        s.ok(&command);
        for directory in [".agents/skills/doco", ".claude/skills/doco"] {
            for (file, content) in templates::FILES.iter() {
                assert_eq!(s.read(&format!("{directory}/{file}")), *content);
            }
            assert!(s.path(&format!("{directory}/templates/spec.md")).exists());
        }
        let before = s.files_without_index();
        s.ok(&command);
        assert_eq!(s.files_without_index(), before);
    }
}

#[test]
fn standalone_claude_update_does_not_create_most_agents_files() {
    let s = Sandbox::new();
    s.ok(&["init", "--agent", "claude"]);
    let before = s.files();
    let output = s.ok(&["update"]);
    assert!(!output.contains("APPLIED "), "{output}");
    assert_eq!(s.files(), before);
    assert!(!s.path(".agents").exists());
    assert!(!s.path("AGENTS.md").exists());
}

#[test]
fn same_version_edits_require_refresh_and_dry_run_never_writes() {
    let s = Sandbox::new();
    s.init();
    let reference = ".agents/skills/doco/references/create.md";
    s.write(reference, &format!("{}\nUser edit\n", s.read(reference)));
    let before = s.files();
    s.err(&["update"], "managed file differs");
    assert_eq!(s.files(), before);

    let output = s.ok(&["update", "--refresh", "--dry-run"]);
    assert!(output.contains("UPDATE"), "{output}");
    assert_eq!(s.files(), before);
    s.ok(&["update", "--refresh"]);
    assert_eq!(
        s.read(reference),
        templates::FILES
            .iter()
            .find(|(file, _)| *file == "references/create.md")
            .unwrap()
            .1
    );

    s.write(
        "AGENTS.md",
        &s.read("AGENTS.md")
            .replace("Perform only", "Perform exactly"),
    );
    let before = s.files();
    s.err(&["update"], "managed navigation differs");
    assert_eq!(s.files(), before);
    s.ok(&["update", "--refresh"]);
    assert!(s.read("AGENTS.md").contains("Perform only"));
}

#[test]
fn update_rejects_missing_incomplete_and_legacy_integrations() {
    let s = Sandbox::new();
    s.init();
    fs::remove_dir_all(s.path(".agents")).unwrap();
    fs::remove_file(s.path("AGENTS.md")).unwrap();
    s.err(&["update"], "no installed doco integration found");
    assert!(!s.path(".agents").exists());
    assert!(!s.path("AGENTS.md").exists());

    let s = Sandbox::new();
    s.init();
    fs::remove_file(s.path("AGENTS.md")).unwrap();
    let before = s.files();
    s.err(&["update"], "has no managed doco entry");
    assert_eq!(s.files(), before);

    let s = Sandbox::new();
    s.init();
    fs::remove_dir_all(s.path(".agents")).unwrap();
    let before = s.files();
    s.err(&["update"], "is missing");
    assert_eq!(s.files(), before);

    let s = Sandbox::new();
    s.init();
    s.write(
        "AGENTS.md",
        &s.read("AGENTS.md")
            .replace(".agents/skills/doco", ".claude/skills/doco"),
    );
    let before = s.files();
    s.err(&["update"], "expected .agents/skills/doco");
    assert_eq!(s.files(), before);

    let s = Sandbox::new();
    s.init();
    s.write(".pi/skills/doco/SKILL.md", "legacy");
    let before = s.files();
    s.err(&["update"], "migrate it to .agents/skills/doco");
    assert_eq!(s.files(), before);
}

#[test]
fn claude_import_reuses_most_and_is_not_a_dedicated_install() {
    let s = Sandbox::new();
    s.init();
    s.write("CLAUDE.md", "# Claude\n\n@AGENTS.md\n");
    let before = s.read("CLAUDE.md");
    let output = s.ok(&["update"]);
    assert!(output.contains("reuses the Most agents integration"));
    assert_eq!(s.read("CLAUDE.md"), before);
    assert!(!s.path(".claude/skills/doco").exists());

    for (file, content) in templates::FILES.iter() {
        s.write(&format!(".claude/skills/doco/{file}"), content);
    }
    let before = s.files();
    s.err(&["update"], "has no managed doco entry");
    assert_eq!(s.files(), before);
}

#[test]
fn newer_entry_templates_are_not_downgraded_without_refresh() {
    let s = Sandbox::new();
    s.init();
    s.write(
        "AGENTS.md",
        &s.read("AGENTS.md").replace("template=v1", "template=v2"),
    );
    let before = s.files();
    s.err(&["update"], "managed navigation differs");
    assert_eq!(s.files(), before);
    s.ok(&["update", "--refresh"]);
    assert!(s.read("AGENTS.md").contains("template=v1"));
}
