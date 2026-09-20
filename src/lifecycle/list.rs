use crate::{
    PackageMode, Project, State,
    check::tasks,
    lifecycle_times, now_utc, package_mode, safety,
    ui::{ChangeRow, LifecycleAge, TaskCount},
};
use anyhow::{Context, Result};
use std::time::Duration;
use time::OffsetDateTime;

/// Collect display counts without validating completion or changing lifecycle state.
pub fn list_changes(project: &Project) -> Result<Vec<ChangeRow>> {
    list_changes_at(project, now_utc()?, true, true)
}

pub fn list_changes_filtered(
    project: &Project,
    include_completed: bool,
    include_archived: bool,
) -> Result<Vec<ChangeRow>> {
    list_changes_at(project, now_utc()?, include_completed, include_archived)
}

fn list_changes_at(
    project: &Project,
    now: OffsetDateTime,
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
            let proposal_path = change.path.join("proposal.md");
            let proposal = safety::text(project.root(), &proposal_path)
                .with_context(|| format!("cannot read proposal: {}", proposal_path.display()))?;
            let times = lifecycle_times(&proposal)?;
            let count = if change.state == State::Archived {
                TaskCount::Archived
            } else {
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
            let event = match change.state {
                State::Active => times.created_at,
                State::Completed => times.completed_at,
                State::Archived => times.archived_at,
            };
            let age = event.map_or(LifecycleAge::Unknown, |event| {
                let seconds = (now - event).whole_seconds();
                LifecycleAge::Known(if seconds <= 0 {
                    Duration::ZERO
                } else {
                    Duration::from_secs(seconds as u64)
                })
            });
            Ok(ChangeRow {
                id: change.id,
                state: change.state,
                tasks: count,
                age,
            })
        })
        .collect()
}
