mod output;
mod prompt;

pub use output::TerminalReporter;
pub use prompt::TerminalPrompter;

#[derive(Debug, PartialEq, Eq)]
pub enum PromptOutcome<T> {
    Answer(T),
    Cancelled,
}

use clap::ValueEnum;
use std::io::{self, IsTerminal};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InteractionMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub stdin_tty: bool,
    pub stdout_tty: bool,
    pub stderr_tty: bool,
    pub term_dumb: bool,
    pub no_color: bool,
    pub columns: usize,
    pub rows: usize,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let term_dumb = std::env::var_os("TERM")
            .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("dumb"));
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        let (rows, columns) = console::Term::stdout().size();
        Self {
            stdin_tty: io::stdin().is_terminal(),
            stdout_tty: io::stdout().is_terminal(),
            stderr_tty: io::stderr().is_terminal(),
            term_dumb,
            no_color,
            columns: usize::from(columns),
            rows: usize::from(rows),
        }
    }

    pub fn color(&self, mode: ColorMode, stream_tty: bool) -> bool {
        match mode {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => stream_tty && !self.no_color && !self.term_dumb,
        }
    }

    pub fn interactive(&self) -> bool {
        self.stdin_tty && self.stdout_tty && self.stderr_tty && !self.term_dumb
    }

    pub fn rich_list(&self) -> bool {
        self.stdout_tty && !self.term_dumb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> TerminalCapabilities {
        TerminalCapabilities {
            stdin_tty: true,
            stdout_tty: true,
            stderr_tty: false,
            term_dumb: false,
            no_color: false,
            columns: 80,
            rows: 24,
        }
    }

    #[test]
    fn color_is_per_stream_and_explicit_modes_override_environment() {
        let mut value = caps();
        assert!(value.color(ColorMode::Auto, value.stdout_tty));
        assert!(!value.color(ColorMode::Auto, value.stderr_tty));
        value.no_color = true;
        assert!(!value.color(ColorMode::Auto, true));
        assert!(value.color(ColorMode::Always, false));
        assert!(!value.color(ColorMode::Never, true));
        value.no_color = false;
        value.term_dumb = true;
        assert!(!value.color(ColorMode::Auto, true));
        assert!(value.color(ColorMode::Always, false));
    }

    #[test]
    fn interaction_and_layout_are_independent_of_color() {
        let mut value = caps();
        assert!(!value.interactive());
        assert!(value.rich_list());
        value.stderr_tty = true;
        assert!(value.interactive());
        value.term_dumb = true;
        assert!(!value.interactive());
        assert!(!value.rich_list());
    }
}
