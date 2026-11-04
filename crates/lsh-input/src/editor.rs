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
}

impl Editor<BufWriter<Stdout>> {
    /// Create a new editor writing to standard output wrapped in a `BufWriter` with default keybindings.
    pub fn new() -> Self {
        Self {
            writer: BufWriter::new(io::stdout()),
            keymap: Keymap::default(),
            formatter: None,
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
        }
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
        let prompt_text = prompt.render();
        let prompt_width = crate::prompt::printable_width(&prompt_text);

        let mut line_buffer = LineBuffer::new();

        let _guard = RawModeGuard::new(&mut self.writer)?;

        // Initial render
        self.render_line(&prompt_text, prompt_width, &line_buffer)?;

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
                                queue!(self.writer, Print("\r\n"))?;
                                self.writer.flush()?;
                                return Ok(Signal::Success(line_buffer.into_string()));
                            }
                            EditorAction::Interrupt => {
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
                            EditorAction::MoveWordLeft => {
                                changed = line_buffer.move_word_left()
                            }
                            EditorAction::MoveWordRight => {
                                changed = line_buffer.move_word_right()
                            }
                            EditorAction::MoveHome => changed = line_buffer.move_home(),
                            EditorAction::MoveEnd => changed = line_buffer.move_end(),
                            EditorAction::Backspace => {
                                changed = line_buffer.delete_prev_grapheme()
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
                                line_buffer.insert_char(c);
                                changed = true;
                            }
                            _ => {}
                        }
                    }

                    // Only redraw if the line buffer or cursor actually changed
                    if changed {
                        self.render_line(&prompt_text, prompt_width, &line_buffer)?;
                    }
                }

                // Terminal resize -> re-render line to adjust layout
                Event::Resize(..) => {
                    self.render_line(&prompt_text, prompt_width, &line_buffer)?;
                }

                _ => {}
            }
        }
    }

    /// Redraws the prompt, buffer, and positions the cursor.
    pub fn render_line(
        &mut self,
        prompt_text: &str,
        prompt_width: usize,
        line_buffer: &LineBuffer,
    ) -> io::Result<()> {
        let styled_text = match &self.formatter {
            Some(f) => f.format(line_buffer.as_str()),
            None => StyledText::plain(line_buffer.as_str()),
        };

        queue!(
            self.writer,
            cursor::Hide,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Print(prompt_text),
        )?;

        for (style, text) in &styled_text {
            queue!(
                self.writer,
                crossterm::style::PrintStyledContent(style.apply(text))
            )?;
        }
        queue!(self.writer, crossterm::style::ResetColor)?;

        queue!(
            self.writer,
            // Position cursor: prompt visual width + visual width of buffer up to cursor
            MoveToColumn((prompt_width + line_buffer.cursor_visual_width()) as u16),
            cursor::Show,
        )?;
        self.writer.flush()
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
        editor.render_line("> ", 2, &line_buf).unwrap();
        let out = editor.into_writer();
        assert!(!out.is_empty());
    }

    #[test]
    fn test_render_line_with_formatter() {
        let buffer = Vec::new();
        let mut editor =
            Editor::with_writer(buffer).with_formatter(|s: &str| format!("***{}***", s));
        let line_buf = LineBuffer::with_content("secret".to_string());
        editor.render_line("> ", 2, &line_buf).unwrap();
        let out = String::from_utf8(editor.into_writer()).unwrap();
        assert!(out.contains("***secret***"));
    }
}
