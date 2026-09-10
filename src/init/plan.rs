use crate::{
    Project,
    safety::{self, Snapshot},
};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub struct FileOp {
    pub path: PathBuf,
    pub before: Option<Snapshot>,
    pub after: Vec<u8>,
}
#[derive(Default)]
pub struct Plan {
    pub directories: Vec<(PathBuf, bool)>,
    pub files: Vec<FileOp>,
    reads: Vec<(PathBuf, Option<Snapshot>)>,
    pub conflicts: Vec<String>,
    pub warnings: Vec<String>,
}
impl Plan {
    pub fn directory(&mut self, project: &Project, relative: &str) {
        let path = project.path(relative);
        let result = safety::inspect(project.root(), &path).and_then(|()| {
            if path.exists() && !path.is_dir() {
                bail!("not a directory: {}", path.display());
            }
            Ok(path.exists())
        });
        match result {
            Ok(exists) => self.directories.push((path, exists)),
            Err(e) => self.conflicts.push(format!("{relative}: {e:#}")),
        }
    }
    pub fn file<F>(&mut self, project: &Project, relative: &str, generate: F)
    where
        F: FnOnce(Option<&str>) -> Result<String>,
    {
        let path = project.path(relative);
        let result: Result<FileOp> = (|| {
            let before = safety::snapshot(project.root(), &path)?;
            let text = before
                .as_ref()
                .map(|s| safety::decode(&s.bytes))
                .transpose()?;
            let after = generate(text.as_deref())?.into_bytes();
            Ok(FileOp {
                path,
                before,
                after,
            })
        })();
        match result {
            Ok(op) => self.files.push(op),
            Err(e) => self.conflicts.push(format!("{relative}: {e:#}")),
        }
    }
    pub fn effective_text(&mut self, project: &Project, relative: &str) -> Result<Option<String>> {
        let path = project.path(relative);
        if let Some(op) = self.files.iter().find(|op| op.path == path) {
            return Ok(Some(safety::decode(&op.after)?));
        }
        let snapshot = safety::snapshot(project.root(), &path)?;
        let text = snapshot
            .as_ref()
            .map(|s| safety::decode(&s.bytes))
            .transpose()?;
        self.reads.push((path, snapshot));
        Ok(text)
    }
    pub fn show(&self, project: &Project) {
        let rel = |p: &Path| {
            p.strip_prefix(project.root())
                .unwrap_or(p)
                .display()
                .to_string()
        };
        for (path, exists) in &self.directories {
            println!("{} {}/", if *exists { "SKIP" } else { "CREATE" }, rel(path));
        }
        for op in &self.files {
            let status = match &op.before {
                None => "CREATE",
                Some(before) if before.bytes == op.after => "SKIP",
                Some(_) => "UPDATE",
            };
            println!("{status} {}", rel(&op.path));
            if status == "UPDATE" {
                let old = String::from_utf8_lossy(&op.before.as_ref().unwrap().bytes);
                let new = String::from_utf8_lossy(&op.after);
                println!(
                    "{}",
                    similar::TextDiff::from_lines(&old, &new)
                        .unified_diff()
                        .context_radius(2)
                        .header("existing", "planned")
                );
            }
        }
        for message in &self.conflicts {
            println!("CONFLICT {message}");
        }
        for message in &self.warnings {
            println!("WARNING {message}");
        }
    }
    pub fn ensure_valid(&self) -> Result<()> {
        if !self.conflicts.is_empty() {
            bail!(
                "initialization preflight found {} conflict(s); no installation writes made",
                self.conflicts.len()
            );
        }
        Ok(())
    }
    pub fn apply(self, project: &Project) -> Result<()> {
        self.ensure_valid()?;
        let _lock = safety::Lock::acquire(project.root())?;
        // Verify every file before the first integration write, then again per replacement.
        for op in &self.files {
            safety::verify(project.root(), &op.path, &op.before)?;
        }
        for (path, snapshot) in &self.reads {
            safety::verify(project.root(), path, snapshot)?;
        }
        for (path, _) in &self.directories {
            safety::inspect(project.root(), path)?;
        }
        for (path, _) in &self.directories {
            if let Err(e) = safety::mkdir(project.root(), path) {
                bail!(
                    "partial initialization: {} failed: {e:#}; retain successful items and retry",
                    path.display()
                );
            }
        }
        for op in self.files {
            if op.before.as_ref().is_some_and(|s| s.bytes == op.after) {
                continue;
            }
            match safety::atomic_write(project.root(), &op.path, &op.before, &op.after) {
                Ok(()) => println!("APPLIED {}", rel(project, &op.path)),
                Err(e) => bail!(
                    "partial initialization: {} failed: {e:#}; earlier APPLIED items remain; retry safely",
                    rel(project, &op.path)
                ),
            }
        }
        Ok(())
    }
}
fn rel(project: &Project, path: &Path) -> String {
    path.strip_prefix(project.root())
        .unwrap_or(path)
        .display()
        .to_string()
}
