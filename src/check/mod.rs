pub mod tasks;
use crate::{
    Change, PackageMode, Project, State, lifecycle_times, markdown,
    package::work_specs,
    package_mode, safety,
    ui::{self, Reporter, Tone},
};
use anyhow::{Result, bail};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

#[derive(Default)]
pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
impl Report {
    pub fn show(&self) {
        let mut reporter = ui::PlainReporter::default();
        let _ = self.show_with_ui(&mut reporter);
    }
    pub fn show_with_ui(&self, reporter: &mut dyn Reporter) -> Result<()> {
        for issue in &self.errors {
            ui::line(reporter, Tone::Danger, "ERROR", issue)?;
        }
        for issue in &self.warnings {
            ui::line(reporter, Tone::Warning, "WARNING", issue)?;
        }
        Ok(())
    }
    pub fn ensure(&self) -> Result<()> {
        if !self.errors.is_empty() {
            bail!("{} mechanical check(s) failed", self.errors.len());
        }
        Ok(())
    }
}
pub fn placeholders(text: &str) -> bool {
    regex::Regex::new(r"(?i)\bTODO\b|\bTBD\b|\{\{[^}]+\}\}|<change-id>|<变更标题>|待填写|待补充")
        .unwrap()
        .is_match(text)
}
pub fn pending(text: &str) -> bool {
    regex::Regex::new(r"(?im)^\s*(?:(?:status|状态)\s*[:：]\s*)?(?:pending\b|not completed\b|not complete\b|尚未完成|未完成|尚未交付)")
        .unwrap()
        .is_match(text)
}

fn required(text: &str, aliases: &[&[&str]], file: &str, report: &mut Report) {
    for names in aliases {
        let sections: Vec<_> = markdown::sections(text)
            .into_iter()
            .filter(|s| markdown::title_matches(&s.title, names))
            .collect();
        if sections.len() != 1 {
            report
                .errors
                .push(format!("{file}: expected one ## {} section", names[0]));
        } else if sections[0].content.is_empty()
            || markdown::content_lines(&sections[0].content)
                .iter()
                .all(|(_, l)| l.trim().is_empty())
        {
            report
                .errors
                .push(format!("{file}: empty {} section", names[0]));
        }
    }
    if placeholders(text) {
        report.errors.push(format!(
            "{file}: unresolved template markers (TODO/TBD/placeholders)"
        ));
    }
}
pub fn proposal(text: &str, final_result: bool) -> Report {
    let mut report = Report::default();
    required(
        text,
        &[
            &["Purpose", "目的"],
            &[
                "Scope and acceptance",
                "Scope and completion criteria",
                "范围与完成标准",
            ],
            &["Result", "Results", "Outcome", "结果"],
        ],
        "proposal.md",
        &mut report,
    );
    if final_result && markdown::result_section(text).is_some_and(|s| pending(&s.content)) {
        report.errors.push(
            "proposal.md: Result is still pending; record actual delivery and verification".into(),
        );
    }
    report
}

fn links(project: &Project, change: &Change, name: &str, text: &str, report: &mut Report) {
    let file = change.path.join(name);
    for target in markdown::links(text) {
        if let Some(id) = target.strip_prefix("doco:") {
            if let Err(error) = project.resolve(id.split('#').next().unwrap_or(id)) {
                report.errors.push(format!(
                    "{name}: invalid stable reference {target}: {error}"
                ));
            }
        } else if let Some(path) = markdown::local_link(file.parent().unwrap(), &target) {
            if !path.starts_with(project.root()) {
                report
                    .errors
                    .push(format!("{name}: link outside project: {target}"));
            } else if !path.exists() {
                report.warnings.push(format!("{name}: missing local reference {target}; confirm whether it is a planned source path"));
            }
        }
    }
}

// A heading, whitespace or hidden comment alone is not a specification body.
fn spec_has_body(text: &str) -> bool {
    let mut in_heading = false;
    for event in Parser::new(text.trim_start_matches('\u{feff}')) {
        match event {
            Event::Start(Tag::Heading { .. }) => in_heading = true,
            Event::End(TagEnd::Heading(_)) => in_heading = false,
            Event::Text(text) | Event::Code(text) if !in_heading && !text.trim().is_empty() => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn add_blockers(report: &mut Report, blockers: Vec<String>, strict: bool) {
    for blocker in blockers {
        let issue = format!("unresolved blocker: {blocker}");
        if strict {
            report.errors.push(issue);
        } else {
            report.warnings.push(issue);
        }
    }
}

pub fn inspect(project: &Project, change: &Change, completing: bool) -> Result<Report> {
    safety::tree(project.root(), &change.path)?;
    let proposal_text = safety::text(project.root(), &change.path.join("proposal.md"))?;
    let strict = completing || change.state == State::Completed;
    let mut report = proposal(&proposal_text, completing || change.state != State::Active);
    let mode = match package_mode(&proposal_text) {
        Ok(mode) => Some(mode),
        Err(error) => {
            report.errors.push(error.to_string());
            None
        }
    };
    if let Err(error) = lifecycle_times(&proposal_text) {
        report.errors.push(error.to_string());
    }
    links(project, change, "proposal.md", &proposal_text, &mut report);
    if change.state == State::Archived {
        for entry in std::fs::read_dir(&change.path)? {
            let entry = entry?;
            if entry.file_name() != "proposal.md" {
                report.errors.push(format!(
                    "archived change contains unexpected {}",
                    entry.path().display()
                ));
            }
        }
        return Ok(report);
    }
    let Some(mode) = mode else {
        return Ok(report);
    };
    if mode == PackageMode::ProposalOnly {
        let work = change.path.join("work");
        safety::inspect(project.root(), &work)?;
        if work.try_exists()? {
            report
                .errors
                .push("proposal-only change must not contain work/".into());
        }
        add_blockers(&mut report, tasks::blockers(&proposal_text), strict);
        if strict && !tasks::verification(&proposal_text) {
            report
                .errors
                .push("proposal.md: missing Verification/验证记录 evidence".into());
        }
        return Ok(report);
    }

    let implement = safety::text(project.root(), &change.path.join("work/implement.md"))?;
    let tasks_text = safety::text(project.root(), &change.path.join("work/tasks.md"))?;
    required(
        &implement,
        &[
            &["Baseline and goals", "现状、目标与设计基线"],
            &["Overall approach", "整体方案"],
            &["APIs and data model", "关键 API 与数据模型"],
            &["Algorithms and rules", "核心算法与实现规则"],
            &["Fixed decisions and discretion", "固定决策与可自主调整范围"],
            &[
                "Verification and documentation impact",
                "验证与长期文档影响",
            ],
        ],
        "work/implement.md",
        &mut report,
    );
    if placeholders(&tasks_text) {
        report
            .errors
            .push("work/tasks.md: unresolved template markers".into());
    }
    let parsed = tasks::parse(&tasks_text);
    report.errors.extend(parsed.errors);
    let unfinished: Vec<_> = parsed
        .items
        .iter()
        .filter(|t| !t.done)
        .map(|t| t.id.as_str())
        .collect();
    if !unfinished.is_empty() {
        let issue = format!("unfinished tasks: {}", unfinished.join(", "));
        if strict {
            report.errors.push(issue);
        } else {
            report.warnings.push(issue);
        }
    }
    let mut blockers = parsed.blockers;
    blockers.extend(tasks::blockers(&implement));
    blockers.extend(tasks::blockers(&proposal_text));
    add_blockers(&mut report, blockers, strict);
    if strict && !parsed.verification {
        report
            .errors
            .push("work/tasks.md: missing Verification/验证记录 evidence".into());
    }
    links(
        project,
        change,
        "work/implement.md",
        &implement,
        &mut report,
    );
    links(project, change, "work/tasks.md", &tasks_text, &mut report);
    for path in work_specs(project, change)? {
        let name = path
            .strip_prefix(&change.path)?
            .to_string_lossy()
            .replace('\\', "/");
        let text = safety::text(project.root(), &path)?;
        if !spec_has_body(&text) {
            report
                .errors
                .push(format!("{name}: empty specification body"));
        }
        if placeholders(&text) {
            report
                .errors
                .push(format!("{name}: unresolved template markers"));
        }
        links(project, change, &name, &text, &mut report);
        add_blockers(
            &mut report,
            tasks::blockers(&text)
                .into_iter()
                .map(|blocker| format!("{name}: {blocker}"))
                .collect(),
            strict,
        );
    }
    Ok(report)
}
