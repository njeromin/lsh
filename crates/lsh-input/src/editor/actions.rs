use std::io::{self, Write};

use crossterm::{
    cursor, queue,
    style::Print,
    terminal::{Clear, ClearType},
};

use crate::edit_mode::{EditMode, ViMode, ViOperator};
use crate::hinter::default_next_hint_word;
use crate::keymap::EditorAction;
use crate::prompt::Prompt;
use crate::validator::ValidationResult;

use super::Editor;
use super::Signal;
use super::paste::normalise_paste;
use super::session::EditorSession;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ActionResult {
    Noop,
    Changed,
    Signal(Signal),
}

impl<W: Write> Editor<W> {
    /// Handles an `EditorAction` resulting from keymap lookup.
    pub(crate) fn handle_action(
        &mut self,
        action: EditorAction,
        prompt: &impl Prompt,
        session: &mut EditorSession,
        previous_cursor_row: &mut usize,
        current_hint: &Option<String>,
    ) -> io::Result<ActionResult> {
        let mut changed = false;

        match action {
            EditorAction::Complete => {
                if let Some(menu) = &mut self.menu {
                    menu.activate(&mut session.line_buffer, self.completer.as_deref());
                    changed = true;
                } else if let Some(completer) = &self.completer {
                    let (start, candidates) = completer
                        .complete(session.line_buffer.as_str(), session.line_buffer.cursor());
                    if candidates.len() == 1 {
                        let c = &candidates[0];
                        let mut rep = c.replacement.clone();
                        if c.append_space && !rep.ends_with(' ') {
                            rep.push(' ');
                        }
                        session
                            .line_buffer
                            .replace_range(start..session.line_buffer.cursor(), &rep);
                        session.line_buffer.set_cursor(start + rep.len());
                        changed = true;
                    }
                }
            }
            EditorAction::CompletePrevious => {
                // If a menu is active, handle_key already dealt with reverse cycling
            }
            EditorAction::Submit => {
                let is_complete = match &self.validator {
                    Some(v) => matches!(
                        v.validate(session.line_buffer.as_str()),
                        ValidationResult::Complete
                    ),
                    None => true,
                };

                if is_complete {
                    if let Some(menu) = &mut self.menu {
                        menu.dismiss(&mut session.line_buffer);
                    }
                    // Cleanly erase any ghost text / menu before moving to next line
                    let (final_layout, _) = self.render_line_internal(
                        prompt,
                        &session.line_buffer,
                        *previous_cursor_row,
                        false,
                    )?;
                    let rows_to_move_down =
                        final_layout.end_row.saturating_sub(final_layout.cursor_row);
                    if rows_to_move_down > 0 {
                        queue!(self.writer, cursor::MoveDown(rows_to_move_down as u16))?;
                    }
                    queue!(self.writer, Print("\r\n"))?;
                    self.writer.flush()?;
                    let line = std::mem::take(&mut session.line_buffer).into_string();
                    if let Some(history) = &mut self.history {
                        history.append(&line);
                    }
                    return Ok(ActionResult::Signal(Signal::Success(line)));
                } else {
                    // Incomplete input -> query validator if we should indent
                    let should_indent = self.validator.as_ref().is_some_and(|v| {
                        v.should_indent(session.line_buffer.current_line_prefix())
                    });
                    session
                        .line_buffer
                        .insert_newline_preserve_indent(should_indent, self.indent_size);
                    changed = true;
                }
            }
            EditorAction::InsertNewline => {
                if let Some(menu) = &mut self.menu {
                    menu.dismiss(&mut session.line_buffer);
                }
                let should_indent = self.validator.as_ref().is_some_and(|v| {
                    v.should_indent(session.line_buffer.current_line_prefix())
                });
                session
                    .line_buffer
                    .insert_newline_preserve_indent(should_indent, self.indent_size);
                changed = true;
            }
            EditorAction::Interrupt => {
                if let Some(menu) = &mut self.menu {
                    menu.dismiss(&mut session.line_buffer);
                }
                // Cleanly erase any ghost text / menu before printing ^C
                let (final_layout, _) = self.render_line_internal(
                    prompt,
                    &session.line_buffer,
                    *previous_cursor_row,
                    false,
                )?;
                let rows_to_move_down =
                    final_layout.end_row.saturating_sub(final_layout.cursor_row);
                if rows_to_move_down > 0 {
                    queue!(self.writer, cursor::MoveDown(rows_to_move_down as u16))?;
                }
                queue!(self.writer, Print("^C\r\n"))?;
                self.writer.flush()?;
                return Ok(ActionResult::Signal(Signal::CtrlC));
            }
            EditorAction::Eof if session.line_buffer.is_empty() => {
                queue!(self.writer, Print("\r\n"))?;
                self.writer.flush()?;
                return Ok(ActionResult::Signal(Signal::CtrlD));
            }
            EditorAction::AcceptGhostText => {
                if let Some(hint) = current_hint {
                    session.line_buffer.insert_str(hint);
                    changed = true;
                }
            }
            EditorAction::AcceptGhostTextWord => {
                if let Some(hint) = current_hint {
                    let word = self.hinter.as_ref().map_or_else(
                        || default_next_hint_word(hint),
                        |h| h.next_hint_word(hint),
                    );
                    if !word.is_empty() {
                        session.line_buffer.insert_str(word);
                        changed = true;
                    }
                }
            }
            EditorAction::MoveLeft => {
                session.in_insert_session = false;
                changed = session.line_buffer.move_left();
            }
            EditorAction::MoveRight => {
                session.in_insert_session = false;
                if self.edit_mode == EditMode::Vi && self.vi_mode == ViMode::Normal {
                    if session.line_buffer.cursor() + 1 < session.line_buffer.len() {
                        changed = session.line_buffer.move_right();
                    }
                } else if !session.line_buffer.move_right() {
                    // At end of line: accept ghost text if present
                    if let Some(hint) = current_hint {
                        session.line_buffer.insert_str(hint);
                        changed = true;
                    }
                } else {
                    changed = true;
                }
            }
            EditorAction::MoveUp => {
                session.in_insert_session = false;
                if session.line_buffer.move_up() {
                    changed = true;
                } else if let Some(history) = self.history.as_deref() {
                    changed = session.history_move_up(history);
                }
            }
            EditorAction::MoveDown => {
                session.in_insert_session = false;
                if session.line_buffer.move_down() {
                    changed = true;
                } else if let Some(history) = self.history.as_deref() {
                    changed = session.history_move_down(history);
                }
            }
            EditorAction::MoveWordLeft => {
                session.in_insert_session = false;
                changed = session.line_buffer.move_word_left();
            }
            EditorAction::MoveWordRight => {
                session.in_insert_session = false;
                if !session.line_buffer.move_word_right() {
                    // At end of line: accept next hint word if present
                    if let Some(hint) = current_hint {
                        let word = self.hinter.as_ref().map_or_else(
                            || default_next_hint_word(hint),
                            |h| h.next_hint_word(hint),
                        );
                        if !word.is_empty() {
                            session.line_buffer.insert_str(word);
                            changed = true;
                        }
                    }
                } else {
                    if self.edit_mode == EditMode::Vi
                        && self.vi_mode == ViMode::Normal
                        && session.line_buffer.cursor() > 0
                        && session.line_buffer.cursor() == session.line_buffer.len()
                    {
                        session.line_buffer.move_left();
                    }
                    changed = true;
                }
            }
            EditorAction::MoveWordEnd => {
                session.in_insert_session = false;
                changed = session.line_buffer.move_word_end();
            }
            EditorAction::MoveHome => {
                session.in_insert_session = false;
                changed = session.line_buffer.move_home();
            }
            EditorAction::MoveToFirstNonWhitespace => {
                session.in_insert_session = false;
                changed = session.line_buffer.move_to_first_non_whitespace();
            }
            EditorAction::MoveEnd => {
                session.in_insert_session = false;
                if !session.line_buffer.move_end() {
                    // Already at end: accept ghost text if present
                    if let Some(hint) = current_hint {
                        session.line_buffer.insert_str(hint);
                        changed = true;
                    }
                } else {
                    if self.edit_mode == EditMode::Vi
                        && self.vi_mode == ViMode::Normal
                        && session.line_buffer.cursor() > 0
                        && session.line_buffer.cursor() == session.line_buffer.len()
                    {
                        session.line_buffer.move_left();
                    }
                    changed = true;
                }
            }
            EditorAction::Backspace => {
                if !session.in_insert_session {
                    session.save_undo();
                    session.in_insert_session = true;
                }
                changed = session
                    .line_buffer
                    .delete_prev_grapheme_or_dedent(self.indent_size);
            }
            EditorAction::Delete => {
                if !session.in_insert_session {
                    session.save_undo();
                    session.in_insert_session = true;
                }
                changed = session.line_buffer.delete_next_grapheme();
            }
            EditorAction::DeleteWordLeft | EditorAction::CutWordLeft => {
                session.save_undo();
                session.in_insert_session = false;
                if let Some(cut) = session.line_buffer.cut_word_left() {
                    if let Some(cb) = &mut self.clipboard {
                        cb.set(&cut);
                    }
                    changed = true;
                }
            }
            EditorAction::DeleteWordRight | EditorAction::CutWordRight => {
                session.save_undo();
                session.in_insert_session = false;
                if let Some(cut) = session.line_buffer.cut_word_right() {
                    if let Some(cb) = &mut self.clipboard {
                        cb.set(&cut);
                    }
                    changed = true;
                }
            }
            EditorAction::ClearToStart | EditorAction::CutToStart => {
                session.save_undo();
                session.in_insert_session = false;
                if let Some(cut) = session.line_buffer.cut_to_start() {
                    if let Some(cb) = &mut self.clipboard {
                        cb.set(&cut);
                    }
                    changed = true;
                }
            }
            EditorAction::ClearToEnd | EditorAction::CutToEnd => {
                session.save_undo();
                session.in_insert_session = false;
                if let Some(cut) = session.line_buffer.cut_to_end() {
                    if let Some(cb) = &mut self.clipboard {
                        cb.set(&cut);
                    }
                    changed = true;
                }
            }
            EditorAction::CutChar => {
                session.save_undo();
                session.in_insert_session = false;
                if let Some(cut) = session.line_buffer.cut_char_under_cursor() {
                    if let Some(cb) = &mut self.clipboard {
                        cb.set(&cut);
                    }
                    changed = true;
                }
            }
            EditorAction::CutCharBefore => {
                session.save_undo();
                session.in_insert_session = false;
                if let Some(cut) = session.line_buffer.cut_char_before_cursor() {
                    if let Some(cb) = &mut self.clipboard {
                        cb.set(&cut);
                    }
                    changed = true;
                }
            }
            EditorAction::Yank => {
                if let Some(cb) = &mut self.clipboard
                    && let Some(text) = cb.get()
                {
                    let normalised = normalise_paste(&text);
                    if !normalised.is_empty() {
                        session.save_undo();
                        session.in_insert_session = false;
                        session.line_buffer.insert_str(&normalised);
                        changed = true;
                    }
                }
            }
            EditorAction::PasteAfter => {
                if let Some(cb) = &mut self.clipboard
                    && let Some(text) = cb.get()
                {
                    let normalised = normalise_paste(&text);
                    if !normalised.is_empty() {
                        session.save_undo();
                        session.in_insert_session = false;
                        session.line_buffer.paste_after(&normalised);
                        changed = true;
                    }
                }
            }
            EditorAction::PasteBefore => {
                if let Some(cb) = &mut self.clipboard
                    && let Some(text) = cb.get()
                {
                    let normalised = normalise_paste(&text);
                    if !normalised.is_empty() {
                        session.save_undo();
                        session.in_insert_session = false;
                        session.line_buffer.paste_before(&normalised);
                        changed = true;
                    }
                }
            }
            EditorAction::YankLine => {
                if let Some(cb) = &mut self.clipboard
                    && let Some(text) = session.line_buffer.yank_line()
                {
                    cb.set(&text);
                }
            }
            EditorAction::ToggleCase => {
                session.save_undo();
                session.in_insert_session = false;
                changed = session.line_buffer.toggle_case_at_cursor();
            }
            EditorAction::Undo => {
                changed = session.undo();
            }
            EditorAction::Redo => {
                changed = session.redo();
            }
            EditorAction::SwitchToNormalMode => {
                session.in_insert_session = false;
                if self.edit_mode == EditMode::Vi && self.vi_mode != ViMode::Normal {
                    self.vi_mode = ViMode::Normal;
                    self.keymap = self.vi_normal_keymap();
                    if session.line_buffer.cursor() > 0 {
                        session.line_buffer.move_left();
                    }
                    let _ = queue!(self.writer, cursor::SetCursorStyle::SteadyBlock);
                    changed = true;
                }
            }
            EditorAction::SwitchToInsertMode => {
                if self.edit_mode == EditMode::Vi && self.vi_mode != ViMode::Insert {
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                    changed = true;
                }
            }
            EditorAction::Append => {
                if self.edit_mode == EditMode::Vi {
                    if session.line_buffer.cursor() < session.line_buffer.len() {
                        session.line_buffer.move_right();
                    }
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                    changed = true;
                }
            }
            EditorAction::AppendEnd => {
                if self.edit_mode == EditMode::Vi {
                    session.line_buffer.move_end();
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                    changed = true;
                }
            }
            EditorAction::InsertAtFirstNonWhitespace => {
                if self.edit_mode == EditMode::Vi {
                    session.line_buffer.move_to_first_non_whitespace();
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                    changed = true;
                }
            }
            EditorAction::OpenBelow => {
                if self.edit_mode == EditMode::Vi {
                    session.save_undo();
                    session.line_buffer.move_end();
                    let should_indent = self.validator.as_ref().is_some_and(|v| {
                        v.should_indent(session.line_buffer.current_line_prefix())
                    });
                    session
                        .line_buffer
                        .insert_newline_preserve_indent(should_indent, self.indent_size);
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                    changed = true;
                }
            }
            EditorAction::OpenAbove => {
                if self.edit_mode == EditMode::Vi {
                    session.save_undo();
                    session.line_buffer.move_home();
                    session.line_buffer.insert_char('\n');
                    session.line_buffer.move_up();
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                    changed = true;
                }
            }
            EditorAction::SubstituteChar => {
                session.save_undo();
                if let Some(cut) = session.line_buffer.cut_char_under_cursor()
                    && let Some(cb) = &mut self.clipboard
                {
                    cb.set(&cut);
                }
                if self.edit_mode == EditMode::Vi {
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                }
                changed = true;
            }
            EditorAction::SubstituteLine => {
                session.save_undo();
                if let Some(cut) = session.line_buffer.cut_line()
                    && let Some(cb) = &mut self.clipboard
                {
                    cb.set(&cut);
                }
                if self.edit_mode == EditMode::Vi {
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                }
                changed = true;
            }
            EditorAction::ChangeToEnd => {
                session.save_undo();
                if let Some(cut) = session.line_buffer.cut_to_end()
                    && let Some(cb) = &mut self.clipboard
                {
                    cb.set(&cut);
                }
                if self.edit_mode == EditMode::Vi {
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                }
                changed = true;
            }
            EditorAction::ViDeleteOperator => {
                session.vi_operator = Some(ViOperator::Delete);
            }
            EditorAction::ViChangeOperator => {
                session.vi_operator = Some(ViOperator::Change);
            }
            EditorAction::ViYankOperator => {
                session.vi_operator = Some(ViOperator::Yank);
            }
            EditorAction::ViReplaceCharOperator => {
                session.vi_operator = Some(ViOperator::ReplaceChar);
            }
            EditorAction::ClearLine => {
                if let Some(menu) = &mut self.menu {
                    menu.dismiss(&mut session.line_buffer);
                }
                session.save_undo();
                session.in_insert_session = false;
                session.line_buffer.clear();
                if self.edit_mode == EditMode::Vi && self.vi_mode != ViMode::Insert {
                    self.vi_mode = ViMode::Insert;
                    self.keymap = self.vi_insert_keymap();
                    let _ = queue!(self.writer, self.cursor_style);
                }
                session.vi_operator = None;
                session.reset_history_search();
                changed = true;
            }
            EditorAction::ClearScreen => {
                queue!(self.writer, Clear(ClearType::All), cursor::MoveTo(0, 0))?;
                *previous_cursor_row = 0;
                changed = true;
            }
            EditorAction::Ignore | EditorAction::Eof => {}
        }

        if changed && !matches!(action, EditorAction::MoveUp | EditorAction::MoveDown) {
            session.reset_history_search();
        }

        if changed {
            Ok(ActionResult::Changed)
        } else {
            Ok(ActionResult::Noop)
        }
    }

    /// Handles typing an unbound printable character.
    pub(crate) fn handle_char_insert(
        &mut self,
        c: char,
        session: &mut EditorSession,
    ) -> ActionResult {
        if !session.in_insert_session {
            session.save_undo();
            session.in_insert_session = true;
        }

        // Unbound printable characters are inserted into the buffer
        let should_dedent = self
            .validator
            .as_ref()
            .is_some_and(|v| v.should_dedent_on_char(c));
        if should_dedent {
            session.line_buffer.dedent_line(self.indent_size);
        }
        session.line_buffer.insert_char(c);
        if self.edit_mode != EditMode::Vi && (c == ' ' || c == '\t' || c == '\n') {
            session.in_insert_session = false;
        }

        session.reset_history_search();
        ActionResult::Changed
    }
}
