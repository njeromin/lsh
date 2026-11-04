use crossterm::event::{KeyCode, KeyModifiers};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::buffer::LineBuffer;
use crate::completer::{Candidate, Completer};

/// Outcome of a key event handled by an active menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuOutcome {
    /// The menu consumed the key event and remains active.
    Consumed,
    /// A candidate was accepted and committed to the buffer; the menu is now closed.
    Accepted,
    /// The menu was dismissed (e.g. Esc pressed); preview was reverted; the menu is closed.
    Dismissed,
    /// The menu did not consume this key event; the menu should close and the editor should handle the key normally.
    PassThrough,
}

/// A pluggable completion menu (e.g. dropdown, column grid, fuzzy picker).
pub trait Menu: Send + Sync {
    /// Returns true if the menu is currently visible/active.
    fn is_active(&self) -> bool;

    /// Activates completion for the current line buffer and cursor position.
    fn activate(&mut self, buffer: &mut LineBuffer, completer: Option<&dyn Completer>);

    /// Handles a key event while the menu is active.
    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        buffer: &mut LineBuffer,
        completer: Option<&dyn Completer>,
    ) -> MenuOutcome;

    /// Renders the visual menu lines to be displayed beneath the line editor.
    fn render_lines(&self, terminal_width: usize, cursor_col: usize) -> Vec<String>;

    /// Dismisses the menu, reverting any temporary preview in the buffer.
    fn dismiss(&mut self, buffer: &mut LineBuffer);
}

impl<M: Menu + ?Sized> Menu for Box<M> {
    fn is_active(&self) -> bool {
        (**self).is_active()
    }

    fn activate(&mut self, buffer: &mut LineBuffer, completer: Option<&dyn Completer>) {
        (**self).activate(buffer, completer);
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        buffer: &mut LineBuffer,
        completer: Option<&dyn Completer>,
    ) -> MenuOutcome {
        (**self).handle_key(code, modifiers, buffer, completer)
    }

    fn render_lines(&self, terminal_width: usize, cursor_col: usize) -> Vec<String> {
        (**self).render_lines(terminal_width, cursor_col)
    }

    fn dismiss(&mut self, buffer: &mut LineBuffer) {
        (**self).dismiss(buffer);
    }
}

/// Helper function to calculate the visible viewport (start index, end index) of items.
pub fn calculate_viewport(total: usize, selected: usize, max_visible: usize) -> (usize, usize) {
    if total <= max_visible {
        return (0, total);
    }

    let half = max_visible / 2;
    let start = if selected < half {
        0
    } else if selected + (max_visible - half) >= total {
        total - max_visible
    } else {
        selected - half
    };

    (start, start + max_visible)
}

/// A standard, lightweight dropdown completion menu provided out-of-the-box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicMenu {
    active: bool,
    start: usize,
    original_text: String,
    current_replacement_len: usize,
    candidates: Vec<Candidate>,
    selected: usize,
    navigated: bool,
    max_height: usize,
}

impl BasicMenu {
    /// Create a new basic menu with the specified maximum visible rows.
    pub fn new(max_height: usize) -> Self {
        Self {
            active: false,
            start: 0,
            original_text: String::new(),
            current_replacement_len: 0,
            candidates: Vec::new(),
            selected: 0,
            navigated: false,
            max_height: max_height.max(1),
        }
    }

    /// Set the maximum number of visible rows in the dropdown.
    pub fn with_max_height(mut self, max_height: usize) -> Self {
        self.max_height = max_height.max(1);
        self
    }

    /// Computes the longest common prefix among candidate replacements.
    pub fn longest_common_prefix(candidates: &[Candidate]) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }
        let first = &candidates[0].replacement;
        let mut common_chars = first.chars().collect::<Vec<_>>();
        for c in &candidates[1..] {
            let mut len = 0;
            for (a, b) in common_chars.iter().zip(c.replacement.chars()) {
                if *a == b {
                    len += 1;
                } else {
                    break;
                }
            }
            common_chars.truncate(len);
            if common_chars.is_empty() {
                break;
            }
        }
        if common_chars.is_empty() {
            None
        } else {
            Some(common_chars.into_iter().collect())
        }
    }

    fn preview_selected(&mut self, buffer: &mut LineBuffer) {
        if let Some(cand) = self.candidates.get(self.selected) {
            let rep = cand.replacement.clone();
            let rep_len = rep.len();
            buffer.replace_range(
                self.start..(self.start + self.current_replacement_len),
                &rep,
            );
            self.current_replacement_len = rep_len;
            buffer.set_cursor(self.start + rep_len);
        }
    }

    fn commit_selected(&mut self, buffer: &mut LineBuffer) {
        if let Some(cand) = self.candidates.get(self.selected) {
            let mut rep = cand.replacement.clone();
            if cand.append_space && !rep.ends_with(' ') {
                rep.push(' ');
            }
            let rep_len = rep.len();
            buffer.replace_range(
                self.start..(self.start + self.current_replacement_len),
                &rep,
            );
            self.current_replacement_len = rep_len;
            buffer.set_cursor(self.start + rep_len);
        }
    }

    fn revert_preview(&mut self, buffer: &mut LineBuffer) {
        if self.navigated {
            buffer.replace_range(
                self.start..(self.start + self.current_replacement_len),
                &self.original_text,
            );
            buffer.set_cursor(self.start + self.original_text.len());
            self.current_replacement_len = self.original_text.len();
        }
    }
}

impl Default for BasicMenu {
    fn default() -> Self {
        Self::new(7)
    }
}

impl Menu for BasicMenu {
    fn is_active(&self) -> bool {
        self.active
    }

    fn activate(&mut self, buffer: &mut LineBuffer, completer: Option<&dyn Completer>) {
        let Some(completer) = completer else {
            return;
        };

        let (start, candidates) = completer.complete(buffer.as_str(), buffer.cursor());
        if candidates.is_empty() {
            self.active = false;
            return;
        }

        // Quick complete if exactly 1 candidate
        if candidates.len() == 1 {
            let c = &candidates[0];
            let mut rep = c.replacement.clone();
            if c.append_space && !rep.ends_with(' ') {
                rep.push(' ');
            }
            buffer.replace_range(start..buffer.cursor(), &rep);
            buffer.set_cursor(start + rep.len());
            self.active = false;
            return;
        }

        self.start = start;
        self.original_text = if start <= buffer.cursor() && buffer.cursor() <= buffer.len() {
            buffer.as_str()[start..buffer.cursor()].to_string()
        } else {
            String::new()
        };
        self.current_replacement_len = self.original_text.len();

        // Complete common prefix if longer than typed text
        if let Some(common) = Self::longest_common_prefix(&candidates)
            && common.len() > self.original_text.len()
        {
            buffer.replace_range(start..buffer.cursor(), &common);
            self.current_replacement_len = common.len();
            self.original_text = common.clone();
            buffer.set_cursor(start + common.len());
        }

        self.candidates = candidates;
        self.selected = 0;
        self.navigated = false;
        self.active = true;
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        buffer: &mut LineBuffer,
        completer: Option<&dyn Completer>,
    ) -> MenuOutcome {
        if !self.active {
            return MenuOutcome::PassThrough;
        }

        match (code, modifiers) {
            // Next: Down, Tab (without Shift), Ctrl+N
            (KeyCode::Down, _)
            | (KeyCode::Tab, KeyModifiers::NONE)
            | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                if !self.candidates.is_empty() {
                    if !self.navigated {
                        self.selected = 0;
                        self.navigated = true;
                    } else {
                        self.selected = (self.selected + 1) % self.candidates.len();
                    }
                    self.preview_selected(buffer);
                }
                MenuOutcome::Consumed
            }

            // Previous: Up, BackTab, Shift+Tab, Ctrl+P
            (KeyCode::Up, _)
            | (KeyCode::BackTab, _)
            | (KeyCode::Tab, KeyModifiers::SHIFT)
            | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                if !self.candidates.is_empty() {
                    if !self.navigated {
                        self.selected = self.candidates.len().saturating_sub(1);
                        self.navigated = true;
                    } else {
                        self.selected = if self.selected == 0 {
                            self.candidates.len() - 1
                        } else {
                            self.selected - 1
                        };
                    }
                    self.preview_selected(buffer);
                }
                MenuOutcome::Consumed
            }

            // Page Down
            (KeyCode::PageDown, _) => {
                if !self.candidates.is_empty() {
                    self.selected = (self.selected + self.max_height)
                        .min(self.candidates.len().saturating_sub(1));
                    self.navigated = true;
                    self.preview_selected(buffer);
                }
                MenuOutcome::Consumed
            }

            // Page Up
            (KeyCode::PageUp, _) => {
                if !self.candidates.is_empty() {
                    self.selected = self.selected.saturating_sub(self.max_height);
                    self.navigated = true;
                    self.preview_selected(buffer);
                }
                MenuOutcome::Consumed
            }

            // Home / End in menu
            (KeyCode::Home, _) => {
                if !self.candidates.is_empty() {
                    self.selected = 0;
                    self.navigated = true;
                    self.preview_selected(buffer);
                }
                MenuOutcome::Consumed
            }
            (KeyCode::End, _) => {
                if !self.candidates.is_empty() {
                    self.selected = self.candidates.len().saturating_sub(1);
                    self.navigated = true;
                    self.preview_selected(buffer);
                }
                MenuOutcome::Consumed
            }

            // Accept / Commit
            (KeyCode::Enter, _) => {
                if self.navigated {
                    self.commit_selected(buffer);
                    self.active = false;
                    MenuOutcome::Accepted
                } else {
                    self.active = false;
                    MenuOutcome::PassThrough
                }
            }

            // Right arrow commits if navigated
            (KeyCode::Right, _) => {
                if self.navigated {
                    self.commit_selected(buffer);
                    self.active = false;
                    MenuOutcome::Accepted
                } else {
                    self.active = false;
                    MenuOutcome::PassThrough
                }
            }

            // Left arrow reverts preview
            (KeyCode::Left, _) => {
                if self.navigated {
                    self.revert_preview(buffer);
                }
                self.active = false;
                MenuOutcome::PassThrough
            }

            // Dismiss: Esc or Ctrl+G
            (KeyCode::Esc, _) | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                self.revert_preview(buffer);
                self.active = false;
                MenuOutcome::Dismissed
            }

            // Interrupt: Ctrl+C reverts preview and passes through to editor
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.revert_preview(buffer);
                self.active = false;
                MenuOutcome::PassThrough
            }

            // Backspace deletes char and re-queries completer
            (KeyCode::Backspace, _) => {
                self.revert_preview(buffer);
                let deleted = buffer.delete_prev_grapheme();
                if deleted {
                    if buffer.cursor() < self.start {
                        self.active = false;
                    } else if let Some(completer) = completer {
                        let (start, candidates) =
                            completer.complete(buffer.as_str(), buffer.cursor());
                        if candidates.is_empty() {
                            self.active = false;
                        } else {
                            self.start = start;
                            self.original_text =
                                buffer.as_str()[start..buffer.cursor()].to_string();
                            self.current_replacement_len = self.original_text.len();
                            self.candidates = candidates;
                            self.selected = 0;
                            self.navigated = false;
                        }
                    } else {
                        self.active = false;
                    }
                }
                MenuOutcome::Consumed
            }

            // Typing characters: commit active preview if navigated, then insert character and re-filter
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                if self.navigated {
                    self.commit_selected(buffer);
                }
                buffer.insert_char(c);
                if let Some(completer) = completer {
                    let (start, candidates) = completer.complete(buffer.as_str(), buffer.cursor());
                    if candidates.is_empty() {
                        self.active = false;
                    } else {
                        self.start = start;
                        self.original_text = buffer.as_str()[start..buffer.cursor()].to_string();
                        self.current_replacement_len = self.original_text.len();
                        self.candidates = candidates;
                        self.selected = 0;
                        self.navigated = false;
                    }
                } else {
                    self.active = false;
                }
                MenuOutcome::Consumed
            }

            // Any other key dismisses menu and passes through
            _ => {
                self.revert_preview(buffer);
                self.active = false;
                MenuOutcome::PassThrough
            }
        }
    }

    fn render_lines(&self, terminal_width: usize, cursor_col: usize) -> Vec<String> {
        if !self.active || self.candidates.is_empty() {
            return Vec::new();
        }

        let visible_count = self.candidates.len().min(self.max_height);
        let (start_idx, end_idx) =
            calculate_viewport(self.candidates.len(), self.selected, visible_count);

        let max_item_len = self.candidates[start_idx..end_idx]
            .iter()
            .map(|c| {
                UnicodeWidthStr::width(c.display.as_str())
                    + c.description
                        .as_ref()
                        .map_or(0, |d| UnicodeWidthStr::width(d.as_str()) + 2)
            })
            .max()
            .unwrap_or(20);

        let width = (max_item_len + 6).clamp(24, terminal_width.saturating_sub(4).max(24));
        let left_pad = cursor_col.min(terminal_width.saturating_sub(width));
        let indent = " ".repeat(left_pad);
        let inner_width = width.saturating_sub(2);

        let mut lines = Vec::with_capacity((end_idx - start_idx) + 2);

        // Top border
        let top_title = if start_idx > 0 {
            format!("▲ {} more ", start_idx)
        } else {
            String::new()
        };
        let top_title_width = UnicodeWidthStr::width(top_title.as_str());
        let top_fill = width.saturating_sub(top_title_width + 2);
        lines.push(format!(
            "{indent}\x1b[38;5;240m┌{top_title}{}┐\x1b[0m",
            "─".repeat(top_fill)
        ));

        // Candidate item rows: " " (1) + marker (2) + display + desc + pad + " " (1)
        let overhead = 4;
        let max_text_width = inner_width.saturating_sub(overhead);

        for (i, c) in self.candidates[start_idx..end_idx].iter().enumerate() {
            let idx = start_idx + i;
            let is_selected = idx == self.selected;
            let marker = if is_selected { "▶ " } else { "  " };

            let display = &c.display;
            let display_width = UnicodeWidthStr::width(display.as_str());

            let (visible_display, visible_display_width) =
                if display_width > max_text_width && max_text_width > 1 {
                    let mut s = String::new();
                    let mut w = 0;
                    for ch in display.chars() {
                        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                        if w + cw + 1 > max_text_width {
                            break;
                        }
                        s.push(ch);
                        w += cw;
                    }
                    s.push('…');
                    let total_w = w + 1;
                    (s, total_w)
                } else {
                    (display.clone(), display_width)
                };

            let rem_for_desc = max_text_width.saturating_sub(visible_display_width);
            let (desc_part, desc_width) = if let Some(desc) = &c.description {
                if rem_for_desc >= 6 {
                    let avail_desc = rem_for_desc - 2;
                    let d_width = UnicodeWidthStr::width(desc.as_str());
                    if d_width > avail_desc {
                        let mut s = String::new();
                        let mut w = 0;
                        for ch in desc.chars() {
                            let cw = UnicodeWidthChar::width(ch).unwrap_or(0);
                            if w + cw + 1 > avail_desc {
                                break;
                            }
                            s.push(ch);
                            w += cw;
                        }
                        s.push('…');
                        (format!("  {s}"), w + 1 + 2)
                    } else {
                        (format!("  {desc}"), d_width + 2)
                    }
                } else {
                    (String::new(), 0)
                }
            } else {
                (String::new(), 0)
            };

            let used_len = 1 + 2 + visible_display_width + desc_width + 1;
            let pad_len = inner_width.saturating_sub(used_len);
            let pad = " ".repeat(pad_len);

            let row_content = if is_selected {
                format!("\x1b[7m {marker}{visible_display}{desc_part}{pad} \x1b[0m")
            } else {
                let styled_desc = if !desc_part.is_empty() {
                    format!("\x1b[38;5;244m{desc_part}\x1b[0m")
                } else {
                    String::new()
                };
                format!(" {marker}{visible_display}{styled_desc}{pad} ")
            };

            lines.push(format!(
                "{indent}\x1b[38;5;240m│\x1b[0m{row_content}\x1b[38;5;240m│\x1b[0m"
            ));
        }

        // Bottom border
        let bottom_title = if end_idx < self.candidates.len() {
            format!("▼ {} more ", self.candidates.len() - end_idx)
        } else {
            String::new()
        };
        let bottom_title_width = UnicodeWidthStr::width(bottom_title.as_str());
        let bottom_fill = width.saturating_sub(bottom_title_width + 2);
        lines.push(format!(
            "{indent}\x1b[38;5;240m└{bottom_title}{}┘\x1b[0m",
            "─".repeat(bottom_fill)
        ));

        lines
    }

    fn dismiss(&mut self, buffer: &mut LineBuffer) {
        if self.active {
            self.revert_preview(buffer);
            self.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_longest_common_prefix() {
        let candidates = vec![
            Candidate::new("cargo-build"),
            Candidate::new("cargo-clippy"),
            Candidate::new("cargo-check"),
        ];

        assert_eq!(
            BasicMenu::longest_common_prefix(&candidates),
            Some("cargo-".to_string())
        );
    }

    #[test]
    fn test_menu_lifecycle() {
        let mut menu = BasicMenu::default();
        assert!(!menu.is_active());

        let mut buf = LineBuffer::new();
        buf.insert_str("ca");

        let completer = |_line: &str, _cursor: usize| {
            (
                0,
                vec![
                    Candidate::new("cargo").with_append_space(true),
                    Candidate::new("cat").with_append_space(true),
                ],
            )
        };

        menu.activate(&mut buf, Some(&completer));
        assert!(menu.is_active());

        // Navigating down selects "cargo"
        let outcome = menu.handle_key(
            KeyCode::Down,
            KeyModifiers::NONE,
            &mut buf,
            Some(&completer),
        );
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert_eq!(buf.as_str(), "cargo");

        // Next item selects "cat"
        let outcome = menu.handle_key(
            KeyCode::Down,
            KeyModifiers::NONE,
            &mut buf,
            Some(&completer),
        );
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert_eq!(buf.as_str(), "cat");

        // Enter commits "cat " (appends space)
        let outcome = menu.handle_key(
            KeyCode::Enter,
            KeyModifiers::NONE,
            &mut buf,
            Some(&completer),
        );
        assert_eq!(outcome, MenuOutcome::Accepted);
        assert_eq!(buf.as_str(), "cat ");
        assert!(!menu.is_active());
    }

    #[test]
    fn test_menu_dismiss_reverts_preview() {
        let mut menu = BasicMenu::default();
        let mut buf = LineBuffer::new();
        buf.insert_str("ca");

        let completer =
            |_line: &str, _cursor: usize| (0, vec![Candidate::new("cargo"), Candidate::new("cat")]);

        menu.activate(&mut buf, Some(&completer));
        menu.handle_key(
            KeyCode::Down,
            KeyModifiers::NONE,
            &mut buf,
            Some(&completer),
        );
        assert_eq!(buf.as_str(), "cargo");

        // Esc reverts to original "ca"
        let outcome = menu.handle_key(KeyCode::Esc, KeyModifiers::NONE, &mut buf, Some(&completer));
        assert_eq!(outcome, MenuOutcome::Dismissed);
        assert_eq!(buf.as_str(), "ca");
        assert!(!menu.is_active());
    }

    #[test]
    fn test_menu_render_lines_borders_align_to_exact_width() {
        let mut menu = BasicMenu::default();
        let mut buf = LineBuffer::new();
        buf.insert_str("ca");

        let completer = |_line: &str, _cursor: usize| {
            (
                0,
                vec![
                    Candidate::new("cargo").with_description("Rust package manager"),
                    Candidate::new("cat").with_description("Concatenate files"),
                    Candidate::new("cargo-clippy").with_description("Rust linter"),
                ],
            )
        };

        menu.activate(&mut buf, Some(&completer));
        assert!(menu.is_active());

        let lines = menu.render_lines(80, 5);
        assert!(!lines.is_empty());

        let first_len = crate::prompt::strip_ansi(&lines[0]).chars().count();
        assert!(crate::prompt::strip_ansi(&lines[0]).starts_with("     ┌"));
        assert!(crate::prompt::strip_ansi(&lines[0]).ends_with('┐'));

        for (i, line) in lines.iter().enumerate() {
            let stripped = crate::prompt::strip_ansi(line);
            assert_eq!(
                stripped.chars().count(),
                first_len,
                "Line {i} visual length mismatch: '{stripped}'"
            );
            if i == 0 {
                assert!(stripped.ends_with('┐'));
            } else if i == lines.len() - 1 {
                assert!(stripped.ends_with('┘'));
            } else {
                assert!(stripped.ends_with('│'));
            }
        }
    }

    #[test]
    fn test_menu_render_lines_scroll_indicators_align() {
        let mut menu = BasicMenu::new(2);
        let mut buf = LineBuffer::new();
        buf.insert_str("c");

        let completer = |_line: &str, _cursor: usize| {
            (
                0,
                vec![
                    Candidate::new("c1"),
                    Candidate::new("c2"),
                    Candidate::new("c3"),
                    Candidate::new("c4"),
                ],
            )
        };

        menu.activate(&mut buf, Some(&completer));
        menu.handle_key(
            KeyCode::Down,
            KeyModifiers::NONE,
            &mut buf,
            Some(&completer),
        );
        menu.handle_key(
            KeyCode::Down,
            KeyModifiers::NONE,
            &mut buf,
            Some(&completer),
        );

        let lines = menu.render_lines(80, 0);
        let first_len = crate::prompt::strip_ansi(&lines[0]).chars().count();

        for (i, line) in lines.iter().enumerate() {
            let stripped = crate::prompt::strip_ansi(line);
            assert_eq!(
                stripped.chars().count(),
                first_len,
                "Line {i} visual length mismatch: '{stripped}'"
            );
            if i == 0 {
                assert!(stripped.ends_with('┐'));
            } else if i == lines.len() - 1 {
                assert!(stripped.ends_with('┘'));
            } else {
                assert!(stripped.ends_with('│'));
            }
        }
    }
}
