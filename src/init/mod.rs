//! Build a conflict-checked plan first. Integration files are individually atomic,
//! not a cross-file transaction; successful writes are explicitly reported.
mod entry;
mod plan;
mod skill;
#[cfg(test)]
mod tests;

use crate::{
    Project, safety, templates,
    ui::{self, Reporter},
};
use anyhow::{Result, bail};
use plan::Plan;

pub fn run(project: &Project, agents: &[&str], dry_run: bool, refresh: bool) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    run_with_ui(project, agents, dry_run, refresh, &mut reporter)
}

pub fn run_with_ui(
    project: &Project,
    agents: &[&str],
    dry_run: bool,
    refresh: bool,
    reporter: &mut dyn Reporter,
) -> Result<()> {
    if agents.is_empty()
        || agents
            .iter()
            .any(|a| !["codex", "claude", "pi"].contains(a))
    {
        bail!("explicit known agent selection required");
    }
    let plan = build(project, agents, refresh);
    plan.show(project, reporter)?;
    plan.ensure_valid()?;
    if dry_run {
        ui::text(reporter, "Dry run: no files written.\n")?;
        return Ok(());
    }
    plan.apply(project, reporter)?;
    ui::text(
        reporter,
        "Disk installation complete. Agent loading is NOT verified; check project trust and discovery/context settings in your agent.\n",
    )?;
    Ok(())
}

fn build(project: &Project, agents: &[&str], refresh: bool) -> Plan {
    let mut plan = Plan::default();
    for directory in [
        "doco",
        "doco/decisions",
        "doco/specs",
        "doco/tmp",
        "doco/changes",
        "doco/changes/active",
        "doco/changes/completed",
        "doco/changes/archived",
    ] {
        plan.directory(project, directory);
    }
    for (path, default) in [
        ("doco/.gitignore", "/tmp/\n"),
        ("doco/architecture.md", templates::ARCHITECTURE),
    ] {
        plan.file(project, path, |old| Ok(old.unwrap_or(default).to_string()));
    }
    let shared = agents.contains(&"codex") || agents.contains(&"pi");
    let claude = agents.contains(&"claude");
    let mut shared_dir = ".agents/skills/doco";
    if shared && project.path(".pi/skills/doco").symlink_metadata().is_ok() {
        match skill::compatible(project, ".pi/skills/doco") {
            Ok(true) if !project.path(shared_dir).exists() && !agents.contains(&"codex") => {
                shared_dir = ".pi/skills/doco";
                plan.warnings.push("reusing compatible project .pi/skills/doco instead of installing a duplicate".into());
            }
            _ => plan.conflicts.push(".pi/skills/doco: another discoverable doco skill exists; explicitly migrate/merge it into .agents/skills/doco before retrying".into()),
        }
    }
    if agents.contains(&"codex")
        && project
            .path(".codex/skills/doco")
            .symlink_metadata()
            .is_ok()
    {
        plan.conflicts.push(".codex/skills/doco: alternative same-name skill detected; explicitly migrate/merge before installation".into());
    }
    if shared {
        skill::install(&mut plan, project, shared_dir, refresh);
    }
    if claude {
        skill::install(&mut plan, project, ".claude/skills/doco", refresh);
    }

    if shared {
        plan.file(project, "AGENTS.md", |old| {
            entry::update(old, "Project instructions", shared_dir, refresh)
        });
        if project
            .path("AGENTS.override.md")
            .symlink_metadata()
            .is_ok()
        {
            plan.warnings.push("AGENTS.override.md may shadow AGENTS.md: files can be installed but the default entry may not take effect; override is not modified".into());
        }
    }
    // Also inspect an existing Claude import when only adding the shared integration:
    // adding AGENTS could otherwise create a duplicate effective workflow.
    if claude || (shared && project.path("CLAUDE.md").exists()) {
        let result = (|| -> Result<bool> {
            let Some(claude_text) = plan.effective_text(project, "CLAUDE.md")? else {
                return Ok(false);
            };
            crate::markdown::safe_append(&claude_text)?;
            if !entry::imports_agents(&claude_text)? {
                return Ok(false);
            }
            let agents_text = plan
                .effective_text(project, "AGENTS.md")?
                .ok_or_else(|| anyhow::anyhow!("CLAUDE.md imports missing AGENTS.md"))?;
            if entry::imports_agents(&agents_text)? {
                bail!("recursive AGENTS.md import");
            }
            let Some(skill_path) = entry::visible_skill(&agents_text)? else {
                return Ok(false);
            };
            if entry::block_range(&claude_text)?.is_some() {
                bail!(
                    "CLAUDE.md and imported AGENTS.md both contain doco blocks; manual deduplication required"
                );
            }
            // Confirm the whole referenced bundle, including files not selected for installation.
            for (file, generated) in templates::FILES {
                let full = format!("{skill_path}/{file}");
                let content = plan.effective_text(project, &full)?.ok_or_else(|| {
                    anyhow::anyhow!("imported navigation references missing {full}")
                })?;
                if crate::markdown::normalize(&content) != crate::markdown::normalize(generated) {
                    bail!("imported skill {full} is not a verified compatible bundle");
                }
            }
            // Recognize custom second instructions as a conflict, too.
            if entry::visible_skill(&claude_text)?.is_some() {
                bail!("duplicate Claude workflow");
            }
            Ok(true)
        })();
        match result {
            Ok(true) => {
                if claude {
                    plan.file(project, "CLAUDE.md", |old| {
                        Ok(old.unwrap_or_default().to_string())
                    });
                }
                plan.warnings.push("reusing effective doco navigation via existing @AGENTS.md import; no new import added".into());
            }
            Ok(false) if claude => plan.file(project, "CLAUDE.md", |old| {
                entry::update(old, "Claude instructions", ".claude/skills/doco", refresh)
            }),
            Ok(false) => {}
            Err(e) => plan.conflicts.push(format!("CLAUDE.md import: {e:#}")),
        }
    }
    for path in [
        ".pi/settings.json",
        ".claude/settings.json",
        ".codex/config.toml",
    ] {
        if project.path(path).exists() {
            plan.warnings.push(format!("{path} exists; custom discovery, disabled resources and trust are not overridden or certified"));
        }
    }
    // Existing roots are inspected even if there was no file operation under them.
    if let Err(e) = safety::inspect(project.root(), &project.path("doco")) {
        plan.conflicts.push(format!("doco: {e:#}"));
    }
    plan
}
