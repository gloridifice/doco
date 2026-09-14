//! Rebuild the disposable local change index cache on demand.
use crate::{
    Project, index,
    ui::{self, Reporter, Tone},
};
use anyhow::Result;

pub fn run(project: &Project, dry_run: bool) -> Result<()> {
    let mut reporter = ui::PlainReporter::default();
    run_with_ui(project, dry_run, &mut reporter)
}

pub fn run_with_ui(project: &Project, dry_run: bool, reporter: &mut dyn Reporter) -> Result<()> {
    project.initialized()?;
    if dry_run {
        let entries = index::scan(project)?;
        ui::line(
            reporter,
            Tone::Info,
            "INDEX",
            &format!("Would rebuild cache: {}.", index::counts(&entries)),
        )?;
        ui::text(reporter, "Dry run: no files written.\n")?;
        return Ok(());
    }
    index::rebuild_with_ui(project, reporter)
}
