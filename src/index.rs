//! Disposable local cache of the change directories under `doco/changes`.
//!
//! The directories stay authoritative. The cache only avoids re-enumerating them,
//! never grants file ownership and never replaces the existing safety checks. A
//! cache hit is a heuristic (equal directory modification times), so exact targets
//! are always verified against the three real state directories.
use crate::{
    Change, Project, State, safety,
    ui::{self, Reporter, Stream, Tone},
    validate_id,
};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    fs,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Single local cache file; `doco/.gitignore` ignores `tmp/`, so it is not shared.
pub(crate) const CACHE_RELATIVE: &str = "doco/tmp/changes-index.csv";
const FORMAT_VERSION: &str = "1";
const HEADER: &str = "type,key,value";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DirectoryMtimes([i128; 3]);

struct Parsed {
    mtimes: DirectoryMtimes,
    entries: BTreeMap<String, State>,
}

/// All changes, from the cache when it is still trusted, otherwise from the dirs.
pub(crate) fn entries(project: &Project) -> Result<Vec<Change>> {
    Ok(load(project)?
        .into_iter()
        .map(|(id, state)| Change {
            path: project.path(format!("doco/changes/{state}/{id}")),
            id,
            state,
        })
        .collect())
}

pub(crate) fn resolve(project: &Project, id: &str) -> Result<Change> {
    validate_id(id)?;
    let known = load(project)?.get(id).copied();
    let probed = probe(project, id)?;
    if known == probed.as_ref().map(|change| change.state) {
        return probed.with_context(|| format!("unknown change ID: {id}"));
    }
    // The cache and the three real paths disagree; rescan and trust only the disk.
    scan(project)?;
    probe(project, id)?.with_context(|| format!("unknown change ID: {id}"))
}

/// Index of every change, using the cache only when its directory stamps match.
pub(crate) fn load(project: &Project) -> Result<BTreeMap<String, State>> {
    project.initialized()?;
    match cached(project)? {
        Some(entries) => Ok(entries),
        None => scan_initialized(project),
    }
}

/// Authoritative directory scan that ignores any cache.
pub(crate) fn scan(project: &Project) -> Result<BTreeMap<String, State>> {
    project.initialized()?;
    scan_initialized(project)
}

/// Verify the three real paths for one ID without trusting a cached entry.
pub(crate) fn probe(project: &Project, id: &str) -> Result<Option<Change>> {
    validate_id(id)?;
    let mut found: Option<Change> = None;
    for state in State::ALL {
        let path = project.path(format!("doco/changes/{state}/{id}"));
        safety::inspect(project.root(), &path)?;
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => bail!("unexpected file in lifecycle directory: {}", path.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("inspect {}", path.display()));
            }
        }
        if let Some(previous) = &found {
            bail!("duplicate change ID {id}: {} and {state}", previous.state);
        }
        found = Some(Change {
            id: id.to_string(),
            state,
            path,
        });
    }
    Ok(found)
}

/// Refuse a new ID that exists on disk, refreshing a cache that disagrees.
pub(crate) fn ensure_absent(
    project: &Project,
    id: &str,
    entries: &mut BTreeMap<String, State>,
) -> Result<()> {
    let already = || format!("change ID {id} already exists; choose an explicit new ID");
    if probe(project, id)?.is_some() {
        bail!("{}", already());
    }
    if entries.contains_key(id) {
        *entries = scan(project)?;
        if probe(project, id)?.is_some() {
            bail!("{}", already());
        }
        // The scan and the three verified paths agree that the ID is absent.
        entries.remove(id);
    }
    Ok(())
}

/// Drop the cache before the first business write; callers already hold the lock.
pub(crate) fn invalidate(project: &Project, _lock: &safety::Lock) -> Result<()> {
    let path = project.path(CACHE_RELATIVE);
    let Some(snapshot) = safety::snapshot(project.root(), &path)? else {
        return Ok(());
    };
    safety::remove_file(project.root(), &path, &snapshot)
        .context("cannot remove the change index cache before writing")
}

/// Write the cache after the business operation succeeded; callers hold the lock.
pub(crate) fn publish(
    project: &Project,
    _lock: &safety::Lock,
    entries: &BTreeMap<String, State>,
) -> Result<()> {
    let mtimes = read_mtimes(project)?
        .context("this platform does not expose directory modification times")?;
    let text = encode(entries, mtimes, &now_utc()?);
    if read_mtimes(project)? != Some(mtimes) {
        bail!("change directories changed while building the cache; run doco fix");
    }
    safety::atomic_write(
        project.root(),
        &project.path(CACHE_RELATIVE),
        &None,
        text.as_bytes(),
    )
    .context("cannot write the change index cache")
}

/// Publish after a committed operation; a cache failure only warns.
pub(crate) fn publish_after_commit(
    project: &Project,
    lock: &safety::Lock,
    entries: &BTreeMap<String, State>,
    reporter: &mut dyn Reporter,
) -> Result<()> {
    if let Err(error) = publish(project, lock, entries) {
        ui::warning(
            reporter,
            &format!(
                "the operation is committed but the change index cache was not written: {error:#}; run doco fix to rebuild it"
            ),
        )?;
    }
    Ok(())
}

pub(crate) fn counts(entries: &BTreeMap<String, State>) -> String {
    let active = entries
        .values()
        .filter(|state| **state == State::Active)
        .count();
    let completed = entries
        .values()
        .filter(|state| **state == State::Completed)
        .count();
    let archived = entries.len() - active - completed;
    format!("{active} active, {completed} completed, {archived} archived")
}

/// Force a rebuild: announce, lock, drop the old cache, scan, publish.
pub(crate) fn rebuild_with_ui(project: &Project, reporter: &mut dyn Reporter) -> Result<()> {
    project.initialized()?;
    ui::line(
        reporter,
        Tone::Info,
        "INDEX",
        "Building change index cache...",
    )?;
    reporter.flush(Stream::Stdout)?;
    let lock = safety::Lock::acquire(project.root())?;
    invalidate(project, &lock)?;
    let entries = scan_initialized(project)?;
    publish(project, &lock, &entries)?;
    ui::line(
        reporter,
        Tone::Success,
        "INDEX",
        &format!("Cache built: {}.", counts(&entries)),
    )?;
    Ok(())
}

fn scan_initialized(project: &Project) -> Result<BTreeMap<String, State>> {
    let before = read_mtimes(project)?;
    let entries = collect(project)?;
    let after = read_mtimes(project)?;
    if before != after {
        bail!("change directories changed during the scan; retry");
    }
    Ok(entries)
}

fn cached(project: &Project) -> Result<Option<BTreeMap<String, State>>> {
    let before = read_mtimes(project)?;
    let path = project.path(CACHE_RELATIVE);
    let Some(snapshot) = safety::snapshot(project.root(), &path)? else {
        return Ok(None);
    };
    let Ok(text) = std::str::from_utf8(&snapshot.bytes) else {
        return Ok(None);
    };
    let Some(parsed) = parse(text) else {
        return Ok(None);
    };
    if before != Some(parsed.mtimes) || read_mtimes(project)? != Some(parsed.mtimes) {
        return Ok(None);
    }
    Ok(Some(parsed.entries))
}

/// Authoritative enumeration; unknown or unexpected entries are errors, not skips.
fn collect(project: &Project) -> Result<BTreeMap<String, State>> {
    let mut found = BTreeMap::<String, State>::new();
    for state in State::ALL {
        let parent = project.path(format!("doco/changes/{state}"));
        for entry in fs::read_dir(&parent)? {
            let entry = entry?;
            let path = entry.path();
            safety::inspect(project.root(), &path)?;
            let id = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("non-UTF-8 change ID"))?;
            validate_id(&id)?;
            if !entry.file_type()?.is_dir() {
                bail!("unexpected file in lifecycle directory: {}", path.display());
            }
            if let Some(previous) = found.get(&id) {
                bail!("duplicate change ID {id}: {previous} and {state}");
            }
            found.insert(id, state);
        }
    }
    Ok(found)
}

fn read_mtimes(project: &Project) -> Result<Option<DirectoryMtimes>> {
    let mut values = [0i128; 3];
    for (slot, state) in values.iter_mut().zip(State::ALL) {
        let relative = format!("doco/changes/{state}");
        let path = project.path(&relative);
        safety::inspect(project.root(), &path)?;
        let meta = fs::metadata(&path).with_context(|| format!("inspect {relative}"))?;
        match meta.modified() {
            Ok(modified) => *slot = to_nanos(modified)?,
            Err(_) => return Ok(None),
        }
    }
    Ok(Some(DirectoryMtimes(values)))
}

fn encode(entries: &BTreeMap<String, State>, mtimes: DirectoryMtimes, updated_at: &str) -> String {
    use std::fmt::Write as _;
    let mut text = String::from(HEADER);
    text.push('\n');
    let _ = writeln!(text, "meta,version,{FORMAT_VERSION}");
    let _ = writeln!(text, "meta,updated_at,{updated_at}");
    for (slot, state) in mtimes.0.iter().zip(State::ALL) {
        let _ = writeln!(text, "directory,{state},{slot}");
    }
    for (id, state) in entries {
        let _ = writeln!(text, "change,{id},{state}");
    }
    text
}

/// Restricted reader for the fixed three-column format; `None` means cache miss.
fn parse(text: &str) -> Option<Parsed> {
    let mut lines = text.lines();
    if lines.next()? != HEADER {
        return None;
    }
    let mut version: Option<&str> = None;
    let mut updated_at: Option<&str> = None;
    let mut mtimes = [None; 3];
    let mut entries = BTreeMap::new();
    for line in lines {
        let mut fields = line.split(',');
        let kind = fields.next()?;
        let key = fields.next()?;
        let value = fields.next()?;
        if fields.next().is_some() {
            return None;
        }
        match kind {
            "meta" => {
                let slot = match key {
                    "version" => &mut version,
                    "updated_at" => &mut updated_at,
                    _ => return None,
                };
                if slot.replace(value).is_some() {
                    return None;
                }
            }
            "directory" => {
                let index = State::ALL.iter().position(|state| state.as_str() == key)?;
                if mtimes[index].replace(parse_nanos(value)?).is_some() {
                    return None;
                }
            }
            "change" => {
                validate_id(key).ok()?;
                let state = state_from(value)?;
                if entries.insert(key.to_string(), state).is_some() {
                    return None;
                }
            }
            _ => return None,
        }
    }
    if version? != FORMAT_VERSION || !valid_updated_at(updated_at?) {
        return None;
    }
    let mtimes = DirectoryMtimes([mtimes[0]?, mtimes[1]?, mtimes[2]?]);
    if mtimes
        .0
        .iter()
        .any(|value| to_system_time(*value).is_none())
    {
        return None;
    }
    Some(Parsed { mtimes, entries })
}

fn state_from(value: &str) -> Option<State> {
    State::ALL.into_iter().find(|state| state.as_str() == value)
}

fn parse_nanos(value: &str) -> Option<i128> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse::<i128>().ok()
}

fn to_nanos(time: SystemTime) -> Result<i128> {
    let (duration, negative) = match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration, false),
        Err(error) => (error.duration(), true),
    };
    let magnitude = i128::try_from(duration.as_nanos()).context("timestamp out of range")?;
    Ok(if negative { -magnitude } else { magnitude })
}

fn to_system_time(nanos: i128) -> Option<SystemTime> {
    let magnitude = u64::try_from(nanos.unsigned_abs()).ok()?;
    let duration = Duration::new(
        magnitude / 1_000_000_000,
        (magnitude % 1_000_000_000) as u32,
    );
    if nanos < 0 {
        UNIX_EPOCH.checked_sub(duration)
    } else {
        UNIX_EPOCH.checked_add(duration)
    }
}

fn now_utc() -> Result<String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?;
    let seconds = i64::try_from(elapsed.as_secs()).context("system clock is out of range")?;
    OffsetDateTime::from_unix_timestamp(seconds)
        .context("invalid system clock value")?
        .format(&Rfc3339)
        .context("cannot format the cache timestamp")
}

fn valid_updated_at(value: &str) -> bool {
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .filter(|parsed| parsed.nanosecond() == 0)
        .and_then(|parsed| parsed.format(&Rfc3339).ok())
        .is_some_and(|canonical| canonical == value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nanos(value: i128) -> i128 {
        to_nanos(to_system_time(value).unwrap()).unwrap()
    }

    #[test]
    fn cache_round_trips_and_stays_deterministic() {
        let mut entries = BTreeMap::new();
        entries.insert("zeta".to_string(), State::Archived);
        entries.insert("alpha".to_string(), State::Active);
        entries.insert("middle".to_string(), State::Completed);
        let mtimes = DirectoryMtimes([1, -2, 3]);
        let text = encode(&entries, mtimes, "2026-09-14T10:00:00Z");
        assert_eq!(
            text,
            "type,key,value\nmeta,version,1\nmeta,updated_at,2026-09-14T10:00:00Z\n\
             directory,active,1\ndirectory,completed,-2\ndirectory,archived,3\n\
             change,alpha,active\nchange,middle,completed\nchange,zeta,archived\n"
        );
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed.mtimes, mtimes);
        assert_eq!(parsed.entries, entries);
    }

    #[test]
    fn corrupt_or_unknown_caches_are_rejected() {
        let good = "type,key,value\nmeta,version,1\nmeta,updated_at,2026-09-14T10:00:00Z\n\
                    directory,active,1\ndirectory,completed,2\ndirectory,archived,3\n\
                    change,alpha,active\n";
        assert!(parse(good).is_some());
        assert!(parse(&good.replace('\n', "\r\n")).is_some());
        assert!(parse(good.trim_end_matches('\n')).is_some());
        let cases = [
            good.replace("type,key,value", "id,state"),
            good.replace("meta,version,1", "meta,version,2"),
            good.replace(
                "meta,updated_at,2026-09-14T10:00:00Z",
                "meta,updated_at,2026-09-14T10:00:00+00:00",
            ),
            good.replace("2026-09-14T10:00:00Z", "not-a-date"),
            good.replace("2026-09-14T10:00:00Z", "2026-02-30T10:00:00Z"),
            good.replace("meta,version,1", ""),
            good.replace("directory,archived,3", ""),
            good.replace("directory,archived,3", "directory,unknown,3"),
            good.replace("directory,archived,3", "directory,active,3"),
            good.replace("directory,archived,3", "directory,archived,1e3"),
            good.replace("directory,archived,3", "directory,archived,"),
            good.replace(
                "directory,archived,3",
                "directory,archived,99999999999999999999",
            ),
            good.replace("change,alpha,active", "change,alpha,unknown"),
            good.replace("change,alpha,active", "change,Alpha,active"),
            good.replace(
                "change,alpha,active",
                format!("{}{}", "change,alpha,active\n", "change,alpha,active").as_str(),
            ),
            good.replace("change,alpha,active", "change,alpha"),
            good.replace("change,alpha,active", "change,alpha,active,extra"),
            good.replace("change,alpha,active", "unexpected,alpha,active"),
            format!("{good}\n"),
        ];
        for case in cases {
            assert!(parse(&case).is_none(), "accepted: {case:?}");
        }
    }

    #[test]
    fn timestamps_keep_nanoseconds_and_support_the_epoch() {
        // Round trips use whole seconds; platforms store coarser ticks than one nanosecond.
        assert_eq!(nanos(0), 0);
        assert_eq!(nanos(1_500_000_000), 1_500_000_000);
        assert_eq!(nanos(-1_500_000_000), -1_500_000_000);
        assert!(to_system_time(-1_500_000_000).unwrap() < UNIX_EPOCH);
        assert!(to_system_time(i128::MAX).is_none());
        assert_eq!(parse_nanos("-42"), Some(-42));
        assert_eq!(parse_nanos("42"), Some(42));
        assert!(parse_nanos("+42").is_none());
        assert!(parse_nanos("1.5").is_none());
        assert!(parse_nanos("").is_none());
        assert!(parse_nanos("infinite").is_none());
        assert!(!valid_updated_at("2026-09-14T10:00:00.5Z"));
        assert!(valid_updated_at("2026-09-14T10:00:00Z"));
    }

    #[test]
    fn counts_are_reported_per_state() {
        let mut entries = BTreeMap::new();
        entries.insert("a".to_string(), State::Active);
        entries.insert("b".to_string(), State::Active);
        entries.insert("c".to_string(), State::Completed);
        entries.insert("d".to_string(), State::Archived);
        assert_eq!(counts(&entries), "2 active, 1 completed, 1 archived");
        assert_eq!(
            counts(&BTreeMap::new()),
            "0 active, 0 completed, 0 archived"
        );
    }

    fn fixture() -> (tempfile::TempDir, Project) {
        let directory = tempfile::tempdir().unwrap();
        for relative in [
            "doco/changes/active",
            "doco/changes/completed",
            "doco/changes/archived",
            "doco/tmp",
        ] {
            fs::create_dir_all(directory.path().join(relative)).unwrap();
        }
        fs::write(directory.path().join("doco/architecture.md"), "# Fixture\n").unwrap();
        let project = Project::open(directory.path()).unwrap();
        (directory, project)
    }

    #[test]
    fn an_unavailable_cache_file_warns_after_commit_without_failing_it() {
        let (_temporary, project) = fixture();
        let lock = safety::Lock::acquire(project.root()).unwrap();
        // A directory on the cache path cannot be replaced by an atomic write.
        fs::create_dir(project.path(CACHE_RELATIVE)).unwrap();
        assert!(publish(&project, &lock, &BTreeMap::new()).is_err());
        let mut stderr = Vec::new();
        let mut reporter = crate::ui::PlainReporter::new(Vec::new(), &mut stderr);
        publish_after_commit(&project, &lock, &BTreeMap::new(), &mut reporter).unwrap();
        let text = String::from_utf8(stderr).unwrap();
        assert!(text.contains("WARNING"), "{text}");
        assert!(text.contains("operation is committed"), "{text}");
        assert!(text.contains("run doco fix"), "{text}");
    }

    #[test]
    fn a_cached_snapshot_is_only_trusted_while_the_directory_stamps_match() {
        let (_temporary, project) = fixture();
        let lock = safety::Lock::acquire(project.root()).unwrap();
        let mut entries = BTreeMap::new();
        entries.insert("goal".to_string(), State::Active);
        publish(&project, &lock, &entries).unwrap();
        fs::create_dir_all(project.path("doco/changes/active/goal")).unwrap();
        assert_eq!(load(&project).unwrap(), entries, "stamps still match");
        assert_eq!(load(&project).unwrap(), entries);

        // Changing a state directory invalidates the cache even when bytes are intact.
        fs::create_dir_all(project.path("doco/changes/completed/other")).unwrap();
        let scanned = load(&project).unwrap();
        assert_eq!(scanned.get("other"), Some(&State::Completed));
        assert!(cached(&project).unwrap().is_none());
    }
}
