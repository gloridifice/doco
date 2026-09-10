#![allow(dead_code)]
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use tempfile::TempDir;

pub struct Sandbox {
    pub dir: TempDir,
}
impl Sandbox {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }
    pub fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }
    pub fn write(&self, relative: &str, content: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    pub fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path(relative)).unwrap()
    }
    pub fn run(&self, args: &[&str]) -> (bool, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_doco"))
            .arg("--root")
            .arg(self.dir.path())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        (
            output.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        )
    }
    pub fn ok(&self, args: &[&str]) -> String {
        let (ok, output) = self.run(args);
        assert!(ok, "{args:?}\n{output}");
        output
    }
    pub fn err(&self, args: &[&str], expected: &str) -> String {
        let (ok, output) = self.run(args);
        assert!(!ok, "unexpected success: {args:?}\n{output}");
        assert!(
            output.contains(expected),
            "expected {expected:?}: {args:?}\n{output}"
        );
        output
    }
    pub fn init(&self) {
        self.ok(&["init", "--agent", "codex"]);
    }
    pub fn ready(&self, id: &str) {
        self.ok(&["new", id]);
        self.write(&format!("doco/changes/active/{id}/proposal.md"), "# Bounded queue\n\n## Purpose\nAvoid unbounded event memory growth.\n\n## Scope and acceptance\nBound storage at two events; preserve FIFO and return Full on overflow.\n\n## Result\nDelivered bounded FIFO; queue boundary tests passed. No contract or architecture changes outside this module.\n");
        self.write(&format!("doco/changes/active/{id}/work/implement.md"), "# Design\n\n## 1. Baseline and goals\nThe fixture represents an empty project; add a standalone queue.\n\n## 2. Overall approach\nThe caller owns a VecDeque and never blocks.\n\n## 3. APIs and data model\npush(Event) returns Result<(), Full>; pop() returns Option<Event>. State belongs to the caller.\n\n## 4. Algorithms and rules\nReject a push at length two; pop from the front. No threads or persistence.\n\n## 5. Fixed decisions and discretion\nCapacity, order and overflow are fixed. Local names are discretionary.\nBlocked: none\n\n## 6. Verification and documentation impact\nTest zero, one, two and three pushes, then FIFO pops. Current architecture impact: none in this fixture.\n");
        self.write(&format!("doco/changes/active/{id}/work/tasks.md"), "# Tasks\n\n- [x] T001 Implement bounded queue\n  - Acceptance: FIFO and capacity two are enforced.\n  - Verification: fixture queue boundary checks passed.\n\n- [x] T002 Run regression checks\n  - Dependencies: T001\n  - Acceptance: expected overflow and FIFO behavior verified.\n  - Verification: fixture regression passed.\n");
    }
    pub fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(at).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    out.insert(
                        path.strip_prefix(root).unwrap().to_path_buf(),
                        fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(self.dir.path(), self.dir.path(), &mut out);
        out
    }
}
