use std::cell::Cell;

use reedline::{
    Completer, Editor, Menu, MenuBuilder, MenuEvent, MenuSettings, Painter, Suggestion,
    UndoBehavior,
};

use crate::editor::LshCompleter;
use crate::menu::layout::calculate_menu_dimensions;
use crate::menu::render::{format_menu_lines, strip_ansi};
use crate::menu::{CompletionItem, CompletionKind};

pub struct LshMenu {
    settings: MenuSettings,
    active: bool,
    selected: usize,
    items: Vec<CompletionItem>,
    values: Vec<Suggestion>,
    completer: LshCompleter,
    event: Option<MenuEvent>,
    cursor_col: u16,
    anchor_col: u16,
    terminal_width: u16,
    terminal_height: u16,
    max_height: usize,
    start: usize,
    end: usize,
    original_text: String,
    current_replacement_len: usize,
    navigated: bool,
    accepted: Cell<bool>,
}

impl LshMenu {
    pub fn new(name: impl Into<String>, completer: LshCompleter) -> Self {
        Self {
            settings: MenuSettings::default()
                .with_name(&name.into())
                .with_marker(""),
            active: false,
            selected: 0,
            items: Vec::new(),
            values: Vec::new(),
            completer,
            event: None,
            cursor_col: 0,
            anchor_col: 0,
            terminal_width: 80,
            terminal_height: 24,
            max_height: 7,
            start: 0,
            end: 0,
            original_text: String::new(),
            current_replacement_len: 0,
            navigated: false,
            accepted: Cell::new(false),
        }
    }

    pub fn with_max_height(mut self, max_height: usize) -> Self {
        self.max_height = max_height;
        self
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn items(&self) -> &[CompletionItem] {
        &self.items
    }

    fn update_buffer_for_selected(&mut self, editor: &mut Editor) {
        if let Some(item) = self.items.get(self.selected) {
            let replacement = item.replacement.clone();
            let rep_len = replacement.len();
            let start = self.start;
            let end = self.start + self.current_replacement_len;
            editor.edit_buffer(
                move |b| {
                    b.replace_range(start..end, &replacement);
                    b.set_insertion_point(start + rep_len);
                },
                UndoBehavior::CreateUndoPoint,
            );
            self.current_replacement_len = rep_len;
        }
    }

    fn revert_buffer(&self, editor: &mut Editor) {
        let start = self.start;
        let end = self.start + self.current_replacement_len;
        let orig = self.original_text.clone();
        let orig_len = orig.len();
        editor.edit_buffer(
            move |b| {
                b.replace_range(start..end, &orig);
                b.set_insertion_point(start + orig_len);
            },
            UndoBehavior::CreateUndoPoint,
        );
    }

    pub fn apply_event(
        &mut self,
        event: MenuEvent,
        editor: &mut Editor,
        completer: &mut dyn Completer,
    ) {
        match event {
            MenuEvent::Activate(updated) | MenuEvent::Edit(updated) => {
                self.reset_position();
                if !updated || self.items.is_empty() {
                    self.update_values(editor, completer);
                }
            }
            MenuEvent::Deactivate => {
                if !self.accepted.get() && self.navigated {
                    self.revert_buffer(editor);
                }
                self.active = false;
                self.items.clear();
                self.values.clear();
                self.selected = 0;
                self.navigated = false;
                self.accepted.set(false);
            }
            MenuEvent::NextElement | MenuEvent::MoveDown => {
                if !self.items.is_empty() {
                    if !self.navigated {
                        self.selected = 0;
                        self.navigated = true;
                    } else {
                        self.selected = (self.selected + 1) % self.items.len();
                    }
                    self.update_buffer_for_selected(editor);
                }
            }
            MenuEvent::PreviousElement | MenuEvent::MoveUp => {
                if !self.items.is_empty() {
                    if !self.navigated {
                        self.selected = self.items.len().saturating_sub(1);
                        self.navigated = true;
                    } else {
                        self.selected = if self.selected == 0 {
                            self.items.len() - 1
                        } else {
                            self.selected - 1
                        };
                    }
                    self.update_buffer_for_selected(editor);
                }
            }
            MenuEvent::NextPage => {
                if !self.items.is_empty() {
                    self.selected = (self.selected + self.max_height).min(self.items.len() - 1);
                    self.navigated = true;
                    self.update_buffer_for_selected(editor);
                }
            }
            MenuEvent::PreviousPage => {
                if !self.items.is_empty() {
                    self.selected = self.selected.saturating_sub(self.max_height);
                    self.navigated = true;
                    self.update_buffer_for_selected(editor);
                }
            }
            MenuEvent::MoveRight => {
                self.replace_in_buffer(editor);
                self.active = false;
                self.items.clear();
                self.values.clear();
                self.selected = 0;
                self.navigated = false;
            }
            MenuEvent::MoveLeft => {
                if self.navigated {
                    self.revert_buffer(editor);
                }
                self.active = false;
                self.items.clear();
                self.values.clear();
                self.selected = 0;
                self.navigated = false;
            }
        }
    }
}

impl MenuBuilder for LshMenu {
    fn settings_mut(&mut self) -> &mut MenuSettings {
        &mut self.settings
    }
}

impl Menu for LshMenu {
    fn settings(&self) -> &MenuSettings {
        &self.settings
    }

    fn indicator(&self) -> &str {
        ""
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.items.clear();
            self.values.clear();
            self.selected = 0;
            self.navigated = false;
            self.accepted.set(false);
        }
    }

    fn clear_input(&mut self) {
        self.items.clear();
        self.values.clear();
        self.selected = 0;
        self.navigated = false;
        self.accepted.set(false);
    }

    fn menu_event(&mut self, event: MenuEvent) {
        self.handle_menu_event(&event);
        self.event = Some(event);
    }

    fn can_quick_complete(&self) -> bool {
        true
    }

    fn can_partially_complete(
        &mut self,
        _values_updated: bool,
        editor: &mut Editor,
        _completer: &mut dyn Completer,
    ) -> bool {
        if self.items.len() <= 1 {
            return false;
        }

        let first = &self.items[0].replacement;
        let mut common_len = first.len();

        for item in &self.items[1..] {
            let len = first
                .chars()
                .zip(item.replacement.chars())
                .take_while(|(a, b)| a == b)
                .count();
            common_len = common_len.min(len);
        }

        let common: String = first.chars().take(common_len).collect();

        if common.len() > self.original_text.len() {
            let start = self.start;
            let end = self.start + self.current_replacement_len;
            let rep = common.clone();
            let rep_len = rep.len();
            editor.edit_buffer(
                move |b| {
                    b.replace_range(start..end, &rep);
                    b.set_insertion_point(start + rep_len);
                },
                UndoBehavior::CreateUndoPoint,
            );
            self.original_text = common;
            self.current_replacement_len = rep_len;
            true
        } else {
            false
        }
    }

    fn update_values(&mut self, editor: &mut Editor, _completer: &mut dyn Completer) {
        let line = editor.get_buffer().to_string();
        let pos = editor.line_buffer().insertion_point();
        let (start, items) = self.completer.get_candidates(&line, pos);

        self.start = start;
        self.end = pos;
        self.original_text = if start <= pos && pos <= line.len() {
            line[start..pos].to_string()
        } else {
            String::new()
        };
        self.current_replacement_len = self.original_text.len();
        self.navigated = false;
        self.accepted.set(false);

        if items.is_empty() {
            self.items.clear();
            self.values.clear();
            self.active = false;
            self.selected = 0;
            return;
        }

        self.items = items;
        self.values = self
            .items
            .iter()
            .map(|item| {
                let is_dir = matches!(item.kind, CompletionKind::Directory);
                Suggestion {
                    value: item.replacement.clone(),
                    display_override: Some(item.display.clone()),
                    description: item.description.clone(),
                    style: None,
                    extra: None,
                    span: reedline::Span::new(start, pos),
                    append_whitespace: !is_dir,
                    match_indices: None,
                }
            })
            .collect();

        if self.selected >= self.items.len() {
            self.selected = 0;
        }
    }

    fn reset_position(&mut self) {
        self.selected = 0;
        self.navigated = false;
    }

    fn update_working_details(
        &mut self,
        editor: &mut Editor,
        completer: &mut dyn Completer,
        painter: &Painter,
    ) {
        self.terminal_width = painter.screen_width();
        self.terminal_height = painter.screen_height();

        if let Some(event) = self.event.take() {
            self.apply_event(event, editor, completer);
        }
    }

    fn replace_in_buffer(&self, editor: &mut Editor) {
        self.accepted.set(true);
        if let Some(item) = self.items.get(self.selected) {
            let is_dir = matches!(item.kind, CompletionKind::Directory);
            let mut replacement = item.replacement.clone();
            if !is_dir {
                replacement.push(' ');
            }
            let start = self.start;
            let end = self.start + self.current_replacement_len;
            let rep_len = replacement.len();
            editor.edit_buffer(
                move |b| {
                    b.replace_range(start..end, &replacement);
                    b.set_insertion_point(start + rep_len);
                },
                UndoBehavior::CreateUndoPoint,
            );
        }
    }

    fn menu_required_lines(&self, _terminal_columns: u16) -> u16 {
        if !self.active || self.items.is_empty() {
            0
        } else {
            let visible_items = self.items.len().min(self.max_height);
            (visible_items + 2) as u16
        }
    }

    fn menu_string(&self, available_lines: u16, use_ansi_coloring: bool) -> String {
        if !self.active || self.items.is_empty() || available_lines < 3 {
            return String::new();
        }

        let term_cols = if self.terminal_width == 0 {
            80
        } else {
            self.terminal_width
        };

        let max_visible = (available_lines as usize)
            .saturating_sub(2)
            .min(self.max_height)
            .max(1);

        let (width, visible_height) = calculate_menu_dimensions(
            &self.items,
            term_cols,
            max_visible,
            self.completer.nerd_icons(),
        );

        let left_pad = (self.anchor_col as usize).min((term_cols as usize).saturating_sub(width));

        let lines = format_menu_lines(
            &self.items,
            self.selected,
            width,
            visible_height,
            self.completer.nerd_icons(),
            left_pad,
        );

        let mut result = lines.join("\r\n");
        if !use_ansi_coloring {
            result = strip_ansi(&result);
        }
        result
    }

    fn min_rows(&self) -> u16 {
        if !self.active || self.items.is_empty() {
            0
        } else {
            3
        }
    }

    fn get_values(&self) -> &[Suggestion] {
        &self.values
    }

    fn set_cursor_pos(&mut self, pos: (u16, u16)) {
        self.cursor_col = pos.0;
        if !self.navigated {
            self.anchor_col = pos
                .0
                .saturating_sub(self.original_text.chars().count() as u16);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::LineEditorHelper;

    fn make_test_editor(text: &str, pos: usize) -> Editor {
        let mut editor = Editor::default();
        let s = text.to_string();
        editor.edit_buffer(
            move |b| {
                b.clear();
                b.insert_str(&s);
                b.set_insertion_point(pos);
            },
            UndoBehavior::CreateUndoPoint,
        );
        editor
    }

    #[test]
    fn new_menu_is_inactive_and_empty() {
        let helper = LineEditorHelper::new();
        let menu = LshMenu::new("test_menu", helper);

        assert_eq!(menu.name(), "test_menu");
        assert_eq!(menu.indicator(), "");
        assert!(!menu.is_active());
        assert_eq!(menu.menu_required_lines(80), 0);
        assert_eq!(menu.menu_string(10, true), "");
        assert_eq!(menu.selected_index(), 0);
        assert!(menu.get_values().is_empty());
    }

    #[test]
    fn updates_values_and_calculates_required_lines() {
        let helper = LineEditorHelper::new();
        let mut menu = LshMenu::new("completion_menu", helper);
        let mut editor = make_test_editor("git c", 5);

        let mut dummy_completer = LineEditorHelper::new();
        menu.set_active(true);
        menu.update_values(&mut editor, &mut dummy_completer);

        assert!(!menu.items().is_empty());
        assert!(!menu.get_values().is_empty());
        let expected_lines = (menu.items().len().min(menu.max_height) + 2) as u16;
        assert_eq!(menu.menu_required_lines(80), expected_lines);

        let menu_str = menu.menu_string(10, true);
        assert!(menu_str.contains('┌'));
        assert!(menu_str.contains('└'));
        assert!(menu_str.contains("▶ "));
    }

    #[test]
    fn navigates_next_and_previous_with_wrap_and_updates_editor_buffer() {
        let helper = LineEditorHelper::new();
        let mut menu = LshMenu::new("completion_menu", helper);
        let mut editor = make_test_editor("git c", 5);

        let mut dummy_completer = LineEditorHelper::new();
        menu.set_active(true);
        menu.update_values(&mut editor, &mut dummy_completer);

        let total = menu.items().len();
        assert!(total > 1, "Need multiple candidates to test navigation");

        assert_eq!(menu.selected_index(), 0);
        // Initially before navigation, buffer is still "git c"
        assert_eq!(editor.get_buffer(), "git c");

        // First Tab / NextElement sets candidate 0 into prompt
        menu.apply_event(MenuEvent::NextElement, &mut editor, &mut dummy_completer);
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(
            editor.get_buffer(),
            format!("git {}", menu.items()[0].replacement)
        );

        // Second Tab / NextElement sets candidate 1 into prompt
        menu.apply_event(MenuEvent::NextElement, &mut editor, &mut dummy_completer);
        assert_eq!(menu.selected_index(), 1);
        assert_eq!(
            editor.get_buffer(),
            format!("git {}", menu.items()[1].replacement)
        );

        // PreviousElement moves back to candidate 0
        menu.apply_event(
            MenuEvent::PreviousElement,
            &mut editor,
            &mut dummy_completer,
        );
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(
            editor.get_buffer(),
            format!("git {}", menu.items()[0].replacement)
        );

        // MoveUp wraps backwards to last candidate
        menu.apply_event(MenuEvent::MoveUp, &mut editor, &mut dummy_completer);
        assert_eq!(menu.selected_index(), total - 1);
        assert_eq!(
            editor.get_buffer(),
            format!("git {}", menu.items()[total - 1].replacement)
        );

        // MoveDown wraps forwards to candidate 0
        menu.apply_event(MenuEvent::MoveDown, &mut editor, &mut dummy_completer);
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(
            editor.get_buffer(),
            format!("git {}", menu.items()[0].replacement)
        );
    }

    #[test]
    fn replace_in_buffer_inserts_selected_item_with_proper_spacing() {
        let helper = LineEditorHelper::new();
        let mut menu = LshMenu::new("completion_menu", helper);
        let mut editor = make_test_editor("git che", 7);

        let mut dummy_completer = LineEditorHelper::new();
        menu.set_active(true);
        menu.update_values(&mut editor, &mut dummy_completer);

        assert!(!menu.items().is_empty());
        assert_eq!(menu.items()[0].replacement, "checkout");

        menu.replace_in_buffer(&mut editor);
        assert_eq!(editor.get_buffer(), "git checkout ");
        assert_eq!(editor.line_buffer().insertion_point(), 13);
    }

    #[test]
    fn deactivation_clears_values_and_selection_and_reverts_buffer() {
        let helper = LineEditorHelper::new();
        let mut menu = LshMenu::new("completion_menu", helper);
        let mut editor = make_test_editor("git c", 5);

        let mut dummy_completer = LineEditorHelper::new();
        menu.set_active(true);
        menu.update_values(&mut editor, &mut dummy_completer);

        assert!(!menu.items().is_empty());

        // Navigate so buffer changes
        menu.apply_event(MenuEvent::NextElement, &mut editor, &mut dummy_completer);
        assert_ne!(editor.get_buffer(), "git c");

        // Cancel with Deactivate (e.g. Esc)
        menu.apply_event(MenuEvent::Deactivate, &mut editor, &mut dummy_completer);

        assert!(!menu.is_active());
        assert!(menu.items().is_empty());
        assert!(menu.get_values().is_empty());
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(menu.menu_required_lines(80), 0);
        // Prompt buffer reverted back to "git c"!
        assert_eq!(editor.get_buffer(), "git c");
    }

    #[test]
    fn partially_completes_common_prefix() {
        use lsh_completions::{CandidateSpec, CompletionKind, CompletionRule};

        let helper = LineEditorHelper::new();
        {
            let mut reg = helper.completion_registry.write().unwrap();
            let mut rule = CompletionRule::new();
            rule.candidates = vec![
                CandidateSpec::from_raw("test-apple", CompletionKind::Subcommand),
                CandidateSpec::from_raw("test-banana", CompletionKind::Subcommand),
            ];
            reg.register("my-cli", rule);
        }

        let mut menu = LshMenu::new("completion_menu", helper);
        let mut editor = make_test_editor("my-cli t", 8);

        let mut dummy_completer = LineEditorHelper::new();
        menu.set_active(true);
        menu.update_values(&mut editor, &mut dummy_completer);

        // "test-apple" and "test-banana" share "test-"
        let completed = menu.can_partially_complete(true, &mut editor, &mut dummy_completer);
        assert!(completed);
        assert_eq!(editor.get_buffer(), "my-cli test-");
    }
}
