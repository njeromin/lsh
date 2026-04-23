use std::borrow::Cow;

use rustyline::{
    Helper,
    completion::Completer,
    highlight::{CmdKind, Highlighter},
    hint::Hinter,
    validate::Validator,
};

use crate::terminal::{
    highlight::SyntaxHighlighter,
    osc::{self, Osc133PromptHighlighter},
};

pub struct LineEditorHelper {
    osc133: Osc133PromptHighlighter,
    syntax: SyntaxHighlighter,
}

impl LineEditorHelper {
    pub fn new() -> Self {
        Self {
            osc133: Osc133PromptHighlighter::new(osc::enabled()),
            syntax: SyntaxHighlighter::new(),
        }
    }

    pub fn reset_prompt_marks(&self) {
        self.osc133.reset_prompt_marks();
    }
}

impl Completer for LineEditorHelper {
    type Candidate = String;
}

impl Hinter for LineEditorHelper {
    type Hint = String;
}

impl Validator for LineEditorHelper {}

impl Highlighter for LineEditorHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.syntax.highlight(line, pos)
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        self.osc133.highlight_prompt(prompt, default)
    }

    fn highlight_char(&self, line: &str, pos: usize, kind: CmdKind) -> bool {
        self.syntax.highlight_char(line, pos, kind)
    }
}

impl Helper for LineEditorHelper {}
