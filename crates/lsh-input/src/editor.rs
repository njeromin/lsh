use std::io::{self, BufWriter, Stdout, Write};

use crossterm::{
    cursor::{self, MoveToColumn},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};

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
}

impl Editor<BufWriter<Stdout>> {
    /// Create a new editor writing to standard output wrapped in a `BufWriter` with default keybindings.
    pub fn new() -> Self {
        Self {
            writer: BufWriter::new(io::stdout()),
            keymap: Keymap::default(),
        }
    }
}

impl<W: Write> Editor<W> {
    /// Create an editor with a custom writer (e.g. `stderr`, `Vec<u8>`, or a custom buffer).
    pub fn with_writer(writer: W) -> Self {
        Self {
            writer,
            keymap: Keymap::default(),
        }
    }

    /// Set a custom keymap.
    pub fn with_keymap(mut self, keymap: Keymap) -> Self {
        self.keymap = keymap;
        self
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

        let mut buf = String::new();
        let mut cursor_pos = 0;

        let _guard = RawModeGuard::new(&mut self.writer)?;

        // Initial render
        self.render_line(&prompt_text, prompt_width, &buf, cursor_pos)?;

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
                                return Ok(Signal::Success(buf));
                            }
                            EditorAction::Interrupt => {
                                queue!(self.writer, Print("^C\r\n"))?;
                                self.writer.flush()?;
                                return Ok(Signal::CtrlC);
                            }
                            EditorAction::Eof if buf.is_empty() => {
                                queue!(self.writer, Print("\r\n"))?;
                                self.writer.flush()?;
                                return Ok(Signal::CtrlD);
                            }
                            EditorAction::MoveLeft if cursor_pos > 0 => {
                                cursor_pos -= 1;
                                changed = true;
                            }
                            EditorAction::MoveRight if cursor_pos < buf.len() => {
                                cursor_pos += 1;
                                changed = true;
                            }
                            EditorAction::MoveWordLeft if cursor_pos > 0 => {
                                let new_pos = prev_word_boundary(&buf, cursor_pos);
                                if new_pos != cursor_pos {
                                    cursor_pos = new_pos;
                                    changed = true;
                                }
                            }
                            EditorAction::MoveWordRight if cursor_pos < buf.len() => {
                                let new_pos = next_word_boundary(&buf, cursor_pos);
                                if new_pos != cursor_pos {
                                    cursor_pos = new_pos;
                                    changed = true;
                                }
                            }
                            EditorAction::MoveHome if cursor_pos > 0 => {
                                cursor_pos = 0;
                                changed = true;
                            }
                            EditorAction::MoveEnd if cursor_pos < buf.len() => {
                                cursor_pos = buf.len();
                                changed = true;
                            }
                            EditorAction::Backspace if cursor_pos > 0 => {
                                cursor_pos -= 1;
                                buf.remove(cursor_pos);
                                changed = true;
                            }
                            EditorAction::Delete if cursor_pos < buf.len() => {
                                buf.remove(cursor_pos);
                                changed = true;
                            }
                            EditorAction::DeleteWordLeft if cursor_pos > 0 => {
                                let target = prev_word_boundary(&buf, cursor_pos);
                                buf.drain(target..cursor_pos);
                                cursor_pos = target;
                                changed = true;
                            }
                            EditorAction::DeleteWordRight if cursor_pos < buf.len() => {
                                let target = next_word_boundary(&buf, cursor_pos);
                                buf.drain(cursor_pos..target);
                                changed = true;
                            }
                            EditorAction::ClearToStart if cursor_pos > 0 => {
                                buf.drain(..cursor_pos);
                                cursor_pos = 0;
                                changed = true;
                            }
                            EditorAction::ClearToEnd if cursor_pos < buf.len() => {
                                buf.truncate(cursor_pos);
                                changed = true;
                            }
                            EditorAction::ClearLine => {
                                if !buf.is_empty() {
                                    buf.clear();
                                    cursor_pos = 0;
                                    changed = true;
                                }
                            }
                            EditorAction::Ignore | EditorAction::Eof => {}
                            _ => {}
                        }
                    } else {
                        // Unbound printable characters are inserted into the buffer
                        match (code, modifiers) {
                            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                                buf.insert(cursor_pos, c);
                                cursor_pos += 1;
                                changed = true;
                            }
                            _ => {}
                        }
                    }

                    // Only redraw if the line buffer or cursor actually changed
                    if changed {
                        self.render_line(&prompt_text, prompt_width, &buf, cursor_pos)?;
                    }
                }

                // Terminal resize -> re-render line to adjust layout
                Event::Resize(..) => {
                    self.render_line(&prompt_text, prompt_width, &buf, cursor_pos)?;
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
        buffer: &str,
        cursor_pos: usize,
    ) -> io::Result<()> {
        queue!(
            self.writer,
            cursor::Hide,
            MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            Print(prompt_text),
            Print(buffer),
            // Position cursor: prompt visual width + characters typed before cursor
            MoveToColumn((prompt_width + cursor_pos) as u16),
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

/// Find previous word boundary before `cursor_pos`.
fn prev_word_boundary(buf: &str, cursor_pos: usize) -> usize {
    let prefix = &buf[..cursor_pos];
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

/// Find next word boundary after `cursor_pos`.
fn next_word_boundary(buf: &str, cursor_pos: usize) -> usize {
    let suffix = &buf[cursor_pos..];
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
        .map(|&(idx, _)| cursor_pos + idx)
        .unwrap_or(buf.len())
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
    fn test_word_boundaries() {
        let s = "hello   world  test";
        // From after "world  " (index 15) -> prev word boundary should be index 8 ("world")
        assert_eq!(prev_word_boundary(s, 15), 8);
        // From start of "world" (index 8) -> prev word should be index 0 ("hello")
        assert_eq!(prev_word_boundary(s, 8), 0);

        // From index 0 -> next word boundary should be index 8 (start of "world")
        assert_eq!(next_word_boundary(s, 0), 8);
        // From index 8 -> next word boundary should be index 15 (start of "test")
        assert_eq!(next_word_boundary(s, 8), 15);
    }

    #[test]
    fn test_render_line_to_buffer() {
        let buffer = Vec::new();
        let mut editor = Editor::with_writer(buffer);
        editor.render_line("> ", 2, "echo 1", 6).unwrap();
        let out = editor.into_writer();
        assert!(!out.is_empty());
    }
}
