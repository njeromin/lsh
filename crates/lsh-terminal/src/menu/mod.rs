pub mod fuzzy;
pub mod layout;
pub mod render;

pub use fuzzy::*;
pub use layout::*;
pub use lsh_completions::{CompletionItem, CompletionKind};
pub use render::*;

use crossterm::event::{KeyCode, KeyModifiers};
use lsh_input::{LineBuffer, Menu, MenuOutcome};

use crate::editor::LshCompleter;

#[derive(Clone, Debug)]
struct DescentLevel {
    start: usize,
    initial_query: String,
    filter_suffix: String,
    root_items: Vec<CompletionItem>,
    original_text: String,
}

/// An interactive dropdown completion menu for LSH with live filtering, Nerd Font, and category badge support.
pub struct LshMenu {
    active: bool,
    selected: usize,
    root_items: Vec<CompletionItem>,
    items: Vec<CompletionItem>,
    completer: LshCompleter,
    max_height: usize,
    start: usize,
    original_text: String,
    initial_query: String,
    filter_suffix: String,
    current_replacement_len: usize,
    navigated: bool,
    descent_stack: Vec<DescentLevel>,
}

impl LshMenu {
    /// Create a new interactive completion menu using the given completer.
    pub fn new(_name: impl Into<String>, completer: LshCompleter) -> Self {
        Self {
            active: false,
            selected: 0,
            root_items: Vec::new(),
            items: Vec::new(),
            completer,
            max_height: 7,
            start: 0,
            original_text: String::new(),
            initial_query: String::new(),
            filter_suffix: String::new(),
            current_replacement_len: 0,
            navigated: false,
            descent_stack: Vec::new(),
        }
    }

    /// Set the maximum number of visible completion rows in the dropdown.
    pub fn with_max_height(mut self, max_height: usize) -> Self {
        self.max_height = max_height;
        self
    }

    /// Returns the index of the currently highlighted candidate.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Returns the slice of active completion candidates.
    pub fn items(&self) -> &[CompletionItem] {
        &self.items
    }

    fn revert_preview(&mut self, buffer: &mut LineBuffer) {
        if self.navigated {
            let start = self.start;
            let end = (start + self.current_replacement_len).min(buffer.len());
            let current_typed = format!("{}{}", self.initial_query, self.filter_suffix);
            buffer.replace_range(start..end, &current_typed);
            buffer.set_cursor(start + current_typed.len());
            self.current_replacement_len = current_typed.len();
            self.navigated = false;
        }
    }

    fn update_buffer_preview(&mut self, buffer: &mut LineBuffer) {
        if let Some(item) = self.items.get(self.selected) {
            let replacement = item.replacement.clone();
            let rep_len = replacement.len();
            let start = self.start;
            let end = (start + self.current_replacement_len).min(buffer.len());
            buffer.replace_range(start..end, &replacement);
            buffer.set_cursor(start + rep_len);
            self.current_replacement_len = rep_len;
        }
    }

    fn commit_selected(&mut self, buffer: &mut LineBuffer) {
        if let Some(item) = self.items.get(self.selected) {
            let is_dir =
                matches!(item.kind, CompletionKind::Directory) || item.replacement.ends_with('/');
            let mut replacement = item.replacement.clone();
            if !is_dir && !replacement.ends_with(' ') {
                replacement.push(' ');
            }
            let rep_len = replacement.len();
            let start = self.start;
            let end = (start + self.current_replacement_len).min(buffer.len());
            buffer.replace_range(start..end, &replacement);
            buffer.set_cursor(start + rep_len);
            self.current_replacement_len = rep_len;
        }
    }

    fn descend(&mut self, buffer: &mut LineBuffer) -> bool {
        let Some(item) = self.items.get(self.selected).cloned() else {
            return false;
        };
        let is_dir =
            matches!(item.kind, CompletionKind::Directory) || item.replacement.ends_with('/');
        if !is_dir {
            return false;
        }

        let mut dir_rep = item.replacement.clone();
        if !dir_rep.ends_with('/') {
            dir_rep.push('/');
        }

        self.descent_stack.push(DescentLevel {
            start: self.start,
            initial_query: self.initial_query.clone(),
            filter_suffix: self.filter_suffix.clone(),
            root_items: self.root_items.clone(),
            original_text: self.original_text.clone(),
        });

        let start = self.start;
        let end = (start + self.current_replacement_len).min(buffer.len());
        buffer.replace_range(start..end, &dir_rep);
        buffer.set_cursor(start + dir_rep.len());

        let (new_start, new_items) = self
            .completer
            .get_candidates(buffer.as_str(), buffer.cursor());
        self.start = new_start;
        self.root_items = new_items.clone();
        self.items = new_items;
        self.initial_query = dir_rep.clone();
        self.filter_suffix.clear();
        self.original_text = dir_rep.clone();
        self.current_replacement_len = dir_rep.len();
        self.selected = 0;
        self.navigated = false;
        true
    }

    fn ascend(&mut self, buffer: &mut LineBuffer) -> bool {
        let Some(parent) = self.descent_stack.pop() else {
            return false;
        };

        let current_end = (self.start + self.current_replacement_len).min(buffer.len());
        let parent_rep = format!("{}{}", parent.initial_query, parent.filter_suffix);
        buffer.replace_range(self.start..current_end, &parent_rep);

        self.start = parent.start;
        self.initial_query = parent.initial_query;
        self.filter_suffix = parent.filter_suffix;
        self.root_items = parent.root_items;
        self.original_text = parent.original_text;

        let query = format!("{}{}", self.initial_query, self.filter_suffix);
        self.items = filter_completion_items(&self.root_items, &query);
        self.current_replacement_len = parent_rep.len();
        buffer.set_cursor(self.start + parent_rep.len());
        self.selected = 0;
        self.navigated = false;
        true
    }
}

impl Menu for LshMenu {
    fn is_active(&self) -> bool {
        self.active
    }

    fn start_position(&self) -> Option<usize> {
        if self.active { Some(self.start) } else { None }
    }

    fn activate(&mut self, buffer: &mut LineBuffer, _completer: Option<&dyn lsh_input::Completer>) {
        let (start, items) = self
            .completer
            .get_candidates(buffer.as_str(), buffer.cursor());
        if items.is_empty() {
            self.active = false;
            return;
        }

        self.start = start;
        self.original_text = if buffer.cursor() >= start {
            buffer.as_str()[start..buffer.cursor()].to_string()
        } else {
            String::new()
        };
        self.initial_query = self.original_text.clone();
        self.filter_suffix.clear();
        self.current_replacement_len = self.original_text.len();
        self.descent_stack.clear();

        // 1 item: accept immediately
        if items.len() == 1 {
            let item = &items[0];
            let is_dir = matches!(item.kind, CompletionKind::Directory);
            let mut rep = item.replacement.clone();
            if !is_dir && !rep.ends_with(' ') {
                rep.push(' ');
            }
            buffer.replace_range(start..buffer.cursor(), &rep);
            buffer.set_cursor(start + rep.len());
            self.active = false;
            return;
        }

        // Multiple items: check longest common prefix
        let first = &items[0].replacement;
        let mut common_len = first.len();
        for item in &items[1..] {
            let len = first
                .chars()
                .zip(item.replacement.chars())
                .take_while(|(a, b)| a == b)
                .count();
            common_len = common_len.min(len);
        }
        let common: String = first.chars().take(common_len).collect();
        if common.len() > self.original_text.len() {
            buffer.replace_range(start..buffer.cursor(), &common);
            self.current_replacement_len = common.len();
            self.original_text = common.clone();
            self.initial_query = common.clone();
            buffer.set_cursor(start + common.len());
        }

        self.root_items = items.clone();
        self.items = items;
        self.selected = 0;
        self.navigated = false;
        self.active = true;
    }

    fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        buffer: &mut LineBuffer,
        _completer: Option<&dyn lsh_input::Completer>,
    ) -> MenuOutcome {
        match (code, modifiers) {
            (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                if !self.items.is_empty() {
                    self.selected = (self.selected + 1) % self.items.len();
                    self.navigated = true;
                    self.update_buffer_preview(buffer);
                }
                MenuOutcome::Consumed
            }
            (KeyCode::Up, _)
            | (KeyCode::BackTab, _)
            | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                if !self.items.is_empty() {
                    self.selected = if self.selected == 0 {
                        self.items.len().saturating_sub(1)
                    } else {
                        self.selected - 1
                    };
                    self.navigated = true;
                    self.update_buffer_preview(buffer);
                }
                MenuOutcome::Consumed
            }
            (KeyCode::Tab, m) if m.contains(KeyModifiers::SHIFT) => {
                if !self.items.is_empty() {
                    self.selected = if self.selected == 0 {
                        self.items.len().saturating_sub(1)
                    } else {
                        self.selected - 1
                    };
                    self.navigated = true;
                    self.update_buffer_preview(buffer);
                }
                MenuOutcome::Consumed
            }
            (KeyCode::PageDown, _) => {
                if !self.items.is_empty() {
                    self.selected =
                        (self.selected + self.max_height).min(self.items.len().saturating_sub(1));
                    self.navigated = true;
                    self.update_buffer_preview(buffer);
                }
                MenuOutcome::Consumed
            }
            (KeyCode::PageUp, _) => {
                if !self.items.is_empty() {
                    self.selected = self.selected.saturating_sub(self.max_height);
                    self.navigated = true;
                    self.update_buffer_preview(buffer);
                }
                MenuOutcome::Consumed
            }
            (KeyCode::Home, _) => {
                if !self.items.is_empty() {
                    self.selected = 0;
                    self.navigated = true;
                    self.update_buffer_preview(buffer);
                }
                MenuOutcome::Consumed
            }
            (KeyCode::End, _) => {
                if !self.items.is_empty() {
                    self.selected = self.items.len().saturating_sub(1);
                    self.navigated = true;
                    self.update_buffer_preview(buffer);
                }
                MenuOutcome::Consumed
            }
            (KeyCode::Tab, KeyModifiers::NONE) => {
                if !self.items.is_empty() {
                    if self.descend(buffer) {
                        MenuOutcome::Consumed
                    } else {
                        self.commit_selected(buffer);
                        self.active = false;
                        MenuOutcome::Accepted
                    }
                } else {
                    self.active = false;
                    self.items.clear();
                    MenuOutcome::PassThrough
                }
            }
            (KeyCode::Right, KeyModifiers::NONE) if self.navigated => {
                if self.descend(buffer) {
                    MenuOutcome::Consumed
                } else {
                    self.commit_selected(buffer);
                    self.active = false;
                    MenuOutcome::Accepted
                }
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                self.dismiss(buffer);
                MenuOutcome::PassThrough
            }
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                if self.navigated && self.descend(buffer) {
                    MenuOutcome::Consumed
                } else {
                    self.revert_preview(buffer);
                    self.filter_suffix.push('/');
                    buffer.insert_char('/');
                    self.current_replacement_len += 1;

                    let query = format!("{}{}", self.initial_query, self.filter_suffix);
                    let filtered = filter_completion_items(&self.root_items, &query);
                    if filtered.is_empty() {
                        self.active = false;
                        self.items.clear();
                    } else {
                        self.items = filtered;
                        self.selected = 0;
                    }
                    MenuOutcome::Consumed
                }
            }
            (KeyCode::Char(' '), KeyModifiers::NONE) => {
                if self.navigated {
                    self.commit_selected(buffer);
                    self.active = false;
                    MenuOutcome::Accepted
                } else {
                    self.active = false;
                    self.items.clear();
                    MenuOutcome::PassThrough
                }
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.revert_preview(buffer);
                self.filter_suffix.push(c);
                buffer.insert_char(c);
                self.current_replacement_len += c.len_utf8();

                let query = format!("{}{}", self.initial_query, self.filter_suffix);
                let filtered = filter_completion_items(&self.root_items, &query);
                if filtered.is_empty() {
                    self.active = false;
                    self.items.clear();
                } else {
                    self.items = filtered;
                    self.selected = 0;
                }
                MenuOutcome::Consumed
            }
            (KeyCode::Backspace, KeyModifiers::NONE) => {
                if self.navigated {
                    self.revert_preview(buffer);
                }

                if !self.filter_suffix.is_empty() {
                    self.filter_suffix.pop();
                    buffer.delete_prev_grapheme();
                    self.current_replacement_len = self.current_replacement_len.saturating_sub(1);

                    let query = format!("{}{}", self.initial_query, self.filter_suffix);
                    let filtered = filter_completion_items(&self.root_items, &query);
                    if filtered.is_empty() {
                        self.active = false;
                        self.items.clear();
                    } else {
                        self.items = filtered;
                        self.selected = 0;
                    }
                    MenuOutcome::Consumed
                } else if !self.initial_query.is_empty() {
                    self.initial_query.pop();
                    buffer.delete_prev_grapheme();
                    self.current_replacement_len = self.current_replacement_len.saturating_sub(1);

                    let query = self.initial_query.clone();
                    let filtered = filter_completion_items(&self.root_items, &query);
                    if filtered.is_empty() {
                        self.active = false;
                        self.items.clear();
                    } else {
                        self.items = filtered;
                        self.selected = 0;
                    }
                    MenuOutcome::Consumed
                } else if self.ascend(buffer) {
                    MenuOutcome::Consumed
                } else {
                    self.dismiss(buffer);
                    MenuOutcome::PassThrough
                }
            }
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if !self.items.is_empty() {
                    if self.descend(buffer) {
                        MenuOutcome::Consumed
                    } else {
                        self.commit_selected(buffer);
                        self.active = false;
                        MenuOutcome::Accepted
                    }
                } else {
                    self.active = false;
                    self.items.clear();
                    MenuOutcome::PassThrough
                }
            }
            (KeyCode::Esc, _)
            | (KeyCode::Left, _)
            | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                self.dismiss(buffer);
                MenuOutcome::Dismissed
            }
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.dismiss(buffer);
                MenuOutcome::PassThrough
            }
            _ => {
                self.dismiss(buffer);
                MenuOutcome::PassThrough
            }
        }
    }

    fn render_lines(&self, terminal_width: usize, cursor_col: usize) -> Vec<String> {
        if !self.active || self.items.is_empty() {
            return Vec::new();
        }

        let nerd_icons = self.completer.nerd_icons();
        let (width, visible_height) = calculate_menu_dimensions(
            &self.items,
            terminal_width as u16,
            self.max_height,
            nerd_icons,
        );

        let left_pad = cursor_col.min(terminal_width.saturating_sub(width));
        format_menu_lines(
            &self.items,
            self.selected,
            width,
            visible_height,
            nerd_icons,
            left_pad,
        )
    }

    fn dismiss(&mut self, buffer: &mut LineBuffer) {
        if self.active {
            if self.navigated {
                let start = self.start;
                let end = (start + self.current_replacement_len).min(buffer.len());
                let typed = format!("{}{}", self.initial_query, self.filter_suffix);
                let restore = if !typed.is_empty() {
                    &typed
                } else {
                    &self.original_text
                };
                buffer.replace_range(start..end, restore);
                buffer.set_cursor(start + restore.len());
            } else if !self.descent_stack.is_empty() {
                let first_level = &self.descent_stack[0];
                let start = first_level.start;
                let end = (start + self.current_replacement_len).min(buffer.len());
                buffer.replace_range(start..end, &first_level.original_text);
                buffer.set_cursor(start + first_level.original_text.len());
            }
        }
        self.active = false;
        self.items.clear();
        self.root_items.clear();
        self.selected = 0;
        self.navigated = false;
        self.filter_suffix.clear();
        self.initial_query.clear();
        self.original_text.clear();
        self.current_replacement_len = 0;
        self.descent_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::LineEditorHelper;

    #[test]
    fn new_menu_is_inactive_and_empty() {
        let helper = LineEditorHelper::new();
        let menu = LshMenu::new("completion_menu", helper);

        assert!(!menu.is_active());
        assert_eq!(menu.selected_index(), 0);
        assert!(menu.items().is_empty());
    }

    #[test]
    fn activates_and_navigates_candidates() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec!["git-clone".to_string(), "git-commit".to_string()]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("git-");

        menu.activate(&mut buffer, None);
        assert!(menu.is_active());
        assert_eq!(menu.items().len(), 2);
        // Common prefix "git-c" is automatically expanded
        assert_eq!(buffer.as_str(), "git-c");
        assert_eq!(menu.start_position(), Some(0));

        // Down navigates to next candidate and previews it
        let outcome = menu.handle_key(KeyCode::Down, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert_eq!(menu.selected_index(), 1);
        assert_eq!(buffer.as_str(), "git-commit");

        // Up navigates back to first candidate
        let outcome = menu.handle_key(KeyCode::Up, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(buffer.as_str(), "git-clone");

        // Enter accepts candidate with trailing space
        let outcome = menu.handle_key(KeyCode::Enter, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Accepted);
        assert!(!menu.is_active());
        assert_eq!(buffer.as_str(), "git-clone ");
    }

    #[test]
    fn enter_immediately_commits_highlighted_candidate() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec!["git-clone".to_string(), "git-commit".to_string()]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("git-");

        menu.activate(&mut buffer, None);
        assert!(menu.is_active());

        // Enter immediately accepts the highlighted candidate #0 without requiring navigation
        let outcome = menu.handle_key(KeyCode::Enter, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Accepted);
        assert!(!menu.is_active());
        assert_eq!(buffer.as_str(), "git-clone ");
    }

    #[test]
    fn tab_immediately_commits_highlighted_candidate() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec!["git-clone".to_string(), "git-commit".to_string()]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("git-");

        menu.activate(&mut buffer, None);
        assert!(menu.is_active());

        // Tab immediately accepts the highlighted candidate #0
        let outcome = menu.handle_key(KeyCode::Tab, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Accepted);
        assert!(!menu.is_active());
        assert_eq!(buffer.as_str(), "git-clone ");
    }

    #[test]
    fn dismiss_reverts_buffer_preview() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec!["cargo-build".to_string(), "cargo-check".to_string()]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("cargo-");

        menu.activate(&mut buffer, None);
        menu.handle_key(KeyCode::Down, KeyModifiers::NONE, &mut buffer, None);
        assert!(buffer.as_str().starts_with("cargo-"));

        // Esc dismisses and reverts buffer
        let outcome = menu.handle_key(KeyCode::Esc, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Dismissed);
        assert!(!menu.is_active());
        assert_eq!(buffer.as_str(), "cargo-");
    }

    #[test]
    fn single_candidate_accepted_immediately() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec!["unique_cmd".to_string()]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("uniq");

        menu.activate(&mut buffer, None);
        assert!(!menu.is_active());
        assert_eq!(buffer.as_str(), "unique_cmd ");
    }

    #[test]
    fn live_typing_filters_and_highlights_candidates() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec![
            "cargo-build".to_string(),
            "cargo-check".to_string(),
            "cargo-clippy".to_string(),
            "cat".to_string(),
        ]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("ca");

        menu.activate(&mut buffer, None);
        assert!(menu.is_active());
        assert_eq!(menu.items().len(), 4);

        // Type 'r' -> buffer becomes "car", items filter to cargo-*
        let outcome = menu.handle_key(KeyCode::Char('r'), KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert!(menu.is_active());
        assert_eq!(buffer.as_str(), "car");
        assert_eq!(menu.items().len(), 3);
        assert!(
            menu.items()
                .iter()
                .all(|it| it.replacement.starts_with("cargo"))
        );

        // Type 'b' -> buffer becomes "carb", items filter to cargo-build via fuzzy match
        let outcome = menu.handle_key(KeyCode::Char('b'), KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert!(menu.is_active());
        assert_eq!(buffer.as_str(), "carb");
        assert_eq!(menu.items().len(), 1);
        assert_eq!(menu.items()[0].replacement, "cargo-build");

        // Tab commits the single match
        let outcome = menu.handle_key(KeyCode::Tab, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Accepted);
        assert!(!menu.is_active());
        assert_eq!(buffer.as_str(), "cargo-build ");
    }

    #[test]
    fn live_backspace_widens_candidates() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec![
            "cargo".to_string(),
            "cat".to_string(),
            "cd".to_string(),
        ]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("c");

        menu.activate(&mut buffer, None);
        assert!(menu.is_active());
        let initial_len = menu.items().len();

        // Type 'a' -> narrows to cargo and cat
        menu.handle_key(KeyCode::Char('a'), KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(buffer.as_str(), "ca");
        assert_eq!(menu.items().len(), 2);

        // Type 'r' -> narrows to cargo
        menu.handle_key(KeyCode::Char('r'), KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(buffer.as_str(), "car");
        assert_eq!(menu.items().len(), 1);
        assert_eq!(menu.items()[0].replacement, "cargo");

        // Backspace -> widens back to 2 items
        let outcome = menu.handle_key(KeyCode::Backspace, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert!(menu.is_active());
        assert_eq!(buffer.as_str(), "ca");
        assert_eq!(menu.items().len(), 2);

        // Backspace again -> widens to all initial items
        let outcome = menu.handle_key(KeyCode::Backspace, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert!(menu.is_active());
        assert_eq!(buffer.as_str(), "c");
        assert_eq!(menu.items().len(), initial_len);
    }

    #[test]
    fn typing_no_match_dismisses_menu_preserving_buffer() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec!["cargo".to_string(), "cat".to_string()]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("ca");

        menu.activate(&mut buffer, None);
        assert!(menu.is_active());

        // Type 'z' -> no match for "caz"
        let outcome = menu.handle_key(KeyCode::Char('z'), KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert!(!menu.is_active());
        assert_eq!(buffer.as_str(), "caz");
    }

    #[test]
    fn navigation_preview_reverts_when_typing_continues() {
        let helper = LineEditorHelper::new();
        helper.set_binaries(vec!["cargo".to_string(), "cat".to_string()]);

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut buffer = LineBuffer::new();
        buffer.insert_str("ca");

        menu.activate(&mut buffer, None);

        // Navigate with Down -> advances to next candidate ("cargo") and previews it
        menu.handle_key(KeyCode::Down, KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(buffer.as_str(), "cargo");

        // Type 'r' -> reverts preview of "cargo", appends 'r' to "ca" -> "car"
        let outcome = menu.handle_key(KeyCode::Char('r'), KeyModifiers::NONE, &mut buffer, None);
        assert_eq!(outcome, MenuOutcome::Consumed);
        assert!(menu.is_active());
        assert_eq!(buffer.as_str(), "car");
        assert_eq!(menu.items().len(), 1);
        assert_eq!(menu.items()[0].replacement, "cargo");
    }
}
