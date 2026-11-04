use std::io::{self, BufWriter, Stdout, Write};

use crossterm::{
    cursor::{self, MoveToColumn},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};

use crate::buffer::LineBuffer;
use crate::formatter::{StyledText, TextFormatter};
use crate::keymap::{EditorAction, Keymap};
use crate::prompt::Prompt;
use crate::validator::{ValidationResult, Validator};

/// RAII guard to ensure terminal raw mode is disabled when exiting `read_line`,
/// even in the case of errors or panics.
struct RawModeGuard {
    enhanced_keyboard: bool,
}

impl RawModeGuard {
    fn new<W: Write>(writer: &mut W) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let enhanced_keyboard = matches!(terminal::supports_keyboard_enhancement(), Ok(true));
        if enhanced_keyboard {
            let _ = queue!(
                writer,
                event::PushKeyboardEnhancementFlags(
                    event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            );
            let _ = writer.flush();
        }
        Ok(Self { enhanced_keyboard })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.enhanced_keyboard {
            let mut stdout = io::stdout();
            let _ = queue!(stdout, event::PopKeyboardEnhancementFlags);
            let _ = stdout.flush();
        }
        let _ = terminal::disable_raw_mode();
    }
}

pub struct Editor<W: Write = BufWriter<Stdout>> {
    writer: W,
    keymap: Keymap,
    formatter: Option<Box<dyn TextFormatter>>,
    validator: Option<Box<dyn Validator>>,
    indent_size: usize,
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
        }
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
        let mut previous_cursor_row = 0;

        let _guard = RawModeGuard::new(&mut self.writer)?;

        // Initial render
        self.render_line(&prompt, &line_buffer, &mut previous_cursor_row)?;

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
                                    let total_rows =
                                        line_buffer.line_count().saturating_sub(1);
                                    let rows_to_move_down =
                                        total_rows.saturating_sub(previous_cursor_row);
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
                                let total_rows =
                                    line_buffer.line_count().saturating_sub(1);
                                let rows_to_move_down =
                                    total_rows.saturating_sub(previous_cursor_row);
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
                        self.render_line(&prompt, &line_buffer, &mut previous_cursor_row)?;
                    }
                }

                // Terminal resize -> re-render line to adjust layout
                Event::Resize(..) => {
                    self.render_line(&prompt, &line_buffer, &mut previous_cursor_row)?;
                }

                _ => {}
            }
        }
    }

    /// Redraws the prompt, buffer (including multiline rows), and positions the cursor.
    pub fn render_line(
        &mut self,
        prompt: &impl Prompt,
        line_buffer: &LineBuffer,
        previous_cursor_row: &mut usize,
    ) -> io::Result<()> {
        let prompt_text = prompt.render();
        let prompt_width = crate::prompt::printable_width(&prompt_text);
        let indicator_text = prompt.render_multiline_indicator();
        let indicator_width = crate::prompt::printable_width(&indicator_text);

        // 1. Move up to row 0 if we were previously on a lower row
        if *previous_cursor_row > 0 {
            queue!(self.writer, cursor::MoveUp(*previous_cursor_row as u16))?;
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

        // 4. Calculate target cursor row and column
        let pos = line_buffer.cursor_position();
        let target_row = pos.line_index;
        let target_col = if target_row == 0 {
            prompt_width + pos.col_visual_width
        } else {
            indicator_width + pos.col_visual_width
        };

        // 5. Position cursor: we are currently on the last rendered line
        let total_rendered_rows = line_buffer.line_count().saturating_sub(1);
        let rows_to_move_up = total_rendered_rows.saturating_sub(target_row);

        if rows_to_move_up > 0 {
            queue!(self.writer, cursor::MoveUp(rows_to_move_up as u16))?;
        }

        queue!(
            self.writer,
            MoveToColumn(target_col as u16),
            cursor::Show,
        )?;
        self.writer.flush()?;

        *previous_cursor_row = target_row;
        Ok(())
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
        let mut prev_row = 0;
        editor.render_line(&"> ", &line_buf, &mut prev_row).unwrap();
        let out = editor.into_writer();
        assert!(!out.is_empty());
    }

    #[test]
    fn test_render_line_with_formatter() {
        let buffer = Vec::new();
        let mut editor =
            Editor::with_writer(buffer).with_formatter(|s: &str| format!("***{}***", s));
        let line_buf = LineBuffer::with_content("secret".to_string());
        let mut prev_row = 0;
        editor.render_line(&"> ", &line_buf, &mut prev_row).unwrap();
        let out = String::from_utf8(editor.into_writer()).unwrap();
        assert!(out.contains("***secret***"));
    }

    #[test]
    fn test_render_multiline() {
        let buffer = Vec::new();
        let mut editor = Editor::with_writer(buffer);
        let line_buf = LineBuffer::with_content("first\nsecond".to_string());
        let mut prev_row = 0;
        editor.render_line(&"> ", &line_buf, &mut prev_row).unwrap();
        let out = String::from_utf8(editor.into_writer()).unwrap();
        // Should contain continuation indicator "… "
        assert!(out.contains("… "));
        assert_eq!(prev_row, 1);
    }
}
