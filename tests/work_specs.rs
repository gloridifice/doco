mod common;
use common::Sandbox;
use doco::templates;
use std::fs;

const SPEC: &str = "# Queue contract\n\nReject overflow without changing FIFO order.\n\n## Acceptance\nAfter two pushes, a third push returns Full and both queued events remain.\n";

#[test]
fn specs_are_optional_and_reads_do_not_create_or_modify_them() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let directory = s.path("doco/changes/active/goal/work/specs");
    assert!(!directory.exists());
    let before = s.files();
    s.ok(&["check", "goal"]);
    assert!(
        !s.ok(&["context", "goal"])
            .contains("missing document entry")
    );
    assert!(!directory.exists());
    assert_eq!(s.files(), before);

    fs::create_dir(&directory).unwrap();
    s.ok(&["check", "goal"]);
    assert!(
        !s.ok(&["context", "goal"])
            .contains("missing document entry")
    );
    assert_eq!(s.files(), before);
    s.ok(&["complete", "goal"]);
    assert!(s.path("doco/changes/completed/goal/work/specs").is_dir());
}

#[test]
fn context_discovers_sorted_nested_specs_and_follows_links_within_existing_boundaries() {
    let s = Sandbox::new();
    s.init();
    s.ready("selected");
    s.ready("other");
    s.write("doco/specs/current.md", "# Current\nExisting contract.\n");
    s.write("src/queue.rs", "pub struct Queue;\n");
    s.write("doco/tmp/notes.md", "Temporary material\n");
    s.write("doco/changes/active/other/work/specs/other.md", SPEC);
    s.write(
        "doco/changes/active/selected/work/notes.md",
        "Unlinked notes\n",
    );
    s.write(
        "doco/changes/active/selected/work/specs/ignored.txt",
        "TODO\nBlocked: ignored\n",
    );
    s.write(
        "doco/changes/active/selected/work/specs/ignored.MD",
        "TODO\nBlocked: ignored\n",
    );
    s.write("doco/changes/active/selected/work/specs/a/api.md", &format!(
        "{SPEC}\n[Peer](../z.md)\n[Current](../../../../../../specs/current.md)\n[Source](../../../../../../../src/queue.rs)\n[Other](../../../../other/work/specs/other.md)\n[Tmp](../../../../../../tmp/notes.md)\n"
    ));
    // Use package-relative references to exercise discovery independently of architecture links.
    s.write("doco/changes/active/selected/work/specs/z.md", &format!(
        "{SPEC}\n[Peer](a/api.md)\n[Current](../../../../../specs/current.md)\n[Source](../../../../../../src/queue.rs)\n[Other](../../../other/work/specs/other.md)\n[Tmp](../../../../../tmp/notes.md)\n"
    ));
    let before = s.files();
    let output = s.ok(&["context", "selected"]).replace('\\', "/");
    let nested = "READ doco/changes/active/selected/work/specs/a/api.md";
    let flat = "READ doco/changes/active/selected/work/specs/z.md";
    assert_eq!(output.matches(nested).count(), 1, "{output}");
    assert_eq!(output.matches(flat).count(), 1, "{output}");
    assert!(output.find(nested).unwrap() < output.find(flat).unwrap());
    assert!(output.contains("READ doco/specs/current.md"), "{output}");
    assert!(output.contains("READ src/queue.rs"), "{output}");
    for excluded in [
        "other/work/specs",
        "work/notes.md",
        "ignored.txt",
        "ignored.MD",
        "READ doco/tmp",
    ] {
        assert!(!output.contains(excluded), "{output}");
    }
    assert!(output.contains("target contracts, not current facts"));
    assert_eq!(s.files(), before);
}

#[test]
fn check_rejects_empty_bodies_and_template_residue_without_moving_the_package() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let path = "doco/changes/active/goal/work/specs/api.md";
    for (text, expected) in [
        ("", "empty specification body"),
        (" \n\t", "empty specification body"),
        ("<!-- hidden text -->\n", "empty specification body"),
        ("# Heading\n\n## Requirements\n", "empty specification body"),
        ("Title\n=====\n", "empty specification body"),
        ("# Contract\n\nTODO: specify it.\n", "template markers"),
        ("# Contract\n\nTBD\n", "template markers"),
        ("# Contract\n\n{{requirement}}\n", "template markers"),
        ("# 规格\n\n待补充\n", "template markers"),
    ] {
        s.write(path, text);
        let before = s.files();
        let output = s.err(&["check", "goal"], expected).replace('\\', "/");
        assert!(output.contains("work/specs/api.md"), "{output}");
        s.err(&["complete", "goal"], expected);
        assert_eq!(s.files(), before);
        assert!(!s.path("doco/changes/completed/goal").exists());
    }
    s.write(path, &templates::change_file("spec.md", "goal"));
    s.err(&["check", "goal"], "template markers");
}

#[test]
fn specs_need_no_fixed_headings_task_syntax_or_separate_verification_field() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.write("doco/changes/active/goal/work/specs/api.md", "\u{feff}# 目标规格\r\n\r\n容量为二，溢出不得改变原队列。\r\n\r\n- [ ] 验收场景不是执行任务，无需任务编号。\r\n\r\n```text\r\nBlocked: example only\r\n```\r\n<!-- Blocked: hidden example -->\r\n");
    s.write(
        "doco/changes/active/goal/work/specs/notes.txt",
        "TODO\nBlocked: not a spec\n",
    );
    s.ok(&["check", "goal"]);
    s.ok(&["complete", "goal"]);
    s.ok(&["check", "goal"]);
}

#[test]
fn spec_blockers_warn_in_active_and_block_completion_and_completed_checks() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let path = "doco/changes/active/goal/work/specs/api.md";
    for blocker in [
        "Blocked: API decision",
        "Open questions: compatibility",
        "未决问题：错误语义",
    ] {
        s.write(path, &format!("{SPEC}\n{blocker}\n"));
        let output = s.ok(&["check", "goal"]);
        assert!(
            output.contains("WARNING unresolved blocker: work/specs/api.md"),
            "{output}"
        );
        let before = s.files();
        s.err(&["complete", "goal"], "unresolved blocker");
        assert_eq!(s.files(), before);
    }
    s.write(
        path,
        &format!("{SPEC}\nBlocked: none\nOpen questions: resolved\n阻塞：已解决\n"),
    );
    s.ok(&["complete", "goal"]);
    s.write(
        "doco/changes/completed/goal/work/specs/api.md",
        &format!("{SPEC}\nBlocked: discovered after delivery\n"),
    );
    s.err(
        &["check", "goal"],
        "ERROR unresolved blocker: work/specs/api.md",
    );
}

#[test]
fn spec_links_use_existing_reference_validation() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let path = "doco/changes/active/goal/work/specs/api.md";
    for (link, expected) in [
        ("[Related](doco:missing)", "invalid stable reference"),
        (
            "[Outside](../../../../../../../outside.md)",
            "link outside project",
        ),
    ] {
        s.write(path, &format!("{SPEC}\n{link}\n"));
        let output = s.err(&["check", "goal"], expected);
        assert!(output.contains("work/specs/api.md"));
        s.err(&["complete", "goal"], expected);
    }
    s.write(path, &format!("{SPEC}\n[Planned source](planned.rs)\n"));
    assert!(
        s.ok(&["check", "goal"])
            .contains("missing local reference planned.rs")
    );
    s.ok(&["complete", "goal"]);
}

#[test]
fn invalid_spec_directory_and_encoding_are_errors() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let directory = "doco/changes/active/goal/work/specs";
    s.write(directory, "not a directory");
    for command in ["context", "check", "complete"] {
        s.err(&[command, "goal"], "work/specs must be a directory");
    }
    fs::remove_file(s.path(directory)).unwrap();
    s.write(&format!("{directory}/api.md"), SPEC);
    fs::write(s.path(&format!("{directory}/api.md")), [0xff, 0xfe]).unwrap();
    for command in ["context", "check", "complete"] {
        s.err(&[command, "goal"], "only UTF-8");
    }
    assert!(!s.path("doco/changes/completed/goal").exists());
}

#[test]
fn lifecycle_retains_spec_bytes_until_archive_and_protects_retained_references() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.write("doco/changes/active/goal/work/specs/nested/api.md", SPEC);
    s.ok(&["complete", "goal"]);
    assert_eq!(
        s.read("doco/changes/completed/goal/work/specs/nested/api.md"),
        SPEC
    );
    s.err(&["context", "goal"], "--history");
    assert!(
        s.ok(&["context", "goal", "--history"])
            .replace('\\', "/")
            .contains("SNAPSHOT doco/changes/completed/goal/work/specs/nested/api.md")
    );
    s.ok(&["reopen", "goal"]);
    assert_eq!(
        s.read("doco/changes/active/goal/work/specs/nested/api.md"),
        SPEC
    );
    s.ok(&["complete", "goal"]);
    let proposal_path = "doco/changes/completed/goal/proposal.md";
    let proposal = s.read(proposal_path);
    s.write(
        proposal_path,
        &format!("{proposal}\n[Contract](work/specs/nested/api.md)\n"),
    );
    let before = s.files();
    s.err(&["archive", "goal", "--dry-run"], "references work");
    assert_eq!(s.files(), before);
    s.write(proposal_path, &proposal);
    s.write(
        "doco/specs/retained.md",
        "[Contract](../changes/completed/goal/work/specs/nested/api.md)\n",
    );
    s.err(&["archive", "goal"], "references work");
    fs::remove_file(s.path("doco/specs/retained.md")).unwrap();
    let before = s.files();
    let preview = s.ok(&["archive", "goal", "--dry-run"]).replace('\\', "/");
    assert!(preview.contains("work/specs/nested/api.md"));
    assert_eq!(s.files(), before);
    s.ok(&["archive", "goal"]);
    assert_eq!(
        fs::read_dir(s.path("doco/changes/archived/goal"))
            .unwrap()
            .count(),
        1
    );
    s.ok(&["check", "goal"]);
    assert!(
        !s.ok(&["context", "goal", "--history"])
            .contains("missing document entry")
    );
}

#[test]
fn cancellation_discards_unfinished_specs_but_proposal_only_still_forbids_work() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.write(
        "doco/changes/active/goal/work/specs/api.md",
        "TODO\nBlocked: abandoned decision\n",
    );
    let args = [
        "cancel",
        "goal",
        "--reason",
        "No longer needed",
        "--disposition",
        "No code implemented",
    ];
    let before = s.files();
    let mut preview = args.to_vec();
    preview.push("--dry-run");
    assert!(
        s.ok(&preview)
            .replace('\\', "/")
            .contains("work/specs/api.md")
    );
    assert_eq!(s.files(), before);
    s.ok(&args);
    assert!(!s.path("doco/changes/archived/goal/work").exists());

    s.proposal_only_ready("small");
    assert!(!s.path("doco/changes/active/small/work").exists());
    s.write("doco/changes/active/small/work/specs/api.md", SPEC);
    s.err(&["check", "small"], "must not contain work");
    s.err(&["complete", "small"], "must not contain work");
}
