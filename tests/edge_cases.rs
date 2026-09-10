mod common;
use common::Sandbox;
use doco::templates;

#[test]
fn skill_bom_is_preserved_during_idempotent_install_and_refresh() {
    let s = Sandbox::new();
    for (file, content) in templates::FILES {
        s.write(
            &format!(".agents/skills/doco/{file}"),
            &format!("\u{feff}{}", content.replace('\n', "\r\n")),
        );
    }
    s.init();
    let before = s.files();
    s.init();
    assert_eq!(s.files(), before);
    let file = ".agents/skills/doco/references/create.md";
    s.write(file, &format!("{}\r\nUser customization\r\n", s.read(file)));
    s.ok(&["init", "--agent", "codex", "--refresh"]);
    assert!(s.read(file).starts_with('\u{feff}'));
    assert!(!s.read(file).replace("\r\n", "").contains('\n'));
}

#[test]
fn cancellation_can_explain_why_implementation_was_not_completed() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&[
        "cancel",
        "goal",
        "--reason",
        "Not completed because the API was retired",
        "--disposition",
        "未完成的实现已移交新变更，不回滚代码",
        "--yes",
    ]);
    s.ok(&["check", "goal"]);
}

#[test]
fn bom_at_managed_block_and_unmarked_body_is_preserved() {
    for marked in [false, true] {
        let s = Sandbox::new();
        let body = templates::navigation(".agents/skills/doco");
        let text = if marked {
            format!("\u{feff}<!-- DOCO:START -->\n{body}<!-- DOCO:END -->\n")
        } else {
            format!("\u{feff}{body}")
        };
        s.write("AGENTS.md", &text.replace('\n', "\r\n"));
        s.init();
        assert!(s.read("AGENTS.md").starts_with('\u{feff}'));
        let before = s.files();
        s.init();
        assert_eq!(s.files(), before);
        let edited = s
            .read("AGENTS.md")
            .replace("Perform only", "Perform exactly");
        s.write("AGENTS.md", &edited);
        s.ok(&["init", "--agent", "codex", "--refresh"]);
        assert!(s.read("AGENTS.md").starts_with('\u{feff}'));
    }
}
#[test]
fn unclosed_frontmatter_is_a_no_write_conflict() {
    let s = Sandbox::new();
    s.write("AGENTS.md", "---\ntitle: project\n");
    s.err(&["init", "--agent", "codex"], "unclosed front matter");
    assert!(!s.path("doco").exists());
}
#[test]
fn commented_tasks_and_sections_are_not_completion_evidence() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.write("doco/changes/active/goal/work/tasks.md", "# Tasks\n\n<!--\n- [x] T001 Pretend delivered\n  - Acceptance: all good\n  - Verification: all passed\n-->\n");
    s.err(&["complete", "goal"], "no executable tasks");
    s.write("doco/changes/active/goal/proposal.md", "<!--\n## Purpose\nFake purpose\n## Scope and acceptance\nFake scope\n## Result\nFake result\n-->\n");
    s.err(&["check", "goal"], "expected one ## Purpose");
}
#[test]
fn html_work_links_block_archive() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&["complete", "goal"]);
    let path = "doco/changes/completed/goal/proposal.md";
    s.write(
        path,
        &format!(
            "{}\n<a href='work/implement.md'>Required detail</a>\n",
            s.read(path)
        ),
    );
    s.err(&["archive", "goal", "--yes"], "references work");
}
#[test]
fn adding_codex_does_not_silently_duplicate_a_prior_pi_installation() {
    let s = Sandbox::new();
    for (file, content) in templates::FILES {
        s.write(&format!(".pi/skills/doco/{file}"), content);
    }
    s.ok(&["init", "--agent", "pi"]);
    let before = s.files();
    s.err(&["init", "--agent", "codex"], "migrate");
    assert_eq!(s.files(), before);
}
#[cfg(windows)]
#[test]
fn case_variants_cannot_bypass_archive_and_context_boundaries() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&["complete", "goal"]);
    let path = "doco/changes/completed/goal/proposal.md";
    s.write(
        path,
        &format!("{}\n[Required detail](WORK/implement.md)\n", s.read(path)),
    );
    s.err(&["archive", "goal", "--yes"], "references work");
    s.ready("selected");
    s.write(
        "doco/architecture.md",
        "# Architecture\n[Old](CHANGES/COMPLETED/goal/work/implement.md)\n",
    );
    let out = s.ok(&["context", "selected"]);
    assert!(!out.contains("READ doco\\CHANGES"));
    assert!(!out.contains("READ doco/CHANGES"));
}
#[cfg(windows)]
#[test]
fn init_reports_partial_success_without_damaging_existing_entry_and_can_retry() {
    use std::{fs, os::windows::fs::OpenOptionsExt};
    let s = Sandbox::new();
    s.write(
        "AGENTS.md",
        "# Existing instructions\nPreserve these bytes.\n",
    );
    let old = s.read("AGENTS.md");
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(s.path("AGENTS.md"))
        .unwrap();
    let out = s.err(&["init", "--agent", "codex"], "partial initialization");
    assert!(out.contains("APPLIED"));
    assert!(!out.contains("Disk installation complete"));
    assert_eq!(s.read("AGENTS.md"), old);
    assert!(s.path(".agents/skills/doco/SKILL.md").exists());
    drop(handle);
    s.init();
    assert!(s.read("AGENTS.md").starts_with(&old));
    let before = s.files();
    s.init();
    assert_eq!(s.files(), before);
}
#[cfg(windows)]
#[test]
fn completed_proposal_survives_move_failure_after_work_cleanup() {
    use std::{fs, os::windows::fs::OpenOptionsExt};
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&["complete", "goal"]);
    let path = s.path("doco/changes/completed/goal/proposal.md");
    let old = fs::read(&path).unwrap();
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    s.err(&["archive", "goal", "--yes"], "archive move failed");
    assert_eq!(fs::read(&path).unwrap(), old);
    assert!(!s.path("doco/changes/completed/goal/work").exists());
    drop(handle);
    s.ok(&["archive", "goal", "--yes"]);
    assert_eq!(
        fs::read(s.path("doco/changes/archived/goal/proposal.md")).unwrap(),
        old
    );
}
