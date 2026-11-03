use std::{
    borrow::Cow,
    io,
    io::{IsTerminal, Write},
    path::Path,
};

pub struct Osc133PromptHighlighter {
    enabled: bool,
}

impl Osc133PromptHighlighter {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn reset_prompt_marks(&self) {}

    pub fn format_prompt<'a>(&self, prompt: &'a str) -> Cow<'a, str> {
        if !self.enabled {
            return Cow::Borrowed(prompt);
        }

        let (leading_newlines, rest) = match prompt.find(|c| c != '\n' && c != '\r') {
            Some(idx) => prompt.split_at(idx),
            None => ("", prompt),
        };

        if rest.contains('\n') {
            let multiline_marked = rest.replace('\n', "\n\x1b]133;A;k=s\x07");
            Cow::Owned(format!(
                "{leading_newlines}\x1b]133;A\x07{multiline_marked}\x1b]133;B\x07"
            ))
        } else {
            Cow::Owned(format!(
                "{leading_newlines}\x1b]133;A\x07{rest}\x1b]133;B\x07"
            ))
        }
    }
}

pub fn enabled() -> bool {
    io::stdout().is_terminal()
}

pub fn write_133(marker: &str) {
    if !enabled() {
        return;
    }

    let mut stdout = io::stdout();
    let _ = write!(stdout, "\x1b]133;{marker}\x07");
    let _ = stdout.flush();
}

pub fn write_7_cwd(path: &Path) {
    if !enabled() {
        return;
    }

    let Some(path) = path.to_str() else {
        return;
    };

    let mut stdout = io::stdout();
    let _ = write!(stdout, "\x1b]7;file://{}\x07", path);
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_prompt_wrapped_with_osc133_marks() {
        let highlighter = Osc133PromptHighlighter::new(true);
        let prompt = "❯ ";
        let formatted = highlighter.format_prompt(prompt);
        assert_eq!(formatted, "\x1b]133;A\x07❯ \x1b]133;B\x07");
    }

    #[test]
    fn leading_newlines_emitted_before_osc133_a_mark() {
        let highlighter = Osc133PromptHighlighter::new(true);
        let prompt = "\n/path/to/cwd\n❯ ";
        let formatted = highlighter.format_prompt(prompt);
        assert_eq!(
            formatted,
            "\n\x1b]133;A\x07/path/to/cwd\n\x1b]133;A;k=s\x07❯ \x1b]133;B\x07"
        );
    }

    #[test]
    fn disabled_highlighter_returns_borrowed_prompt_unchanged() {
        let highlighter = Osc133PromptHighlighter::new(false);
        let prompt = "\n❯ ";
        let formatted = highlighter.format_prompt(prompt);
        assert_eq!(formatted, prompt);
    }
}
