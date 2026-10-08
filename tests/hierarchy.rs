mod common;
use common::Sandbox;
use std::{
    fs,
    process::{Command, Output, Stdio},
};

fn output_at(s: &Sandbox, directory: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_doco"))
        .current_dir(s.path(directory))
        .args(["--no-interactive", "--color", "never"])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn at(s: &Sandbox, directory: &str, args: &[&str]) -> String {
    let output = output_at(s, directory, args);
    assert!(
        output.status.success(),
        "{directory} {args:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn child(s: &Sandbox, directory: &str) {
    fs::create_dir_all(s.path(directory)).unwrap();
    at(s, directory, &["init"]);
}

#[test]
fn child_init_only_creates_local_documents_and_preserves_all_integrations() {
    let s = Sandbox::new();
    s.init();
    fs::create_dir_all(s.path("api")).unwrap();
    let before = s.files();
    at(&s, "api", &["init", "--dry-run"]);
    assert_eq!(s.files(), before);
    assert!(!s.path("api/doco").exists());

    at(&s, "api", &["init"]);
    assert!(s.path("api/doco/architecture.md").is_file());
    for state in ["active", "completed", "archived"] {
        assert!(s.path(&format!("api/doco/changes/{state}")).is_dir());
    }
    for path in [
        "api/AGENTS.md",
        "api/CLAUDE.md",
        "api/.agents",
        "api/.claude",
    ] {
        assert!(!s.path(path).exists(), "unexpected integration: {path}");
    }
    for (path, bytes) in before {
        assert_eq!(fs::read(s.dir.path().join(path)).unwrap(), bytes);
    }
    s.write("api/AGENTS.md", "Custom child instructions\n");
    s.write("api/CLAUDE.md", "Custom Claude instructions\n");
    s.write("api/.agents/skills/doco/SKILL.md", "User-owned workflow\n");
    s.write("api/.pi/skills/doco/SKILL.md", "Legacy user workflow\n");
    s.write("api/doco/architecture.md", "# API\nImplemented facts.\n");
    let mut before = s.files();
    at(
        &s,
        "api",
        &["init", "--agent", "most", "--agent", "claude", "--refresh"],
    );
    let mut after = s.files();
    before.remove(std::path::Path::new("api/doco/tmp/changes-index.csv"));
    after.remove(std::path::Path::new("api/doco/tmp/changes-index.csv"));
    assert_eq!(after, before);

    let before = s.files();
    let output = at(&s, "api", &["update"]);
    assert!(output.contains("--root"), "{output}");
    assert_eq!(s.files(), before);

    fs::create_dir_all(s.path("doco/tmp/scratch")).unwrap();
    let before = s.files();
    let output = output_at(&s, "doco/tmp/scratch", &["init"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("inside doco storage"));
    assert_eq!(s.files(), before);
}

#[test]
fn list_prioritizes_current_library_then_parent_first_tree_with_local_id_order() {
    let s = Sandbox::new();
    s.init();
    for directory in ["web", "api/auth", "api"] {
        child(&s, directory);
    }
    for directory in ["", "api", "api/auth", "web"] {
        at(&s, directory, &["new", "shared", "--proposal-only"]);
    }
    s.ok(&["new", "aaa", "--proposal-only"]);
    // State is directory-backed, independent of the current library and cache.
    fs::rename(
        s.path("web/doco/changes/active/shared"),
        s.path("web/doco/changes/completed/shared"),
    )
    .unwrap();
    at(&s, "api", &["new", "old", "--proposal-only"]);
    fs::rename(
        s.path("api/doco/changes/active/old"),
        s.path("api/doco/changes/archived/old"),
    )
    .unwrap();
    fs::create_dir_all(s.path("api/auth/src")).unwrap();
    fs::remove_dir_all(s.path("api/auth/doco/tmp")).unwrap();
    let before = s.files();

    for (directory, expected) in [
        (
            "",
            vec![
                ".:aaa",
                ".:shared",
                "api:old",
                "api:shared",
                "api/auth:shared",
                "web:shared",
            ],
        ),
        (
            "api",
            vec![
                "api:old",
                "api:shared",
                ".:aaa",
                ".:shared",
                "api/auth:shared",
                "web:shared",
            ],
        ),
        (
            "api/auth/src",
            vec![
                "api/auth:shared",
                ".:aaa",
                ".:shared",
                "api:old",
                "api:shared",
                "web:shared",
            ],
        ),
        (
            "web",
            vec![
                "web:shared",
                ".:aaa",
                ".:shared",
                "api:old",
                "api:shared",
                "api/auth:shared",
            ],
        ),
    ] {
        for flag in [None, Some("-c"), Some("-a")] {
            let mut args = vec!["list"];
            args.extend(flag);
            let output = at(&s, directory, &args);
            let rows: Vec<_> = output
                .lines()
                .map(|line| {
                    let fields: Vec<_> = line.split('\t').collect();
                    assert_eq!(fields.len(), 5, "{output}");
                    format!("{}:{}", fields[0], fields[2])
                })
                .collect();
            let expected: Vec<_> = expected
                .iter()
                .filter(|row| {
                    (flag.is_some() || **row != "web:shared")
                        && (flag == Some("-a") || **row != "api:old")
                })
                .copied()
                .collect();
            assert_eq!(rows, expected, "{directory} {args:?}");
        }
    }
    assert_eq!(s.files(), before);
    assert!(!s.path("api/auth/doco/tmp").exists());
}

#[test]
fn nearest_library_owns_writes_and_explicit_root_never_falls_back() {
    let s = Sandbox::new();
    s.init();
    s.proposal_only_ready("shared");
    child(&s, "api");
    fs::create_dir_all(s.path("api/src")).unwrap();
    let parent_proposal = s.read("doco/changes/active/shared/proposal.md");
    let parent_cache = s.index_text();
    at(&s, "api/src", &["new", "shared", "--proposal-only"]);
    assert!(
        s.path("api/doco/changes/active/shared/proposal.md")
            .exists()
    );
    assert!(!s.path("api/src/doco").exists());
    s.write(
        "api/doco/changes/active/shared/proposal.md",
        &parent_proposal,
    );
    at(
        &s,
        "api/src",
        &[
            "cancel",
            "shared",
            "--reason",
            "not needed",
            "--disposition",
            "none",
        ],
    );
    assert!(
        s.path("api/doco/changes/archived/shared/proposal.md")
            .exists()
    );
    assert_eq!(
        s.read("doco/changes/active/shared/proposal.md"),
        parent_proposal
    );
    assert_eq!(s.index_text(), parent_cache);

    let root = s.dir.path().to_str().unwrap();
    at(
        &s,
        "api/src",
        &["--root", root, "new", "parent-only", "--proposal-only"],
    );
    assert!(
        s.path("doco/changes/active/parent-only/proposal.md")
            .exists()
    );
    assert!(!s.path("api/doco/changes/active/parent-only").exists());
    let output = output_at(&s, "api/src", &["--root", ".", "list"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("run doco init"));
}

#[test]
fn discovery_stops_at_git_roots_and_skips_storage_dependencies_and_builds() {
    let s = Sandbox::new();
    s.write(".git/HEAD", "ref: refs/heads/main\n");
    s.init();
    s.ok(&["new", "parent", "--proposal-only"]);
    child(&s, "api");
    at(&s, "api", &["new", "child", "--proposal-only"]);
    for path in [
        "doco/tmp/scratch",
        "node_modules/pkg",
        "target/debug",
        ".agents/sandbox",
    ] {
        // If visited this incomplete library would fail the whole query.
        s.write(
            &format!("{path}/doco/architecture.md"),
            "# Not a project library\n",
        );
    }
    for (directory, marker) in [("independent", ".git/HEAD"), ("worktree", ".git")] {
        s.write(&format!("{directory}/{marker}"), "git boundary\n");
        let output = output_at(&s, directory, &["list"]);
        assert!(!output.status.success(), "ancestor escaped Git boundary");
        let output = output_at(&s, directory, &["init"]);
        assert!(
            !output.status.success(),
            "independent root requires an agent"
        );
        at(&s, directory, &["init", "--agent", "most"]);
        at(&s, directory, &["new", "independent", "--proposal-only"]);
        assert!(s.path(&format!("{directory}/AGENTS.md")).exists());
        assert_eq!(at(&s, directory, &["list"]), "active\tindependent\t-\t0m\n");
    }
    assert_eq!(
        at(&s, "", &["list"]),
        ".\tactive\tparent\t-\t0m\napi\tactive\tchild\t-\t0m\n"
    );
}

#[test]
fn malformed_discovered_library_is_reported_without_partial_results() {
    let s = Sandbox::new();
    s.init();
    s.ok(&["new", "visible", "--proposal-only"]);
    s.write("broken/doco/architecture.md", "# Incomplete library\n");
    let before = s.files();
    let output = output_at(&s, "", &["list"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("broken"));
    assert_eq!(s.files(), before);
}
