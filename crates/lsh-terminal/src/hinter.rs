use lsh_input::{Hinter, History};

use crate::editor::LshCompleter;

pub struct LshHinter {
    completer: LshCompleter,
    history: Option<Box<dyn History>>,
    min_chars: usize,
}

impl LshHinter {
    pub fn new(completer: LshCompleter) -> Self {
        Self {
            completer,
            history: None,
            min_chars: 1,
        }
    }

    pub fn with_history(mut self, history: impl History + 'static) -> Self {
        self.history = Some(Box::new(history));
        self
    }

    pub fn with_min_chars(mut self, min_chars: usize) -> Self {
        self.min_chars = min_chars;
        self
    }
}

impl Hinter for LshHinter {
    fn hint(&self, line: &str, cursor: usize) -> Option<String> {
        if line.chars().count() < self.min_chars || cursor != line.len() {
            return None;
        }

        // 1. Check history for ghost text first (highest relevance)
        if let Some(history) = &self.history {
            for i in (0..history.len()).rev() {
                if let Some(entry) = history.get(i)
                    && entry.starts_with(line)
                    && entry.len() > line.len()
                {
                    return Some(entry[line.len()..].to_string());
                }
            }
        }

        // 2. Fallback: single / top completion candidate
        if !line.is_empty() {
            let (start, items) = self.completer.get_candidates(line, cursor);
            if !items.is_empty() {
                let replacement = &items[0].replacement;
                let typed_prefix = &line[start..cursor];
                if !typed_prefix.is_empty()
                    && replacement.starts_with(typed_prefix)
                    && replacement.len() > typed_prefix.len()
                {
                    return Some(replacement[typed_prefix.len()..].to_string());
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsh_input::DefaultHistory;

    #[test]
    fn hints_from_history() {
        let completer = LshCompleter::new();
        let mut history = DefaultHistory::new(10);
        history.append("git commit -m 'initial'");

        let hinter = LshHinter::new(completer).with_history(history);
        let hint = hinter.hint("git com", 7);
        assert_eq!(hint.as_deref(), Some("mit -m 'initial'"));
    }

    #[test]
    fn hints_from_completion_fallback() {
        let completer = LshCompleter::new();
        completer.set_binaries(vec!["cargo".to_string()]);

        let hinter = LshHinter::new(completer);
        let hint = hinter.hint("car", 3);
        assert_eq!(hint.as_deref(), Some("go"));
    }
}
