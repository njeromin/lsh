use std::borrow::Cow;

use reedline::{Prompt, PromptEditMode, PromptHistorySearch};

pub struct LshPrompt {
    left: String,
}

impl LshPrompt {
    pub fn new(left: String) -> Self {
        let osc = crate::osc::Osc133PromptHighlighter::new(crate::osc::enabled());
        let formatted = osc.format_prompt(&left).into_owned();
        Self { left: formatted }
    }
}

impl Prompt for LshPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _prompt_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("… ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Owned(format!("(search: {})> ", history_search.term))
    }
}
