use crate::{
    Project,
    safety::{self, Snapshot},
    ui::{self, Event, Reporter, Tone},
};
use anyhow::{Context, Result, anyhow, bail};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub struct FileOp {
    pub path: PathBuf,
    pub before: Option<Snapshot>,
    pub after: Vec<u8>,
}

struct FileGroup {
    label: String,
    root: PathBuf,
    root_existed: bool,
    files: Vec<FileOp>,
    directories: Vec<(PathBuf, bool)>,
}

enum WriteOp {
    File(FileOp),
    Group(FileGroup),
}

impl WriteOp {
    fn files(&self) -> Box<dyn Iterator<Item = &FileOp> + '_> {
        match self {
            Self::File(file) => Box::new(std::iter::once(file)),
            Self::Group(group) => Box::new(group.files.iter()),
        }
    }
}

#[derive(Default)]
pub struct Plan {
    pub directories: Vec<(PathBuf, bool)>,
    writes: Vec<WriteOp>,
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

    pub fn prepare_file<F>(
        &mut self,
        project: &Project,
        relative: &str,
        generate: F,
    ) -> Option<FileOp>
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
            Ok(op) => Some(op),
            Err(e) => {
                self.conflicts.push(format!("{relative}: {e:#}"));
                None
            }
        }
    }

    pub fn file<F>(&mut self, project: &Project, relative: &str, generate: F)
    where
        F: FnOnce(Option<&str>) -> Result<String>,
    {
        if let Some(op) = self.prepare_file(project, relative, generate) {
            self.writes.push(WriteOp::File(op));
        }
    }

    pub fn file_group(
        &mut self,
        project: &Project,
        label: &str,
        root_relative: &str,
        files: Vec<FileOp>,
    ) {
        let root = project.path(root_relative);
        let result = collect_directories(project, &files).and_then(|directories| {
            let root_existed = directories
                .iter()
                .find_map(|(path, existed)| (path == &root).then_some(*existed))
                .with_context(|| format!("group root is not a file parent: {root_relative}"))?;
            Ok(FileGroup {
                label: label.to_string(),
                root,
                root_existed,
                files,
                directories,
            })
        });
        match result {
            Ok(group) => self.writes.push(WriteOp::Group(group)),
            Err(error) => self.conflicts.push(format!("{root_relative}: {error:#}")),
        }
    }

    pub fn effective_text(&mut self, project: &Project, relative: &str) -> Result<Option<String>> {
        let path = project.path(relative);
        if let Some(op) = self
            .writes
            .iter()
            .flat_map(WriteOp::files)
            .find(|op| op.path == path)
        {
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

    pub fn show(&self, project: &Project, reporter: &mut dyn Reporter) -> Result<()> {
        for (path, exists) in &self.directories {
            show_directory(project, reporter, path, *exists)?;
        }
        for write in &self.writes {
            match write {
                WriteOp::File(op) => show_file(project, reporter, op)?,
                WriteOp::Group(group) => {
                    show_directory(project, reporter, &group.root, group.root_existed)?;
                    for op in &group.files {
                        show_file(project, reporter, op)?;
                    }
                }
            }
        }
        for message in &self.conflicts {
            ui::line(reporter, Tone::Danger, "CONFLICT", message)?;
        }
        for message in &self.warnings {
            ui::line(reporter, Tone::Warning, "WARNING", message)?;
        }
        Ok(())
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

    pub fn apply(self, project: &Project, reporter: &mut dyn Reporter) -> Result<()> {
        self.apply_with_hook(project, reporter, &mut |_| Ok(()))
    }

    fn apply_with_hook(
        self,
        project: &Project,
        reporter: &mut dyn Reporter,
        before_group_write: &mut dyn FnMut(&Path) -> Result<()>,
    ) -> Result<()> {
        self.ensure_valid()?;
        let _lock = safety::Lock::acquire(project.root())?;
        // Verify every dependency before the first integration write, then again per replacement.
        for write in &self.writes {
            for op in write.files() {
                safety::verify(project.root(), &op.path, &op.before)?;
            }
            if let WriteOp::Group(group) = write {
                verify_directories(project, &group.directories)?;
            }
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
        for write in self.writes {
            match write {
                WriteOp::File(op) => apply_file(project, reporter, op)?,
                WriteOp::Group(group) => {
                    if let Err(error) = apply_group(project, reporter, &group, before_group_write) {
                        bail!(
                            "partial initialization: {:#}; earlier APPLIED items outside this skill bundle remain",
                            error
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

fn collect_directories(project: &Project, files: &[FileOp]) -> Result<Vec<(PathBuf, bool)>> {
    let mut directories = BTreeMap::new();
    for op in files {
        let mut current = op.path.parent().context("missing file parent")?;
        while current != project.root() {
            if !current.starts_with(project.root()) {
                bail!("path outside project: {}", current.display());
            }
            safety::inspect(project.root(), current)?;
            let exists = current.try_exists()?;
            if exists && !current.is_dir() {
                bail!("not a directory: {}", current.display());
            }
            directories.insert(current.to_path_buf(), exists);
            current = current.parent().context("missing project parent")?;
        }
    }
    Ok(directories.into_iter().collect())
}

fn verify_directories(project: &Project, directories: &[(PathBuf, bool)]) -> Result<()> {
    for (path, expected_exists) in directories {
        safety::inspect(project.root(), path)?;
        let exists = path.try_exists()?;
        if exists != *expected_exists {
            bail!(
                "concurrent modification: {}; refusing skill update",
                path.display()
            );
        }
        if exists && !path.is_dir() {
            bail!("not a directory: {}", path.display());
        }
    }
    Ok(())
}

fn show_directory(
    project: &Project,
    reporter: &mut dyn Reporter,
    path: &Path,
    exists: bool,
) -> Result<()> {
    let (label, tone) = if exists {
        ("SKIP", Tone::Muted)
    } else {
        ("CREATE", Tone::Success)
    };
    ui::line(reporter, tone, label, &format!("{}/", rel(project, path)))
}

fn show_file(project: &Project, reporter: &mut dyn Reporter, op: &FileOp) -> Result<()> {
    let status = match &op.before {
        None => "CREATE",
        Some(before) if before.bytes == op.after => "SKIP",
        Some(_) => "UPDATE",
    };
    let tone = match status {
        "CREATE" => Tone::Success,
        "UPDATE" => Tone::Warning,
        _ => Tone::Muted,
    };
    ui::line(reporter, tone, status, &rel(project, &op.path))?;
    if status == "UPDATE" {
        let old = String::from_utf8_lossy(&op.before.as_ref().unwrap().bytes);
        let new = String::from_utf8_lossy(&op.after);
        let diff = similar::TextDiff::from_lines(&old, &new)
            .unified_diff()
            .context_radius(2)
            .header("existing", "planned")
            .to_string();
        reporter.emit(Event::Diff { text: &diff })?;
    }
    Ok(())
}

fn apply_file(project: &Project, reporter: &mut dyn Reporter, op: FileOp) -> Result<()> {
    if op.before.as_ref().is_some_and(|s| s.bytes == op.after) {
        return Ok(());
    }
    match safety::atomic_write(project.root(), &op.path, &op.before, &op.after) {
        Ok(()) => ui::line(reporter, Tone::Success, "APPLIED", &rel(project, &op.path)),
        Err(e) => bail!(
            "partial initialization: {} failed: {e:#}; earlier APPLIED items remain; retry safely",
            rel(project, &op.path)
        ),
    }
}

fn apply_group(
    project: &Project,
    reporter: &mut dyn Reporter,
    group: &FileGroup,
    before_write: &mut dyn FnMut(&Path) -> Result<()>,
) -> Result<()> {
    let mut applied: Vec<(&FileOp, Snapshot)> = Vec::new();
    for (index, op) in group.files.iter().enumerate() {
        // The last entry is the root SKILL.md commit marker. Recheck everything
        // prepared before it so a skipped or already-written file cannot change
        // unnoticed while the group is in progress.
        if index + 1 == group.files.len() {
            let failures = changed_group_files(project, &group.files[..index], &applied);
            if !failures.is_empty() {
                return fail_group(
                    project,
                    reporter,
                    group,
                    &applied,
                    anyhow!("skill bundle changed concurrently before commit"),
                    failures,
                );
            }
        }
        if op.before.as_ref().is_some_and(|s| s.bytes == op.after) {
            continue;
        }
        let attempt = (|| -> Result<Snapshot> {
            before_write(&op.path)?;
            safety::atomic_write(project.root(), &op.path, &op.before, &op.after)?;
            let written = safety::snapshot(project.root(), &op.path)?
                .with_context(|| format!("written file disappeared: {}", op.path.display()))?;
            if written.bytes != op.after {
                bail!("written bytes changed unexpectedly: {}", op.path.display());
            }
            Ok(written)
        })();
        match attempt {
            Ok(written) => applied.push((op, written)),
            Err(error) => {
                let mut failures = Vec::new();
                match safety::snapshot(project.root(), &op.path) {
                    Ok(current) if current == op.before => {}
                    Ok(Some(current)) if current.bytes == op.after => applied.push((op, current)),
                    Ok(_) => failures.push(format!(
                        "{} changed concurrently or has an indeterminate state",
                        rel(project, &op.path)
                    )),
                    Err(snapshot_error) => failures.push(format!(
                        "{} could not be inspected after failure: {snapshot_error:#}",
                        rel(project, &op.path)
                    )),
                }
                return fail_group(project, reporter, group, &applied, error, failures);
            }
        }
    }
    let failures = changed_group_files(project, &group.files, &applied);
    if !failures.is_empty() {
        return fail_group(
            project,
            reporter,
            group,
            &applied,
            anyhow!("skill bundle changed concurrently after its writes"),
            failures,
        );
    }
    for (op, _) in applied {
        ui::line(reporter, Tone::Success, "APPLIED", &rel(project, &op.path))?;
    }
    Ok(())
}

fn changed_group_files(
    project: &Project,
    files: &[FileOp],
    applied: &[(&FileOp, Snapshot)],
) -> Vec<String> {
    let mut failures = Vec::new();
    for op in files {
        let expected = applied
            .iter()
            .find_map(|(written_op, snapshot)| {
                (written_op.path == op.path).then_some(Some(snapshot.clone()))
            })
            .unwrap_or_else(|| op.before.clone());
        if let Err(error) = safety::verify(project.root(), &op.path, &expected) {
            failures.push(format!(
                "{} changed before the bundle commit: {error:#}",
                rel(project, &op.path)
            ));
        }
    }
    failures
}

fn fail_group(
    project: &Project,
    reporter: &mut dyn Reporter,
    group: &FileGroup,
    applied: &[(&FileOp, Snapshot)],
    error: anyhow::Error,
    mut rollback_failures: Vec<String>,
) -> Result<()> {
    rollback_group(project, group, applied, &mut rollback_failures);
    if rollback_failures.is_empty() {
        ui::line(reporter, Tone::Warning, "ROLLED BACK", &group.label)?;
        return Err(anyhow!(
            "{} update failed and was rolled back: {error:#}",
            group.label
        ));
    }
    let details = rollback_failures.join("; ");
    ui::line(
        reporter,
        Tone::Danger,
        "ROLLBACK FAILED",
        &format!("{}: {details}", group.label),
    )?;
    Err(anyhow!(
        "{} update failed: {error:#}; rollback failed: {details}",
        group.label
    ))
}

fn rollback_group(
    project: &Project,
    group: &FileGroup,
    applied: &[(&FileOp, Snapshot)],
    failures: &mut Vec<String>,
) {
    for (op, written) in applied.iter().rev() {
        let result = match &op.before {
            Some(before) => safety::atomic_write(
                project.root(),
                &op.path,
                &Some(written.clone()),
                &before.bytes,
            ),
            None => safety::remove_file(project.root(), &op.path, written),
        };
        if let Err(error) = result {
            failures.push(format!(
                "{} was not restored: {error:#}",
                rel(project, &op.path)
            ));
        }
    }
    let mut created_directories: Vec<_> = group
        .directories
        .iter()
        .filter_map(|(path, existed)| (!existed).then_some(path))
        .collect();
    created_directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in created_directories {
        if let Err(error) = safety::remove_empty_directory(project.root(), path) {
            failures.push(format!("{} was not removed: {error:#}", rel(project, path)));
        }
    }
}

fn rel(project: &Project, path: &Path) -> String {
    path.strip_prefix(project.root())
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::PlainReporter;

    fn group_plan(project: &Project, existing: bool) -> Plan {
        if existing {
            std::fs::create_dir_all(project.path("skill/references")).unwrap();
            std::fs::write(project.path("skill/references/a.md"), "old reference").unwrap();
            std::fs::write(project.path("skill/SKILL.md"), "old skill").unwrap();
        }
        let mut plan = Plan::default();
        let reference = plan
            .prepare_file(project, "skill/references/a.md", |_| {
                Ok("new reference".to_string())
            })
            .unwrap();
        let skill = plan
            .prepare_file(project, "skill/SKILL.md", |_| Ok("new skill".to_string()))
            .unwrap();
        plan.file_group(project, "skill", "skill", vec![reference, skill]);
        plan
    }

    #[test]
    fn every_group_write_failure_restores_existing_files() {
        for fail_at in 1..=2 {
            let dir = tempfile::tempdir().unwrap();
            let project = Project::open(dir.path()).unwrap();
            let plan = group_plan(&project, true);
            let mut writes = 0;
            let error = plan
                .apply_with_hook(
                    &project,
                    &mut PlainReporter::new(Vec::new(), Vec::new()),
                    &mut |_| {
                        writes += 1;
                        if writes == fail_at {
                            bail!("injected failure");
                        }
                        Ok(())
                    },
                )
                .unwrap_err();
            assert!(error.to_string().contains("rolled back"));
            assert_eq!(
                std::fs::read_to_string(project.path("skill/references/a.md")).unwrap(),
                "old reference"
            );
            assert_eq!(
                std::fs::read_to_string(project.path("skill/SKILL.md")).unwrap(),
                "old skill"
            );
        }
    }

    #[test]
    fn failed_new_group_removes_files_and_directories_it_created() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::open(dir.path()).unwrap();
        let plan = group_plan(&project, false);
        let mut writes = 0;
        plan.apply_with_hook(
            &project,
            &mut PlainReporter::new(Vec::new(), Vec::new()),
            &mut |_| {
                writes += 1;
                if writes == 2 {
                    bail!("injected failure");
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(!project.path("skill").exists());
    }

    #[test]
    fn rollback_does_not_overwrite_a_concurrent_edit() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::open(dir.path()).unwrap();
        let plan = group_plan(&project, true);
        let reference = project.path("skill/references/a.md");
        let mut writes = 0;
        let error = plan
            .apply_with_hook(
                &project,
                &mut PlainReporter::new(Vec::new(), Vec::new()),
                &mut |_| {
                    writes += 1;
                    if writes == 2 {
                        std::fs::write(&reference, "external edit").unwrap();
                        bail!("injected failure");
                    }
                    Ok(())
                },
            )
            .unwrap_err();
        assert!(error.to_string().contains("rollback failed"));
        assert_eq!(std::fs::read_to_string(reference).unwrap(), "external edit");
        assert_eq!(
            std::fs::read_to_string(project.path("skill/SKILL.md")).unwrap(),
            "old skill"
        );
    }

    #[test]
    fn group_commits_skill_last() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::open(dir.path()).unwrap();
        let plan = group_plan(&project, false);
        let mut order = Vec::new();
        plan.apply_with_hook(
            &project,
            &mut PlainReporter::new(Vec::new(), Vec::new()),
            &mut |path| {
                order.push(path.strip_prefix(project.root()).unwrap().to_path_buf());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            order,
            [
                PathBuf::from("skill/references/a.md"),
                PathBuf::from("skill/SKILL.md")
            ]
        );
    }
}
