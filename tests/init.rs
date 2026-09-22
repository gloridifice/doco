mod common;
use common::Sandbox;
use doco::templates;
use std::fs;

#[test]
fn selects_only_requested_integrations_and_deduplicates() {
    for (args, most, claude) in [
        (vec!["init", "--agent", "most"], true, false),
        (vec!["init", "--agent", "claude"], false, true),
        (
            vec![
                "init", "--agent", "most", "--agent", "most", "--agent", "most",
            ],
            true,
            false,
        ),
        (
            vec!["init", "--agent", "most", "--agent", "claude"],
            true,
            true,
        ),
    ] {
        let s = Sandbox::new();
        s.ok(&args);
        assert_eq!(s.path("AGENTS.md").exists(), most);
        assert_eq!(s.path("CLAUDE.md").exists(), claude);
        for directory in [
            "doco/specs",
            "doco/decisions",
            "doco/tmp",
            "doco/changes/active",
            "doco/changes/completed",
            "doco/changes/archived",
        ] {
            assert!(s.path(directory).is_dir());
        }
        assert_eq!(s.read("doco/.gitignore"), "/tmp/\n");
        for (enabled, directory) in [
            (most, ".agents/skills/doco"),
            (claude, ".claude/skills/doco"),
        ] {
            if enabled {
                assert!(
                    templates::FILES
                        .iter()
                        .any(|(file, _)| *file == "references/migrate.md")
                );
                for (file, content) in templates::FILES.iter() {
                    assert_eq!(s.read(&format!("{directory}/{file}")), *content);
                }
                assert!(
                    s.read(&format!("{directory}/references/migrate.md"))
                        .contains("# Migrate existing documentation")
                );
            }
        }
        let before = s.files_without_index();
        s.ok(&args);
        assert_eq!(s.files_without_index(), before);
        assert!(s.index_changes().is_empty());
    }
}
#[test]
fn skill_version_is_stored_only_in_the_root_skill() {
    let s = Sandbox::new();
    s.init();
    assert!(
        s.read(".agents/skills/doco/SKILL.md")
            .contains("<!-- doco:skill version=v4 -->")
    );
    for (file, _) in templates::FILES.iter() {
        if *file != "SKILL.md" {
            assert!(
                !s.read(&format!(".agents/skills/doco/{file}"))
                    .contains("doco:skill version="),
                "unexpected skill version in {file}"
            );
        }
    }
}

#[test]
fn old_managed_skill_is_upgraded_as_a_bundle_without_refresh() {
    let s = Sandbox::new();
    s.init();
    let skill = ".agents/skills/doco/SKILL.md";
    s.write(
        skill,
        &s.read(skill).replace("<!-- doco:skill version=v4 -->", ""),
    );
    let reference = ".agents/skills/doco/references/create.md";
    s.write(
        reference,
        &format!("{}\nOld managed edit\n", s.read(reference)),
    );
    s.write(".agents/skills/doco/custom.txt", "keep");

    let output = s.ok(&["init", "--agent", "most"]);
    let applied: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("APPLIED "))
        .collect();
    assert!(applied[0].ends_with("create.md"), "{output}");
    assert!(applied[1].ends_with("SKILL.md"), "{output}");
    for (file, content) in templates::FILES.iter() {
        assert_eq!(s.read(&format!(".agents/skills/doco/{file}")), *content);
    }
    assert_eq!(s.read(".agents/skills/doco/custom.txt"), "keep");
    let before = s.files_without_index();
    s.init();
    assert_eq!(s.files_without_index(), before);
}

#[test]
fn newer_installed_skill_is_not_automatically_downgraded() {
    let s = Sandbox::new();
    s.init();
    let skill = ".agents/skills/doco/SKILL.md";
    s.write(skill, &s.read(skill).replace("version=v4", "version=v5"));
    s.err(&["init", "--agent", "most"], "managed file differs");
    assert!(s.read(skill).contains("version=v5"));
    s.ok(&["init", "--agent", "most", "--refresh"]);
    assert!(s.read(skill).contains("version=v4"));
}

#[test]
fn noninteractive_and_unknown_integrations_do_not_write() {
    let s = Sandbox::new();
    let help = s.ok(&["init", "--help"]);
    assert!(help.contains("possible values: most, claude"), "{help}");
    assert!(!help.contains("possible values: codex"), "{help}");
    assert!(!help.contains("possible values: pi"), "{help}");
    s.err(&["init"], "requires --agent");
    for value in ["unknown", "codex", "pi"] {
        s.err(&["init", "--agent", value], "invalid value");
    }
    assert!(s.files().is_empty());
    assert_eq!(fs::read_dir(s.dir.path()).unwrap().count(), 0);
}
#[test]
fn dry_run_has_no_writes() {
    let s = Sandbox::new();
    let out = s.ok(&["init", "--agent", "most", "--dry-run"]);
    assert!(out.contains("CREATE"));
    assert_eq!(fs::read_dir(s.dir.path()).unwrap().count(), 0);
}
#[test]
fn preserves_existing_facts_crlf_bom_and_adds_agents() {
    let s = Sandbox::new();
    let original = "\u{feff}# 项目规则\r\n\r\n保留这段中文；doco 是一个工具。\r\n";
    s.write("AGENTS.md", original);
    s.init();
    assert!(s.read("AGENTS.md").starts_with(original));
    assert!(!s.read("AGENTS.md").replace("\r\n", "").contains('\n'));
    s.write(
        "doco/architecture.md",
        "# Actual architecture\nRetain me.\n",
    );
    s.write("doco/specs/contract.md", "Existing contract\n");
    s.ok(&["new", "existing-goal"]);
    let before = s.files_without_index();
    s.ok(&["init", "--agent", "claude", "--refresh"]);
    for (path, bytes) in before {
        assert_eq!(fs::read(s.dir.path().join(path)).unwrap(), bytes);
    }
    assert!(s.path("CLAUDE.md").exists());
}
#[test]
fn refresh_is_scoped_and_conflicts_preflight_all_files() {
    let s = Sandbox::new();
    s.init();
    let old = s
        .read("AGENTS.md")
        .replace("Perform only", "Perform exactly");
    s.write("AGENTS.md", &format!("# Before\n{old}\n# After\n保留\n"));
    let before = s.files();
    s.err(
        &["init", "--agent", "most", "--agent", "claude"],
        "--refresh",
    );
    assert_eq!(s.files(), before);
    assert!(!s.path(".claude").exists());
    s.ok(&["init", "--agent", "most", "--refresh"]);
    let updated = s.read("AGENTS.md");
    assert!(updated.starts_with("# Before\n"));
    assert!(updated.ends_with("# After\n保留\n"));
    let file = ".agents/skills/doco/references/create.md";
    s.write(file, &format!("{}\nUser edits\n", templates::MARKER));
    s.write(".agents/skills/doco/custom.txt", "keep");
    s.err(&["init", "--agent", "most"], "managed file differs");
    s.ok(&["init", "--agent", "most", "--refresh"]);
    assert_eq!(s.read(".agents/skills/doco/custom.txt"), "keep");
    s.write(file, "Not managed\n");
    let before = s.files();
    s.err(&["init", "--agent", "most", "--refresh"], "not recognized");
    assert_eq!(s.files(), before);
}
#[test]
fn rejects_malformed_markers_with_removal_guidance_even_on_refresh() {
    for text in [
        "<!-- DOCO:START -->\nOops\n",
        "<!-- DOCO:END -->\n",
        "<!-- DOCO:START -->\n<!-- DOCO:START -->\n<!-- DOCO:END -->\n",
    ] {
        let s = Sandbox::new();
        s.write("AGENTS.md", text);
        let before = s.files();
        s.err(
            &["init", "--agent", "most", "--refresh"],
            "delete the entire DOCO block and retry",
        );
        assert_eq!(s.files(), before);
        assert!(!s.path("doco").exists());
    }
}
#[test]
fn preserves_doco_related_text_outside_the_managed_block() {
    let s = Sandbox::new();
    let custom = "## Doco custom workflow\n- Run doco manually.\nRead .agents/skills/doco/SKILL.md when needed.\n";
    s.write("AGENTS.md", custom);
    s.init();
    assert!(s.read("AGENTS.md").starts_with(custom));

    s.write(
        "AGENTS.md",
        &s.read("AGENTS.md")
            .replace("Perform only", "Perform exactly"),
    );
    s.ok(&["init", "--agent", "most", "--refresh"]);
    let result = s.read("AGENTS.md");
    assert!(result.starts_with(custom));
    assert!(result.contains("Perform only"));
}
#[test]
fn unmarked_navigation_is_preserved_and_does_not_claim_managed_ownership() {
    let s = Sandbox::new();
    let plain = templates::navigation(".agents/skills/doco");
    s.write("AGENTS.md", &format!("# Before\n\n{plain}\n# After\n"));
    s.init();
    let result = s.read("AGENTS.md");
    assert_eq!(
        result.matches("For current project documentation").count(),
        2
    );
    assert_eq!(result.matches("<!-- DOCO:START -->").count(), 1);
    let before = s.files_without_index();
    s.init();
    assert_eq!(s.files_without_index(), before);
}
#[test]
fn fenced_examples_are_not_real_markers_or_imports() {
    let s = Sandbox::new();
    s.write(
        "AGENTS.md",
        "# Examples\n\n```md\n<!-- DOCO:START -->\n@AGENTS.md\n<!-- DOCO:END -->\n```\n",
    );
    s.write("CLAUDE.md", "```md\n@AGENTS.md\n```\n");
    s.ok(&["init", "--agent", "most", "--agent", "claude"]);
    assert_eq!(
        s.read("AGENTS.md").matches("<!-- DOCO:START -->").count(),
        2
    );
    assert!(s.read("CLAUDE.md").contains(".claude/skills/doco/SKILL.md"));
}
#[test]
fn rejects_unsafe_markdown_and_encoding() {
    for text in [
        "# Rules\n```md\nexample",
        "~~~\nexample",
        "<!-- unfinished",
        "<details>\nexample",
        "<script>stuff",
    ] {
        let s = Sandbox::new();
        s.write("AGENTS.md", text);
        let before = s.files();
        s.err(&["init", "--agent", "most"], "CONFLICT");
        assert_eq!(s.files(), before);
    }
    let s = Sandbox::new();
    fs::write(s.path("AGENTS.md"), [0xff, 0xfe, 0x61, 0]).unwrap();
    s.err(&["init", "--agent", "most"], "CONFLICT");
    assert!(!s.path("doco").exists());
}
#[test]
fn claude_import_reuses_most_but_conflicts_with_a_dedicated_claude_install() {
    let s = Sandbox::new();
    s.init();
    s.write("CLAUDE.md", "# Claude\n\n@AGENTS.md\n");
    let old = s.read("CLAUDE.md");
    let output = s.ok(&["init", "--agent", "most"]);
    assert!(output.contains("reuses the Most agents integration"));
    assert_eq!(s.read("CLAUDE.md"), old);
    let before = s.files();
    s.err(
        &["init", "--agent", "claude"],
        "remove the import before installing a dedicated Claude integration",
    );
    assert_eq!(s.files(), before);
    assert!(!s.path(".claude").exists());
}
#[test]
fn rejects_missing_complex_and_recursive_imports() {
    for text in [
        "@AGENTS.md\n",
        "Please also read @AGENTS.md\n",
        "@sub/instructions.md\n",
    ] {
        let s = Sandbox::new();
        s.write("CLAUDE.md", text);
        s.err(&["init", "--agent", "claude"], "CONFLICT");
        assert!(!s.path("doco").exists());
    }
    let s = Sandbox::new();
    s.init();
    s.write("CLAUDE.md", "@AGENTS.md\n");
    s.write(
        "AGENTS.md",
        &format!("{}\n@CLAUDE.md\n", s.read("AGENTS.md")),
    );
    s.err(&["init", "--agent", "claude"], "CONFLICT");
}
#[test]
fn override_warns_and_is_not_modified() {
    let s = Sandbox::new();
    s.write("AGENTS.override.md", "Existing override");
    let out = s.ok(&["init", "--agent", "most"]);
    assert!(out.contains("may not take effect"));
    assert!(out.contains("NOT verified"));
    assert_eq!(s.read("AGENTS.override.md"), "Existing override");
}
#[test]
fn legacy_agent_specific_skill_paths_require_manual_migration() {
    for path in [".pi/skills/doco/SKILL.md", ".codex/skills/doco/SKILL.md"] {
        for integration in ["most", "claude"] {
            let s = Sandbox::new();
            s.write(path, "legacy");
            let before = s.files();
            s.err(&["init", "--agent", integration, "--refresh"], "migrate");
            assert_eq!(s.files(), before);
            assert!(!s.path("doco").exists());
        }
    }
}
