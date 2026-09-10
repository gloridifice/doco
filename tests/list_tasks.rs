mod common;
use common::Sandbox;
use doco::{
    Project, lifecycle,
    ui::{Event, PlainReporter, Reporter},
};
use std::fs;

#[test]
fn list_counts_checkboxes_not_examples_and_tracks_lifecycle() {
    let s = Sandbox::new();
    s.init();
    s.ready("mixed");
    s.write("doco/changes/active/mixed/work/tasks.md", "\u{feff}# Tasks\r\n\r\n- [x] 1.1 完成\r\n- [ ] 1.2 未完成\r\n* [x] 2.1 Done\r\n```md\r\n- [x] 2.2 example\r\n```\r\n<!--\r\n- [x] 2.3 comment\r\n-->\r\n- [ ] not-a-task\r\n");
    s.ready("completed");
    s.ok(&["complete", "completed"]);
    s.ready("archived");
    s.ok(&["complete", "archived"]);
    s.ok(&["archive", "archived"]);
    // Archived counts must not attempt to read even an unexpected work entry.
    s.write(
        "doco/changes/archived/archived/work/tasks.md",
        "- [x] 9.9 Ignored\n",
    );
    s.ready("empty");
    s.write("doco/changes/active/empty/work/tasks.md", "# Empty\n");
    s.ready("missing");
    fs::remove_file(s.path("doco/changes/active/missing/work/tasks.md")).unwrap();
    let before = s.files();
    let active = "active\tempty\t0/0\t0m\nactive\tmissing\t?\t0m\nactive\tmixed\t2/3\t0m\n";
    let completed = "completed\tcompleted\t2/2\t0m\nactive\tempty\t0/0\t0m\nactive\tmissing\t?\t0m\nactive\tmixed\t2/3\t0m\n";
    let archived = "archived\tarchived\t-\t0m\nactive\tempty\t0/0\t0m\nactive\tmissing\t?\t0m\nactive\tmixed\t2/3\t0m\n";
    let all = "archived\tarchived\t-\t0m\ncompleted\tcompleted\t2/2\t0m\nactive\tempty\t0/0\t0m\nactive\tmissing\t?\t0m\nactive\tmixed\t2/3\t0m\n";
    assert_eq!(s.ok(&["list"]), active);
    assert_eq!(s.ok(&["list", "--completed"]), completed);
    assert_eq!(s.ok(&["list", "--archived"]), archived);
    assert_eq!(s.ok(&["list", "--completed", "--archived"]), all);
    assert_eq!(s.files(), before);
    let project = Project::open(s.dir.path()).unwrap();
    let rows = lifecycle::list_changes(&project).unwrap();
    let mut stdout = Vec::new();
    PlainReporter::new(&mut stdout, Vec::new())
        .emit(Event::Changes { changes: &rows })
        .unwrap();
    assert_eq!(String::from_utf8(stdout).unwrap(), all);
}

#[test]
fn hidden_states_are_not_read_until_their_flag_is_enabled() {
    let s = Sandbox::new();
    s.init();
    s.ready("hidden");
    s.ok(&["complete", "hidden"]);
    let tasks = s.path("doco/changes/completed/hidden/work/tasks.md");
    fs::hard_link(&tasks, s.path("linked.md")).unwrap();

    assert_eq!(s.ok(&["list"]), "");
    s.err(&["list", "--completed"], "hard");
}

#[test]
fn list_rejects_unsafe_or_unreadable_tasks_instead_of_reporting_zero() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let tasks = s.path("doco/changes/active/goal/work/tasks.md");
    fs::hard_link(&tasks, s.path("linked.md")).unwrap();
    s.err(&["list"], "hard");
    fs::remove_file(s.path("linked.md")).unwrap();
    fs::write(&tasks, [0xff]).unwrap();
    s.err(&["list"], "invalid UTF-8");
    fs::remove_file(&tasks).unwrap();
    fs::create_dir(&tasks).unwrap();
    s.err(&["list"], "expected regular file");
}
