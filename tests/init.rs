mod common;
use common::Sandbox;
use doco::templates;
use std::fs;

#[test]
fn selects_only_requested_agents_and_deduplicates() {
    for (args, shared, claude) in [
        (vec!["init", "--agent", "codex"], true, false),
        (vec!["init", "--agent", "pi"], true, false),
        (vec!["init", "--agent", "claude"], false, true),
        (
            vec![
                "init", "--agent", "codex", "--agent", "pi", "--agent", "codex",
            ],
            true,
            false,
        ),
        (
            vec!["init", "--agent", "codex", "--agent", "claude"],
            true,
            true,
        ),
    ] {
        let s = Sandbox::new();
        s.ok(&args);
        assert_eq!(s.path("AGENTS.md").exists(), shared);
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
            (shared, ".agents/skills/doco"),
            (claude, ".claude/skills/doco"),
        ] {
            if enabled {
                assert!(
                    templates::FILES
                        .iter()
                        .any(|(file, _)| *file == "references/migrate.md")
                );
                for (file, content) in templates::FILES {
                    assert_eq!(s.read(&format!("{directory}/{file}")), *content);
                }
                assert!(
                    s.read(&format!("{directory}/references/migrate.md"))
                        .contains("# Migrate existing documentation")
                );
            }
        }
        let before = s.files();
        s.ok(&args);
        assert_eq!(s.files(), before);
    }
}
#[test]
fn noninteractive_and_unknown_agents_do_not_write() {
    let s = Sandbox::new();
    s.err(&["init"], "requires --agent");
    s.err(&["init", "--agent", "unknown"], "invalid value");
    assert!(s.files().is_empty());
    assert_eq!(fs::read_dir(s.dir.path()).unwrap().count(), 0);
}
#[test]
fn dry_run_has_no_writes() {
    let s = Sandbox::new();
    let out = s.ok(&["init", "--agent", "codex", "--dry-run"]);
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
    let before = s.files();
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
        &["init", "--agent", "codex", "--agent", "claude"],
        "--refresh",
    );
    assert_eq!(s.files(), before);
    assert!(!s.path(".claude").exists());
    s.ok(&["init", "--agent", "codex", "--refresh"]);
    let updated = s.read("AGENTS.md");
    assert!(updated.starts_with("# Before\n"));
    assert!(updated.ends_with("# After\n保留\n"));
    let file = ".agents/skills/doco/references/create.md";
    s.write(file, &format!("{}\nUser edits\n", templates::MARKER));
    s.write(".agents/skills/doco/custom.txt", "keep");
    s.err(&["init", "--agent", "codex"], "managed file differs");
    s.ok(&["init", "--agent", "codex", "--refresh"]);
    assert_eq!(s.read(".agents/skills/doco/custom.txt"), "keep");
    s.write(file, "Not managed\n");
    let before = s.files();
    s.err(&["init", "--agent", "codex", "--refresh"], "not recognized");
    assert_eq!(s.files(), before);
}
#[test]
fn rejects_malformed_markers_and_custom_workflows_even_on_refresh() {
    for text in [
        "<!-- DOCO:START -->\nOops\n",
        "<!-- DOCO:END -->\n",
        "<!-- DOCO:START -->\n<!-- DOCO:START -->\n<!-- DOCO:END -->\n",
        "## Doco custom workflow\nRun everything automatically.\n",
    ] {
        let s = Sandbox::new();
        s.write("AGENTS.md", text);
        let before = s.files();
        s.err(&["init", "--agent", "codex", "--refresh"], "CONFLICT");
        assert_eq!(s.files(), before);
        assert!(!s.path("doco").exists());
    }
}
#[test]
fn adopts_unmarked_templates_without_duplication() {
    let s = Sandbox::new();
    let plain = templates::navigation(".agents/skills/doco");
    s.write("AGENTS.md", &format!("# Before\n\n{plain}\n# After\n"));
    for (file, generated) in templates::FILES {
        s.write(
            &format!(".agents/skills/doco/{file}"),
            &generated.replace(&format!("{}\n", templates::MARKER), ""),
        );
    }
    s.init();
    let result = s.read("AGENTS.md");
    assert_eq!(result.matches("For project documentation").count(), 1);
    assert_eq!(result.matches("<!-- DOCO:START -->").count(), 1);
    let before = s.files();
    s.init();
    assert_eq!(s.files(), before);
    let s = Sandbox::new();
    s.write("AGENTS.md", &format!("{plain}\n{plain}"));
    s.err(&["init", "--agent", "codex"], "multiple unmarked");
}
#[test]
fn fenced_examples_are_not_real_markers_or_imports() {
    let s = Sandbox::new();
    s.write(
        "AGENTS.md",
        "# Examples\n\n```md\n<!-- DOCO:START -->\n@AGENTS.md\n<!-- DOCO:END -->\n```\n",
    );
    s.write("CLAUDE.md", "```md\n@AGENTS.md\n```\n");
    s.ok(&["init", "--agent", "codex", "--agent", "claude"]);
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
        s.err(&["init", "--agent", "codex"], "CONFLICT");
        assert_eq!(s.files(), before);
    }
    let s = Sandbox::new();
    fs::write(s.path("AGENTS.md"), [0xff, 0xfe, 0x61, 0]).unwrap();
    s.err(&["init", "--agent", "codex"], "CONFLICT");
    assert!(!s.path("doco").exists());
}
#[test]
fn reuses_valid_claude_import_without_widening_rules() {
    let s = Sandbox::new();
    s.init();
    s.write("CLAUDE.md", "# Claude\n\n@AGENTS.md\n");
    let old = s.read("CLAUDE.md");
    s.ok(&["init", "--agent", "claude"]);
    assert_eq!(s.read("CLAUDE.md"), old);
    assert!(s.path(".claude/skills/doco/SKILL.md").exists());
    s.ok(&["init", "--agent", "codex", "--agent", "claude"]);
    s.write(
        "CLAUDE.md",
        &format!(
            "{old}\n<!-- DOCO:START -->\n{}<!-- DOCO:END -->\n",
            templates::navigation(".claude/skills/doco")
        ),
    );
    let before = s.files();
    s.err(&["init", "--agent", "claude", "--refresh"], "both contain");
    assert_eq!(s.files(), before);
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
    let out = s.ok(&["init", "--agent", "pi"]);
    assert!(out.contains("may not take effect"));
    assert!(out.contains("NOT verified"));
    assert_eq!(s.read("AGENTS.override.md"), "Existing override");
}
#[test]
fn alternate_pi_skill_is_reused_only_when_compatible() {
    let s = Sandbox::new();
    for (file, content) in templates::FILES {
        s.write(&format!(".pi/skills/doco/{file}"), content);
    }
    s.ok(&["init", "--agent", "pi"]);
    assert!(!s.path(".agents").exists());
    assert!(s.read("AGENTS.md").contains(".pi/skills/doco/SKILL.md"));
    s.err(&["init", "--agent", "pi", "--agent", "codex"], "migrate");
    let s = Sandbox::new();
    s.write(".pi/skills/doco/SKILL.md", "unrelated");
    s.err(&["init", "--agent", "pi", "--refresh"], "migrate");
    assert!(!s.path("doco").exists());
}
