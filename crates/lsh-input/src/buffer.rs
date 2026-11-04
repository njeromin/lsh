use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A Unicode-safe line editing buffer.
///
/// Invariants:
/// 1. `cursor` is always in `0..=content.len()`.
/// 2. `cursor` is always at a valid UTF-8 character boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineBuffer {
    content: String,
    cursor: usize, // Byte offset in `content`
}

impl LineBuffer {
    /// Create a new, empty line buffer.
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
        }
    }

    /// Create a line buffer initialized with content, cursor placed at the end.
    pub fn with_content(content: String) -> Self {
        let cursor = content.len();
        Self { content, cursor }
    }

    /// Access the raw string slice of the buffer.
    pub fn as_str(&self) -> &str {
        &self.content
    }

    /// Returns the byte length of the buffer.
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Returns true if the buffer has no text.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Current cursor position as a byte offset.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Calculates the visual column width on screen from line start up to cursor.
    pub fn cursor_visual_width(&self) -> usize {
        UnicodeWidthStr::width(&self.content[..self.cursor])
    }

    /// Calculates the total visual column width on screen of the entire buffer.
    pub fn total_visual_width(&self) -> usize {
        UnicodeWidthStr::width(&self.content[..])
    }

    /// Insert a character at the current cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Insert a string slice at the current cursor position.
    pub fn insert_str(&mut self, s: &str) {
        self.content.insert_str(self.cursor, s);
        self.cursor += s.len();
    }

    /// Delete the grapheme cluster immediately before the cursor (Backspace).
    /// Returns true if a character was deleted.
    pub fn delete_prev_grapheme(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        let prev_idx = self
            .content[..self.cursor]
            .grapheme_indices(true)
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        self.content.drain(prev_idx..self.cursor);
        self.cursor = prev_idx;
        true
    }

    /// Delete the grapheme cluster immediately at the cursor (Delete).
    /// Returns true if a character was deleted.
    pub fn delete_next_grapheme(&mut self) -> bool {
        if self.cursor >= self.content.len() {
            return false;
        }

        let next_idx = self.content[self.cursor..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(idx, _)| self.cursor + idx)
            .unwrap_or(self.content.len());

        self.content.drain(self.cursor..next_idx);
        true
    }

    /// Move cursor one grapheme cluster to the left.
    /// Returns true if the cursor moved.
    pub fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        let prev_idx = self
            .content[..self.cursor]
            .grapheme_indices(true)
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        self.cursor = prev_idx;
        true
    }

    /// Move cursor one grapheme cluster to the right.
    /// Returns true if the cursor moved.
    pub fn move_right(&mut self) -> bool {
        if self.cursor >= self.content.len() {
            return false;
        }

        let next_idx = self.content[self.cursor..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(idx, _)| self.cursor + idx)
            .unwrap_or(self.content.len());

        self.cursor = next_idx;
        true
    }

    /// Move cursor to the beginning of the line (0).
    pub fn move_home(&mut self) -> bool {
        if self.cursor > 0 {
            self.cursor = 0;
            true
        } else {
            false
        }
    }

    /// Move cursor to the end of the line.
    pub fn move_end(&mut self) -> bool {
        if self.cursor < self.content.len() {
            self.cursor = self.content.len();
            true
        } else {
            false
        }
    }

    /// Move cursor to the previous word boundary.
    pub fn move_word_left(&mut self) -> bool {
        let target = self.prev_word_boundary();
        if target != self.cursor {
            self.cursor = target;
            true
        } else {
            false
        }
    }

    /// Move cursor to the next word boundary.
    pub fn move_word_right(&mut self) -> bool {
        let target = self.next_word_boundary();
        if target != self.cursor {
            self.cursor = target;
            true
        } else {
            false
        }
    }

    /// Delete backwards to the previous word boundary (Alt+Backspace / Ctrl+W).
    pub fn delete_word_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let target = self.prev_word_boundary();
        self.content.drain(target..self.cursor);
        self.cursor = target;
        true
    }

    /// Delete forwards to the next word boundary (Alt+D).
    pub fn delete_word_right(&mut self) -> bool {
        if self.cursor >= self.content.len() {
            return false;
        }
        let target = self.next_word_boundary();
        self.content.drain(self.cursor..target);
        true
    }

    /// Clear text from start of line up to cursor (Ctrl+U).
    pub fn clear_to_start(&mut self) -> bool {
        if self.cursor > 0 {
            self.content.drain(..self.cursor);
            self.cursor = 0;
            true
        } else {
            false
        }
    }

    /// Clear text from cursor to end of line (Ctrl+K).
    pub fn clear_to_end(&mut self) -> bool {
        if self.cursor < self.content.len() {
            self.content.truncate(self.cursor);
            true
        } else {
            false
        }
    }

    /// Clear entire line buffer.
    pub fn clear(&mut self) -> bool {
        if !self.content.is_empty() {
            self.content.clear();
            self.cursor = 0;
            true
        } else {
            false
        }
    }

    /// Consume the buffer and return the underlying string.
    pub fn into_string(self) -> String {
        self.content
    }

    fn prev_word_boundary(&self) -> usize {
        let prefix = &self.content[..self.cursor];
        let mut chars = prefix.char_indices().rev().peekable();

        // 1. Skip trailing whitespace
        while let Some(&(_, c)) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        // 2. Skip non-whitespace to find start of word
        while let Some(&(_, c)) = chars.peek() {
            if !c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        chars.peek().map(|&(idx, _)| idx + 1).unwrap_or(0)
    }

    fn next_word_boundary(&self) -> usize {
        let suffix = &self.content[self.cursor..];
        let mut chars = suffix.char_indices().peekable();

        // 1. Skip non-whitespace
        while let Some(&(_, c)) = chars.peek() {
            if !c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        // 2. Skip whitespace to find start of next word
        while let Some(&(_, c)) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        chars
            .peek()
            .map(|&(idx, _)| self.cursor + idx)
            .unwrap_or(self.content.len())
    }
}

impl Default for LineBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_editing() {
        let mut buf = LineBuffer::new();
        buf.insert_str("hello world");
        assert_eq!(buf.as_str(), "hello world");
        assert_eq!(buf.cursor(), 11);

        buf.move_word_left();
        assert_eq!(buf.cursor(), 6); // at "world"

        buf.insert_str("brave ");
        assert_eq!(buf.as_str(), "hello brave world");

        buf.delete_prev_grapheme(); // delete space
        assert_eq!(buf.as_str(), "hello braveworld");
    }

    #[test]
    fn test_unicode_multibyte_and_emoji() {
        let mut buf = LineBuffer::new();
        buf.insert_char('a');
        buf.insert_char('🦀'); // 4-byte UTF-8
        buf.insert_char('b');
        assert_eq!(buf.as_str(), "a🦀b");
        assert_eq!(buf.len(), 6);
        assert_eq!(buf.cursor(), 6);

        // Move left over 'b'
        assert!(buf.move_left());
        assert_eq!(buf.cursor(), 5);

        // Move left over emoji '🦀'
        assert!(buf.move_left());
        assert_eq!(buf.cursor(), 1); // Jumped 4 bytes cleanly

        // Move right over emoji
        assert!(buf.move_right());
        assert_eq!(buf.cursor(), 5);

        // Delete emoji backwards
        assert!(buf.delete_prev_grapheme());
        assert_eq!(buf.as_str(), "ab");
        assert_eq!(buf.cursor(), 1);
    }

    #[test]
    fn test_visual_widths() {
        let mut buf = LineBuffer::new();
        buf.insert_str("abc");
        assert_eq!(buf.cursor_visual_width(), 3);

        buf.insert_char('🦀'); // Width 2
        assert_eq!(buf.cursor_visual_width(), 5);
    }

    #[test]
    fn test_word_deletion() {
        let mut buf = LineBuffer::with_content("git commit -m".to_string());
        assert!(buf.delete_word_left());
        assert_eq!(buf.as_str(), "git commit ");
        assert!(buf.delete_word_left());
        assert_eq!(buf.as_str(), "git ");
    }
}
