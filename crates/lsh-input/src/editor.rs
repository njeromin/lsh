use std::io::{self, BufWriter, Stdout, Write};

use crossterm::{
    cursor::{self, MoveToColumn},
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::buffer::LineBuffer;
use crate::formatter::{StyledText, TextFormatter};
use crate::keymap::{EditorAction, Keymap};
use crate::prompt::Prompt;
use crate::validator::{ValidationResult, Validator};

/// Screen layout coordinates for the cursor and the rendered text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenLayout {
    /// Visual row of the cursor relative to the first row of the prompt (0-indexed).
    pub cursor_row: usize,
    /// Visual column of the cursor (0-indexed).
    pub cursor_col: usize,
    /// Visual row where rendered text ends (relative to the first row of the prompt).
    pub end_row: usize,
    /// Visual column where rendered text ends.
    pub end_col: usize,
}

fn resolve_cursor_wrap(
    (col, row): (usize, usize),
    is_at_buffer_end: bool,
    term_cols: usize,
) -> (usize, usize) {
    if col >= term_cols {
        if is_at_buffer_end {
            (term_cols.saturating_sub(1), row)
        } else {
            (0, row + 1)
        }
    } else {
        (col, row)
    }
}

/// Computes the visual screen layout (soft-wrapped rows and cursor coordinates)
/// given the prompt, multiline indicator, line buffer, and terminal column width.
pub fn compute_screen_layout(
    prompt_stripped: &str,
    indicator_stripped: &str,
    line_buffer: &LineBuffer,
    term_cols: usize,
) -> ScreenLayout {
    let term_cols = term_cols.max(1);
    let mut current_row: usize = 0;
    let mut current_col: usize = 0;
    let mut cursor_screen: Option<(usize, usize)> = None;

    let target_cursor_byte = line_buffer.cursor();
    let mut current_byte_offset: usize = 0;

    let lines: Vec<&str> = line_buffer.as_str().split('\n').collect();

    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx > 0 {
            current_row += 1;
            current_col = 0;
        }

        let prefix = if line_idx == 0 {
            prompt_stripped
        } else {
            indicator_stripped
        };

        // Advance through prefix graphemes
        for grapheme in prefix.graphemes(true) {
            let w = UnicodeWidthStr::width(grapheme);
            if current_col >= term_cols {
                current_row += 1;
                current_col = 0;
            }
            if current_col > 0 && current_col + w > term_cols {
                current_row += 1;
                current_col = 0;
            }
            current_col += w;
        }

        // Check if cursor sits at the beginning of this line's content
        if cursor_screen.is_none() && current_byte_offset == target_cursor_byte {
            let is_at_end = current_byte_offset == line_buffer.len();
            cursor_screen = Some(resolve_cursor_wrap(
                (current_col, current_row),
                is_at_end,
                term_cols,
            ));
        }

        // Advance through line content graphemes
        for grapheme in line.graphemes(true) {
            let w = UnicodeWidthStr::width(grapheme);
            if current_col >= term_cols {
                current_row += 1;
                current_col = 0;
            }
            if current_col > 0 && current_col + w > term_cols {
                current_row += 1;
                current_col = 0;
            }
            current_col += w;
            current_byte_offset += grapheme.len();

            if cursor_screen.is_none() && current_byte_offset == target_cursor_byte {
                let is_at_end = current_byte_offset == line_buffer.len();
                cursor_screen = Some(resolve_cursor_wrap(
                    (current_col, current_row),
                    is_at_end,
                    term_cols,
                ));
            }
        }

        // Account for '\n' byte between lines
        if line_idx + 1 < lines.len() {
            current_byte_offset += 1; // for '\n'
        }
    }

    let end_col = if current_col >= term_cols {
        term_cols.saturating_sub(1)
    } else {
        current_col
    };
    let end_row = current_row;

    let (cursor_col, cursor_row) = cursor_screen.unwrap_or((end_col, end_row));

    ScreenLayout {
        cursor_row,
        cursor_col,
        end_row,
        end_col,
    }
}

/// Normalises pasted text by converting CRLF and CR line endings to standard LF (`\n`),
/// preserving valid whitespace (`\n`, `\t`) and printable Unicode characters,
/// and stripping non-printable control characters to prevent escape sequence injection.
pub fn normalise_paste(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                result.push('\n');
            }
            '\n' | '\t' => result.push(c),
            c if c.is_control() => {} // Strip control characters / escape injections
            c => result.push(c),
        }
    }

    result
}

/// RAII guard to ensure terminal raw mode is disabled when exiting `read_line`,
/// even in the case of errors or panics.
struct RawModeGuard {
    enhanced_keyboard: bool,
}

impl RawModeGuard {
    fn new<W: Write>(writer: &mut W) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let _ = queue!(writer, EnableBracketedPaste);
        let enhanced_keyboard = matches!(terminal::supports_keyboard_enhancement(), Ok(true));
        if enhanced_keyboard {
            let _ = queue!(
                writer,
                event::PushKeyboardEnhancementFlags(
                    event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            );
        }
        let _ = writer.flush();
        Ok(Self { enhanced_keyboard })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = queue!(stdout, DisableBracketedPaste);
        if self.enhanced_keyboard {
            let _ = queue!(stdout, event::PopKeyboardEnhancementFlags);
        }
        let _ = stdout.flush();
        let _ = terminal::disable_raw_mode();
    }
}

pub struct Editor<W: Write = BufWriter<Stdout>> {
    writer: W,
    keymap: Keymap,
    formatter: Option<Box<dyn TextFormatter>>,
    validator: Option<Box<dyn Validator>>,
    indent_size: usize,
    terminal_width: Option<usize>,
}

impl Editor<BufWriter<Stdout>> {
    /// Create a new editor writing to standard output wrapped in a `BufWriter` with default keybindings.
    pub fn new() -> Self {
        Self {
            writer: BufWriter::new(io::stdout()),
            keymap: Keymap::default(),
            formatter: None,
            validator: None,
            indent_size: 4,
            terminal_width: None,
        }
    }
}

impl<W: Write> Editor<W> {
    /// Create an editor with a custom writer (e.g. `stderr`, `Vec<u8>`, or a custom buffer).
    pub fn with_writer(writer: W) -> Self {
        Self {
            writer,
            keymap: Keymap::default(),
            formatter: None,
            validator: None,
            indent_size: 4,
            terminal_width: None,
        }
    }

    /// Set an explicit terminal column width override (useful for testing or fixed-width rendering).
    pub fn with_terminal_width(mut self, width: usize) -> Self {
        self.terminal_width = Some(width);
        self
    }

    /// Dynamically set or clear the terminal width override.
    pub fn set_terminal_width(&mut self, width: Option<usize>) {
        self.terminal_width = width;
    }

    /// Returns the effective terminal column width (queries crossterm if not overridden).
    pub fn terminal_width(&self) -> usize {
        self.terminal_width
            .unwrap_or_else(|| terminal::size().map(|(w, _)| w as usize).unwrap_or(80))
            .max(1)
    }

    /// Set indentation size in spaces for auto-indent and smart dedent (defaults to 4).
    pub fn with_indent_size(mut self, size: usize) -> Self {
        self.indent_size = size;
        self
    }

    /// Get the current indentation size in spaces.
    pub fn indent_size(&self) -> usize {
        self.indent_size
    }

    /// Set the indentation size dynamically.
    pub fn set_indent_size(&mut self, size: usize) {
        self.indent_size = size;
    }

    /// Set a custom keymap.
    pub fn with_keymap(mut self, keymap: Keymap) -> Self {
        self.keymap = keymap;
        self
    }

    /// Set a custom text formatter.
    pub fn with_formatter(mut self, formatter: impl TextFormatter + 'static) -> Self {
        self.formatter = Some(Box::new(formatter));
        self
    }

    /// Dynamically set or clear the text formatter.
    pub fn set_formatter(&mut self, formatter: Option<Box<dyn TextFormatter>>) {
        self.formatter = formatter;
    }

    /// Immutably borrow the active formatter, if any.
    pub fn formatter(&self) -> Option<&dyn TextFormatter> {
        self.formatter.as_deref()
    }

    /// Set a custom syntax/multiline validator.
    pub fn with_validator(mut self, validator: impl Validator + 'static) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    /// Dynamically set or clear the validator.
    pub fn set_validator(&mut self, validator: Option<Box<dyn Validator>>) {
        self.validator = validator;
    }

    /// Immutably borrow the active validator, if any.
    pub fn validator(&self) -> Option<&dyn Validator> {
        self.validator.as_deref()
    }

    /// Immutably borrow the active keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Mutably borrow the active keymap to edit bindings at runtime.
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }

    /// Read a line of input with the given prompt.
    pub fn read_line(&mut self, prompt: impl Prompt) -> io::Result<Signal> {
        let mut line_buffer = LineBuffer::new();

        let _guard = RawModeGuard::new(&mut self.writer)?;

        // Initial render
        let layout = self.render_line(&prompt, &line_buffer, 0)?;
        let mut previous_cursor_row = layout.cursor_row;
        let mut last_end_row = layout.end_row;

        loop {
            let event = event::read()?;

            match event {
                Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                }) => {
                    // Ignore key release events to avoid duplicate execution
                    if kind == KeyEventKind::Release {
                        continue;
                    }

                    let mut changed = false;

                    if let Some(action) = self.keymap.get(code, modifiers) {
                        match action {
                            EditorAction::Submit => {
                                let is_complete = match &self.validator {
                                    Some(v) => matches!(
                                        v.validate(line_buffer.as_str()),
                                        ValidationResult::Complete
                                    ),
                                    None => true,
                                };

                                if is_complete {
                                    // Move cursor past the last rendered row before submitting
                                    let rows_to_move_down =
                                        last_end_row.saturating_sub(previous_cursor_row);
                                    if rows_to_move_down > 0 {
                                        queue!(
                                            self.writer,
                                            cursor::MoveDown(rows_to_move_down as u16)
                                        )?;
                                    }
                                    queue!(self.writer, Print("\r\n"))?;
                                    self.writer.flush()?;
                                    return Ok(Signal::Success(line_buffer.into_string()));
                                } else {
                                    // Incomplete input -> query validator if we should indent
                                    let should_indent = self
                                        .validator
                                        .as_ref()
                                        .map_or(false, |v| v.should_indent(line_buffer.current_line_prefix()));
                                    line_buffer
                                        .insert_newline_preserve_indent(should_indent, self.indent_size);
                                    changed = true;
                                }
                            }
                            EditorAction::InsertNewline => {
                                let should_indent = self
                                    .validator
                                    .as_ref()
                                    .map_or(false, |v| v.should_indent(line_buffer.current_line_prefix()));
                                line_buffer
                                    .insert_newline_preserve_indent(should_indent, self.indent_size);
                                changed = true;
                            }
                            EditorAction::Interrupt => {
                                let rows_to_move_down =
                                    last_end_row.saturating_sub(previous_cursor_row);
                                if rows_to_move_down > 0 {
                                    queue!(
                                        self.writer,
                                        cursor::MoveDown(rows_to_move_down as u16)
                                    )?;
                                }
                                queue!(self.writer, Print("^C\r\n"))?;
                                self.writer.flush()?;
                                return Ok(Signal::CtrlC);
                            }
                            EditorAction::Eof if line_buffer.is_empty() => {
                                queue!(self.writer, Print("\r\n"))?;
                                self.writer.flush()?;
                                return Ok(Signal::CtrlD);
                            }
                            EditorAction::MoveLeft => changed = line_buffer.move_left(),
                            EditorAction::MoveRight => changed = line_buffer.move_right(),
                            EditorAction::MoveUp => changed = line_buffer.move_up(),
                            EditorAction::MoveDown => changed = line_buffer.move_down(),
                            EditorAction::MoveWordLeft => {
                                changed = line_buffer.move_word_left()
                            }
                            EditorAction::MoveWordRight => {
                                changed = line_buffer.move_word_right()
                            }
                            EditorAction::MoveHome => changed = line_buffer.move_home(),
                            EditorAction::MoveEnd => changed = line_buffer.move_end(),
                            EditorAction::Backspace => {
                                changed = line_buffer
                                    .delete_prev_grapheme_or_dedent(self.indent_size);
                            }
                            EditorAction::Delete => {
                                changed = line_buffer.delete_next_grapheme()
                            }
                            EditorAction::DeleteWordLeft => {
                                changed = line_buffer.delete_word_left()
                            }
                            EditorAction::DeleteWordRight => {
                                changed = line_buffer.delete_word_right()
                            }
                            EditorAction::ClearToStart => {
                                changed = line_buffer.clear_to_start()
                            }
                            EditorAction::ClearToEnd => changed = line_buffer.clear_to_end(),
                            EditorAction::ClearLine => changed = line_buffer.clear(),
                            EditorAction::Ignore | EditorAction::Eof => {}
                        }
                    } else {
                        // Unbound printable characters are inserted into the buffer
                        match (code, modifiers) {
                            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                                let should_dedent = self
                                    .validator
                                    .as_ref()
                                    .map_or(false, |v| v.should_dedent_on_char(c));
                                if should_dedent {
                                    line_buffer.dedent_line(self.indent_size);
                                }
                                line_buffer.insert_char(c);
                                changed = true;
                            }
                            _ => {}
                        }
                    }

                    // Only redraw if the line buffer or cursor actually changed
                    if changed {
                        let layout =
                            self.render_line(&prompt, &line_buffer, previous_cursor_row)?;
                        previous_cursor_row = layout.cursor_row;
                        last_end_row = layout.end_row;
                    }
                }

                // Bracketed paste event
                Event::Paste(text) => {
                    let normalised = normalise_paste(&text);
                    if !normalised.is_empty() {
                        line_buffer.insert_str(&normalised);
                        let layout =
                            self.render_line(&prompt, &line_buffer, previous_cursor_row)?;
                        previous_cursor_row = layout.cursor_row;
                        last_end_row = layout.end_row;
                    }
                }

                // Terminal resize -> re-render line to adjust layout
                Event::Resize(..) => {
                    let layout =
                        self.render_line(&prompt, &line_buffer, previous_cursor_row)?;
                    previous_cursor_row = layout.cursor_row;
                    last_end_row = layout.end_row;
                }

                _ => {}
            }
        }
    }

    /// Redraws the prompt, buffer (including soft-wrapped and multiline rows), and positions the cursor.
    pub fn render_line(
        &mut self,
        prompt: &impl Prompt,
        line_buffer: &LineBuffer,
        previous_cursor_row: usize,
    ) -> io::Result<ScreenLayout> {
        let term_cols = self.terminal_width();
        let prompt_text = prompt.render();
        let prompt_stripped = crate::prompt::strip_ansi(&prompt_text);
        let indicator_text = prompt.render_multiline_indicator();
        let indicator_stripped = crate::prompt::strip_ansi(&indicator_text);

        let layout = compute_screen_layout(
            &prompt_stripped,
            &indicator_stripped,
            line_buffer,
            term_cols,
        );

        // 1. Move up to row 0 if we were previously on a lower row
        if previous_cursor_row > 0 {
            queue!(self.writer, cursor::MoveUp(previous_cursor_row as u16))?;
        }

        // 2. Clear from row 0 downwards and print the prompt
        queue!(
            self.writer,
            cursor::Hide,
            MoveToColumn(0),
            Clear(ClearType::FromCursorDown),
            Print(&prompt_text),
        )?;

        let styled_text = match &self.formatter {
            Some(f) => f.format(line_buffer.as_str()),
            None => StyledText::plain(line_buffer.as_str()),
        };

        // 3. Render spans, inserting indicator on newlines
        for (style, text) in &styled_text {
            let mut parts = text.split('\n').peekable();
            while let Some(part) = parts.next() {
                if !part.is_empty() {
                    queue!(
                        self.writer,
                        crossterm::style::PrintStyledContent(style.apply(part))
                    )?;
                }
                if parts.peek().is_some() {
                    queue!(
                        self.writer,
                        crossterm::style::ResetColor,
                        Print("\r\n"),
                        Print(&indicator_text),
                    )?;
                }
            }
        }
        queue!(self.writer, crossterm::style::ResetColor)?;

        // 4. Move cursor from the end of the rendered content back to layout.cursor_row
        let rows_to_move_up = layout.end_row.saturating_sub(layout.cursor_row);
        if rows_to_move_up > 0 {
            queue!(self.writer, cursor::MoveUp(rows_to_move_up as u16))?;
        }

        // 5. Position cursor horizontally at layout.cursor_col
        queue!(
            self.writer,
            MoveToColumn(layout.cursor_col as u16),
            cursor::Show,
        )?;
        self.writer.flush()?;

        Ok(layout)
    }

    /// Immutably borrow the underlying writer.
    pub fn writer(&self) -> &W {
        &self.writer
    }

    /// Mutably borrow the underlying writer.
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Consume the editor and return the underlying writer.
    pub fn into_writer(self) -> W {
        self.writer
    }
}

impl Default for Editor<BufWriter<Stdout>> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// The user typed a command and pressed Enter.
    Success(String),

    /// The user pressed Ctrl+C.
    /// In a shell, this means: cancel the current input, print a newline,
    /// and show a fresh prompt (do not execute anything).
    CtrlC,

    /// The user pressed Ctrl+D on an empty line.
    /// In Unix shells, this signals EOF (End of File), meaning "exit the shell".
    CtrlD,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::KeyCombo;

    #[test]
    fn test_editor_creation_default() {
        let editor = Editor::new();
        assert!(
            editor
                .keymap()
                .contains((KeyCode::Enter, KeyModifiers::NONE))
        );
    }

    #[test]
    fn test_editor_with_custom_writer_and_keymap() {
        let buffer = Vec::new();
        let mut keymap = Keymap::empty();
        keymap.bind(KeyCombo::ctrl('q'), EditorAction::Interrupt);

        let mut editor = Editor::with_writer(buffer).with_keymap(keymap);
        assert!(editor.keymap().contains(KeyCombo::ctrl('q')));
        assert!(!editor.keymap().contains(KeyCode::Enter));

        editor
            .keymap_mut()
            .bind(KeyCode::Enter, EditorAction::Submit);
        assert!(editor.keymap().contains(KeyCode::Enter));
    }

    #[test]
    fn test_render_line_to_buffer() {
        let buffer = Vec::new();
        let mut editor = Editor::with_writer(buffer);
        let line_buf = LineBuffer::with_content("echo 1".to_string());
        let layout = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 8); // "> " (2) + "echo 1" (6)
        let out = editor.into_writer();
        assert!(!out.is_empty());
    }

    #[test]
    fn test_render_line_with_formatter() {
        let buffer = Vec::new();
        let mut editor =
            Editor::with_writer(buffer).with_formatter(|s: &str| format!("***{}***", s));
        let line_buf = LineBuffer::with_content("secret".to_string());
        let layout = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 0);
        let out = String::from_utf8(editor.into_writer()).unwrap();
        assert!(out.contains("***secret***"));
    }

    #[test]
    fn test_render_multiline() {
        let buffer = Vec::new();
        let mut editor = Editor::with_writer(buffer);
        let line_buf = LineBuffer::with_content("first\nsecond".to_string());
        let layout = editor.render_line(&"> ", &line_buf, 0).unwrap();
        let out = String::from_utf8(editor.into_writer()).unwrap();
        // Should contain continuation indicator "… "
        assert!(out.contains("… "));
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.end_row, 1);
    }

    #[test]
    fn test_compute_screen_layout_soft_wrap() {
        // Terminal of width 10 columns
        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("12345678"); // 8 chars. Prompt is "> " (2 chars). Total = 10 chars.

        // At end of row (10 chars in 10-col terminal)
        let layout = compute_screen_layout("> ", "… ", &line_buf, 10);
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 9); // sits at right margin
        assert_eq!(layout.end_row, 0);
        assert_eq!(layout.end_col, 9);

        // Type 1 more character -> wraps to row 1, col 1
        line_buf.insert_char('9');
        let layout = compute_screen_layout("> ", "… ", &line_buf, 10);
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.cursor_col, 1);
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 1);

        // Move cursor back to beginning: cursor sits on row 0, col 2 (right after prompt)
        line_buf.move_home();
        let layout = compute_screen_layout("> ", "… ", &line_buf, 10);
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 2);
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 1);
    }

    #[test]
    fn test_compute_screen_layout_wide_characters() {
        // Terminal of width 10 columns
        let mut line_buf = LineBuffer::new();
        // Prompt is "> " (2 cols)
        // Insert 7 single-width chars -> col reaches 9 (1 col left)
        line_buf.insert_str("1234567");

        let layout = compute_screen_layout("> ", "… ", &line_buf, 10);
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 9);

        // Insert double-width emoji 🦀 (width 2). It cannot fit in col 9, so it wraps to row 1!
        line_buf.insert_str("🦀");
        let layout = compute_screen_layout("> ", "… ", &line_buf, 10);
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.cursor_col, 2); // on row 1, occupies cols 0 and 1, cursor at 2
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 2);
    }

    #[test]
    fn test_render_line_with_soft_wrap_override() {
        let buffer = Vec::new();
        let mut editor = Editor::with_writer(buffer).with_terminal_width(10);
        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("123456789"); // 2 + 9 = 11 cols in 10-col terminal

        let layout = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.cursor_col, 1);
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 1);
    }

    #[test]
    fn test_normalise_paste_crlf_and_cr() {
        let input = "line1\r\nline2\rline3\nline4";
        let normalised = normalise_paste(input);
        assert_eq!(normalised, "line1\nline2\nline3\nline4");
    }

    #[test]
    fn test_normalise_paste_preserves_tabs_and_strips_control_chars() {
        let input = "hello\tworld\x00\x1b[31mred\x1b[0m\x08test";
        let normalised = normalise_paste(input);
        // \x00, \x1b, and \x08 are stripped; \t is retained
        assert_eq!(normalised, "hello\tworld[31mred[0mtest");
    }

    #[test]
    fn test_normalise_paste_multiline_code_block() {
        let input = "def foo():\r\n    return 42\r\n";
        let normalised = normalise_paste(input);
        assert_eq!(normalised, "def foo():\n    return 42\n");
    }
}
