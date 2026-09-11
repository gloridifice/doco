//! Build a conflict-checked plan first. Integration files are individually atomic,
//! not a cross-file transaction; successful writes are explicitly reported.
pub(crate) mod entry;
pub(crate) mod plan;
pub(crate) mod skill;
#[cfg(test)]
mod tests;

use crate::{
    Project, safety, templates,
    ui::{self, Reporter},
};
use anyhow::{Result, bail};
use plan::Plan;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Integration {
    Most,
    Claude,
}

impl Integration {
    pub(crate) const ALL: [Self; 2] = [Self::Most, Self::Claude];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Most => "most",
            Self::Claude => "claude",
        }
    }

    pub(crate) fn skill_dir(self) -> &'static str {
        match self {
            Self::Most => ".agents/skills/doco",
            Self::Claude => ".claude/skills/doco",
        }
    }

    pub(crate) fn entry_file(self) -> &'static str {
        match self {
            Self::Most => "AGENTS.md",
            Self::Claude => "CLAUDE.md",
        }
    }

    fn entry_title(self) -> &'static str {
        match self {
            Self::Most => "Project instructions",
            Self::Claude => "Claude instructions",
        }
    }
}

pub fn run(project: &Project, integrations: &[&str], dry_run: bool, refresh: bool) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    run_with_ui(project, integrations, dry_run, refresh, &mut reporter)
}

pub fn run_with_ui(
    project: &Project,
    integrations: &[&str],
    dry_run: bool,
    refresh: bool,
    reporter: &mut dyn Reporter,
) -> Result<()> {
    if integrations.is_empty()
        || integrations
            .iter()
            .any(|integration| !["most", "claude"].contains(integration))
    {
        bail!("explicit known integration selection required");
    }
    let plan = build(project, integrations, refresh);
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

fn build(project: &Project, selected: &[&str], refresh: bool) -> Plan {
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
    let integrations: Vec<_> = selected
        .iter()
        .map(|integration| match *integration {
            "most" => Integration::Most,
            "claude" => Integration::Claude,
            _ => unreachable!("integrations were validated"),
        })
        .collect();
    add_legacy_conflicts(&mut plan, project);
    plan_integrations(&mut plan, project, &integrations, refresh);
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

pub(crate) fn add_legacy_conflicts(plan: &mut Plan, project: &Project) {
    for path in [".pi/skills/doco", ".codex/skills/doco"] {
        if project.path(path).symlink_metadata().is_ok() {
            plan.conflicts.push(format!(
                "{path}: legacy or alternative doco skill detected; explicitly migrate it to .agents/skills/doco before retrying"
            ));
        }
    }
}

pub(crate) fn plan_integrations(
    plan: &mut Plan,
    project: &Project,
    integrations: &[Integration],
    refresh: bool,
) {
    let most = integrations.contains(&Integration::Most);
    let claude = integrations.contains(&Integration::Claude);

    for integration in Integration::ALL {
        if integrations.contains(&integration) {
            skill::install(plan, project, integration.skill_dir(), refresh);
        }
    }

    if most {
        plan.file(project, Integration::Most.entry_file(), |old| {
            entry::update(
                old,
                Integration::Most.entry_title(),
                Integration::Most.skill_dir(),
                refresh,
            )
        });
        if project
            .path("AGENTS.override.md")
            .symlink_metadata()
            .is_ok()
        {
            plan.warnings.push("AGENTS.override.md may shadow AGENTS.md: files can be installed but the default entry may not take effect; override is not modified".into());
        }
    }

    let imports_agents = match plan.effective_text(project, "CLAUDE.md") {
        Ok(Some(text)) => match entry::imports_agents(&text) {
            Ok(imports) => Some(imports),
            Err(error) => {
                plan.conflicts.push(format!("CLAUDE.md import: {error:#}"));
                None
            }
        },
        Ok(None) => Some(false),
        Err(error) => {
            plan.conflicts.push(format!("CLAUDE.md import: {error:#}"));
            None
        }
    };

    if claude {
        match imports_agents {
            Some(true) => plan.conflicts.push(
                "CLAUDE.md imports AGENTS.md; remove the import before installing a dedicated Claude integration"
                    .into(),
            ),
            Some(false) => plan.file(project, Integration::Claude.entry_file(), |old| {
                entry::update(
                    old,
                    Integration::Claude.entry_title(),
                    Integration::Claude.skill_dir(),
                    refresh,
                )
            }),
            None => {}
        }
    } else if most && imports_agents == Some(true) {
        if let Err(error) = validate_reused_claude_import(plan, project) {
            plan.conflicts.push(format!("CLAUDE.md import: {error:#}"));
        } else {
            plan.warnings.push("Claude reuses the Most agents integration through @AGENTS.md; no Claude files changed".into());
        }
    }
}

fn validate_reused_claude_import(plan: &mut Plan, project: &Project) -> Result<()> {
    let claude_text = plan
        .effective_text(project, "CLAUDE.md")?
        .ok_or_else(|| anyhow::anyhow!("missing CLAUDE.md"))?;
    let agents_text = plan
        .effective_text(project, "AGENTS.md")?
        .ok_or_else(|| anyhow::anyhow!("CLAUDE.md imports missing AGENTS.md"))?;
    if entry::imports_agents(&agents_text)? {
        bail!("recursive AGENTS.md import");
    }
    let Some(skill_path) = entry::visible_skill(&agents_text)? else {
        bail!("imported AGENTS.md has no recognized doco navigation");
    };
    if entry::block_range(&claude_text)?.is_some() {
        bail!(
            "CLAUDE.md and imported AGENTS.md both contain doco blocks; manual deduplication required"
        );
    }
    for (file, generated) in templates::FILES {
        let full = format!("{skill_path}/{file}");
        let content = plan
            .effective_text(project, &full)?
            .ok_or_else(|| anyhow::anyhow!("imported navigation references missing {full}"))?;
        if crate::markdown::normalize(&content) != crate::markdown::normalize(generated) {
            bail!("imported skill {full} is not a verified compatible bundle");
        }
    }
    if entry::visible_skill(&claude_text)?.is_some() {
        bail!("duplicate Claude workflow");
    }
    Ok(())
}
