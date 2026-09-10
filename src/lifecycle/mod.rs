mod archive;
mod context;
mod list;
use crate::{
    Project, State, check as validation, safety, templates,
    ui::{self, Reporter},
    validate_id,
};
use anyhow::{Result, bail};
pub use archive::{archive, archive_with_ui};
pub use context::{context, context_with_ui};
pub use list::{list_changes, list_changes_filtered};
use std::fs;

pub fn new_change(project: &Project, id: &str) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    new_change_with_ui(project, id, &mut reporter)
}

pub fn new_change_with_ui(project: &Project, id: &str, reporter: &mut dyn Reporter) -> Result<()> {
    validate_id(id)?;
    project.initialized()?;
    let _lock = safety::Lock::acquire(project.root())?;
    if project.changes()?.iter().any(|c| c.id == id) {
        bail!("change ID {id} already exists; choose an explicit new ID");
    }
    let destination = project.path(format!("doco/changes/active/{id}"));
    safety::inspect(project.root(), &destination)?;
    let temporary = tempfile::Builder::new()
        .prefix(".new-")
        .tempdir_in(project.path("doco/tmp"))?;
    fs::create_dir(temporary.path().join("work"))?;
    for name in ["proposal.md", "implement.md", "tasks.md"] {
        let path = if name == "proposal.md" {
            temporary.path().join(name)
        } else {
            temporary.path().join("work").join(name)
        };
        safety::atomic_write(
            project.root(),
            &path,
            &None,
            templates::change_file(name, id).as_bytes(),
        )?;
    }
    safety::move_directory(project.root(), temporary.path(), &destination)?;
    ui::text(
        reporter,
        &format!(
            "Created active/{id}: proposal.md, work/implement.md, work/tasks.md.\nSkeleton only: fill the design and acceptance criteria before execution; check is expected to fail until placeholders are replaced.\n"
        ),
    )?;
    Ok(())
}

pub fn check(project: &Project, id: &str) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    check_with_ui(project, id, &mut reporter)
}

pub fn check_with_ui(project: &Project, id: &str, reporter: &mut dyn Reporter) -> Result<()> {
    let change = project.resolve(id)?;
    let report = validation::inspect(project, &change, false)?;
    report.show_with_ui(reporter)?;
    report.ensure()?;
    ui::text(
        reporter,
        &format!(
            "Mechanical checks passed for {id} ({}); design quality, tests and semantic acceptance are not certified.\n",
            change.state
        ),
    )?;
    Ok(())
}

pub fn complete(project: &Project, id: &str) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    complete_with_ui(project, id, &mut reporter)
}

pub fn complete_with_ui(project: &Project, id: &str, reporter: &mut dyn Reporter) -> Result<()> {
    project.initialized()?;
    validate_id(id)?;
    let _lock = safety::Lock::acquire(project.root())?;
    let change = project.resolve(id)?;
    if change.state != State::Active {
        bail!("complete requires active; {id} is {}", change.state);
    }
    let before = safety::tree(project.root(), &change.path)?;
    let report = validation::inspect(project, &change, true)?;
    report.show_with_ui(reporter)?;
    report.ensure()?;
    if safety::tree(project.root(), &change.path)? != before {
        bail!("work package changed during completion checks; retry");
    }
    safety::move_directory(
        project.root(),
        &change.path,
        &project.path(format!("doco/changes/completed/{id}")),
    )?;
    ui::text(
        reporter,
        &format!(
            "Moved {id} to completed with full work package. Mechanical checks only; semantic acceptance and current-document synchronization remain the reviewer's responsibility. No Git or release action taken.\n"
        ),
    )?;
    Ok(())
}

pub fn reopen(project: &Project, id: &str) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    reopen_with_ui(project, id, &mut reporter)
}

pub fn reopen_with_ui(project: &Project, id: &str, reporter: &mut dyn Reporter) -> Result<()> {
    project.initialized()?;
    validate_id(id)?;
    let _lock = safety::Lock::acquire(project.root())?;
    let change = project.resolve(id)?;
    if change.state != State::Completed {
        bail!(
            "reopen requires completed; archived changes need a new ID referencing the old proposal"
        );
    }
    for name in ["proposal.md", "work/implement.md", "work/tasks.md"] {
        safety::text(project.root(), &change.path.join(name))?;
    }
    safety::move_directory(
        project.root(),
        &change.path,
        &project.path(format!("doco/changes/active/{id}")),
    )?;
    ui::text(
        reporter,
        &format!(
            "Reopened {id}; add/reset affected tasks, update the result and reverify before completing again.\n"
        ),
    )?;
    Ok(())
}
