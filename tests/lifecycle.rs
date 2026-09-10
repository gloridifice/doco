mod common;
use common::Sandbox;
use std::fs;

#[test]
fn full_lifecycle_preserves_snapshots_then_only_proposal() {
    let s = Sandbox::new();
    s.init();
    s.ready("queue");
    s.ok(&["check", "queue"]);
    let proposal = s.read("doco/changes/active/queue/proposal.md");
    s.ok(&["complete", "queue"]);
    assert!(!s.path("doco/changes/active/queue").exists());
    assert!(
        s.path("doco/changes/completed/queue/work/tasks.md")
            .exists()
    );
    s.ok(&["check", "queue"]);
    s.err(&["context", "queue"], "--history");
    assert!(
        s.ok(&["context", "queue", "--history"])
            .contains("HISTORICAL SNAPSHOT")
    );
    let before = s.files();
    s.ok(&["archive", "queue", "--dry-run"]);
    assert_eq!(s.files(), before);
    s.err(&["archive", "queue"], "requires --yes");
    assert_eq!(s.files(), before);
    s.ok(&["archive", "queue", "--yes"]);
    assert_eq!(s.read("doco/changes/archived/queue/proposal.md"), proposal);
    assert_eq!(
        fs::read_dir(s.path("doco/changes/archived/queue"))
            .unwrap()
            .count(),
        1
    );
    s.ok(&["check", "queue"]);
    s.err(&["reopen", "queue"], "new ID");
    s.err(&["new", "queue"], "already exists");
    assert!(s.ok(&["list"]).contains("archived\tqueue"));
}
#[test]
fn skeleton_is_not_a_design_and_no_implicit_selection_exists() {
    let s = Sandbox::new();
    s.init();
    s.ok(&["new", "goal"]);
    assert!(
        s.path("doco/changes/active/goal/work/implement.md")
            .exists()
    );
    s.err(&["check", "goal"], "template markers");
    s.err(&["complete", "goal"], "mechanical check");
    s.err(&["complete"], "required");
    assert!(s.path("doco/changes/active/goal").exists());
}
#[test]
fn incomplete_tasks_pending_results_blockers_and_missing_evidence_block_completion() {
    for (file, from, to, expected) in [
        ("work/tasks.md", "[x] T002", "[ ] T002", "unfinished tasks"),
        (
            "proposal.md",
            "Delivered bounded FIFO; queue boundary tests passed. No contract or architecture changes outside this module.",
            "Pending — not completed.",
            "still pending",
        ),
        (
            "work/implement.md",
            "Blocked: none",
            "Blocked: API owner decision needed",
            "unresolved blocker",
        ),
        (
            "work/tasks.md",
            "Verification:",
            "Notes:",
            "missing Verification",
        ),
    ] {
        let s = Sandbox::new();
        s.init();
        s.ready("goal");
        let path = format!("doco/changes/active/goal/{file}");
        s.write(&path, &s.read(&path).replace(from, to));
        s.err(&["complete", "goal"], expected);
        assert!(!s.path("doco/changes/completed/goal").exists());
    }
}
#[test]
fn checks_tasks_ids_dependencies_acceptance_and_checkbox_syntax() {
    for (from, to, expected) in [
        ("T002 Run", "T001 Run", "duplicate task ID"),
        (
            "Dependencies: T001",
            "Dependencies: T999",
            "unknown dependency",
        ),
        (
            "Dependencies: T001",
            "Dependencies: T002",
            "dependency cycle",
        ),
        ("[x] T001", "[ ] T001", "dependency T001 is unfinished"),
        ("[x] T001", "[~] T001", "invalid task"),
        ("Acceptance:", "Comment:", "missing Acceptance"),
    ] {
        let s = Sandbox::new();
        s.init();
        s.ready("goal");
        let path = "doco/changes/active/goal/work/tasks.md";
        s.write(path, &s.read(path).replace(from, to));
        s.err(&["check", "goal"], expected);
    }
}
#[test]
fn fences_do_not_create_tasks_and_chinese_fields_work() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.write("doco/changes/active/goal/work/tasks.md", "# 任务\n\n```md\n- [x] T001 ignored duplicate\n```\n\n- [x] T001 实现队列\n  - 完成条件：队列容量和 FIFO 通过测试。\n  - 验证记录：运行队列边界验证，通过。\n\n- [x] T002 整体验证\n  - 依赖：T001\n  - 完成条件：所有约定回归通过。\n  - 阻塞：无\n");
    s.ok(&["check", "goal"]);
    s.ok(&["complete", "goal"]);
}
#[test]
fn reopens_work_package_without_resetting_user_tasks() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let tasks = s.read("doco/changes/active/goal/work/tasks.md");
    s.ok(&["complete", "goal"]);
    s.ok(&["reopen", "goal"]);
    assert_eq!(s.read("doco/changes/active/goal/work/tasks.md"), tasks);
    s.ok(&["complete", "goal"]);
}
#[test]
fn cancellation_is_explicit_and_does_not_touch_code() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.write("code.rs", "keep my code");
    let path = "doco/changes/active/goal/work/tasks.md";
    s.write(path, &s.read(path).replace("[x]", "[ ]"));
    s.err(&["archive", "goal", "--yes"], "requires completed");
    let args = [
        "cancel",
        "goal",
        "--reason",
        "Goal superseded",
        "--disposition",
        "Retain independent queue implementation",
        "--dry-run",
    ];
    let before = s.files();
    s.ok(&args);
    assert_eq!(s.files(), before);
    s.ok(&[
        "cancel",
        "goal",
        "--reason",
        "Goal superseded",
        "--disposition",
        "Retain independent queue implementation",
        "--yes",
    ]);
    let proposal = s.read("doco/changes/archived/goal/proposal.md");
    assert!(proposal.contains("Cancelled."));
    assert!(proposal.contains("Retain independent"));
    assert_eq!(s.read("code.rs"), "keep my code");
    assert!(!s.path("doco/changes/completed/goal").exists());
    assert!(!s.path("doco/changes/archived/goal/work").exists());
}
#[test]
fn archive_rejects_retained_dependencies_and_unknown_root_material() {
    for reference in [
        "[design](work/implement.md)",
        "[design](%77ork/implement.md)",
        "`work/implement.md`",
        "[design][impl]\n\n[impl]: work/implement.md",
    ] {
        let s = Sandbox::new();
        s.init();
        s.ready("goal");
        s.ok(&["complete", "goal"]);
        let file = "doco/changes/completed/goal/proposal.md";
        s.write(file, &format!("{}\n{reference}\n", s.read(file)));
        let before = s.files();
        s.err(&["archive", "goal", "--yes"], "references work");
        assert_eq!(s.files(), before);
    }
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&["complete", "goal"]);
    s.write(
        "doco/specs/api.md",
        "[old API](../changes/completed/goal/work/implement.md)\n",
    );
    s.err(&["archive", "goal", "--yes"], "references work");
    fs::remove_file(s.path("doco/specs/api.md")).unwrap();
    s.write("doco/changes/completed/goal/notes.md", "valuable notes");
    s.err(&["archive", "goal", "--yes"], "outside work/");
    assert!(s.path("doco/changes/completed/goal/work").exists());
}
#[test]
fn archive_can_retry_after_partial_cleanup_without_requiring_lost_work() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&["complete", "goal"]);
    fs::remove_file(s.path("doco/changes/completed/goal/work/tasks.md")).unwrap();
    s.ok(&["archive", "goal", "--yes"]);
    s.ready("other");
    s.ok(&["complete", "other"]);
    fs::remove_dir_all(s.path("doco/changes/completed/other/work")).unwrap();
    assert!(s.ok(&["archive", "other", "--yes"]).contains("RECOVERY"));
}
#[test]
fn ids_are_safe_unique_and_state_errors_do_not_move_anything() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    for id in [
        "../bad", "UPPER", "a/b", "a\\b", "con", "nul", "com1", "a--b", "bad.", "-bad",
    ] {
        s.err(&["new", "--", id], "error:");
    }
    s.err(&["reopen", "goal"], "requires completed");
    fs::create_dir(s.path("doco/changes/completed/goal")).unwrap();
    for args in [
        vec!["list"],
        vec!["check", "goal"],
        vec!["complete", "goal"],
        vec!["new", "other"],
    ] {
        s.err(&args, "duplicate change ID");
    }
}
#[test]
fn context_follows_explicit_current_links_not_history_or_superseded_adrs() {
    let s = Sandbox::new();
    s.init();
    s.ready("selected");
    s.ready("other");
    s.ok(&["complete", "other"]);
    s.write("doco/specs/api.md", "# API\nCurrent contract\n");
    s.write(
        "doco/decisions/0001-old.md",
        "# Old decision\n\nStatus: Superseded by 0002\n",
    );
    s.write(
        "doco/decisions/0002-current.md",
        "# Current decision\n\nStatus: Accepted\n",
    );
    s.write("src/queue.rs", "pub struct Queue;\n");
    s.write("doco/tmp/selected/explanation.html", "<p>temporary</p>");
    s.write("doco/architecture.md", "# Architecture\n[API](specs/api.md)\n[Old](decisions/0001-old.md)\n[Current](decisions/0002-current.md)\n[Queue](../src/queue.rs)\n[Snapshot](changes/completed/other/work/implement.md)\n[Tmp](tmp/selected/explanation.html)\n");
    let output = s.ok(&["context", "selected"]).replace('\\', "/");
    assert!(output.contains("READ doco/specs/api.md"));
    assert!(output.contains("READ src/queue.rs"));
    assert!(output.contains("READ doco/decisions/0002-current.md"));
    assert!(!output.contains("READ doco/decisions/0001-old.md"));
    assert!(!output.contains("READ doco/changes/completed"));
    assert!(!output.contains("READ doco/tmp"));
}
