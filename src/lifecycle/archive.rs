use crate::{
    Change, Project, State, check, markdown, safety,
    ui::{self, Reporter, Tone},
    validate_id,
};
use anyhow::{Context, Result, bail};
use std::{fs, path::Path};

type Cancellation<'a> = Option<(&'a str, &'a str)>;

fn cancellation_result(original: &str, reason: &str, disposition: &str) -> Result<String> {
    for (label, value) in [("reason", reason), ("disposition", disposition)] {
        if value.trim().is_empty() || check::placeholders(value) {
            bail!("cancellation {label} must be explicit, nonempty and not a placeholder");
        }
    }
    let section =
        markdown::result_section(original).context("proposal must have a Result/结果 section")?;
    let mut result = original.to_string();
    let body = format!(
        "\nCancelled.\n\nReason: {}\n\nImplemented code disposition: {}\n\nCancellation does not roll back code; work materials are discarded.\n\n",
        reason.trim(),
        disposition.trim()
    );
    result.replace_range(section.body, &markdown::styled(&body, original));
    Ok(result)
}

fn retained_links(project: &Project, change: &Change, proposal: &str) -> Result<()> {
    let work_paths: Vec<_> = State::ALL
        .iter()
        .map(|state| project.path(format!("doco/changes/{state}/{}/work", change.id)))
        .collect();
    fn inspect_links(
        project: &Project,
        file: &Path,
        text: &str,
        targets: &[std::path::PathBuf],
    ) -> Result<()> {
        for link in markdown::links(text) {
            let candidates = [
                markdown::local_link(file.parent().unwrap(), &link),
                markdown::local_link(project.root(), &link),
            ];
            if candidates
                .into_iter()
                .flatten()
                .any(|p| targets.iter().any(|work| markdown::within(&p, work)))
            {
                bail!(
                    "retained file {} references work being deleted: {link}; make it independently readable first",
                    file.display()
                );
            }
        }
        Ok(())
    }
    fn walk(
        project: &Project,
        change: &Change,
        path: &Path,
        proposal: &str,
        targets: &[std::path::PathBuf],
    ) -> Result<()> {
        if path == project.path("doco/tmp") || path == change.path.join("work") {
            return Ok(());
        }
        safety::inspect(project.root(), path)?;
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                walk(project, change, &entry?.path(), proposal, targets)?;
            }
        } else if path.extension().is_some_and(|e| e == "md") {
            let text = if path == change.path.join("proposal.md") {
                proposal.to_string()
            } else {
                safety::text(project.root(), path)?
            };
            inspect_links(project, path, &text, targets)?;
        }
        Ok(())
    }
    walk(
        project,
        change,
        &project.path("doco"),
        proposal,
        &work_paths,
    )
}

pub fn archive(
    project: &Project,
    id: &str,
    yes: bool,
    dry_run: bool,
    cancellation: Cancellation<'_>,
) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    archive_with_ui(project, id, yes, dry_run, cancellation, &mut reporter)
}

pub fn archive_with_ui(
    project: &Project,
    id: &str,
    yes: bool,
    dry_run: bool,
    cancellation: Cancellation<'_>,
    reporter: &mut dyn Reporter,
) -> Result<()> {
    validate_id(id)?;
    let change = project.resolve(id)?;
    let required = if cancellation.is_some() {
        State::Active
    } else {
        State::Completed
    };
    if change.state != required {
        bail!(
            "{} requires {required}; {id} is {}",
            if cancellation.is_some() {
                "cancel"
            } else {
                "archive"
            },
            change.state
        );
    }
    let before = safety::tree(project.root(), &change.path)?;
    let proposal_path = change.path.join("proposal.md");
    let snapshot = safety::snapshot(project.root(), &proposal_path)?;
    let original = safety::decode(&snapshot.as_ref().context("missing proposal.md")?.bytes)?;
    let proposal = match cancellation {
        Some((reason, disposition)) => cancellation_result(&original, reason, disposition)?,
        None => original,
    };
    let report = check::proposal(&proposal, true);
    report.show_with_ui(reporter)?;
    report.ensure()?;
    for entry in fs::read_dir(&change.path)? {
        let entry = entry?;
        if entry.file_name() != "proposal.md" && entry.file_name() != "work" {
            bail!(
                "unexpected file outside work/: {}; move valuable content deliberately before archiving",
                entry.path().display()
            );
        }
    }
    let work = change.path.join("work");
    if work.exists() && !work.is_dir() {
        bail!("work must be a directory");
    }
    retained_links(project, &change, &proposal)?;
    let destination = project.path(format!("doco/changes/archived/{id}"));
    safety::inspect(project.root(), &destination)?;
    if destination.exists() {
        bail!("archive destination exists");
    }
    ui::line(
        reporter,
        Tone::Info,
        "RETAIN",
        &format!("{} (complete proposal)", proposal_path.display()),
    )?;
    if cancellation.is_some() {
        ui::line(
            reporter,
            Tone::Warning,
            "UPDATE",
            &format!(
                "proposal Result with cancellation reason and implemented-code disposition:\n{}",
                markdown::result_section(&proposal).unwrap().content
            ),
        )?;
    }
    for entry in &before {
        if entry.path.starts_with(&work) {
            ui::line(
                reporter,
                Tone::Danger,
                "DELETE",
                &format!(
                    "{}{}",
                    entry.path.display(),
                    if entry.directory { "/" } else { "" }
                ),
            )?;
        }
    }
    if !work.exists() {
        ui::line(
            reporter,
            Tone::Warning,
            "RECOVERY",
            "work/ already absent; retry will finish the move without reconstructing discarded material.",
        )?;
    }
    ui::line(
        reporter,
        Tone::Info,
        "MOVE",
        &format!("{} -> {}", change.path.display(), destination.display()),
    )?;
    ui::text(
        reporter,
        "No code rollback, Git history rewrite, or unrelated tmp cleanup.\n",
    )?;
    if dry_run {
        ui::text(reporter, "Dry run: no files written or deleted.\n")?;
        return Ok(());
    }
    let _ = yes; // Retained as a source/CLI compatibility flag; confirmation is no longer required.
    let _lock = safety::Lock::acquire(project.root())?;
    let current = project.resolve(id)?;
    if current.state != change.state || safety::tree(project.root(), &change.path)? != before {
        bail!("change modified since preview; rerun to review the new scope");
    }
    safety::verify(project.root(), &proposal_path, &snapshot)?;
    retained_links(project, &change, &proposal)?;
    if cancellation.is_some() {
        safety::atomic_write(
            project.root(),
            &proposal_path,
            &snapshot,
            proposal.as_bytes(),
        )?;
        ui::line(
            reporter,
            Tone::Success,
            "APPLIED",
            &format!(
                "cancellation result; source remains {} until cleanup and move succeed",
                change.path.display()
            ),
        )?;
    }
    // Remove only preflighted entries, bottom-up. Newly added files make rmdir fail
    // instead of being silently swept up by a broad recursive removal.
    for entry in before.iter().rev().filter(|e| e.path.starts_with(&work)) {
        safety::inspect(project.root(), &entry.path)?;
        let result = if entry.directory {
            fs::remove_dir(&entry.path)
        } else {
            fs::remove_file(&entry.path)
        };
        if let Err(error) = result {
            bail!(
                "partial cleanup at {}: {error}; proposal retained in {}; source is still {}; fix the error and retry the same command",
                entry.path.display(),
                proposal_path.display(),
                change.state
            );
        }
        ui::line(
            reporter,
            Tone::Danger,
            "DELETED",
            &entry.path.display().to_string(),
        )?;
    }
    if safety::text(project.root(), &proposal_path)? != proposal {
        bail!(
            "proposal changed during cleanup; work may be removed, source remains {}; inspect and retry",
            change.path.display()
        );
    }
    for entry in fs::read_dir(&change.path)? {
        if entry?.file_name() != "proposal.md" {
            bail!(
                "unexpected material appeared during cleanup; source remains {}; inspect before retrying",
                change.path.display()
            );
        }
    }
    if let Err(error) = safety::move_directory(project.root(), &change.path, &destination) {
        bail!(
            "work cleanup succeeded but archive move failed: {error:#}; proposal retained at {}; retry the same command",
            proposal_path.display()
        );
    }
    if fs::read_dir(&destination)?.count() != 1
        || safety::text(project.root(), &destination.join("proposal.md"))? != proposal
    {
        bail!(
            "archive moved to {} but final verification failed; inspect actual files, not reporting success",
            destination.display()
        );
    }
    ui::text(
        reporter,
        &format!(
            "{} {id} -> archived; only proposal.md retained.\n",
            if cancellation.is_some() {
                "Cancelled"
            } else {
                "Archived"
            }
        ),
    )?;
    Ok(())
}
