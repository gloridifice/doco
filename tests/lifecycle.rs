mod common;
use common::Sandbox;
use doco::{
    Project, lifecycle,
    ui::{Event, Reporter, Stream},
};
use std::{fs, io};

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
    s.ok(&["archive", "queue"]);
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
    assert!(s.ok(&["list", "--archived"]).contains("archived\tqueue"));
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
    let tasks = s.read("doco/changes/active/goal/work/tasks.md");
    assert!(tasks.contains("- [ ] 1.1"));
    assert!(tasks.contains("- [ ] 2.1"));
    s.err(&["check", "goal"], "template markers");
    s.err(&["complete", "goal"], "mechanical check");
    s.err(&["complete"], "required");
    assert!(s.path("doco/changes/active/goal").exists());
}
#[test]
fn incomplete_tasks_pending_results_blockers_and_missing_evidence_block_completion() {
    for (file, from, to, expected) in [
        ("work/tasks.md", "[x] 2.1", "[ ] 2.1", "unfinished tasks"),
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
fn checks_hierarchical_task_ids_dependencies_acceptance_and_checkbox_syntax() {
    for (from, to, expected) in [
        ("2.1 Run", "1.1 Run", "duplicate task ID"),
        (
            "Dependencies: 1.1",
            "Dependencies: 9.9",
            "unknown dependency",
        ),
        (
            "Dependencies: 1.1",
            concat!("Dependencies: T", "001"),
            "invalid dependency",
        ),
        ("Dependencies: 1.1", "Dependencies: 2.1", "dependency cycle"),
        ("[x] 1.1", "[ ] 1.1", "dependency 1.1 is unfinished"),
        ("[x] 1.1", "[~] 1.1", "invalid task"),
        (
            "1.1 Implement",
            concat!("T", "001 Implement"),
            "invalid task",
        ),
        ("1.1 Implement", "0.1 Implement", "invalid task"),
        ("1.1 Implement", "01.1 Implement", "invalid task"),
        ("1.1 Implement", "1.0 Implement", "invalid task"),
        ("1.1 Implement", "1.01 Implement", "invalid task"),
        ("1.1 Implement", "1 Implement", "invalid task"),
        ("1.1 Implement", "1.1.1 Implement", "invalid task"),
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
    s.write("doco/changes/active/goal/work/tasks.md", "# 任务\n\n```md\n- [x] 1.1 ignored duplicate\n```\n\n- [x] 1.1 实现队列\n  - 完成条件：队列容量和 FIFO 通过测试。\n  - 验证记录：运行队列边界验证，通过。\n\n- [x] 2.1 整体验证\n  - 依赖：1.1\n  - 完成条件：所有约定回归通过。\n  - 阻塞：无\n");
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

struct FailingReporter;

impl Reporter for FailingReporter {
    fn emit(&mut self, _event: Event<'_>) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed output"))
    }

    fn flush(&mut self, _stream: Stream) -> io::Result<()> {
        Ok(())
    }
}

struct MutatingReporter {
    path: std::path::PathBuf,
    mutated: bool,
}

impl Reporter for MutatingReporter {
    fn emit(&mut self, event: Event<'_>) -> io::Result<()> {
        if !self.mutated
            && matches!(event, Event::Text { text, .. } if text.starts_with("No code rollback"))
        {
            fs::write(&self.path, "concurrent change").unwrap();
            self.mutated = true;
        }
        Ok(())
    }

    fn flush(&mut self, _stream: Stream) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn archive_preview_failure_and_concurrent_change_prevent_deletion() {
    let s = Sandbox::new();
    s.init();
    s.ready("output-failure");
    s.ok(&["complete", "output-failure"]);
    let project = Project::open(s.dir.path()).unwrap();
    let error = lifecycle::archive_with_ui(
        &project,
        "output-failure",
        false,
        false,
        None,
        &mut FailingReporter,
    )
    .unwrap_err();
    assert!(error.to_string().contains("closed output"));
    assert!(
        s.path("doco/changes/completed/output-failure/work/tasks.md")
            .exists()
    );

    s.ready("concurrent");
    s.ok(&["complete", "concurrent"]);
    let tasks = s.path("doco/changes/completed/concurrent/work/tasks.md");
    let mut reporter = MutatingReporter {
        path: tasks.clone(),
        mutated: false,
    };
    let error =
        lifecycle::archive_with_ui(&project, "concurrent", false, false, None, &mut reporter)
            .unwrap_err();
    assert!(error.to_string().contains("modified since preview"));
    assert_eq!(fs::read_to_string(tasks).unwrap(), "concurrent change");
    assert!(s.path("doco/changes/completed/concurrent/work").exists());
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
