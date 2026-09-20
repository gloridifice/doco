use super::{ColorMode, TerminalCapabilities};
use console::Style;
use doco::{
    State,
    ui::{ChangeRow, Event, Reporter, Stream, Tone, sanitize, sanitize_line},
};
use std::io::{self, Write};

pub struct TerminalReporter<O = io::Stdout, E = io::Stderr> {
    stdout: O,
    stderr: E,
    stdout_color: bool,
    stderr_color: bool,
    rich_list: bool,
    columns: usize,
}

impl TerminalReporter {
    pub fn stdio(mode: ColorMode, capabilities: &TerminalCapabilities) -> Self {
        Self::new(io::stdout(), io::stderr(), mode, capabilities)
    }
}

impl<O: Write, E: Write> TerminalReporter<O, E> {
    pub fn new(stdout: O, stderr: E, mode: ColorMode, capabilities: &TerminalCapabilities) -> Self {
        Self {
            stdout,
            stderr,
            stdout_color: capabilities.color(mode, capabilities.stdout_tty),
            stderr_color: capabilities.color(mode, capabilities.stderr_tty),
            rich_list: capabilities.rich_list(),
            columns: capabilities.columns,
        }
    }

    #[cfg(test)]
    pub fn into_inner(self) -> (O, E) {
        (self.stdout, self.stderr)
    }

    fn writer(&mut self, stream: Stream) -> &mut dyn Write {
        match stream {
            Stream::Stdout => &mut self.stdout,
            Stream::Stderr => &mut self.stderr,
        }
    }

    fn color(&self, stream: Stream) -> bool {
        match stream {
            Stream::Stdout => self.stdout_color,
            Stream::Stderr => self.stderr_color,
        }
    }

    fn styled(text: &str, tone: Tone, color: bool) -> String {
        let style = match tone {
            Tone::Plain => Style::new(),
            Tone::Info => Style::new().cyan(),
            Tone::Success => Style::new().green(),
            Tone::Warning => Style::new().yellow(),
            Tone::Danger => Style::new().red(),
            Tone::Muted => Style::new().dim(),
        }
        .force_styling(color);
        style.apply_to(text).to_string()
    }

    fn state(change: &ChangeRow) -> Tone {
        match change.state {
            State::Active => Tone::Info,
            State::Completed => Tone::Success,
            State::Archived => Tone::Muted,
        }
    }

    fn render_changes(&mut self, changes: &[ChangeRow]) -> io::Result<()> {
        if !self.rich_list {
            for change in changes {
                writeln!(
                    self.stdout,
                    "{}\t{}\t{}\t{}",
                    Self::styled(
                        change.state.as_str(),
                        Self::state(change),
                        self.stdout_color
                    ),
                    sanitize_line(&change.id),
                    change.tasks,
                    change.age
                )?;
            }
            return Ok(());
        }
        if changes.is_empty() {
            return writeln!(
                self.stdout,
                "No changes found. Create one with: doco new <id>"
            );
        }
        let id_width = changes.iter().map(|c| c.id.len()).max().unwrap_or(0).max(6);
        let tasks_width = changes
            .iter()
            .map(|c| c.tasks.to_string().len())
            .max()
            .unwrap_or(0)
            .max(5);
        let heading = Style::new().bold().force_styling(self.stdout_color);
        if self.columns >= 50 {
            writeln!(
                self.stdout,
                "{}  {}  {}  {}",
                heading.apply_to(format!("{:<9}", "STATE")),
                heading.apply_to(format!("{:<id_width$}", "CHANGE")),
                heading.apply_to(format!("{:<tasks_width$}", "TASKS")),
                heading.apply_to("AGE")
            )?;
            for change in changes {
                let state = format!("{:<9}", change.state.as_str());
                let tasks = change.tasks.to_string();
                writeln!(
                    self.stdout,
                    "{}  {:<id_width$}  {:<tasks_width$}  {}",
                    Self::styled(&state, Self::state(change), self.stdout_color),
                    sanitize_line(&change.id),
                    tasks,
                    change.age
                )?;
            }
        } else {
            for change in changes {
                writeln!(
                    self.stdout,
                    "{}  {}  {}  {}",
                    Self::styled(
                        change.state.as_str(),
                        Self::state(change),
                        self.stdout_color
                    ),
                    sanitize_line(&change.id),
                    change.tasks,
                    change.age
                )?;
            }
        }
        let active = changes.iter().filter(|c| c.state == State::Active).count();
        let completed = changes
            .iter()
            .filter(|c| c.state == State::Completed)
            .count();
        let archived = changes.len() - active - completed;
        writeln!(self.stdout)?;
        writeln!(
            self.stdout,
            "{} changes ({} active, {} completed, {} archived)",
            changes.len(),
            active,
            completed,
            archived
        )
    }

    fn render_diff(&mut self, text: &str) -> io::Result<()> {
        for part in sanitize(text).split_inclusive('\n') {
            let plain = part.trim_end_matches('\n');
            let tone = if plain.starts_with("@@") {
                Tone::Info
            } else if plain.starts_with('+') && !plain.starts_with("+++") {
                Tone::Success
            } else if plain.starts_with('-') && !plain.starts_with("---") {
                Tone::Danger
            } else {
                Tone::Plain
            };
            let suffix = if part.ends_with('\n') { "\n" } else { "" };
            write!(
                self.stdout,
                "{}{}",
                Self::styled(plain, tone, self.stdout_color),
                suffix
            )?;
        }
        Ok(())
    }
}

impl<O: Write, E: Write> Reporter for TerminalReporter<O, E> {
    fn emit(&mut self, event: Event<'_>) -> io::Result<()> {
        match event {
            Event::Line {
                stream,
                tone,
                label,
                body,
            } => {
                let color = self.color(stream);
                let label = Self::styled(&sanitize_line(label), tone, color);
                let body = sanitize(body);
                if label.is_empty() {
                    writeln!(self.writer(stream), "{body}")
                } else if body.is_empty() {
                    writeln!(self.writer(stream), "{label}")
                } else {
                    writeln!(self.writer(stream), "{label} {body}")
                }
            }
            Event::Text { stream, text } => write!(self.writer(stream), "{}", sanitize(text)),
            Event::Diff { text } => self.render_diff(text),
            Event::Changes { changes } => self.render_changes(changes),
        }
    }

    fn flush(&mut self, stream: Stream) -> io::Result<()> {
        self.writer(stream).flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doco::ui::Event;
    use doco::ui::{LifecycleAge, TaskCount};
    use std::time::Duration;

    fn caps(tty: bool, columns: usize) -> TerminalCapabilities {
        TerminalCapabilities {
            stdin_tty: tty,
            stdout_tty: tty,
            stderr_tty: tty,
            term_dumb: false,
            no_color: false,
            columns,
            rows: 24,
        }
    }

    fn changes() -> Vec<ChangeRow> {
        vec![
            ChangeRow {
                id: "alpha".into(),
                state: State::Active,
                tasks: TaskCount::Known { done: 1, total: 3 },
                age: LifecycleAge::Known(Duration::from_secs(10 * 60)),
            },
            ChangeRow {
                id: "done".into(),
                state: State::Completed,
                tasks: TaskCount::Known { done: 2, total: 2 },
                age: LifecycleAge::Known(Duration::from_secs(26 * 60 * 60)),
            },
        ]
    }

    #[test]
    fn redirected_list_remains_tsv_even_when_color_is_forced() {
        let mut reporter =
            TerminalReporter::new(Vec::new(), Vec::new(), ColorMode::Always, &caps(false, 80));
        reporter
            .emit(Event::Changes {
                changes: &changes(),
            })
            .unwrap();
        let (stdout, _) = reporter.into_inner();
        let text = String::from_utf8(stdout).unwrap();
        assert!(text.contains("\u{1b}["));
        assert!(!text.contains("STATE"));
        assert!(text.contains("\talpha\t1/3\t10m"));
    }

    #[test]
    fn tty_list_has_heading_summary_and_narrow_fallback() {
        let mut reporter =
            TerminalReporter::new(Vec::new(), Vec::new(), ColorMode::Never, &caps(true, 80));
        reporter
            .emit(Event::Changes {
                changes: &changes(),
            })
            .unwrap();
        let (stdout, _) = reporter.into_inner();
        let text = String::from_utf8(stdout).unwrap();
        assert!(text.starts_with("STATE      CHANGE  TASKS  AGE\n"));
        assert!(text.contains("active     alpha   1/3    10m\n"), "{text:?}");
        assert!(
            text.contains("completed  done    2/2    1d2h\n"),
            "{text:?}"
        );
        assert!(text.contains("2 changes (1 active, 1 completed, 0 archived)"));
        let mut narrow =
            TerminalReporter::new(Vec::new(), Vec::new(), ColorMode::Never, &caps(true, 30));
        narrow
            .emit(Event::Changes {
                changes: &changes(),
            })
            .unwrap();
        let (stdout, _) = narrow.into_inner();
        let text = String::from_utf8(stdout).unwrap();
        assert!(!text.contains("STATE"));
        assert!(text.contains("active  alpha  1/3  10m\n"));
    }

    #[test]
    fn empty_tty_list_has_guidance_but_redirected_list_is_empty() {
        let mut rich =
            TerminalReporter::new(Vec::new(), Vec::new(), ColorMode::Never, &caps(true, 80));
        rich.emit(Event::Changes { changes: &[] }).unwrap();
        assert!(
            String::from_utf8(rich.into_inner().0)
                .unwrap()
                .contains("No changes found")
        );
        let mut plain =
            TerminalReporter::new(Vec::new(), Vec::new(), ColorMode::Never, &caps(false, 80));
        plain.emit(Event::Changes { changes: &[] }).unwrap();
        assert!(plain.into_inner().0.is_empty());
    }

    #[test]
    fn diff_colors_only_changed_lines() {
        let mut reporter =
            TerminalReporter::new(Vec::new(), Vec::new(), ColorMode::Always, &caps(false, 80));
        reporter
            .emit(Event::Diff {
                text: "--- existing\n+++ planned\n@@ x @@\n-old\n+new\n",
            })
            .unwrap();
        let text = String::from_utf8(reporter.into_inner().0).unwrap();
        assert!(text.starts_with("--- existing\n+++ planned\n"));
        assert!(text.contains("\u{1b}[36m@@ x @@"));
        assert!(text.contains("\u{1b}[31m-old"));
        assert!(text.contains("\u{1b}[32m+new"));
    }
}
