mod archive;
mod context;
mod list;
use crate::{
    PackageMode, Project, State, check as validation, index, package_mode, safety, templates,
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
    new_change_in_mode_with_ui(project, id, PackageMode::Full, reporter)
}

pub fn new_change_in_mode_with_ui(
    project: &Project,
    id: &str,
    mode: PackageMode,
    reporter: &mut dyn Reporter,
) -> Result<()> {
    validate_id(id)?;
    project.initialized()?;
    let lock = safety::Lock::acquire(project.root())?;
    let mut entries = index::load(project)?;
    index::ensure_absent(project, id, &mut entries)?;
    let destination = project.path(format!("doco/changes/active/{id}"));
    safety::inspect(project.root(), &destination)?;
    index::invalidate(project, &lock)?;
    let temporary = tempfile::Builder::new()
        .prefix(".new-")
        .tempdir_in(project.path("doco/tmp"))?;
    let proposal = match mode {
        PackageMode::Full => templates::change_file("proposal.md", id),
        PackageMode::ProposalOnly => templates::proposal_only_file(id),
    };
    safety::atomic_write(
        project.root(),
        &temporary.path().join("proposal.md"),
        &None,
        proposal.as_bytes(),
    )?;
    if mode == PackageMode::Full {
        fs::create_dir(temporary.path().join("work"))?;
        for name in ["implement.md", "tasks.md"] {
            safety::atomic_write(
                project.root(),
                &temporary.path().join("work").join(name),
                &None,
                templates::change_file(name, id).as_bytes(),
            )?;
        }
    }
    safety::move_directory(project.root(), temporary.path(), &destination)?;
    entries.insert(id.to_string(), State::Active);
    index::publish_after_commit(project, &lock, &entries, reporter)?;
    let files = match mode {
        PackageMode::Full => "proposal.md, work/implement.md, work/tasks.md",
        PackageMode::ProposalOnly => "proposal.md (proposal-only)",
    };
    ui::text(
        reporter,
        &format!(
            "Created active/{id}: {files}.\nSkeleton only: fill the scope and acceptance criteria before execution; check is expected to fail until placeholders are replaced.\n"
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
    let lock = safety::Lock::acquire(project.root())?;
    let change = project.resolve(id)?;
    if change.state != State::Active {
        bail!("complete requires active; {id} is {}", change.state);
    }
    let before = safety::tree(project.root(), &change.path)?;
    let mode = package_mode(&safety::text(
        project.root(),
        &change.path.join("proposal.md"),
    )?)?;
    let report = validation::inspect(project, &change, true)?;
    report.show_with_ui(reporter)?;
    report.ensure()?;
    if safety::tree(project.root(), &change.path)? != before {
        bail!("work package changed during completion checks; retry");
    }
    let mut entries = index::load(project)?;
    index::invalidate(project, &lock)?;
    safety::move_directory(
        project.root(),
        &change.path,
        &project.path(format!("doco/changes/completed/{id}")),
    )?;
    entries.insert(id.to_string(), State::Completed);
    index::publish_after_commit(project, &lock, &entries, reporter)?;
    ui::text(
        reporter,
        &format!(
            "Moved {id} to completed with {}. Mechanical checks only; semantic acceptance and current-document synchronization remain the reviewer's responsibility. No Git or release action taken.\n",
            match mode {
                PackageMode::Full => "full work package",
                PackageMode::ProposalOnly => "proposal-only package",
            }
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
    let lock = safety::Lock::acquire(project.root())?;
    let change = project.resolve(id)?;
    if change.state != State::Completed {
        bail!(
            "reopen requires completed; archived changes need a new ID referencing the old proposal"
        );
    }
    let proposal = safety::text(project.root(), &change.path.join("proposal.md"))?;
    match package_mode(&proposal)? {
        PackageMode::Full => {
            for name in ["work/implement.md", "work/tasks.md"] {
                safety::text(project.root(), &change.path.join(name))?;
            }
        }
        PackageMode::ProposalOnly => {
            let work = change.path.join("work");
            safety::inspect(project.root(), &work)?;
            if work.try_exists()? {
                bail!("proposal-only change must not contain work/");
            }
        }
    }
    let mut entries = index::load(project)?;
    index::invalidate(project, &lock)?;
    safety::move_directory(
        project.root(),
        &change.path,
        &project.path(format!("doco/changes/active/{id}")),
    )?;
    entries.insert(id.to_string(), State::Active);
    index::publish_after_commit(project, &lock, &entries, reporter)?;
    ui::text(
        reporter,
        &format!(
            "Reopened {id}; add/reset affected tasks, update the result and reverify before completing again.\n"
        ),
    )?;
    Ok(())
}
