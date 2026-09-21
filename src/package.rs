use crate::{Change, Project, markdown, safety};
use anyhow::{Context, Result, bail};
use std::{ops::Range, path::PathBuf, time::SystemTime};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

pub const PROPOSAL_ONLY_MARKER: &str = "<!-- doco:change mode=proposal-only -->";
const LIFECYCLE_PREFIX: &str = "<!-- doco:lifecycle";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageMode {
    Full,
    ProposalOnly,
}

pub fn package_mode(proposal: &str) -> Result<PackageMode> {
    let declarations: Vec<_> = markdown::body_lines(proposal)
        .into_iter()
        .map(|(_, line)| line.trim())
        .filter(|line| line.starts_with("<!-- doco:change mode="))
        .collect();
    match declarations.as_slice() {
        [] => Ok(PackageMode::Full),
        [marker] if *marker == PROPOSAL_ONLY_MARKER => Ok(PackageMode::ProposalOnly),
        [marker] => bail!("proposal.md: unsupported change mode marker: {marker}"),
        _ => bail!("proposal.md: expected at most one doco change mode marker"),
    }
}

/// Discover optional target specs in a selected full package, never in other work material.
/// Callers select the package mode/state; all descendants are checked before filtering.
pub(crate) fn work_specs(project: &Project, change: &Change) -> Result<Vec<PathBuf>> {
    let directory = change.path.join("work/specs");
    safety::inspect(project.root(), &directory)?;
    if !directory.try_exists()? {
        return Ok(Vec::new());
    }
    if !directory.is_dir() {
        bail!("work/specs must be a directory: {}", directory.display());
    }
    let mut paths: Vec<_> = safety::tree(project.root(), &directory)?
        .into_iter()
        .filter(|entry| !entry.directory && entry.path.extension().is_some_and(|e| e == "md"))
        .map(|entry| entry.path)
        .collect();
    paths.sort();
    Ok(paths)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct LifecycleTimes {
    pub created_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub archived_at: Option<OffsetDateTime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecycleEvent {
    Created,
    Completed,
    Archived,
}

struct ParsedLifecycle {
    times: LifecycleTimes,
    marker: Option<Range<usize>>,
}

pub(crate) fn lifecycle_times(proposal: &str) -> Result<LifecycleTimes> {
    Ok(parse_lifecycle(proposal)?.times)
}

pub(crate) fn now_utc() -> Result<OffsetDateTime> {
    let elapsed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?;
    let seconds = i64::try_from(elapsed.as_secs()).context("system clock is out of range")?;
    OffsetDateTime::from_unix_timestamp(seconds).context("invalid system clock value")
}

pub(crate) fn set_lifecycle_event(
    proposal: &str,
    event: LifecycleEvent,
    at: OffsetDateTime,
) -> Result<String> {
    let parsed = parse_lifecycle(proposal)?;
    let mut times = parsed.times;
    let at = at.to_offset(UtcOffset::UTC).replace_nanosecond(0)?;
    match event {
        LifecycleEvent::Created => times.created_at = Some(at),
        LifecycleEvent::Completed => times.completed_at = Some(at),
        LifecycleEvent::Archived => times.archived_at = Some(at),
    }
    let marker = render_lifecycle(times)?;
    let newline = markdown::newline(proposal);
    let mut updated = proposal.to_string();
    if let Some(range) = parsed.marker {
        let ending = if proposal[range.clone()].ends_with("\r\n") {
            "\r\n"
        } else if proposal[range.clone()].ends_with('\n') {
            "\n"
        } else {
            ""
        };
        updated.replace_range(range, &format!("{marker}{ending}"));
        return Ok(updated);
    }

    let bom_len = if proposal.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let mode = markdown::body_lines(proposal)
        .into_iter()
        .find(|(_, line)| line.trim() == PROPOSAL_ONLY_MARKER)
        .map(|(range, _)| range);
    let (insert_at, insertion) = match mode {
        Some(range) if range.start == 0 && proposal[range.clone()].ends_with('\n') => {
            (range.end, format!("{marker}{newline}"))
        }
        Some(range) if range.start == 0 => (range.end, format!("{newline}{marker}{newline}")),
        _ => (bom_len, format!("{marker}{newline}")),
    };
    updated.insert_str(insert_at, &insertion);
    Ok(updated)
}

fn parse_lifecycle(proposal: &str) -> Result<ParsedLifecycle> {
    let declarations: Vec<_> = markdown::body_lines(proposal)
        .into_iter()
        .filter(|(_, line)| line.trim().starts_with(LIFECYCLE_PREFIX))
        .collect();
    let (range, marker) = match declarations.as_slice() {
        [] => {
            return Ok(ParsedLifecycle {
                times: LifecycleTimes::default(),
                marker: None,
            });
        }
        [(range, marker)] => (range.clone(), marker.trim()),
        _ => bail!("proposal.md: expected at most one doco lifecycle marker"),
    };
    let fields: Vec<_> = marker.split_ascii_whitespace().collect();
    if fields.len() != 7
        || fields[0] != "<!--"
        || fields[1] != "doco:lifecycle"
        || fields[2] != "v=1"
        || fields[6] != "-->"
    {
        bail!("proposal.md: invalid doco lifecycle marker");
    }
    Ok(ParsedLifecycle {
        times: LifecycleTimes {
            created_at: parse_lifecycle_time(fields[3], "created-at=")?,
            completed_at: parse_lifecycle_time(fields[4], "completed-at=")?,
            archived_at: parse_lifecycle_time(fields[5], "archived-at=")?,
        },
        marker: Some(range),
    })
}

fn parse_lifecycle_time(field: &str, name: &str) -> Result<Option<OffsetDateTime>> {
    let value = field
        .strip_prefix(name)
        .with_context(|| format!("proposal.md: invalid doco lifecycle field; expected {name}"))?;
    if value == "-" {
        return Ok(None);
    }
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .with_context(|| format!("proposal.md: invalid lifecycle timestamp: {value}"))?;
    if parsed.nanosecond() != 0 {
        bail!("proposal.md: lifecycle timestamps must use whole seconds: {value}");
    }
    Ok(Some(parsed))
}

fn render_lifecycle(times: LifecycleTimes) -> Result<String> {
    fn field(value: Option<OffsetDateTime>) -> Result<String> {
        match value {
            Some(value) => value
                .to_offset(UtcOffset::UTC)
                .format(&Rfc3339)
                .context("cannot format lifecycle timestamp"),
            None => Ok("-".into()),
        }
    }
    Ok(format!(
        "<!-- doco:lifecycle v=1 created-at={} completed-at={} archived-at={} -->",
        field(times.created_at)?,
        field(times.completed_at)?,
        field(times.archived_at)?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_is_explicit_unique_and_ignores_fenced_examples() {
        assert_eq!(package_mode("# Full").unwrap(), PackageMode::Full);
        assert_eq!(
            package_mode(PROPOSAL_ONLY_MARKER).unwrap(),
            PackageMode::ProposalOnly
        );
        assert_eq!(
            package_mode(&format!("```md\n{PROPOSAL_ONLY_MARKER}\n```")).unwrap(),
            PackageMode::Full
        );
        assert!(package_mode("<!-- doco:change mode=unknown -->").is_err());
        assert!(package_mode(&format!("{PROPOSAL_ONLY_MARKER}\n{PROPOSAL_ONLY_MARKER}")).is_err());
    }

    fn at(value: &str) -> OffsetDateTime {
        OffsetDateTime::parse(value, &Rfc3339).unwrap()
    }

    #[test]
    fn lifecycle_marker_is_inserted_and_updated_without_losing_other_times() {
        let created = at("2026-09-14T10:00:00Z");
        let completed = at("2026-09-15T11:30:00Z");
        let archived = at("2026-09-16T12:45:00Z");
        let proposal = set_lifecycle_event("# Goal\n", LifecycleEvent::Created, created).unwrap();
        assert_eq!(
            proposal,
            "<!-- doco:lifecycle v=1 created-at=2026-09-14T10:00:00Z completed-at=- archived-at=- -->\n# Goal\n"
        );
        let proposal =
            set_lifecycle_event(&proposal, LifecycleEvent::Completed, completed).unwrap();
        let proposal = set_lifecycle_event(&proposal, LifecycleEvent::Archived, archived).unwrap();
        assert_eq!(
            lifecycle_times(&proposal).unwrap(),
            LifecycleTimes {
                created_at: Some(created),
                completed_at: Some(completed),
                archived_at: Some(archived),
            }
        );
        assert!(proposal.ends_with("# Goal\n"));
    }

    #[test]
    fn lifecycle_insertion_preserves_bom_mode_and_crlf() {
        let proposal = format!("\u{feff}{PROPOSAL_ONLY_MARKER}\r\n# Goal\r\n");
        let updated = set_lifecycle_event(
            &proposal,
            LifecycleEvent::Created,
            at("2026-09-14T10:00:00Z"),
        )
        .unwrap();
        assert!(updated.starts_with(&format!(
            "\u{feff}{PROPOSAL_ONLY_MARKER}\r\n<!-- doco:lifecycle"
        )));
        assert!(updated.contains("archived-at=- -->\r\n# Goal\r\n"));
    }

    #[test]
    fn lifecycle_parser_allows_legacy_and_rejects_ambiguous_metadata() {
        assert_eq!(
            lifecycle_times("# Legacy").unwrap(),
            LifecycleTimes::default()
        );
        let marker = "<!-- doco:lifecycle v=1 created-at=2026-09-14T10:00:00Z completed-at=- archived-at=- -->";
        assert!(lifecycle_times(&format!("{marker}\n{marker}")).is_err());
        assert!(
            lifecycle_times(
                "<!-- doco:lifecycle v=2 created-at=- completed-at=- archived-at=- -->"
            )
            .is_err()
        );
        assert!(
            lifecycle_times(
                "<!-- doco:lifecycle v=1 created-at=2026-09-14T10:00:00.123Z completed-at=- archived-at=- -->"
            )
            .is_err()
        );
        assert_eq!(
            lifecycle_times(&format!("```md\n{marker}\n```\n# Legacy")).unwrap(),
            LifecycleTimes::default()
        );
    }
}
