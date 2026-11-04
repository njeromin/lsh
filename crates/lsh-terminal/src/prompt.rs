use std::borrow::Cow;

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

impl lsh_input::Prompt for LshPrompt {
    fn render(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.left)
    }

    fn render_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("… ")
    }
}
