use crate::{
    PackageMode, Project, State,
    check::tasks,
    package_mode, safety,
    ui::{ChangeRow, ModifiedAge, TaskCount},
};
use anyhow::{Context, Result};
use std::time::{Duration, SystemTime};

/// Collect display counts without validating completion or changing lifecycle state.
pub fn list_changes(project: &Project) -> Result<Vec<ChangeRow>> {
    list_changes_at(project, SystemTime::now(), true, true)
}

pub fn list_changes_filtered(
    project: &Project,
    include_completed: bool,
    include_archived: bool,
) -> Result<Vec<ChangeRow>> {
    list_changes_at(
        project,
        SystemTime::now(),
        include_completed,
        include_archived,
    )
}

fn list_changes_at(
    project: &Project,
    now: SystemTime,
    include_completed: bool,
    include_archived: bool,
) -> Result<Vec<ChangeRow>> {
    project
        .changes()?
        .into_iter()
        .filter(|change| match change.state {
            State::Active => true,
            State::Completed => include_completed,
            State::Archived => include_archived,
        })
        .map(|change| {
            let count = if change.state == State::Archived {
                TaskCount::Archived
            } else {
                let proposal_path = change.path.join("proposal.md");
                let proposal = safety::text(project.root(), &proposal_path).with_context(|| {
                    format!("cannot read proposal: {}", proposal_path.display())
                })?;
                match package_mode(&proposal)? {
                    PackageMode::ProposalOnly => TaskCount::NotApplicable,
                    PackageMode::Full => {
                        let path = change.path.join("work/tasks.md");
                        match safety::snapshot(project.root(), &path)
                            .with_context(|| format!("cannot read tasks: {}", path.display()))?
                        {
                            None => TaskCount::Missing,
                            Some(snapshot) => {
                                let text = safety::decode(&snapshot.bytes).with_context(|| {
                                    format!("invalid UTF-8: {}", path.display())
                                })?;
                                let parsed = tasks::parse(&text);
                                TaskCount::Known {
                                    done: parsed.items.iter().filter(|task| task.done).count(),
                                    total: parsed.items.len(),
                                }
                            }
                        }
                    }
                }
            };
            let updated = safety::tree(project.root(), &change.path)
                .with_context(|| format!("cannot inspect change: {}", change.path.display()))?
                .iter()
                .filter_map(safety::TreeEntry::modified)
                .max()
                .map(|modified| {
                    ModifiedAge::Known(modified.duration_since(now).unwrap_or(Duration::ZERO))
                })
                .unwrap_or(ModifiedAge::Unknown);
            Ok(ChangeRow {
                id: change.id,
                state: change.state,
                tasks: count,
                updated,
            })
        })
        .collect()
}
