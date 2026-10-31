pub use lsh_completions::{CompletionItem, CompletionKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuLayout {
    pub width: usize,
    pub height: usize,
    pub menu_col: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub nerd_icons: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuResult {
    /// Accepted a candidate from the dropdown list
    Selected(CompletionItem),
    /// Typed past all matches or committed typed text
    Typed(String),
    /// Cancelled without accepting (e.g. Esc or Ctrl-C)
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Next,
    Previous,
    PageDown,
    PageUp,
    First,
    Last,
    Accept,
    Dismiss,
    Char(char),
    Backspace,
    Tab,
    Slash,
}
