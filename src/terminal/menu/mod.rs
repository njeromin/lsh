#[cfg(not(test))]
use std::io::{self, IsTerminal, Write};

#[cfg(not(test))]
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use crossterm::event::{KeyCode, KeyModifiers};
#[cfg(not(test))]
use crossterm::execute;
#[cfg(not(test))]
use crossterm::{cursor, queue, style, terminal};

pub mod fuzzy;
pub mod layout;
pub mod render;
pub mod types;

pub use fuzzy::*;
pub use layout::*;
pub use render::*;
pub use types::*;

pub fn handle_key_event(code: KeyCode, modifiers: KeyModifiers) -> MenuAction {
    match (code, modifiers) {
        // Next: Down, Ctrl-N
        (KeyCode::Down, _) => MenuAction::Next,
        (KeyCode::Char('n'), m) if m.contains(KeyModifiers::CONTROL) => MenuAction::Next,

        // Previous: Up, BackTab, Shift-Tab, Ctrl-P
        (KeyCode::Up, _) | (KeyCode::BackTab, _) => MenuAction::Previous,
        (KeyCode::Tab, m) if m.contains(KeyModifiers::SHIFT) => MenuAction::Previous,
        (KeyCode::Char('p'), m) if m.contains(KeyModifiers::CONTROL) => MenuAction::Previous,

        // Tab & Right: descend into directory or accept candidate
        (KeyCode::Tab, m) if !m.contains(KeyModifiers::SHIFT) => MenuAction::Tab,
        (KeyCode::Right, _) => MenuAction::Tab,

        // Slash: descend if directory candidate is selected, else regular filter char
        (KeyCode::Char('/'), m)
            if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            MenuAction::Slash
        }

        // Page scroll
        (KeyCode::PageDown, _) => MenuAction::PageDown,
        (KeyCode::PageUp, _) => MenuAction::PageUp,

        // Top / Bottom
        (KeyCode::Home, _) => MenuAction::First,
        (KeyCode::End, _) => MenuAction::Last,

        // Accept
        (KeyCode::Enter, _) => MenuAction::Accept,

        // Dismiss
        (KeyCode::Esc | KeyCode::Left, _) => MenuAction::Dismiss,
        (KeyCode::Char('c') | KeyCode::Char('g'), m) if m.contains(KeyModifiers::CONTROL) => {
            MenuAction::Dismiss
        }

        // Backspace: delete character from filter, or ascend directory
        (KeyCode::Backspace, _) => MenuAction::Backspace,

        // Typing: regular characters (letters, numbers, symbols) without Ctrl or Alt
        (KeyCode::Char(c), m)
            if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            MenuAction::Char(c)
        }

        _ => MenuAction::Dismiss,
    }
}

pub fn show_completion_menu<F>(
    base_prefix: &str,
    items: &[CompletionItem],
    max_visible: usize,
    nerd_icons: bool,
    #[allow(unused_mut)] mut on_descend: F,
) -> MenuResult
where
    F: FnMut(&str) -> Vec<CompletionItem>,
{
    if items.is_empty() {
        return MenuResult::Cancelled;
    }
    if items.len() == 1 {
        return MenuResult::Selected(items[0].clone());
    }
    #[cfg(test)]
    {
        let _ = (max_visible, base_prefix, nerd_icons, on_descend);
        MenuResult::Selected(items[0].clone())
    }
    #[cfg(not(test))]
    {
        if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
            return MenuResult::Selected(items[0].clone());
        }

        let mut cout = io::stdout();

        let (cursor_col, mut cursor_row) = cursor::position().unwrap_or((0, 0));
        let (term_cols, term_rows) = terminal::size().unwrap_or((80, 24));

        let start_col = cursor_col.saturating_sub(base_prefix.chars().count() as u16);
        let mut current_prefix = base_prefix.to_string();
        let mut filter_suffix = String::new();
        let mut initial_query = base_prefix
            .rsplit('/')
            .next()
            .unwrap_or(base_prefix)
            .to_string();
        let mut root_items = items.to_vec();
        let mut active_items = items.to_vec();
        let mut selected = 0;
        let mut cur_col = cursor_col;

        struct DescentLevel {
            prefix: String,
            items: Vec<CompletionItem>,
            initial_query: String,
        }
        let mut descent_stack: Vec<DescentLevel> = Vec::new();

        let (effective_max_visible, scroll_needed) =
            calculate_max_visible_height(cursor_row, term_rows, max_visible, items.len());

        if scroll_needed > 0 {
            let _ = execute!(cout, terminal::ScrollUp(scroll_needed));
            cursor_row = cursor_row.saturating_sub(scroll_needed);
            let _ = queue!(cout, cursor::MoveTo(cur_col, cursor_row));
        }

        let make_layout = |active: &[CompletionItem], anchor_col: u16, f_len: usize| {
            let (w, h) =
                calculate_menu_dimensions(active, term_cols, effective_max_visible, nerd_icons);
            let m_col = anchor_col.min(term_cols.saturating_sub(w as u16 + 2));
            let col = anchor_col + f_len as u16;
            (
                MenuLayout {
                    width: w,
                    height: h,
                    menu_col: m_col,
                    cursor_row,
                    cursor_col: col,
                    nerd_icons,
                },
                h as u16 + 2,
            )
        };

        let (layout, needed_rows) = make_layout(&active_items, cur_col, 0);
        let mut max_needed_rows = needed_rows;
        let _ = execute!(cout, cursor::Hide);
        let _ = draw_menu(&mut cout, &active_items, selected, layout);

        let result = loop {
            match event::read() {
                Ok(Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                })) if kind != KeyEventKind::Release => match handle_key_event(code, modifiers) {
                    MenuAction::Next => {
                        if !active_items.is_empty() {
                            selected = (selected + 1) % active_items.len();
                            let (l, _) =
                                make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                    }
                    MenuAction::Previous => {
                        if !active_items.is_empty() {
                            selected = if selected == 0 {
                                active_items.len() - 1
                            } else {
                                selected - 1
                            };
                            let (l, _) =
                                make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                    }
                    MenuAction::PageDown => {
                        if !active_items.is_empty() {
                            selected = (selected + layout.height).min(active_items.len() - 1);
                            let (l, _) =
                                make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                    }
                    MenuAction::PageUp => {
                        if !active_items.is_empty() {
                            selected = selected.saturating_sub(layout.height);
                            let (l, _) =
                                make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                    }
                    MenuAction::First => {
                        if !active_items.is_empty() {
                            selected = 0;
                            let (l, _) =
                                make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                    }
                    MenuAction::Last => {
                        if !active_items.is_empty() {
                            selected = active_items.len() - 1;
                            let (l, _) =
                                make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                    }
                    MenuAction::Tab => {
                        if active_items.is_empty() {
                            break MenuResult::Cancelled;
                        }
                        let selected_item = &active_items[selected];
                        if selected_item.is_dir() {
                            let mut new_prefix = selected_item.replacement.clone();
                            if !new_prefix.ends_with('/') {
                                new_prefix.push('/');
                            }

                            descent_stack.push(DescentLevel {
                                prefix: current_prefix.clone(),
                                items: root_items.clone(),
                                initial_query: initial_query.clone(),
                            });

                            let _ = erase_menu(
                                &mut cout,
                                max_needed_rows,
                                cursor_row,
                                cur_col + filter_suffix.chars().count() as u16,
                            );

                            let old_len =
                                current_prefix.chars().count() + filter_suffix.chars().count();
                            let new_len = new_prefix.chars().count();
                            let pad = if old_len > new_len {
                                " ".repeat(old_len - new_len)
                            } else {
                                String::new()
                            };
                            let _ = queue!(
                                cout,
                                cursor::MoveTo(start_col, cursor_row),
                                style::Print(format!("{new_prefix}{pad}")),
                                cursor::MoveTo(start_col + new_len as u16, cursor_row),
                            );
                            let _ = cout.flush();

                            let next_items = on_descend(&new_prefix);
                            current_prefix = new_prefix;
                            cur_col = start_col + new_len as u16;
                            filter_suffix.clear();
                            initial_query.clear();
                            root_items = next_items.clone();
                            active_items = next_items;
                            selected = 0;

                            let (l, needed) = make_layout(&active_items, cur_col, 0);
                            max_needed_rows = max_needed_rows.max(needed);
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        } else {
                            break MenuResult::Selected(selected_item.clone());
                        }
                    }
                    MenuAction::Accept => {
                        break if !active_items.is_empty() {
                            MenuResult::Selected(active_items[selected].clone())
                        } else {
                            MenuResult::Typed(format!("{current_prefix}{filter_suffix}"))
                        };
                    }
                    MenuAction::Slash => {
                        filter_suffix.push('/');
                        let query = format!("{initial_query}{filter_suffix}");
                        let new_items = filter_completion_items(&root_items, &query);

                        if new_items.is_empty() {
                            break MenuResult::Typed(format!("{current_prefix}{filter_suffix}"));
                        }

                        let char_col = cur_col + (filter_suffix.chars().count() - 1) as u16;
                        let _ = queue!(
                            cout,
                            cursor::MoveTo(char_col, cursor_row),
                            style::Print('/')
                        );

                        let _ = erase_menu(
                            &mut cout,
                            max_needed_rows,
                            cursor_row,
                            cur_col + filter_suffix.chars().count() as u16,
                        );

                        active_items = new_items;
                        selected = selected.min(active_items.len() - 1);

                        let (l, needed) =
                            make_layout(&active_items, cur_col, filter_suffix.chars().count());
                        max_needed_rows = max_needed_rows.max(needed);
                        let _ = draw_menu(&mut cout, &active_items, selected, l);
                    }
                    MenuAction::Dismiss => {
                        break MenuResult::Cancelled;
                    }
                    MenuAction::Char(c) => {
                        filter_suffix.push(c);
                        let query = format!("{initial_query}{filter_suffix}");
                        let new_items = filter_completion_items(&root_items, &query);

                        if new_items.is_empty() {
                            break MenuResult::Typed(format!("{current_prefix}{filter_suffix}"));
                        }

                        let char_col = cur_col + (filter_suffix.chars().count() - 1) as u16;
                        let _ = queue!(cout, cursor::MoveTo(char_col, cursor_row), style::Print(c));

                        let _ = erase_menu(
                            &mut cout,
                            max_needed_rows,
                            cursor_row,
                            cur_col + filter_suffix.chars().count() as u16,
                        );

                        active_items = new_items;
                        selected = selected.min(active_items.len() - 1);

                        let (l, needed) =
                            make_layout(&active_items, cur_col, filter_suffix.chars().count());
                        max_needed_rows = max_needed_rows.max(needed);
                        let _ = draw_menu(&mut cout, &active_items, selected, l);
                    }
                    MenuAction::Backspace => {
                        if !filter_suffix.is_empty() {
                            let char_col = cur_col + filter_suffix.chars().count() as u16 - 1;
                            let _ = queue!(
                                cout,
                                cursor::MoveTo(char_col, cursor_row),
                                style::Print(' '),
                                cursor::MoveTo(char_col, cursor_row),
                            );

                            filter_suffix.pop();
                            let query = format!("{initial_query}{filter_suffix}");
                            active_items = filter_completion_items(&root_items, &query);
                            selected = selected.min(active_items.len().saturating_sub(1));

                            let _ = erase_menu(
                                &mut cout,
                                max_needed_rows,
                                cursor_row,
                                cur_col + filter_suffix.chars().count() as u16,
                            );

                            let (l, needed) =
                                make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            max_needed_rows = max_needed_rows.max(needed);
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        } else if let Some(parent) = descent_stack.pop() {
                            let _ = erase_menu(&mut cout, max_needed_rows, cursor_row, cur_col);

                            let old_len = current_prefix.chars().count();
                            let new_prefix = parent.prefix;
                            let new_len = new_prefix.chars().count();
                            let pad = if old_len > new_len {
                                " ".repeat(old_len - new_len)
                            } else {
                                String::new()
                            };
                            let _ = queue!(
                                cout,
                                cursor::MoveTo(start_col, cursor_row),
                                style::Print(format!("{new_prefix}{pad}")),
                                cursor::MoveTo(start_col + new_len as u16, cursor_row),
                            );
                            let _ = cout.flush();

                            current_prefix = new_prefix;
                            cur_col = start_col + new_len as u16;
                            filter_suffix.clear();
                            initial_query = parent.initial_query;
                            root_items = parent.items.clone();
                            active_items = parent.items;
                            selected = 0;

                            let (l, needed) = make_layout(&active_items, cur_col, 0);
                            max_needed_rows = max_needed_rows.max(needed);
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        } else {
                            let mut new_prefix = current_prefix.clone();
                            if !new_prefix.is_empty() {
                                new_prefix.pop();
                                break MenuResult::Typed(new_prefix);
                            } else {
                                break MenuResult::Cancelled;
                            }
                        }
                    }
                },
                Err(_) => break MenuResult::Cancelled,
                _ => {}
            }
        };

        let final_col = cur_col + filter_suffix.chars().count() as u16;
        let _ = erase_menu(&mut cout, max_needed_rows, cursor_row, final_col);

        if matches!(result, MenuResult::Cancelled) {
            let current_len = current_prefix.chars().count() + filter_suffix.chars().count();
            let base_len = base_prefix.chars().count();
            let pad = if current_len > base_len {
                " ".repeat(current_len - base_len)
            } else {
                String::new()
            };
            let _ = queue!(
                cout,
                cursor::MoveTo(start_col, cursor_row),
                style::Print(format!("{base_prefix}{pad}")),
                cursor::MoveTo(start_col + base_len as u16, cursor_row),
            );
            let _ = cout.flush();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_navigation_key_events_correctly() {
        // Next: Down, Ctrl-N
        assert_eq!(
            handle_key_event(KeyCode::Down, KeyModifiers::NONE),
            MenuAction::Next
        );
        assert_eq!(
            handle_key_event(KeyCode::Char('n'), KeyModifiers::CONTROL),
            MenuAction::Next
        );

        // Tab & Right: descend into directory or accept candidate
        assert_eq!(
            handle_key_event(KeyCode::Tab, KeyModifiers::NONE),
            MenuAction::Tab
        );
        assert_eq!(
            handle_key_event(KeyCode::Right, KeyModifiers::NONE),
            MenuAction::Tab
        );

        // Slash: descend if on directory, else filter
        assert_eq!(
            handle_key_event(KeyCode::Char('/'), KeyModifiers::NONE),
            MenuAction::Slash
        );

        // Previous: Up, BackTab, Shift-Tab, Ctrl-P
        assert_eq!(
            handle_key_event(KeyCode::Up, KeyModifiers::NONE),
            MenuAction::Previous
        );
        assert_eq!(
            handle_key_event(KeyCode::BackTab, KeyModifiers::NONE),
            MenuAction::Previous
        );
        assert_eq!(
            handle_key_event(KeyCode::Tab, KeyModifiers::SHIFT),
            MenuAction::Previous
        );
        assert_eq!(
            handle_key_event(KeyCode::Char('p'), KeyModifiers::CONTROL),
            MenuAction::Previous
        );

        // Page navigation
        assert_eq!(
            handle_key_event(KeyCode::PageDown, KeyModifiers::NONE),
            MenuAction::PageDown
        );
        assert_eq!(
            handle_key_event(KeyCode::PageUp, KeyModifiers::NONE),
            MenuAction::PageUp
        );

        // First / Last
        assert_eq!(
            handle_key_event(KeyCode::Home, KeyModifiers::NONE),
            MenuAction::First
        );
        assert_eq!(
            handle_key_event(KeyCode::End, KeyModifiers::NONE),
            MenuAction::Last
        );

        // Accept
        assert_eq!(
            handle_key_event(KeyCode::Enter, KeyModifiers::NONE),
            MenuAction::Accept
        );

        // Dismiss
        assert_eq!(
            handle_key_event(KeyCode::Esc, KeyModifiers::NONE),
            MenuAction::Dismiss
        );
        assert_eq!(
            handle_key_event(KeyCode::Left, KeyModifiers::NONE),
            MenuAction::Dismiss
        );
        assert_eq!(
            handle_key_event(KeyCode::Char('c'), KeyModifiers::CONTROL),
            MenuAction::Dismiss
        );
        assert_eq!(
            handle_key_event(KeyCode::Char('g'), KeyModifiers::CONTROL),
            MenuAction::Dismiss
        );

        // Typing and Backspace
        assert_eq!(
            handle_key_event(KeyCode::Char('a'), KeyModifiers::NONE),
            MenuAction::Char('a')
        );
        assert_eq!(
            handle_key_event(KeyCode::Char('A'), KeyModifiers::SHIFT),
            MenuAction::Char('A')
        );
        assert_eq!(
            handle_key_event(KeyCode::Backspace, KeyModifiers::NONE),
            MenuAction::Backspace
        );
    }
}
