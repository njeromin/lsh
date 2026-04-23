use std::{
    borrow::Cow,
    cell::Cell,
    io,
    io::{IsTerminal, Write},
    path::Path,
};

use rustyline::highlight::Highlighter;

pub struct Osc133PromptHighlighter {
    enabled: bool,
    prompt_input_mark_rendered: Cell<bool>,
}

impl Osc133PromptHighlighter {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            prompt_input_mark_rendered: Cell::new(false),
        }
    }

    pub fn reset_prompt_marks(&self) {
        self.prompt_input_mark_rendered.set(false);
    }

    fn prompt_with_input_mark<'a>(&self, prompt: &'a str) -> Cow<'a, str> {
        if !self.enabled || self.prompt_input_mark_rendered.replace(true) {
            return Cow::Borrowed(prompt);
        }

        Cow::Owned(format!("{prompt}\x1b]133;B\x07"))
    }
}

impl Highlighter for Osc133PromptHighlighter {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        _default: bool,
    ) -> Cow<'b, str> {
        self.prompt_with_input_mark(prompt)
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
