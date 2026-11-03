use nu_ansi_term::{Color, Style};
use reedline::{Hinter, History, SearchQuery};

use crate::editor::LshCompleter;

pub struct LshHinter {
    completer: LshCompleter,
    style: Style,
    current_hint: String,
    min_chars: usize,
}

impl LshHinter {
    pub fn new(completer: LshCompleter) -> Self {
        Self {
            completer,
            style: Style::new().fg(Color::Fixed(242)),
            current_hint: String::new(),
            min_chars: 1,
        }
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn with_min_chars(mut self, min_chars: usize) -> Self {
        self.min_chars = min_chars;
        self
    }
}

impl Hinter for LshHinter {
    fn handle(
        &mut self,
        line: &str,
        pos: usize,
        history: &dyn History,
        use_ansi_coloring: bool,
        _cwd: &str,
    ) -> String {
        self.current_hint = String::new();

        if line.chars().count() >= self.min_chars {
            // 1. Try history-based ghost text first (highest relevance)
            let history_hint = history
                .search(SearchQuery::last_with_prefix(
                    line.to_string(),
                    history.session(),
                ))
                .unwrap_or_default()
                .first()
                .and_then(|entry| {
                    entry
                        .command_line
                        .get(line.len()..)
                        .filter(|suffix| !suffix.is_empty())
                        .map(|s| s.to_string())
                });

            if let Some(hint) = history_hint {
                self.current_hint = hint;
            } else if pos == line.len() && !line.is_empty() {
                // 2. Fallback: single / top completion candidate when typing at the end of the line
                let (start, items) = self.completer.get_candidates(line, pos);
                if !items.is_empty() {
                    let replacement = &items[0].replacement;
                    let typed_prefix = &line[start..pos];
                    if !typed_prefix.is_empty()
                        && replacement.starts_with(typed_prefix)
                        && replacement.len() > typed_prefix.len()
                    {
                        self.current_hint = replacement[typed_prefix.len()..].to_string();
                    }
                }
            }
        }

        if use_ansi_coloring && !self.current_hint.is_empty() {
            self.style.paint(&self.current_hint).to_string()
        } else {
            self.current_hint.clone()
        }
    }

    fn complete_hint(&self) -> String {
        self.current_hint.clone()
    }

    fn next_hint_token(&self) -> String {
        let mut token = String::new();
        for c in self.current_hint.chars() {
            token.push(c);
            if c.is_whitespace() {
                break;
            }
        }
        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reedline::FileBackedHistory;

    #[test]
    fn hints_from_completion_fallback() {
        let completer = LshCompleter::new();
        completer.set_binaries(vec!["cargo".to_string()]);
        let mut hinter = LshHinter::new(completer);
        let history = FileBackedHistory::default();
        let hint = hinter.handle("car", 3, &history, false, "");
        assert_eq!(hint, "go");
        assert_eq!(hinter.complete_hint(), "go");
        assert_eq!(hinter.next_hint_token(), "go");
    }

    #[test]
    fn hints_styled_with_dim_grey() {
        let completer = LshCompleter::new();
        completer.set_binaries(vec!["cargo".to_string()]);
        let mut hinter = LshHinter::new(completer);
        let history = FileBackedHistory::default();
        let hint = hinter.handle("car", 3, &history, true, "");
        assert_eq!(hint, "\x1b[38;5;242mgo\x1b[0m");
    }
}
