//! Acceptance tests for the disposable local change index cache.
mod common;
use common::{INDEX_CACHE, Sandbox};
use doco::{
    Project, fix,
    ui::{Event, Reporter, Stream},
};
use std::{fs, io};

/// Timestamps are written as `YYYY-MM-DDTHH:MM:SSZ` in UTC.
fn assert_timestamp(value: &str) {
    let bytes = value.as_bytes();
    assert_eq!(bytes.len(), 20, "unexpected timestamp {value:?}");
    for (index, byte) in bytes.iter().enumerate() {
        let expected = match index {
            4 | 7 => b'-',
            10 => b'T',
            13 | 16 => b':',
            19 => b'Z',
            _ => continue,
        };
        assert_eq!(*byte, expected, "unexpected timestamp {value:?}");
    }
    assert!(
        bytes.iter().enumerate().all(
            |(index, byte)| matches!(index, 4 | 7 | 10 | 13 | 16 | 19) || byte.is_ascii_digit()
        ),
        "unexpected timestamp {value:?}"
    );
}

/// Rewrite the cache file without touching the state directories, so it stays
/// "fresh" for the modification-time check while its records are wrong.
fn forge_cache(s: &Sandbox, transform: impl FnOnce(String) -> String) {
    let text = s.index_text().expect("cache exists");
    s.write(INDEX_CACHE, &transform(text));
}

#[test]
fn init_announces_and_builds_a_documented_cache() {
    let s = Sandbox::new();
    let out = s.ok(&["init", "--agent", "most"]);
    let building = out
        .find("INDEX Building change index cache...")
        .unwrap_or_else(|| panic!("missing announce: {out}"));
    let built = out
        .find("INDEX Cache built: 0 active, 0 completed, 0 archived.")
        .unwrap_or_else(|| panic!("missing result: {out}"));
    assert!(building < built, "announce must precede the result: {out}");

    let text = s.index_text().expect("init must build the cache");
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines[0], "type,key,value");
    assert_eq!(lines[1], "meta,version,1");
    assert_timestamp(lines[2].strip_prefix("meta,updated_at,").unwrap());
    for (line, state) in lines[3..6].iter().zip(["active", "completed", "archived"]) {
        let value = line
            .strip_prefix(&format!("directory,{state},"))
            .unwrap_or_else(|| panic!("unexpected directory row {line:?}"));
        assert!(value.parse::<i128>().is_ok(), "unexpected stamp {value:?}");
    }
    assert_eq!(lines.len(), 6, "no change records expected: {text}");
    assert!(text.ends_with('\n'));
}

#[test]
fn repeated_init_rebuilds_the_cache_with_current_records() {
    let s = Sandbox::new();
    s.init();
    s.ready("full");
    s.proposal_only_ready("small");
    s.ok(&["complete", "full"]);
    let out = s.ok(&["init", "--agent", "most"]);
    assert!(
        out.contains("INDEX Cache built: 1 active, 1 completed, 0 archived."),
        "{out}"
    );
    assert_eq!(s.index_changes(), ["full=completed", "small=active"]);
}

#[test]
fn fix_rebuilds_a_missing_or_corrupt_cache_and_dry_run_never_writes() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    fs::remove_file(s.path(INDEX_CACHE)).unwrap();
    let without_cache = s.files();

    let out = s.ok(&["fix", "--dry-run"]);
    assert!(out.contains("INDEX Would rebuild cache: 1 active, 0 completed, 0 archived."));
    assert!(out.contains("Dry run: no files written."));
    assert_eq!(s.files(), without_cache, "dry run must not write anything");

    let out = s.ok(&["fix"]);
    assert!(out.contains("INDEX Building change index cache..."));
    assert!(out.contains("INDEX Cache built: 1 active, 0 completed, 0 archived."));
    assert_eq!(s.index_changes(), ["goal=active"]);

    s.write(INDEX_CACHE, "type,key,value\nmeta,version,7\n");
    let corrupted = s.files();
    assert!(
        s.ok(&["list"]).contains("active\tgoal"),
        "a corrupt cache must fall back to the directories"
    );
    assert_eq!(s.files(), corrupted, "a read must not repair the cache");
    s.ok(&["fix"]);
    assert_eq!(s.index_changes(), ["goal=active"]);
}

#[test]
fn a_project_without_a_cache_still_works_and_builds_it_on_the_next_write() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    fs::remove_file(s.path(INDEX_CACHE)).unwrap();

    assert_eq!(s.ok(&["list"]), "active\tgoal\t2/2\t0m\n");
    assert!(
        s.ok(&["check", "goal"])
            .contains("Mechanical checks passed")
    );
    assert!(s.index_text().is_none(), "reads must not persist the cache");

    s.ok(&["new", "second"]);
    assert_eq!(s.index_changes(), ["goal=active", "second=active"]);
}

#[test]
fn read_only_commands_and_dry_runs_never_create_the_cache() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ready("done");
    s.ok(&["complete", "done"]);
    s.ok(&["new", "skeleton"]);
    fs::remove_file(s.path(INDEX_CACHE)).unwrap();
    let before = s.files();
    for args in [
        vec!["list"],
        vec!["list", "--completed", "--archived"],
        vec!["check", "goal"],
        vec!["check", "done"],
        vec!["context", "goal"],
        vec!["archive", "done", "--dry-run"],
    ] {
        s.ok(&args);
    }
    s.err(&["complete", "skeleton"], "template markers");
    s.err(&["new", "goal"], "already exists");
    assert_eq!(s.files(), before);

    let fresh = Sandbox::new();
    fresh.ok(&["init", "--agent", "most", "--dry-run"]);
    assert_eq!(fresh.files(), Default::default());
}

#[test]
fn lifecycle_writes_track_every_state_transition() {
    let s = Sandbox::new();
    s.init();
    assert!(s.index_changes().is_empty());

    s.ready("gate");
    assert_eq!(s.index_changes(), ["gate=active"]);
    s.ok(&["complete", "gate"]);
    assert_eq!(s.index_changes(), ["gate=completed"]);
    s.ok(&["reopen", "gate"]);
    assert_eq!(s.index_changes(), ["gate=active"]);
    s.ok(&["complete", "gate"]);
    s.ok(&["archive", "gate"]);
    assert_eq!(s.index_changes(), ["gate=archived"]);

    s.proposal_only_ready("drop");
    assert_eq!(s.index_changes(), ["drop=active", "gate=archived"]);
    s.ok(&[
        "cancel",
        "drop",
        "--reason",
        "No longer needed",
        "--disposition",
        "Nothing was implemented",
    ]);
    assert_eq!(s.index_changes(), ["drop=archived", "gate=archived"]);

    // A rejected operation stops before the cache is dropped, so it stays usable.
    s.ok(&["new", "blocked"]);
    let before = s.index_text().unwrap();
    s.err(&["complete", "blocked"], "template markers");
    assert_eq!(s.index_text().unwrap(), before);
}

#[test]
fn a_stale_cache_never_authorizes_duplicates_or_wrong_states() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");

    // The cache omits an existing change while the directory stamps still match.
    forge_cache(&s, |text| {
        text.lines()
            .filter(|line| !line.starts_with("change,"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    });
    s.err(&["new", "goal"], "already exists");
    assert!(
        s.path("doco/changes/active/goal").exists(),
        "no duplicate may be created"
    );
    s.ok(&["complete", "goal"]);
    assert_eq!(s.index_changes(), ["goal=completed"]);

    // The cache claims an archived state while the change is active on disk.
    s.ok(&["reopen", "goal"]);
    forge_cache(&s, |text| {
        text.replace("change,goal,active", "change,goal,archived")
    });
    s.ok(&["complete", "goal"]);
    assert_eq!(s.index_changes(), ["goal=completed"]);

    // A wiped record set must not hide a completed change from archive.
    forge_cache(&s, |text| {
        text.lines()
            .filter(|line| !line.starts_with("change,"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    });
    s.ok(&["archive", "goal"]);
    assert_eq!(s.index_changes(), ["goal=archived"]);
}

#[test]
fn invalidate_failure_stops_before_any_business_write() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");

    // An unexpected directory on the cache path is unsafe and must not be replaced.
    fs::remove_file(s.path(INDEX_CACHE)).unwrap();
    fs::create_dir(s.path(INDEX_CACHE)).unwrap();
    s.err(&["new", "second"], "expected regular file");
    assert!(!s.path("doco/changes/active/second").exists());
    s.err(&["fix"], "expected regular file");
}

#[test]
fn hard_linked_cache_files_are_rejected() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let outside = Sandbox::new();
    fs::hard_link(s.path(INDEX_CACHE), outside.path("linked.csv")).unwrap();
    s.err(&["list"], "hard-linked");
    s.err(&["fix"], "hard-linked");
    assert!(!s.path("doco/changes/active/other").exists());
}

#[test]
fn fix_requires_an_initialized_project_without_creating_one() {
    let s = Sandbox::new();
    s.err(&["fix"], "run doco init");
    s.err(&["fix", "--dry-run"], "run doco init");
    assert!(s.files().is_empty());
}

#[test]
fn a_failing_cache_rebuild_keeps_a_successful_init() {
    let s = Sandbox::new();
    // An invalid change directory makes the authoritative scan fail.
    fs::create_dir_all(s.path("doco/changes/active/UPPER")).unwrap();
    let out = s.ok(&["init", "--agent", "most"]);
    assert!(out.contains("Disk installation complete"), "{out}");
    assert!(out.contains("change index cache was not rebuilt"), "{out}");
    assert!(out.contains("run doco fix"), "{out}");
    assert!(s.index_text().is_none());
    assert!(s.path(".agents/skills/doco/SKILL.md").exists());
    s.err(&["fix"], "invalid change ID");
}

#[test]
fn an_uninitialized_project_is_never_touched_by_fix() {
    let s = Sandbox::new();
    s.write("README.md", "no doco here\n");
    let before = s.files();
    s.err(&["fix"], "run doco init");
    assert_eq!(s.files(), before);
}

struct RecordingReporter {
    events: Vec<String>,
    flushed_after: Vec<usize>,
}

impl Reporter for RecordingReporter {
    fn emit(&mut self, event: Event<'_>) -> io::Result<()> {
        self.events.push(
            match event {
                Event::Line { label, .. } => label,
                Event::Text { .. } => "TEXT",
                Event::Diff { .. } => "DIFF",
                Event::Changes { .. } => "CHANGES",
            }
            .to_string(),
        );
        Ok(())
    }

    fn flush(&mut self, _stream: Stream) -> io::Result<()> {
        self.flushed_after.push(self.events.len());
        Ok(())
    }
}

#[test]
fn rebuild_announces_and_flushes_before_it_scans() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    let project = Project::open(s.dir.path()).unwrap();
    let mut reporter = RecordingReporter {
        events: Vec::new(),
        flushed_after: Vec::new(),
    };
    fix::run_with_ui(&project, false, &mut reporter).unwrap();
    assert_eq!(reporter.events, ["INDEX", "INDEX"]);
    assert_eq!(
        reporter.flushed_after,
        [1],
        "the announcement must be flushed before the scan starts"
    );
    assert_eq!(s.index_changes(), ["goal=active"]);
}

#[cfg(windows)]
#[test]
fn a_committed_operation_survives_a_failed_archive_cleanup() {
    let s = Sandbox::new();
    s.init();
    s.ready("goal");
    s.ok(&["complete", "goal"]);
    s.write("doco/changes/completed/goal/work/extra.log", "remove later");
    use std::os::windows::fs::OpenOptionsExt;
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(3)
        .open(s.path("doco/changes/completed/goal/work/tasks.md"))
        .unwrap();
    s.err(&["archive", "goal", "--yes"], "partial cleanup");
    assert!(
        s.index_text().is_none(),
        "a partially applied archive must not leave a trusted cache"
    );
    drop(handle);
    s.ok(&["archive", "goal", "--yes"]);
    assert_eq!(s.index_changes(), ["goal=archived"]);
}
