use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Position of the cursor within a multiline buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    /// 0-based line index where cursor is located.
    pub line_index: usize,
    /// Byte offset within the current line.
    pub col_byte_offset: usize,
    /// Visual column width within the current line.
    pub col_visual_width: usize,
}

/// A Unicode-safe line editing buffer supporting multiline text.
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

    /// Explicitly set the cursor position as a byte offset, clamping to content bounds.
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.content.len());
    }

    /// Replaces the byte range `range` with `replacement` and positions cursor at the end of the inserted replacement.
    pub fn replace_range(&mut self, range: std::ops::Range<usize>, replacement: &str) {
        let start = range.start.min(self.content.len());
        let end = range.end.min(self.content.len());
        if start <= end {
            self.content.replace_range(start..end, replacement);
            self.cursor = start + replacement.len();
        }
    }

    /// Total number of lines (separated by '\n') in the buffer.
    pub fn line_count(&self) -> usize {
        self.content.split('\n').count()
    }

    /// Iterator over the lines in the buffer.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.content.split('\n')
    }

    /// Calculates line index and visual column coordinates of the cursor.
    pub fn cursor_position(&self) -> CursorPosition {
        let mut line_index = 0;
        let mut line_start_byte = 0;

        for (idx, ch) in self.content[..self.cursor].char_indices() {
            if ch == '\n' {
                line_index += 1;
                line_start_byte = idx + 1;
            }
        }

        let col_byte_offset = self.cursor - line_start_byte;
        let col_visual_width = UnicodeWidthStr::width(&self.content[line_start_byte..self.cursor]);

        CursorPosition {
            line_index,
            col_byte_offset,
            col_visual_width,
        }
    }

    /// Calculates the visual column width on screen from line start up to cursor (single-line or current line).
    pub fn cursor_visual_width(&self) -> usize {
        self.cursor_position().col_visual_width
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

    /// Returns the text slice of the current line up to the cursor position.
    pub fn current_line_prefix(&self) -> &str {
        let pos = self.cursor_position();
        let line_start = self.cursor - pos.col_byte_offset;
        &self.content[line_start..self.cursor]
    }

    /// Insert a newline, preserving the current line's leading whitespace indentation,
    /// and optionally adding `indent_size` additional spaces if `add_indent` is true.
    pub fn insert_newline_preserve_indent(&mut self, add_indent: bool, indent_size: usize) {
        let prefix = self.current_line_prefix();

        // 1. Detect existing indentation on the current line
        let existing_indent: String = prefix
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();

        let mut new_indent = existing_indent;
        if add_indent {
            new_indent.push_str(&" ".repeat(indent_size));
        }

        self.insert_char('\n');
        self.insert_str(&new_indent);
    }

    /// Delete previous grapheme cluster, or smart-dedent by `indent_size` spaces
    /// if the cursor is currently sitting within leading indentation spaces.
    pub fn delete_prev_grapheme_or_dedent(&mut self, indent_size: usize) -> bool {
        if self.cursor == 0 {
            return false;
        }
        if indent_size == 0 {
            return self.delete_prev_grapheme();
        }

        let pos = self.cursor_position();
        let line_start = self.cursor - pos.col_byte_offset;
        let prefix = &self.content[line_start..self.cursor];

        // If the text before cursor on this line is entirely spaces (at least 1)
        if !prefix.is_empty() && prefix.chars().all(|c| c == ' ') {
            let num_spaces = prefix.len();
            let to_delete = if num_spaces.is_multiple_of(indent_size) {
                indent_size
            } else {
                num_spaces % indent_size
            };
            let delete_start = self.cursor - to_delete;
            self.content.drain(delete_start..self.cursor);
            self.cursor = delete_start;
            return true;
        }

        self.delete_prev_grapheme()
    }

    /// Dedents the current line by up to `indent_size` spaces if the text before the cursor
    /// on the current line consists entirely of whitespace.
    /// Returns `true` if any indentation was removed.
    pub fn dedent_line(&mut self, indent_size: usize) -> bool {
        if indent_size == 0 || self.cursor == 0 {
            return false;
        }

        let pos = self.cursor_position();
        let line_start = self.cursor - pos.col_byte_offset;
        let prefix = &self.content[line_start..self.cursor];

        if !prefix.is_empty() && prefix.chars().all(|ch| ch == ' ') {
            let to_remove = indent_size.min(prefix.len());
            let delete_start = self.cursor - to_remove;
            self.content.drain(delete_start..self.cursor);
            self.cursor = delete_start;
            true
        } else {
            false
        }
    }

    /// Delete the grapheme cluster immediately before the cursor (Backspace).
    /// Returns true if a character was deleted.
    pub fn delete_prev_grapheme(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }

        let prev_idx = self.content[..self.cursor]
            .grapheme_indices(true)
            .next_back()
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

        let prev_idx = self.content[..self.cursor]
            .grapheme_indices(true)
            .next_back()
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

    /// Move cursor up one line to the closest visual column.
    /// Returns `true` if moved, or `false` if already on the first line.
    pub fn move_up(&mut self) -> bool {
        let pos = self.cursor_position();
        if pos.line_index == 0 {
            return false;
        }

        let target_line = pos.line_index - 1;
        self.cursor = self.find_line_col_offset(target_line, pos.col_visual_width);
        true
    }

    /// Move cursor down one line to the closest visual column.
    /// Returns `true` if moved, or `false` if already on the last line.
    pub fn move_down(&mut self) -> bool {
        let pos = self.cursor_position();
        let total = self.line_count();
        if pos.line_index + 1 >= total {
            return false;
        }

        let target_line = pos.line_index + 1;
        self.cursor = self.find_line_col_offset(target_line, pos.col_visual_width);
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

    fn find_line_col_offset(&self, target_line_idx: usize, target_visual_col: usize) -> usize {
        let mut current_line = 0;
        let mut line_start = 0;

        for (idx, ch) in self.content.char_indices() {
            if current_line == target_line_idx {
                line_start = idx;
                break;
            }
            if ch == '\n' {
                current_line += 1;
                if current_line == target_line_idx {
                    line_start = idx + 1;
                    break;
                }
            }
        }

        let line_content = match self.content[line_start..].find('\n') {
            Some(end) => &self.content[line_start..line_start + end],
            None => &self.content[line_start..],
        };

        let mut best_offset = line_start;
        let mut current_width = 0;

        for (g_idx, g_str) in line_content.grapheme_indices(true) {
            let g_width = UnicodeWidthStr::width(g_str);
            if current_width + g_width > target_visual_col {
                break;
            }
            current_width += g_width;
            best_offset = line_start + g_idx + g_str.len();
        }

        best_offset
    }

    fn prev_word_boundary(&self) -> usize {
        let prefix = &self.content[..self.cursor];
        let mut chars = prefix.char_indices().rev().peekable();

        // Skip trailing whitespace
        while let Some(&(_, c)) = chars.peek() {
            if c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        // Skip non-whitespace to find start of word
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

        // Skip non-whitespace
        while let Some(&(_, c)) = chars.peek() {
            if !c.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }

        // Skip whitespace to find start of next word
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

        buf.insert_str("test ");
        assert_eq!(buf.as_str(), "hello test world");

        buf.delete_prev_grapheme(); // delete space
        assert_eq!(buf.as_str(), "hello testworld");
    }

    #[test]
    fn test_multiline_navigation() {
        let mut buf = LineBuffer::with_content("first line\nsecond line\nthird".to_string());
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.cursor_position().line_index, 2);

        // Move up to "second line"
        assert!(buf.move_up());
        assert_eq!(buf.cursor_position().line_index, 1);

        // Move up to "first line"
        assert!(buf.move_up());
        assert_eq!(buf.cursor_position().line_index, 0);

        // Can't move up further
        assert!(!buf.move_up());
        assert_eq!(buf.cursor_position().line_index, 0);

        // Move down to "second line"
        assert!(buf.move_down());
        assert_eq!(buf.cursor_position().line_index, 1);

        // Move down to "third"
        assert!(buf.move_down());
        assert_eq!(buf.cursor_position().line_index, 2);

        // Can't move down further
        assert!(!buf.move_down());
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

    #[test]
    fn test_auto_indent_and_dedent() {
        let mut buf = LineBuffer::new();
        buf.insert_str("fn foo() {");
        assert_eq!(buf.current_line_prefix(), "fn foo() {");
        // Indent requested (e.g. validator returned true)
        buf.insert_newline_preserve_indent(true, 4);
        assert_eq!(buf.as_str(), "fn foo() {\n    ");
        assert_eq!(buf.cursor(), 15);

        // Next line keeps same indent (add_indent = false)
        buf.insert_str("echo 1");
        assert_eq!(buf.current_line_prefix(), "    echo 1");
        buf.insert_newline_preserve_indent(false, 4);
        assert_eq!(buf.as_str(), "fn foo() {\n    echo 1\n    ");

        // Smart dedent on Backspace deletes all 4 spaces
        assert!(buf.delete_prev_grapheme_or_dedent(4));
        assert_eq!(buf.as_str(), "fn foo() {\n    echo 1\n");
    }

    #[test]
    fn test_dedent_line() {
        let mut buf = LineBuffer::new();
        buf.insert_str("fn foo() {");
        buf.insert_newline_preserve_indent(true, 4);
        assert_eq!(buf.as_str(), "fn foo() {\n    ");

        // dedent_line removes leading spaces
        assert!(buf.dedent_line(4));
        assert_eq!(buf.as_str(), "fn foo() {\n");

        // typing closing delimiter
        buf.insert_char('}');
        assert_eq!(buf.as_str(), "fn foo() {\n}");

        // dedent_line does not remove text when non-whitespace is present
        assert!(!buf.dedent_line(4));
    }

    #[test]
    fn test_replace_range_and_set_cursor() {
        let mut buf = LineBuffer::new();
        buf.insert_str("cargo build");
        assert_eq!(buf.cursor(), 11);

        buf.replace_range(6..11, "clippy");
        assert_eq!(buf.as_str(), "cargo clippy");
        assert_eq!(buf.cursor(), 12);

        buf.set_cursor(5);
        assert_eq!(buf.cursor(), 5);

        // Clamping past bounds
        buf.set_cursor(100);
        assert_eq!(buf.cursor(), 12);
    }
}
