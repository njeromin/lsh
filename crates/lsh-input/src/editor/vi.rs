use std::io::Write;

use crossterm::{
    event::{KeyCode, KeyModifiers},
    queue,
};

use crate::edit_mode::{ViMode, ViOperator};

use super::Editor;
use super::session::EditorSession;

impl<W: Write> Editor<W> {
    /// Handles a pending Vi operator (`d`, `c`, `y`, `r`) waiting for a motion or character.
    ///
    /// Returns `None` if the operator was cancelled by Escape or Control (falling through to keymap),
    /// or `Some(changed)` indicating whether the buffer or editor state changed.
    pub(crate) fn handle_vi_operator(
        &mut self,
        op: ViOperator,
        code: KeyCode,
        modifiers: KeyModifiers,
        session: &mut EditorSession,
    ) -> Option<bool> {
        session.in_insert_session = false;

        if modifiers.contains(KeyModifiers::CONTROL) || code == KeyCode::Esc {
            return None;
        }

        let mut changed = false;

        match op {
            ViOperator::ReplaceChar => {
                if !(matches!(code, KeyCode::Esc)
                    || (code == KeyCode::Char('[') && modifiers.contains(KeyModifiers::CONTROL)))
                    && let (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) =
                        (code, modifiers)
                {
                    session.save_undo();
                    changed = session.line_buffer.replace_char_at_cursor(c);
                }
            }
            ViOperator::Delete => match code {
                KeyCode::Char('d') => {
                    session.save_undo();
                    if let Some(cut) = session.line_buffer.cut_line() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set(&cut);
                        }
                        changed = true;
                    }
                }
                KeyCode::Char('w') => {
                    session.save_undo();
                    if let Some(cut) = session.line_buffer.cut_word_right() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set(&cut);
                        }
                        if session.line_buffer.cursor() > 0
                            && session.line_buffer.cursor() >= session.line_buffer.len()
                        {
                            session.line_buffer.move_left();
                        }
                        changed = true;
                    }
                }
                KeyCode::Char('b') => {
                    session.save_undo();
                    if let Some(cut) = session.line_buffer.cut_word_left() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set(&cut);
                        }
                        changed = true;
                    }
                }
                KeyCode::Char('e') => {
                    session.save_undo();
                    if let Some(cut) = session.line_buffer.cut_to_word_end() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set(&cut);
                        }
                        changed = true;
                    }
                }
                KeyCode::Char('$') => {
                    session.save_undo();
                    if let Some(cut) = session.line_buffer.cut_to_end() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set(&cut);
                        }
                        if session.line_buffer.cursor() > 0
                            && session.line_buffer.cursor() >= session.line_buffer.len()
                        {
                            session.line_buffer.move_left();
                        }
                        changed = true;
                    }
                }
                KeyCode::Char('0') => {
                    session.save_undo();
                    if let Some(cut) = session.line_buffer.cut_to_start() {
                        if let Some(cb) = &mut self.clipboard {
                            cb.set(&cut);
                        }
                        changed = true;
                    }
                }
                _ => {} // Cancelled motion
            },
            ViOperator::Change => {
                let mut enter_insert = false;
                match code {
                    KeyCode::Char('c') => {
                        session.save_undo();
                        if let Some(cut) = session.line_buffer.cut_line()
                            && let Some(cb) = &mut self.clipboard
                        {
                            cb.set(&cut);
                        }
                        enter_insert = true;
                    }
                    KeyCode::Char('w' | 'e') => {
                        session.save_undo();
                        if let Some(cut) = session.line_buffer.cut_to_word_end()
                            && let Some(cb) = &mut self.clipboard
                        {
                            cb.set(&cut);
                        }
                        enter_insert = true;
                    }
                    KeyCode::Char('b') => {
                        session.save_undo();
                        if let Some(cut) = session.line_buffer.cut_word_left()
                            && let Some(cb) = &mut self.clipboard
                        {
                            cb.set(&cut);
                        }
                        enter_insert = true;
                    }
                    KeyCode::Char('$') => {
                        session.save_undo();
                        if let Some(cut) = session.line_buffer.cut_to_end()
                            && let Some(cb) = &mut self.clipboard
                        {
                            cb.set(&cut);
                        }
                        enter_insert = true;
                    }
                    KeyCode::Char('0') => {
                        session.save_undo();
                        if let Some(cut) = session.line_buffer.cut_to_start()
                            && let Some(cb) = &mut self.clipboard
                        {
                            cb.set(&cut);
                        }
                        enter_insert = true;
                    }
                    _ => {} // Cancelled motion
                }
                if enter_insert {
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                    changed = true;
                }
            }
            ViOperator::Yank => match code {
                KeyCode::Char('y') => {
                    if let Some(cb) = &mut self.clipboard
                        && let Some(text) = session.line_buffer.yank_line()
                    {
                        cb.set(&text);
                    }
                }
                KeyCode::Char('w') => {
                    if let Some(cb) = &mut self.clipboard
                        && let Some(text) = session.line_buffer.yank_word()
                    {
                        cb.set(&text);
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(cb) = &mut self.clipboard
                        && let Some(text) = session.line_buffer.yank_to_word_end()
                    {
                        cb.set(&text);
                    }
                }
                KeyCode::Char('$') => {
                    if let Some(cb) = &mut self.clipboard
                        && let Some(text) = session.line_buffer.yank_to_end()
                    {
                        cb.set(&text);
                    }
                }
                KeyCode::Char('0') => {
                    if let Some(cb) = &mut self.clipboard
                        && session.line_buffer.cursor() > 0
                    {
                        cb.set(&session.line_buffer.as_str()[..session.line_buffer.cursor()]);
                    }
                }
                _ => {} // Cancelled motion
            },
        }

        Some(changed)
    }
}
