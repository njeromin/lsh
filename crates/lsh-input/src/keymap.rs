use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyModifiers};

/// Represents a keyboard combination (key code + modifier keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    /// Create a new key combination with custom code and modifiers.
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    /// Helper for a plain character without modifiers.
    pub const fn char(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
        }
    }

    /// Helper for a Ctrl+<char> combination.
    pub const fn ctrl(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
        }
    }

    /// Helper for an Alt+<char> combination.
    pub const fn alt(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::ALT,
        }
    }
}

impl From<KeyCode> for KeyCombo {
    fn from(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }
}

impl From<(KeyCode, KeyModifiers)> for KeyCombo {
    fn from((code, modifiers): (KeyCode, KeyModifiers)) -> Self {
        Self { code, modifiers }
    }
}

/// Actions supported by the line editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorAction {
    /// Submit the current line (typically Enter).
    Submit,
    /// Insert a newline into the buffer without submitting (typically Alt+Enter).
    InsertNewline,
    /// Cancel current input and discard line (typically Ctrl+C).
    Interrupt,
    /// Signal EOF on empty line to exit (typically Ctrl+D).
    Eof,
    /// Move cursor one character to the left.
    MoveLeft,
    /// Move cursor one character to the right.
    MoveRight,
    /// Move cursor one line up in multiline input.
    MoveUp,
    /// Move cursor one line down in multiline input.
    MoveDown,
    /// Move cursor one word to the left.
    MoveWordLeft,
    /// Move cursor one word to the right.
    MoveWordRight,
    /// Move cursor to the beginning of the line.
    MoveHome,
    /// Move cursor to the end of the line.
    MoveEnd,
    /// Delete character before the cursor (Backspace).
    Backspace,
    /// Delete character at the cursor (Delete).
    Delete,
    /// Delete word before the cursor (Alt+Backspace / Ctrl+W).
    DeleteWordLeft,
    /// Delete word after the cursor (Alt+D).
    DeleteWordRight,
    /// Clear from beginning of line up to cursor (Ctrl+U).
    ClearToStart,
    /// Clear from cursor to the end of the line (Ctrl+K).
    ClearToEnd,
    /// Cut from beginning of line up to cursor into the clipboard (Ctrl+U).
    CutToStart,
    /// Cut from cursor to the end of the line into the clipboard (Ctrl+K).
    CutToEnd,
    /// Cut word before the cursor into the clipboard (Alt+Backspace / Ctrl+W).
    CutWordLeft,
    /// Cut word after the cursor into the clipboard (Alt+D).
    CutWordRight,
    /// Paste / yank text from the clipboard at cursor position (Ctrl+Y).
    Yank,
    /// Paste text from the clipboard after the cursor (Vi 'p').
    PasteAfter,
    /// Paste text from the clipboard before the cursor (Vi 'P').
    PasteBefore,
    /// Copy the entire line into the clipboard without deleting (Vi 'yy').
    YankLine,
    /// Cut the character under the cursor into the clipboard (Vi 'x').
    CutChar,
    /// Cut the character before the cursor into the clipboard (Vi 'X').
    CutCharBefore,
    /// Move cursor to the end of the current or next word (Vi 'e').
    MoveWordEnd,
    /// Move cursor to the first non-whitespace character on the line (Vi '^').
    MoveToFirstNonWhitespace,
    /// Toggle case of character under cursor and move right (Vi '~').
    ToggleCase,
    /// Undo the last edit action (Vi 'u', Emacs 'Ctrl+_', 'Ctrl+Z').
    Undo,
    /// Redo the last undone edit action (Vi 'Ctrl+R', Emacs 'Alt+R', 'Ctrl+Shift+Z').
    Redo,
    /// Switch from Insert mode to Normal mode (Vi Esc).
    SwitchToNormalMode,
    /// Switch from Normal mode to Insert mode (Vi 'i').
    SwitchToInsertMode,
    /// Move cursor right and switch to Insert mode (Vi 'a').
    Append,
    /// Move cursor to end of line and switch to Insert mode (Vi 'A').
    AppendEnd,
    /// Move cursor to first non-whitespace and switch to Insert mode (Vi 'I').
    InsertAtFirstNonWhitespace,
    /// Open a new line below and switch to Insert mode (Vi 'o').
    OpenBelow,
    /// Open a new line above and switch to Insert mode (Vi 'O').
    OpenAbove,
    /// Cut character under cursor and switch to Insert mode (Vi 's').
    SubstituteChar,
    /// Cut entire line and switch to Insert mode (Vi 'S').
    SubstituteLine,
    /// Cut to end of line and switch to Insert mode (Vi 'C').
    ChangeToEnd,
    /// Vi 'd' operator pending motion or 'd'.
    ViDeleteOperator,
    /// Vi 'c' operator pending motion or 'c'.
    ViChangeOperator,
    /// Vi 'y' operator pending motion or 'y'.
    ViYankOperator,
    /// Vi 'r' operator pending replacement character.
    ViReplaceCharOperator,
    /// Clear the entire line.
    ClearLine,
    /// Clear the terminal screen and redraw the prompt at top (Ctrl+L).
    ClearScreen,
    /// Accept the entire inline ghost text hint.
    AcceptGhostText,
    /// Accept only the next word of the inline ghost text hint.
    AcceptGhostTextWord,
    /// Trigger completion or cycle to next candidate.
    Complete,
    /// Cycle to previous completion candidate.
    CompletePrevious,
    /// Explicitly do nothing for this key combination.
    Ignore,
}

/// A configurable keymap mapping `KeyCombo`s to `EditorAction`s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    bindings: HashMap<KeyCombo, EditorAction>,
}

impl Keymap {
    /// Create an empty keymap with no bindings.
    pub fn empty() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Create a standard Emacs-like default keymap.
    pub fn default_emacs() -> Self {
        let mut km = Self::empty();

        // Control & submission
        km.bind(KeyCode::Enter, EditorAction::Submit);
        km.bind(
            (KeyCode::Enter, KeyModifiers::SHIFT),
            EditorAction::InsertNewline,
        );
        km.bind(
            (KeyCode::Enter, KeyModifiers::ALT),
            EditorAction::InsertNewline,
        );
        km.bind(
            (KeyCode::Char('j'), KeyModifiers::CONTROL),
            EditorAction::InsertNewline,
        );
        km.bind(KeyCombo::ctrl('c'), EditorAction::Interrupt);
        km.bind(KeyCombo::ctrl('d'), EditorAction::Eof);

        // Completion
        km.bind(KeyCode::Tab, EditorAction::Complete);
        km.bind(KeyCode::BackTab, EditorAction::CompletePrevious);
        km.bind(
            (KeyCode::Tab, KeyModifiers::SHIFT),
            EditorAction::CompletePrevious,
        );

        // Basic navigation
        km.bind(KeyCode::Left, EditorAction::MoveLeft);
        km.bind(KeyCombo::ctrl('b'), EditorAction::MoveLeft);
        km.bind(KeyCode::Right, EditorAction::MoveRight);
        km.bind(KeyCombo::ctrl('f'), EditorAction::MoveRight);
        km.bind(KeyCode::Up, EditorAction::MoveUp);
        km.bind(KeyCode::Down, EditorAction::MoveDown);
        km.bind(KeyCombo::ctrl('p'), EditorAction::MoveUp);
        km.bind(KeyCombo::ctrl('n'), EditorAction::MoveDown);
        km.bind(KeyCode::Home, EditorAction::MoveHome);
        km.bind(KeyCombo::ctrl('a'), EditorAction::MoveHome);
        km.bind(KeyCode::End, EditorAction::MoveEnd);
        km.bind(KeyCombo::ctrl('e'), EditorAction::MoveEnd);

        // Word navigation
        km.bind(KeyCombo::alt('b'), EditorAction::MoveWordLeft);
        km.bind(
            (KeyCode::Left, KeyModifiers::ALT),
            EditorAction::MoveWordLeft,
        );
        km.bind(
            (KeyCode::Left, KeyModifiers::CONTROL),
            EditorAction::MoveWordLeft,
        );
        km.bind(KeyCombo::alt('f'), EditorAction::MoveWordRight);
        km.bind(
            (KeyCode::Right, KeyModifiers::ALT),
            EditorAction::MoveWordRight,
        );
        km.bind(
            (KeyCode::Right, KeyModifiers::CONTROL),
            EditorAction::MoveWordRight,
        );

        // Deletions & killing (cutting to clipboard)
        km.bind(KeyCode::Backspace, EditorAction::Backspace);
        km.bind(KeyCode::Delete, EditorAction::Delete);
        km.bind(KeyCombo::ctrl('w'), EditorAction::CutWordLeft);
        km.bind(KeyCombo::ctrl('h'), EditorAction::CutWordLeft);
        km.bind(
            (KeyCode::Backspace, KeyModifiers::ALT),
            EditorAction::CutWordLeft,
        );
        km.bind(
            (KeyCode::Backspace, KeyModifiers::CONTROL),
            EditorAction::CutWordLeft,
        );
        km.bind(KeyCombo::alt('d'), EditorAction::CutWordRight);
        km.bind(
            (KeyCode::Delete, KeyModifiers::CONTROL),
            EditorAction::CutWordRight,
        );
        km.bind(
            (KeyCode::Delete, KeyModifiers::ALT),
            EditorAction::CutWordRight,
        );

        // Line clearing, cutting, and clipboard
        km.bind(KeyCombo::ctrl('u'), EditorAction::CutToStart);
        km.bind(KeyCombo::ctrl('k'), EditorAction::CutToEnd);
        km.bind(KeyCombo::ctrl('y'), EditorAction::Yank);
        km.bind(KeyCombo::ctrl('l'), EditorAction::ClearScreen);

        // Undo & redo
        km.bind(KeyCombo::ctrl('_'), EditorAction::Undo);
        km.bind(
            (
                KeyCode::Char('_'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            EditorAction::Undo,
        );
        km.bind(KeyCombo::ctrl('/'), EditorAction::Undo);
        km.bind(KeyCombo::ctrl('-'), EditorAction::Undo);
        km.bind(KeyCombo::ctrl('z'), EditorAction::Undo);

        km.bind(KeyCombo::alt('r'), EditorAction::Redo);
        km.bind(KeyCombo::alt('_'), EditorAction::Redo);
        km.bind(
            (KeyCode::Char('_'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            EditorAction::Redo,
        );
        km.bind(
            (KeyCode::Char('-'), KeyModifiers::ALT | KeyModifiers::SHIFT),
            EditorAction::Redo,
        );
        km.bind(
            (
                KeyCode::Char('z'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            EditorAction::Redo,
        );
        km.bind(
            (
                KeyCode::Char('Z'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            EditorAction::Redo,
        );
        km.bind(KeyCombo::ctrl('g'), EditorAction::Redo);

        km
    }

    /// Create a standard Vi insert mode keymap.
    pub fn default_vi_insert() -> Self {
        let mut km = Self::empty();

        // Mode transition to normal mode
        km.bind(KeyCode::Esc, EditorAction::SwitchToNormalMode);
        km.bind(KeyCombo::ctrl('['), EditorAction::SwitchToNormalMode);

        // Undo & redo
        km.bind(KeyCombo::ctrl('_'), EditorAction::Undo);
        km.bind(KeyCombo::ctrl('/'), EditorAction::Undo);
        km.bind(KeyCombo::ctrl('z'), EditorAction::Undo);
        km.bind(KeyCombo::alt('r'), EditorAction::Redo);
        km.bind(
            (
                KeyCode::Char('z'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            EditorAction::Redo,
        );
        km.bind(
            (
                KeyCode::Char('Z'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
            EditorAction::Redo,
        );

        // Submissions & control
        km.bind(KeyCode::Enter, EditorAction::Submit);
        km.bind(
            (KeyCode::Enter, KeyModifiers::SHIFT),
            EditorAction::InsertNewline,
        );
        km.bind(
            (KeyCode::Enter, KeyModifiers::ALT),
            EditorAction::InsertNewline,
        );
        km.bind(KeyCombo::ctrl('c'), EditorAction::Interrupt);
        km.bind(KeyCombo::ctrl('d'), EditorAction::Eof);

        // Completion
        km.bind(KeyCode::Tab, EditorAction::Complete);
        km.bind(KeyCode::BackTab, EditorAction::CompletePrevious);
        km.bind(
            (KeyCode::Tab, KeyModifiers::SHIFT),
            EditorAction::CompletePrevious,
        );

        // Deletions & clipboard
        km.bind(KeyCode::Backspace, EditorAction::Backspace);
        km.bind(KeyCode::Delete, EditorAction::Delete);
        km.bind(KeyCombo::ctrl('w'), EditorAction::CutWordLeft);
        km.bind(KeyCombo::ctrl('u'), EditorAction::CutToStart);
        km.bind(KeyCombo::ctrl('k'), EditorAction::CutToEnd);
        km.bind(KeyCombo::ctrl('y'), EditorAction::Yank);

        // Basic arrow navigation in insert mode
        km.bind(KeyCode::Left, EditorAction::MoveLeft);
        km.bind(KeyCode::Right, EditorAction::MoveRight);
        km.bind(KeyCode::Up, EditorAction::MoveUp);
        km.bind(KeyCode::Down, EditorAction::MoveDown);
        km.bind(KeyCode::Home, EditorAction::MoveHome);
        km.bind(KeyCode::End, EditorAction::MoveEnd);

        km
    }

    /// Create a standard Vi normal mode keymap.
    pub fn default_vi_normal() -> Self {
        let mut km = Self::empty();

        // Mode transitions
        km.bind(KeyCode::Char('i'), EditorAction::SwitchToInsertMode);
        km.bind(KeyCode::Char('I'), EditorAction::InsertAtFirstNonWhitespace);
        km.bind(KeyCode::Char('a'), EditorAction::Append);
        km.bind(KeyCode::Char('A'), EditorAction::AppendEnd);
        km.bind(KeyCode::Char('o'), EditorAction::OpenBelow);
        km.bind(KeyCode::Char('O'), EditorAction::OpenAbove);
        km.bind(KeyCode::Char('s'), EditorAction::SubstituteChar);
        km.bind(KeyCode::Char('S'), EditorAction::SubstituteLine);
        km.bind(KeyCode::Char('C'), EditorAction::ChangeToEnd);
        km.bind(KeyCode::Esc, EditorAction::SwitchToNormalMode);

        // Submissions & control
        km.bind(KeyCode::Enter, EditorAction::Submit);
        km.bind(KeyCombo::ctrl('c'), EditorAction::Interrupt);
        km.bind(KeyCombo::ctrl('d'), EditorAction::Eof);
        km.bind(KeyCombo::ctrl('l'), EditorAction::ClearScreen);

        // Navigation
        km.bind(KeyCode::Char('h'), EditorAction::MoveLeft);
        km.bind(KeyCode::Left, EditorAction::MoveLeft);
        km.bind(KeyCode::Char('l'), EditorAction::MoveRight);
        km.bind(KeyCode::Right, EditorAction::MoveRight);
        km.bind(KeyCode::Char('k'), EditorAction::MoveUp);
        km.bind(KeyCode::Up, EditorAction::MoveUp);
        km.bind(KeyCode::Char('j'), EditorAction::MoveDown);
        km.bind(KeyCode::Down, EditorAction::MoveDown);
        km.bind(KeyCode::Char('w'), EditorAction::MoveWordRight);
        km.bind(KeyCode::Char('b'), EditorAction::MoveWordLeft);
        km.bind(KeyCode::Char('e'), EditorAction::MoveWordEnd);
        km.bind(KeyCode::Char('0'), EditorAction::MoveHome);
        km.bind(KeyCode::Home, EditorAction::MoveHome);
        km.bind(KeyCode::Char('^'), EditorAction::MoveToFirstNonWhitespace);
        km.bind(KeyCode::Char('$'), EditorAction::MoveEnd);
        km.bind(KeyCode::End, EditorAction::MoveEnd);

        // Operators
        km.bind(KeyCode::Char('d'), EditorAction::ViDeleteOperator);
        km.bind(KeyCode::Char('c'), EditorAction::ViChangeOperator);
        km.bind(KeyCode::Char('y'), EditorAction::ViYankOperator);
        km.bind(KeyCode::Char('r'), EditorAction::ViReplaceCharOperator);

        // Editing & deletion
        km.bind(KeyCode::Char('x'), EditorAction::CutChar);
        km.bind(KeyCode::Delete, EditorAction::CutChar);
        km.bind(KeyCode::Char('X'), EditorAction::CutCharBefore);
        km.bind(KeyCode::Char('D'), EditorAction::CutToEnd);
        km.bind(KeyCode::Char('~'), EditorAction::ToggleCase);
        km.bind(KeyCode::Char('u'), EditorAction::Undo);
        km.bind(KeyCombo::ctrl('r'), EditorAction::Redo);
        km.bind(KeyCode::Char('p'), EditorAction::PasteAfter);
        km.bind(KeyCode::Char('P'), EditorAction::PasteBefore);

        km
    }

    /// Bind a key combination to an action. Replaces existing binding if already present.
    pub fn bind(&mut self, combo: impl Into<KeyCombo>, action: EditorAction) -> &mut Self {
        self.bindings.insert(combo.into(), action);
        self
    }

    /// Unbind a key combination.
    pub fn unbind(&mut self, combo: impl Into<KeyCombo>) -> Option<EditorAction> {
        self.bindings.remove(&combo.into())
    }

    /// Look up the action bound to the specified key combination.
    pub fn get(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<&EditorAction> {
        self.bindings.get(&KeyCombo::new(code, modifiers))
    }

    /// Check if a key combination is bound.
    pub fn contains(&self, combo: impl Into<KeyCombo>) -> bool {
        self.bindings.contains_key(&combo.into())
    }

    /// Returns the number of keybindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Returns true if no keybindings exist.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl Default for Keymap {
    fn default() -> Self {
        Self::default_emacs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keymap_lookups() {
        let km = Keymap::default();
        assert_eq!(
            km.get(KeyCode::Enter, KeyModifiers::NONE),
            Some(&EditorAction::Submit)
        );
        assert_eq!(
            km.get(KeyCode::Enter, KeyModifiers::SHIFT),
            Some(&EditorAction::InsertNewline)
        );
        assert_eq!(
            km.get(KeyCode::Enter, KeyModifiers::ALT),
            Some(&EditorAction::InsertNewline)
        );
        assert_eq!(
            km.get(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(&EditorAction::Interrupt)
        );
        assert_eq!(
            km.get(KeyCode::Char('a'), KeyModifiers::CONTROL),
            Some(&EditorAction::MoveHome)
        );
        assert_eq!(
            km.get(KeyCode::Tab, KeyModifiers::NONE),
            Some(&EditorAction::Complete)
        );
        assert_eq!(
            km.get(KeyCode::BackTab, KeyModifiers::NONE),
            Some(&EditorAction::CompletePrevious)
        );
        assert_eq!(
            km.get(KeyCode::Tab, KeyModifiers::SHIFT),
            Some(&EditorAction::CompletePrevious)
        );

        // Undo & Redo bindings in Emacs mode
        assert_eq!(
            km.get(KeyCode::Char('_'), KeyModifiers::CONTROL),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            km.get(KeyCode::Char('z'), KeyModifiers::CONTROL),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            km.get(KeyCode::Char('/'), KeyModifiers::CONTROL),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            km.get(KeyCode::Char('r'), KeyModifiers::ALT),
            Some(&EditorAction::Redo)
        );
        assert_eq!(
            km.get(KeyCode::Char('g'), KeyModifiers::CONTROL),
            Some(&EditorAction::Redo)
        );
        assert_eq!(
            km.get(
                KeyCode::Char('z'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            ),
            Some(&EditorAction::Redo)
        );
    }

    #[test]
    fn test_custom_binding_override() {
        let mut km = Keymap::default();
        km.bind(KeyCombo::ctrl('k'), EditorAction::Submit);
        assert_eq!(
            km.get(KeyCode::Char('k'), KeyModifiers::CONTROL),
            Some(&EditorAction::Submit)
        );

        km.unbind(KeyCombo::ctrl('k'));
        assert_eq!(km.get(KeyCode::Char('k'), KeyModifiers::CONTROL), None);
    }
}
