mod common;
use common::Sandbox;
use doco::{Project, safety};
use std::fs;

#[test]
fn hardlinked_entries_and_skill_files_are_not_overwritten() {
    for path in ["AGENTS.md", ".agents/skills/doco/SKILL.md"] {
        let s = Sandbox::new();
        let external = Sandbox::new();
        external.write("original", "original bytes");
        let destination = s.path(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::hard_link(external.path("original"), &destination).unwrap();
        s.err(&["init", "--agent", "most", "--refresh"], "hard-linked");
        assert_eq!(external.read("original"), "original bytes");
        assert!(!s.path("doco").exists());
    }
}
#[test]
fn rejects_concurrent_writers_and_recovers_after_lock_release() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let project = Project::open(s.dir.path()).unwrap();
    let lock = safety::Lock::acquire(project.root()).unwrap();
    s.err(&["complete", "goal"], "another doco writer");
    assert!(s.path("doco/changes/active/goal").exists());
    drop(lock);
    s.ok(&["complete", "goal"]);
}
#[test]
fn atomic_write_rejects_concurrent_edits_and_preserves_user_bytes() {
    let s = Sandbox::new();
    s.write("file.md", "before");
    let project = Project::open(s.dir.path()).unwrap();
    let path = project.path("file.md");
    let snapshot = safety::snapshot(project.root(), &path).unwrap();
    s.write("file.md", "concurrent edit");
    let error = safety::atomic_write(project.root(), &path, &snapshot, b"replacement").unwrap_err();
    assert!(error.to_string().contains("concurrent modification"));
    assert_eq!(s.read("file.md"), "concurrent edit");
    let path = project.path("new.md");
    s.write("new.md", "concurrent create");
    assert!(safety::atomic_write(project.root(), &path, &None, b"replacement").is_err());
    assert_eq!(s.read("new.md"), "concurrent create");
}

fn link_dir(target: &std::path::Path, link: &std::path::Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        // Junctions exercise Windows parent reparse detection without Developer Mode.
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link.to_string_lossy().replace('/', "\\"))
            .arg(target.to_string_lossy().replace('/', "\\"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn refuses_linked_parent_directories_before_init_writes() {
    for path in [".agents", "doco"] {
        let s = Sandbox::new();
        let outside = Sandbox::new();
        outside.write("keep", "external");
        link_dir(outside.dir.path(), &s.path(path));
        s.err(&["init", "--agent", "most"], "CONFLICT");
        assert_eq!(outside.files().len(), 1);
        assert_eq!(outside.read("keep"), "external");
    }
}
#[test]
fn archive_never_traverses_external_work_link() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&["complete", "goal"]);
    let outside = Sandbox::new();
    outside.write("keep.md", "external evidence");
    link_dir(
        outside.dir.path(),
        &s.path("doco/changes/completed/goal/work/external"),
    );
    s.err(&["archive", "goal", "--yes"], "refusing");
    assert_eq!(outside.read("keep.md"), "external evidence");
    assert!(s.path("doco/changes/completed/goal/proposal.md").exists());
}
#[cfg(windows)]
#[test]
fn partial_delete_reports_source_and_retry_finishes() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&["complete", "goal"]);
    s.write(
        "doco/changes/completed/goal/work/z-deleted-first.log",
        "remove this first",
    );
    let path = s.path("doco/changes/completed/goal/work/tasks.md");
    use std::os::windows::fs::OpenOptionsExt;
    // Allow inspection and reads, but deny deletion until the handle is released.
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(&path)
        .unwrap();
    s.err(&["archive", "goal", "--yes"], "partial cleanup");
    assert!(s.path("doco/changes/completed/goal/proposal.md").exists());
    assert!(
        !s.path("doco/changes/completed/goal/work/z-deleted-first.log")
            .exists()
    );
    drop(handle);
    s.ok(&["archive", "goal", "--yes"]);
}
