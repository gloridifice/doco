use crate::State;
use anyhow::Result;
use std::{
    fmt,
    io::{self, Write},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Info,
    Success,
    Warning,
    Danger,
    Muted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifiedAge {
    Known(std::time::Duration),
    Unknown,
}

impl fmt::Display for ModifiedAge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::Known(duration) = self else {
            return f.write_str("?");
        };
        let seconds = duration.as_secs();
        if seconds < 60 * 60 {
            write!(f, "{}m", seconds / 60)
        } else if seconds < 24 * 60 * 60 {
            write!(f, "{}h", seconds / (60 * 60))
        } else {
            let days = seconds / (24 * 60 * 60);
            let hours = (seconds % (24 * 60 * 60)) / (60 * 60);
            write!(f, "{days}d")?;
            if hours > 0 {
                write!(f, "{hours}h")?;
            }
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskCount {
    Known { done: usize, total: usize },
    Missing,
    NotApplicable,
    Archived,
}

impl fmt::Display for TaskCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known { done, total } => write!(f, "{done}/{total}"),
            Self::Missing => f.write_str("?"),
            Self::NotApplicable | Self::Archived => f.write_str("-"),
        }
    }
}

#[derive(Debug)]
pub struct ChangeRow {
    pub id: String,
    pub state: State,
    pub tasks: TaskCount,
    pub updated: ModifiedAge,
}

pub enum Event<'a> {
    Line {
        stream: Stream,
        tone: Tone,
        label: &'a str,
        body: &'a str,
    },
    Text {
        stream: Stream,
        text: &'a str,
    },
    Diff {
        text: &'a str,
    },
    Changes {
        changes: &'a [ChangeRow],
    },
}

pub trait Reporter {
    fn emit(&mut self, event: Event<'_>) -> io::Result<()>;
    fn flush(&mut self, stream: Stream) -> io::Result<()>;
}

impl<R: Reporter + ?Sized> Reporter for &mut R {
    fn emit(&mut self, event: Event<'_>) -> io::Result<()> {
        (**self).emit(event)
    }

    fn flush(&mut self, stream: Stream) -> io::Result<()> {
        (**self).flush(stream)
    }
}

#[derive(Debug)]
pub struct UserCancelled;

impl fmt::Display for UserCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for UserCancelled {}

pub fn cancelled<T>() -> Result<T> {
    Err(UserCancelled.into())
}

pub fn sanitize(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\n' | '\t' | '\r' => output.push(character),
            '\u{00}'..='\u{08}' | '\u{0b}'..='\u{0c}' | '\u{0e}'..='\u{1f}' | '\u{7f}' => {
                use fmt::Write as _;
                let _ = write!(output, "\\u{{{:02x}}}", character as u32);
            }
            '\u{80}'..='\u{9f}' => {
                use fmt::Write as _;
                let _ = write!(output, "\\u{{{:04x}}}", character as u32);
            }
            _ => output.push(character),
        }
    }
    output
}

pub fn sanitize_line(text: &str) -> String {
    sanitize(text)
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

pub struct PlainReporter<O = io::Stdout, E = io::Stderr> {
    stdout: O,
    stderr: E,
}

impl Default for PlainReporter {
    fn default() -> Self {
        Self {
            stdout: io::stdout(),
            stderr: io::stderr(),
        }
    }
}

impl<O: Write, E: Write> PlainReporter<O, E> {
    pub fn new(stdout: O, stderr: E) -> Self {
        Self { stdout, stderr }
    }

    fn writer(&mut self, stream: Stream) -> &mut dyn Write {
        match stream {
            Stream::Stdout => &mut self.stdout,
            Stream::Stderr => &mut self.stderr,
        }
    }
}

impl<O: Write, E: Write> Reporter for PlainReporter<O, E> {
    fn emit(&mut self, event: Event<'_>) -> io::Result<()> {
        match event {
            Event::Line {
                stream,
                label,
                body,
                ..
            } => {
                let label = sanitize_line(label);
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
            Event::Diff { text } => write!(self.stdout, "{}", sanitize(text)),
            Event::Changes { changes } => {
                for change in changes {
                    writeln!(
                        self.stdout,
                        "{}\t{}\t{}\t{}",
                        change.state,
                        sanitize_line(&change.id),
                        change.tasks,
                        change.updated
                    )?;
                }
                Ok(())
            }
        }
    }

    fn flush(&mut self, stream: Stream) -> io::Result<()> {
        self.writer(stream).flush()
    }
}

pub fn line(reporter: &mut dyn Reporter, tone: Tone, label: &str, body: &str) -> Result<()> {
    reporter
        .emit(Event::Line {
            stream: Stream::Stdout,
            tone,
            label,
            body,
        })
        .map_err(Into::into)
}

pub fn text(reporter: &mut dyn Reporter, text: &str) -> Result<()> {
    reporter
        .emit(Event::Text {
            stream: Stream::Stdout,
            text,
        })
        .map_err(Into::into)
}

/// Non-fatal diagnostic on stderr; it never changes the command outcome.
pub(crate) fn warning(reporter: &mut dyn Reporter, body: &str) -> Result<()> {
    reporter
        .emit(Event::Line {
            stream: Stream::Stderr,
            tone: Tone::Warning,
            label: "WARNING",
            body,
        })
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modified_age_uses_compact_whole_units() {
        use std::time::Duration;

        for (seconds, expected) in [
            (0, "0m"),
            (59, "0m"),
            (10 * 60, "10m"),
            (60 * 60 - 1, "59m"),
            (60 * 60, "1h"),
            (20 * 60 * 60 + 59 * 60, "20h"),
            (24 * 60 * 60, "1d"),
            (26 * 60 * 60, "1d2h"),
            ((20 * 24 + 5) * 60 * 60, "20d5h"),
        ] {
            assert_eq!(
                ModifiedAge::Known(Duration::from_secs(seconds)).to_string(),
                expected
            );
        }
        assert_eq!(ModifiedAge::Unknown.to_string(), "?");
    }

    #[test]
    fn plain_reporter_preserves_lines_and_escapes_controls() {
        let mut reporter = PlainReporter::new(Vec::new(), Vec::new());
        reporter
            .emit(Event::Line {
                stream: Stream::Stdout,
                tone: Tone::Danger,
                label: "ERROR",
                body: "bad\u{1b}]title\u{7} done",
            })
            .unwrap();
        assert_eq!(
            String::from_utf8(reporter.stdout).unwrap(),
            "ERROR bad\\u{1b}]title\\u{07} done\n"
        );
    }
}
