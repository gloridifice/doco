use crate::{Project, State, markdown, safety};
use anyhow::{Result, bail};
use std::{
    collections::{BTreeSet, VecDeque},
    path::Path,
};

fn superseded(text: &str) -> bool {
    let header = text
        .lines()
        .take(20)
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    regex::Regex::new(r"(?m)^\s*(?:>\s*)?(?:\*\*)?(?:status\s*:\s*superseded|superseded by\s*:|已被\s*(?:adr[- ]?)?[0-9]+.*替代|状态[：:]\s*已替代)").unwrap().is_match(&header)
}
pub fn context(project: &Project, id: &str, history: bool) -> Result<()> {
    let change = project.resolve(id)?;
    if change.state != State::Active && !history {
        bail!(
            "{id} is {}; use --history to explicitly include its non-current snapshot",
            change.state
        );
    }
    println!("CURRENT ENTRY doco/architecture.md");
    if change.state != State::Active {
        println!(
            "HISTORICAL SNAPSHOT {id} ({}): not a current contract",
            change.state
        );
    }
    let mut paths = BTreeSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(project.path("doco/architecture.md"));
    queue.push_back(change.path.join("proposal.md"));
    if change.state != State::Archived {
        queue.push_back(change.path.join("work/implement.md"));
        queue.push_back(change.path.join("work/tasks.md"));
    }
    while let Some(path) = queue.pop_front() {
        if paths.contains(&path) {
            continue;
        }
        safety::inspect(project.root(), &path)?;
        if !path.is_file() {
            println!("WARNING missing document entry {}", path.display());
            continue;
        }
        let is_doc = path.extension().is_some_and(|e| e == "md");
        let text = if is_doc {
            safety::text(project.root(), &path)?
        } else {
            String::new()
        };
        if markdown::within(&path, &project.path("doco/decisions")) && superseded(&text) {
            println!(
                "EXCLUDED superseded decision {}",
                path.strip_prefix(project.root())?.display()
            );
            continue;
        }
        paths.insert(path.clone());
        // Expand explicit document references, never read source contents automatically.
        for link in markdown::links(&text) {
            if let Some(target_id) = link.strip_prefix("doco:") {
                let target_id = target_id.split('#').next().unwrap_or(target_id);
                match project.resolve(target_id) {
                    Ok(target) => println!(
                        "REFERENCE {target_id} ({}) -> {} [not added to default reading scope]",
                        target.state,
                        target
                            .path
                            .strip_prefix(project.root())?
                            .join("proposal.md")
                            .display()
                    ),
                    Err(e) => println!("WARNING unresolved stable reference {target_id}: {e}"),
                }
                continue;
            }
            let candidate = markdown::local_link(path.parent().unwrap(), &link)
                .filter(|p| p.exists())
                .or_else(|| markdown::local_link(project.root(), &link).filter(|p| p.exists()));
            let Some(candidate) = candidate else {
                continue;
            };
            if !candidate.starts_with(project.root()) || !candidate.is_file() {
                continue;
            }
            if excluded(project, &change.path, &candidate) {
                continue;
            }
            queue.push_back(candidate);
        }
    }
    for path in paths {
        let label = if change.state != State::Active && path.starts_with(&change.path) {
            "SNAPSHOT"
        } else {
            "READ"
        };
        println!("{label} {}", path.strip_prefix(project.root())?.display());
    }
    println!(
        "BOUNDARY: read only relevant current specs, effective decisions, affected source/tests and the selected active package. Explicit references are candidates; the agent must verify relevance.\nEXCLUDE by default: unrelated active changes, completed/archived packages, superseded ADRs and doco/tmp/. --history includes only the selected historical package. External search tools are not constrained by this CLI."
    );
    Ok(())
}
fn excluded(project: &Project, selected: &Path, path: &Path) -> bool {
    markdown::within(path, &project.path("doco/tmp"))
        || (markdown::within(path, &project.path("doco/changes"))
            && !markdown::within(path, selected))
        || markdown::within(path, &project.path(".git"))
}
