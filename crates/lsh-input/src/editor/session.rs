use crate::buffer::LineBuffer;
use crate::edit_mode::ViOperator;
use crate::history::History;

pub(crate) struct EditorSession {
    pub(crate) line_buffer: LineBuffer,
    pub(crate) history_index: Option<usize>,
    pub(crate) saved_buffer: Option<String>,
    pub(crate) history_prefix: Option<String>,
    pub(crate) vi_operator: Option<ViOperator>,
    pub(crate) undo_stack: Vec<(String, usize)>,
    pub(crate) redo_stack: Vec<(String, usize)>,
    pub(crate) in_insert_session: bool,
}

impl EditorSession {
    pub(crate) fn new() -> Self {
        Self {
            line_buffer: LineBuffer::new(),
            history_index: None,
            saved_buffer: None,
            history_prefix: None,
            vi_operator: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            in_insert_session: false,
        }
    }

    pub(crate) fn save_undo(&mut self) {
        self.undo_stack
            .push((self.line_buffer.as_str().to_string(), self.line_buffer.cursor()));
        self.redo_stack.clear();
    }

    pub(crate) fn undo(&mut self) -> bool {
        self.in_insert_session = false;
        if let Some((saved_content, saved_cursor)) = self.undo_stack.pop() {
            self.redo_stack
                .push((self.line_buffer.as_str().to_string(), self.line_buffer.cursor()));
            self.line_buffer
                .replace_range(0..self.line_buffer.len(), &saved_content);
            self.line_buffer
                .set_cursor(saved_cursor.min(self.line_buffer.len()));
            true
        } else {
            false
        }
    }

    pub(crate) fn redo(&mut self) -> bool {
        self.in_insert_session = false;
        if let Some((saved_content, saved_cursor)) = self.redo_stack.pop() {
            self.undo_stack
                .push((self.line_buffer.as_str().to_string(), self.line_buffer.cursor()));
            self.line_buffer
                .replace_range(0..self.line_buffer.len(), &saved_content);
            self.line_buffer
                .set_cursor(saved_cursor.min(self.line_buffer.len()));
            true
        } else {
            false
        }
    }

    pub(crate) fn reset_history_search(&mut self) {
        self.history_index = None;
        self.saved_buffer = None;
        self.history_prefix = None;
    }

    pub(crate) fn history_move_up(&mut self, history: &dyn History) -> bool {
        if history.is_empty() {
            return false;
        }
        let hist_len = history.len();
        let pfx = self.history_prefix.get_or_insert_with(|| {
            self.saved_buffer = Some(self.line_buffer.as_str().to_string());
            self.line_buffer.as_str().to_string()
        });

        let search_start = self.history_index.unwrap_or(hist_len);
        if search_start > 0 {
            for i in (0..search_start).rev() {
                if let Some(entry) = history.get(i)
                    && entry.starts_with(pfx.as_str())
                {
                    self.history_index = Some(i);
                    self.line_buffer
                        .replace_range(0..self.line_buffer.len(), &entry);
                    self.line_buffer.set_cursor(entry.len());
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn history_move_down(&mut self, history: &dyn History) -> bool {
        if let Some(curr_idx) = self.history_index {
            let hist_len = history.len();
            let pfx = self.history_prefix.as_deref().unwrap_or("");
            let mut next_entry = None;

            for i in (curr_idx + 1)..hist_len {
                if let Some(entry) = history.get(i)
                    && entry.starts_with(pfx)
                {
                    next_entry = Some((i, entry));
                    break;
                }
            }

            if let Some((idx, entry)) = next_entry {
                self.history_index = Some(idx);
                self.line_buffer
                    .replace_range(0..self.line_buffer.len(), &entry);
                self.line_buffer.set_cursor(entry.len());
                true
            } else {
                self.history_index = None;
                self.history_prefix = None;
                let orig = self.saved_buffer.take().unwrap_or_default();
                self.line_buffer
                    .replace_range(0..self.line_buffer.len(), &orig);
                self.line_buffer.set_cursor(orig.len());
                true
            }
        } else {
            false
        }
    }
}
