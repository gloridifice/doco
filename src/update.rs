//! Refresh existing project-local integrations without creating missing targets.
use crate::{
    Project, init,
    init::{Integration, plan::Plan},
    safety,
    ui::{self, Reporter},
};
use anyhow::Result;
use std::{fs, io::ErrorKind};

pub fn run(project: &Project, dry_run: bool, refresh: bool) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    run_with_ui(project, dry_run, refresh, &mut reporter)
}

pub fn run_with_ui(
    project: &Project,
    dry_run: bool,
    refresh: bool,
    reporter: &mut dyn Reporter,
) -> Result<()> {
    project.initialized()?;
    let plan = build(project, refresh);
    plan.show(project, reporter)?;
    plan.ensure_valid()?;
    if dry_run {
        ui::text(reporter, "Dry run: no files written.\n")?;
        return Ok(());
    }
    plan.apply(project, reporter)?;
    ui::text(
        reporter,
        "Disk integration update complete. Agent loading is NOT verified; check project trust and discovery/context settings in your agent.\n",
    )?;
    Ok(())
}

fn build(project: &Project, refresh: bool) -> Plan {
    let mut plan = Plan::default();
    init::add_legacy_conflicts(&mut plan, project);
    let mut installed = Vec::new();
    let mut found_any = false;

    for integration in Integration::ALL {
        let skill_file = format!("{}/SKILL.md", integration.skill_dir());
        let skill = node_exists(project, &skill_file, &mut plan);
        let entry = entry_exists(project, integration, &mut plan);
        match (skill, entry) {
            (Some(skill), Some(entry)) => {
                found_any |= skill || entry;
                match (skill, entry) {
                    (true, true) => installed.push(integration),
                    (true, false) => plan.conflicts.push(format!(
                        "{} exists but {} has no managed doco entry; run doco init --agent {}",
                        integration.skill_dir(),
                        integration.entry_file(),
                        integration.id()
                    )),
                    (false, true) => plan.conflicts.push(format!(
                        "{} contains a managed doco entry but {} is missing; run doco init --agent {}",
                        integration.entry_file(),
                        integration.skill_dir(),
                        integration.id()
                    )),
                    (false, false) => {}
                }
            }
            _ => found_any = true,
        }
    }

    if !found_any {
        plan.conflicts
            .push("no installed doco integration found; run doco init --agent most|claude".into());
    }
    init::plan_integrations(&mut plan, project, &installed, refresh);
    plan
}

fn node_exists(project: &Project, relative: &str, plan: &mut Plan) -> Option<bool> {
    let path = project.path(relative);
    if let Err(error) = safety::inspect(project.root(), &path) {
        plan.conflicts.push(format!("{relative}: {error:#}"));
        return None;
    }
    match fs::symlink_metadata(&path) {
        Ok(_) => Some(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Some(false),
        Err(error) => {
            plan.conflicts.push(format!(
                "{relative}: {:#}",
                anyhow::Error::new(error).context("inspect integration target")
            ));
            None
        }
    }
}

fn entry_exists(project: &Project, integration: Integration, plan: &mut Plan) -> Option<bool> {
    let relative = integration.entry_file();
    if !node_exists(project, relative, plan)? {
        return Some(false);
    }
    match safety::text(project.root(), &project.path(relative))
        .and_then(|text| init::entry::installed(&text, integration.skill_dir()))
    {
        Ok(installed) => Some(installed),
        Err(error) => {
            plan.conflicts.push(format!("{relative}: {error:#}"));
            None
        }
    }
}
