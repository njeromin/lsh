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
    /// Cancel current input and discard line (typically Ctrl+C).
    Interrupt,
    /// Signal EOF on empty line to exit (typically Ctrl+D).
    Eof,
    /// Move cursor one character to the left.
    MoveLeft,
    /// Move cursor one character to the right.
    MoveRight,
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
    /// Clear the entire line.
    ClearLine,
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
        km.bind(KeyCombo::ctrl('c'), EditorAction::Interrupt);
        km.bind(KeyCombo::ctrl('d'), EditorAction::Eof);

        // Basic navigation
        km.bind(KeyCode::Left, EditorAction::MoveLeft);
        km.bind(KeyCode::Right, EditorAction::MoveRight);
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

        // Deletions
        km.bind(KeyCode::Backspace, EditorAction::Backspace);
        km.bind(KeyCode::Delete, EditorAction::Delete);
        km.bind(KeyCombo::ctrl('w'), EditorAction::DeleteWordLeft);
        km.bind(KeyCombo::ctrl('h'), EditorAction::DeleteWordLeft);
        km.bind(
            (KeyCode::Backspace, KeyModifiers::ALT),
            EditorAction::DeleteWordLeft,
        );
        km.bind(
            (KeyCode::Backspace, KeyModifiers::CONTROL),
            EditorAction::DeleteWordLeft,
        );
        km.bind(KeyCombo::alt('d'), EditorAction::DeleteWordRight);
        km.bind(
            (KeyCode::Delete, KeyModifiers::CONTROL),
            EditorAction::DeleteWordRight,
        );
        km.bind(
            (KeyCode::Delete, KeyModifiers::ALT),
            EditorAction::DeleteWordRight,
        );

        // Line clearing
        km.bind(KeyCombo::ctrl('u'), EditorAction::ClearToStart);
        km.bind(KeyCombo::ctrl('k'), EditorAction::ClearToEnd);

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
            km.get(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Some(&EditorAction::Interrupt)
        );
        assert_eq!(
            km.get(KeyCode::Char('a'), KeyModifiers::CONTROL),
            Some(&EditorAction::MoveHome)
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
