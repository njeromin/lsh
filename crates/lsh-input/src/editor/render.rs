use std::io::{self, Write};

use crossterm::{
    cursor::{self, MoveToColumn},
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};

use crate::buffer::LineBuffer;
use crate::edit_mode::{EditMode, ViMode};
use crate::formatter::StyledText;
use crate::layout::{ScreenLayout, compute_screen_layout};
use crate::prompt::Prompt;

use super::Editor;

impl<W: Write> Editor<W> {
    /// Redraws the prompt, buffer (including soft-wrapped and multiline rows),
    /// optional inline ghost text hint, and positions the cursor.
    pub fn render_line(
        &mut self,
        prompt: &impl Prompt,
        line_buffer: &LineBuffer,
        previous_cursor_row: usize,
    ) -> io::Result<(ScreenLayout, Option<String>)> {
        self.render_line_internal(prompt, line_buffer, previous_cursor_row, true)
    }

    pub(crate) fn render_line_internal(
        &mut self,
        prompt: &impl Prompt,
        line_buffer: &LineBuffer,
        previous_cursor_row: usize,
        show_ghost_text: bool,
    ) -> io::Result<(ScreenLayout, Option<String>)> {
        let term_cols = self.terminal_width();
        let prompt_text = prompt.render();
        let prompt_stripped = crate::prompt::strip_ansi(&prompt_text);
        let indicator_text = prompt.render_multiline_indicator();
        let indicator_stripped = crate::prompt::strip_ansi(&indicator_text);

        // Query ghost text hint if cursor is at the end of the buffer, content is non-empty, and menu is not active
        let is_menu_active = show_ghost_text && self.menu.as_ref().is_some_and(|m| m.is_active());
        let ghost_text = if is_menu_active
            || !show_ghost_text
            || (self.edit_mode == EditMode::Vi && self.vi_mode == ViMode::Normal)
        {
            None
        } else if let Some(hinter) = &self.hinter {
            if line_buffer.cursor() == line_buffer.len() && !line_buffer.is_empty() {
                hinter
                    .hint(line_buffer.as_str(), line_buffer.cursor())
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        } else {
            None
        };

        let layout = compute_screen_layout(
            &prompt_stripped,
            &indicator_stripped,
            line_buffer,
            ghost_text.as_deref(),
            term_cols,
        );

        // Move up to row 0 if we were previously on a lower row
        if previous_cursor_row > 0 {
            queue!(self.writer, cursor::MoveUp(previous_cursor_row as u16))?;
        }

        // Clear from row 0 downwards and print the prompt with CRLF newlines
        let prompt_render = prompt_text.replace("\r\n", "\n").replace('\n', "\r\n");
        let indicator_render = indicator_text.replace("\r\n", "\n").replace('\n', "\r\n");

        queue!(
            self.writer,
            cursor::Hide,
            MoveToColumn(0),
            Clear(ClearType::FromCursorDown),
            Print(&prompt_render),
        )?;

        let styled_text = match &self.formatter {
            Some(f) => f.format(line_buffer.as_str()),
            None => StyledText::plain(line_buffer.as_str()),
        };

        // Render spans, inserting indicator on newlines
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
                        Print(&indicator_render),
                    )?;
                }
            }
        }

        // Render inline ghost text hint if present
        if let Some(ref hint) = ghost_text {
            queue!(
                self.writer,
                crossterm::style::PrintStyledContent(self.hint_style.apply(hint))
            )?;
        }

        queue!(self.writer, crossterm::style::ResetColor)?;

        // 5. Render completion menu lines beneath the buffer if menu is active
        let mut menu_rows = 0;
        if is_menu_active && let Some(menu) = &self.menu {
            let anchor_col = if let Some(start) = menu.start_position() {
                crate::layout::byte_to_screen_col(
                    &prompt_stripped,
                    &indicator_stripped,
                    line_buffer,
                    start,
                    term_cols,
                )
            } else {
                layout.cursor_col
            };
            let lines = menu.render_lines(term_cols, anchor_col);
            for line in &lines {
                queue!(self.writer, Print("\r\n"), Print(line))?;
            }
            menu_rows = lines.len();
        }

        queue!(self.writer, crossterm::style::ResetColor)?;

        // Move cursor from the end of the rendered content back to layout.cursor_row
        let total_end_row = layout.end_row + menu_rows;
        let rows_to_move_up = total_end_row.saturating_sub(layout.cursor_row);
        if rows_to_move_up > 0 {
            queue!(self.writer, cursor::MoveUp(rows_to_move_up as u16))?;
        }

        // Position cursor horizontally at layout.cursor_col
        queue!(
            self.writer,
            MoveToColumn(layout.cursor_col as u16),
            cursor::Show,
        )?;
        self.writer.flush()?;

        let mut effective_layout = layout;
        effective_layout.end_row = total_end_row;
        Ok((effective_layout, ghost_text))
    }
}
