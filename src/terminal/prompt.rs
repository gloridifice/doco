use super::{ColorMode, PromptOutcome, TerminalCapabilities};
use anyhow::{Result, bail};
use console::{Key, Style, Term};
use doco::{Change, ui::sanitize_line};
use std::io;

struct CursorGuard<'a>(&'a Term);

impl Drop for CursorGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.show_cursor();
        let _ = self.0.flush();
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Selection {
    cursor: usize,
    selected: Vec<bool>,
    multiple: bool,
}

enum SelectionStep {
    Continue,
    Cancelled,
    Answer(Vec<usize>),
}

impl Selection {
    fn new(count: usize, multiple: bool) -> Self {
        Self {
            cursor: 0,
            selected: vec![false; count],
            multiple,
        }
    }

    fn key(&mut self, key: Key) -> SelectionStep {
        let count = self.selected.len();
        match key {
            Key::Escape | Key::CtrlC | Key::Char('q') => SelectionStep::Cancelled,
            Key::ArrowDown | Key::Tab | Key::Char('j') => {
                self.cursor = (self.cursor + 1) % count;
                SelectionStep::Continue
            }
            Key::ArrowUp | Key::BackTab | Key::Char('k') => {
                self.cursor = (self.cursor + count - 1) % count;
                SelectionStep::Continue
            }
            Key::Char(' ') if self.multiple => {
                self.selected[self.cursor] = !self.selected[self.cursor];
                SelectionStep::Continue
            }
            Key::Enter => {
                let selected = if self.multiple {
                    self.selected
                        .iter()
                        .enumerate()
                        .filter_map(|(index, selected)| selected.then_some(index))
                        .collect()
                } else {
                    vec![self.cursor]
                };
                SelectionStep::Answer(selected)
            }
            _ => SelectionStep::Continue,
        }
    }
}

pub struct TerminalPrompter {
    term: Term,
    color: bool,
    page_length: usize,
}

impl TerminalPrompter {
    pub fn new(mode: ColorMode, capabilities: &TerminalCapabilities) -> Self {
        Self {
            term: Term::stderr(),
            color: capabilities.color(mode, capabilities.stderr_tty),
            page_length: capabilities.rows.saturating_sub(5).clamp(1, 10),
        }
    }

    pub fn select_agents(&self) -> Result<PromptOutcome<Vec<usize>>> {
        self.choose(
            "Select agents (Space toggles, Enter confirms, Esc/Ctrl-C cancels)",
            &["Codex", "Claude Code", "Pi"],
            true,
        )
    }

    pub fn select_change(&self, changes: &[Change]) -> Result<PromptOutcome<String>> {
        if changes.is_empty() {
            bail!("no changes match the required state");
        }
        let items: Vec<_> = changes
            .iter()
            .map(|change| format!("{:<9}  {}", change.state, sanitize_line(&change.id)))
            .collect();
        Ok(
            match self.choose(
                "Select change (Enter confirms, Esc/Ctrl-C cancels)",
                &items,
                false,
            )? {
                PromptOutcome::Answer(indices) => {
                    PromptOutcome::Answer(changes[indices[0]].id.clone())
                }
                PromptOutcome::Cancelled => PromptOutcome::Cancelled,
            },
        )
    }

    fn choose<S: AsRef<str>>(
        &self,
        prompt: &str,
        items: &[S],
        multiple: bool,
    ) -> Result<PromptOutcome<Vec<usize>>> {
        if items.is_empty() {
            bail!("cannot select from an empty list");
        }
        self.term.hide_cursor()?;
        let _cursor = CursorGuard(&self.term);
        let mut state = Selection::new(items.len(), multiple);
        let mut rendered = 0;
        loop {
            if rendered != 0 {
                self.term.clear_last_lines(rendered)?;
            }
            rendered = self.render(prompt, items, &state)?;
            self.term.flush()?;
            match state.key(self.term.read_key()?) {
                SelectionStep::Continue => {}
                SelectionStep::Cancelled => {
                    self.term.clear_last_lines(rendered)?;
                    return Ok(PromptOutcome::Cancelled);
                }
                SelectionStep::Answer(indices) => {
                    self.term.clear_last_lines(rendered)?;
                    return Ok(PromptOutcome::Answer(indices));
                }
            }
        }
    }

    fn render<S: AsRef<str>>(
        &self,
        prompt: &str,
        items: &[S],
        state: &Selection,
    ) -> io::Result<usize> {
        self.term.write_line(
            &Style::new()
                .bold()
                .force_styling(self.color)
                .apply_to(prompt)
                .to_string(),
        )?;
        let start = (state.cursor / self.page_length) * self.page_length;
        let end = (start + self.page_length).min(items.len());
        for (index, item) in items.iter().enumerate().take(end).skip(start) {
            let prefix = if state.multiple {
                match (state.selected[index], index == state.cursor) {
                    (true, true) => "> [x]",
                    (true, false) => "  [x]",
                    (false, true) => "> [ ]",
                    (false, false) => "  [ ]",
                }
            } else if index == state.cursor {
                ">"
            } else {
                " "
            };
            let prefix = Style::new()
                .cyan()
                .force_styling(self.color)
                .apply_to(prefix)
                .to_string();
            self.term
                .write_line(&format!("{prefix} {}", item.as_ref()))?;
        }
        Ok(1 + end - start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_and_escape_cancel_without_selecting() {
        for key in [Key::CtrlC, Key::Escape, Key::Char('q')] {
            let mut selection = Selection::new(2, true);
            assert!(matches!(selection.key(key), SelectionStep::Cancelled));
            assert_eq!(selection.selected, [false, false]);
        }
    }

    #[test]
    fn arrows_space_and_enter_preserve_multi_selection() {
        let mut selection = Selection::new(3, true);
        assert!(matches!(
            selection.key(Key::Char(' ')),
            SelectionStep::Continue
        ));
        assert!(matches!(
            selection.key(Key::ArrowDown),
            SelectionStep::Continue
        ));
        assert!(matches!(
            selection.key(Key::Char(' ')),
            SelectionStep::Continue
        ));
        assert!(
            matches!(selection.key(Key::Enter), SelectionStep::Answer(indices) if indices == [0, 1])
        );
    }

    #[test]
    fn single_selection_enters_the_highlighted_item() {
        let mut selection = Selection::new(2, false);
        assert!(matches!(
            selection.key(Key::ArrowDown),
            SelectionStep::Continue
        ));
        assert!(
            matches!(selection.key(Key::Enter), SelectionStep::Answer(indices) if indices == [1])
        );
    }
}
