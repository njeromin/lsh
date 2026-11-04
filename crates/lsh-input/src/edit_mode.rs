/// The editing mode of the line editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditMode {
    /// Standard Emacs keybindings (default).
    #[default]
    Emacs,
    /// Modal Vi editing with Normal and Insert modes.
    Vi,
}

/// The sub-mode when editing in Vi mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViMode {
    /// Insert mode: typed characters are inserted into the line buffer.
    #[default]
    Insert,
    /// Normal mode: typed keys execute commands, motions, or operators.
    Normal,
}

/// Pending operator in Vi Normal mode waiting for a motion or character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViOperator {
    /// Delete ('d'), waiting for motion or 'd' for whole line.
    Delete,
    /// Change ('c'), waiting for motion to cut and enter insert mode.
    Change,
    /// Yank ('y'), waiting for motion to copy to clipboard.
    Yank,
    /// Replace single char ('r'), waiting for replacement char.
    ReplaceChar,
}
