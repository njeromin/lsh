use std::io::{self, Write};
#[cfg(not(test))]
use std::io::IsTerminal;

use crossterm::event::{KeyCode, KeyModifiers};
#[cfg(not(test))]
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
#[cfg(not(test))]
use crossterm::execute;
use crossterm::{cursor, queue, style, terminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Builtin,
    Alias,
    Function,
    Binary,
    Variable,
    File,
    Directory,
}

impl CompletionKind {
    pub fn badge(&self) -> &'static str {
        match self {
            CompletionKind::Builtin => "[builtin]",
            CompletionKind::Alias => "[alias]",
            CompletionKind::Function => "[fn]",
            CompletionKind::Binary => "[bin]",
            CompletionKind::Variable => "[var]",
            CompletionKind::File => "[file]",
            CompletionKind::Directory => "[dir]",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            CompletionKind::Builtin => "\x1b[35m",     // Magenta
            CompletionKind::Alias => "\x1b[36m",       // Cyan
            CompletionKind::Function => "\x1b[34m",    // Blue
            CompletionKind::Binary => "\x1b[32m",      // Green
            CompletionKind::Variable => "\x1b[33m",    // Yellow
            CompletionKind::File => "\x1b[37m",        // White
            CompletionKind::Directory => "\x1b[1;34m", // Bold Blue
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub replacement: String,
    pub display: String,
    pub kind: CompletionKind,
}

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

pub fn calculate_menu_dimensions(
    items: &[CompletionItem],
    term_cols: u16,
    max_visible: usize,
) -> (usize, usize) {
    let max_item_len = items
        .iter()
        .map(|it| it.display.chars().count() + it.kind.badge().len() + 6)
        .max()
        .unwrap_or(20);

    let max_allowed = (term_cols as usize).saturating_sub(4).max(24);
    let width = max_item_len.clamp(26, max_allowed);
    let height = items.len().min(max_visible);

    (width, height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuLayout {
    pub width: usize,
    pub height: usize,
    pub menu_col: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
}

pub fn draw_menu<W: Write>(
    writer: &mut W,
    items: &[CompletionItem],
    selected: usize,
    layout: MenuLayout,
) -> io::Result<()> {
    let (start_idx, end_idx) = calculate_viewport(items.len(), selected, layout.height);

    // Top border
    let border_title = if start_idx > 0 {
        format!("▲ {} more ", start_idx)
    } else {
        String::new()
    };
    let border_fill = layout
        .width
        .saturating_sub(border_title.chars().count() + 2);
    queue!(
        writer,
        cursor::MoveTo(layout.menu_col, layout.cursor_row + 1),
        style::Print(format!(
            "\x1b[38;5;240m┌{border_title}{}┐\x1b[0m",
            "─".repeat(border_fill)
        )),
    )?;

    // Item rows
    let inner_space = layout.width.saturating_sub(2);
    for (r, item) in items[start_idx..end_idx].iter().enumerate() {
        let item_idx = start_idx + r;
        let is_selected = item_idx == selected;
        let marker = if is_selected { "▶ " } else { "  " };

        let name = &item.display;
        let badge = item.kind.badge();

        // Visible inner layout: " " (1) + marker (2) + name + pad + badge + " " (1)
        // Overhead without name: 1 + 2 + 1 (min space before badge) + badge_len + 1 = 5 + badge_len
        let overhead = 5 + badge.chars().count();
        let max_name_len = inner_space.saturating_sub(overhead);

        let name_str: String = if name.chars().count() > max_name_len && max_name_len > 1 {
            let mut s: String = name.chars().take(max_name_len - 1).collect();
            s.push('…');
            s
        } else {
            name.to_string()
        };

        let used_len = 1 + 2 + name_str.chars().count() + badge.chars().count() + 1;
        let pad_len = inner_space.saturating_sub(used_len);
        let pad = " ".repeat(pad_len);

        queue!(writer, cursor::MoveTo(layout.menu_col, layout.cursor_row + 2 + r as u16))?;

        if is_selected {
            queue!(
                writer,
                style::Print(format!(
                    "\x1b[38;5;240m│\x1b[7m {marker}{name_str}{pad}{badge} \x1b[0m\x1b[38;5;240m│\x1b[0m"
                ))
            )?;
        } else {
            let badge_colored = format!("{}{badge}\x1b[0m", item.kind.color());
            queue!(
                writer,
                style::Print(format!(
                    "\x1b[38;5;240m│\x1b[0m {marker}{name_str}{pad}{badge_colored} \x1b[38;5;240m│\x1b[0m"
                ))
            )?;
        }
    }

    // Bottom border
    let footer_title = if end_idx < items.len() {
        format!("▼ {} more ", items.len() - end_idx)
    } else {
        String::new()
    };
    let footer_fill = layout
        .width
        .saturating_sub(footer_title.chars().count() + 2);
    queue!(
        writer,
        cursor::MoveTo(layout.menu_col, layout.cursor_row + 2 + layout.height as u16),
        style::Print(format!(
            "\x1b[38;5;240m└{footer_title}{}┘\x1b[0m",
            "─".repeat(footer_fill)
        )),
        cursor::MoveTo(layout.cursor_col, layout.cursor_row),
    )?;

    writer.flush()
}

pub fn erase_menu<W: Write>(
    writer: &mut W,
    needed_rows: u16,
    cursor_row: u16,
    cursor_col: u16,
) -> io::Result<()> {
    for row_offset in 1..=needed_rows {
        queue!(
            writer,
            cursor::MoveTo(0, cursor_row + row_offset),
            terminal::Clear(terminal::ClearType::CurrentLine),
        )?;
    }
    queue!(
        writer,
        cursor::MoveTo(cursor_col, cursor_row),
        cursor::Show,
    )?;
    writer.flush()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuResult {
    /// Accepted a candidate from the dropdown list
    Selected(CompletionItem),
    /// Typed past all matches or committed typed text
    Typed(String),
    /// Cancelled without accepting (e.g. Esc or Ctrl-C)
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Next,
    Previous,
    PageDown,
    PageUp,
    First,
    Last,
    Accept,
    Dismiss,
    Char(char),
    Backspace,
}

pub fn filter_completion_items(items: &[CompletionItem], query: &str) -> Vec<CompletionItem> {
    if query.is_empty() {
        return items.to_vec();
    }
    let query_lower = query.to_lowercase();
    items
        .iter()
        .filter(|it| {
            it.replacement.starts_with(query)
                || it.display.starts_with(query)
                || it.replacement.to_lowercase().starts_with(&query_lower)
                || it.display.to_lowercase().starts_with(&query_lower)
        })
        .cloned()
        .collect()
}

pub fn handle_key_event(code: KeyCode, modifiers: KeyModifiers) -> MenuAction {
    match (code, modifiers) {
        // Next: Down, Tab (without Shift), Ctrl-N
        (KeyCode::Down, _) => MenuAction::Next,
        (KeyCode::Tab, m) if !m.contains(KeyModifiers::SHIFT) => MenuAction::Next,
        (KeyCode::Char('n'), m) if m.contains(KeyModifiers::CONTROL) => MenuAction::Next,

        // Previous: Up, BackTab, Shift-Tab, Ctrl-P
        (KeyCode::Up, _) | (KeyCode::BackTab, _) => MenuAction::Previous,
        (KeyCode::Tab, m) if m.contains(KeyModifiers::SHIFT) => MenuAction::Previous,
        (KeyCode::Char('p'), m) if m.contains(KeyModifiers::CONTROL) => MenuAction::Previous,

        // Page scroll
        (KeyCode::PageDown, _) => MenuAction::PageDown,
        (KeyCode::PageUp, _) => MenuAction::PageUp,

        // Top / Bottom
        (KeyCode::Home, _) => MenuAction::First,
        (KeyCode::End, _) => MenuAction::Last,

        // Accept
        (KeyCode::Enter | KeyCode::Right, _) => MenuAction::Accept,

        // Dismiss
        (KeyCode::Esc | KeyCode::Left, _) => MenuAction::Dismiss,
        (KeyCode::Char('c') | KeyCode::Char('g'), m) if m.contains(KeyModifiers::CONTROL) => {
            MenuAction::Dismiss
        }

        // Backspace: delete character from filter
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

pub fn show_completion_menu(
    base_prefix: &str,
    items: &[CompletionItem],
    max_visible: usize,
) -> MenuResult {
    if items.is_empty() {
        return MenuResult::Cancelled;
    }
    if items.len() == 1 {
        return MenuResult::Selected(items[0].clone());
    }
    #[cfg(test)]
    {
        let _ = max_visible;
        let _ = base_prefix;
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

        let mut filter_suffix = String::new();
        let mut active_items = items.to_vec();
        let mut selected = 0;

        let (width, height) = calculate_menu_dimensions(&active_items, term_cols, max_visible);
        let menu_col = cursor_col.min(term_cols.saturating_sub(width as u16 + 2));

        let needed_rows = height as u16 + 2;
        if cursor_row + needed_rows >= term_rows {
            let overflow = (cursor_row + needed_rows + 1).saturating_sub(term_rows);
            if overflow > 0 {
                for _ in 0..overflow {
                    let _ = writeln!(cout);
                }
                let _ = cout.flush();
                cursor_row = cursor_row.saturating_sub(overflow);
            }
        }

        let mut max_needed_rows = needed_rows;
        let _ = execute!(cout, cursor::Hide);

        let layout = MenuLayout {
            width,
            height,
            menu_col,
            cursor_row,
            cursor_col,
        };

        let _ = draw_menu(&mut cout, &active_items, selected, layout);

        let result = loop {
            match event::read() {
                Ok(Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                })) if kind != KeyEventKind::Release => {
                    match handle_key_event(code, modifiers) {
                        MenuAction::Next => {
                            if !active_items.is_empty() {
                                if active_items.len() == 1 {
                                    break MenuResult::Selected(active_items[0].clone());
                                }
                                selected = (selected + 1) % active_items.len();
                                let (w, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                let m_col = cursor_col.min(term_cols.saturating_sub(w as u16 + 2));
                                let cur_col =
                                    cursor_col + filter_suffix.chars().count() as u16;
                                let l = MenuLayout {
                                    width: w,
                                    height: h,
                                    menu_col: m_col,
                                    cursor_row,
                                    cursor_col: cur_col,
                                };
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
                                let (w, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                let m_col = cursor_col.min(term_cols.saturating_sub(w as u16 + 2));
                                let cur_col =
                                    cursor_col + filter_suffix.chars().count() as u16;
                                let l = MenuLayout {
                                    width: w,
                                    height: h,
                                    menu_col: m_col,
                                    cursor_row,
                                    cursor_col: cur_col,
                                };
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::PageDown => {
                            if !active_items.is_empty() {
                                let (_, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                selected = (selected + h).min(active_items.len() - 1);
                                let (w, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                let m_col = cursor_col.min(term_cols.saturating_sub(w as u16 + 2));
                                let cur_col =
                                    cursor_col + filter_suffix.chars().count() as u16;
                                let l = MenuLayout {
                                    width: w,
                                    height: h,
                                    menu_col: m_col,
                                    cursor_row,
                                    cursor_col: cur_col,
                                };
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::PageUp => {
                            if !active_items.is_empty() {
                                let (_, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                selected = selected.saturating_sub(h);
                                let (w, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                let m_col = cursor_col.min(term_cols.saturating_sub(w as u16 + 2));
                                let cur_col =
                                    cursor_col + filter_suffix.chars().count() as u16;
                                let l = MenuLayout {
                                    width: w,
                                    height: h,
                                    menu_col: m_col,
                                    cursor_row,
                                    cursor_col: cur_col,
                                };
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::First => {
                            if !active_items.is_empty() {
                                selected = 0;
                                let (w, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                let m_col = cursor_col.min(term_cols.saturating_sub(w as u16 + 2));
                                let cur_col =
                                    cursor_col + filter_suffix.chars().count() as u16;
                                let l = MenuLayout {
                                    width: w,
                                    height: h,
                                    menu_col: m_col,
                                    cursor_row,
                                    cursor_col: cur_col,
                                };
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::Last => {
                            if !active_items.is_empty() {
                                selected = active_items.len() - 1;
                                let (w, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                let m_col = cursor_col.min(term_cols.saturating_sub(w as u16 + 2));
                                let cur_col =
                                    cursor_col + filter_suffix.chars().count() as u16;
                                let l = MenuLayout {
                                    width: w,
                                    height: h,
                                    menu_col: m_col,
                                    cursor_row,
                                    cursor_col: cur_col,
                                };
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::Accept => {
                            break if !active_items.is_empty() {
                                MenuResult::Selected(active_items[selected].clone())
                            } else {
                                MenuResult::Typed(format!("{base_prefix}{filter_suffix}"))
                            };
                        }
                        MenuAction::Dismiss => {
                            break MenuResult::Cancelled;
                        }
                        MenuAction::Char(c) => {
                            filter_suffix.push(c);
                            let query = format!("{base_prefix}{filter_suffix}");
                            let new_items = filter_completion_items(items, &query);

                            if new_items.is_empty() {
                                break MenuResult::Typed(query);
                            }

                            let char_col = cursor_col + (filter_suffix.chars().count() - 1) as u16;
                            let _ = queue!(cout, cursor::MoveTo(char_col, cursor_row), style::Print(c));

                            let _ = erase_menu(
                                &mut cout,
                                max_needed_rows,
                                cursor_row,
                                cursor_col + filter_suffix.chars().count() as u16,
                            );

                            active_items = new_items;
                            selected = selected.min(active_items.len() - 1);

                            let (w, h) =
                                calculate_menu_dimensions(&active_items, term_cols, max_visible);
                            let m_col = cursor_col.min(term_cols.saturating_sub(w as u16 + 2));
                            let needed = h as u16 + 2;
                            max_needed_rows = max_needed_rows.max(needed);
                            let cur_col =
                                cursor_col + filter_suffix.chars().count() as u16;

                            let l = MenuLayout {
                                width: w,
                                height: h,
                                menu_col: m_col,
                                cursor_row,
                                cursor_col: cur_col,
                            };
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                        MenuAction::Backspace => {
                            if filter_suffix.is_empty() {
                                let mut new_prefix = base_prefix.to_string();
                                if !new_prefix.is_empty() {
                                    new_prefix.pop();
                                    break MenuResult::Typed(new_prefix);
                                } else {
                                    break MenuResult::Cancelled;
                                }
                            } else {
                                let char_col =
                                    cursor_col + filter_suffix.chars().count() as u16 - 1;
                                let _ = queue!(
                                    cout,
                                    cursor::MoveTo(char_col, cursor_row),
                                    style::Print(' '),
                                    cursor::MoveTo(char_col, cursor_row),
                                );

                                filter_suffix.pop();
                                let query = format!("{base_prefix}{filter_suffix}");
                                active_items = filter_completion_items(items, &query);
                                selected = selected.min(active_items.len().saturating_sub(1));

                                let _ = erase_menu(
                                    &mut cout,
                                    max_needed_rows,
                                    cursor_row,
                                    cursor_col + filter_suffix.chars().count() as u16,
                                );

                                let (w, h) =
                                    calculate_menu_dimensions(&active_items, term_cols, max_visible);
                                let m_col = cursor_col.min(term_cols.saturating_sub(w as u16 + 2));
                                let needed = h as u16 + 2;
                                max_needed_rows = max_needed_rows.max(needed);
                                let cur_col =
                                    cursor_col + filter_suffix.chars().count() as u16;

                                let l = MenuLayout {
                                    width: w,
                                    height: h,
                                    menu_col: m_col,
                                    cursor_row,
                                    cursor_col: cur_col,
                                };
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                    }
                }
                Err(_) => break MenuResult::Cancelled,
                _ => {}
            }
        };

        let _ = erase_menu(&mut cout, max_needed_rows, cursor_row, cursor_col);
        if matches!(result, MenuResult::Cancelled) && !filter_suffix.is_empty() {
            let _ = queue!(
                cout,
                cursor::MoveTo(cursor_col, cursor_row),
                style::Print(" ".repeat(filter_suffix.chars().count())),
                cursor::MoveTo(cursor_col, cursor_row),
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
    fn viewport_within_bounds_when_items_fit() {
        assert_eq!(calculate_viewport(5, 2, 7), (0, 5));
        assert_eq!(calculate_viewport(7, 6, 7), (0, 7));
    }

    #[test]
    fn viewport_scrolls_centered_when_items_exceed_limit() {
        // total 20, max_visible 6 (half = 3)
        // at 0: start at 0
        assert_eq!(calculate_viewport(20, 0, 6), (0, 6));
        // at 2: still fits near top
        assert_eq!(calculate_viewport(20, 2, 6), (0, 6));
        // at 5: center viewport: 5 - 3 = 2
        assert_eq!(calculate_viewport(20, 5, 6), (2, 8));
        // at 18 (near bottom): 20 - 6 = 14
        assert_eq!(calculate_viewport(20, 18, 6), (14, 20));
        // at 19 (last item)
        assert_eq!(calculate_viewport(20, 19, 6), (14, 20));
    }

    #[test]
    fn menu_dimensions_scale_to_content_and_terminal() {
        let items = vec![
            CompletionItem {
                replacement: "cargo".to_string(),
                display: "cargo".to_string(),
                kind: CompletionKind::Binary,
            },
            CompletionItem {
                replacement: "cat".to_string(),
                display: "cat".to_string(),
                kind: CompletionKind::Binary,
            },
        ];

        let (width, height) = calculate_menu_dimensions(&items, 80, 7);
        assert_eq!(height, 2);
        assert!(width >= 26);
        assert!(width <= 76);
    }

    #[test]
    fn completion_badges_and_colors() {
        assert_eq!(CompletionKind::Builtin.badge(), "[builtin]");
        assert_eq!(CompletionKind::Binary.badge(), "[bin]");
        assert_eq!(CompletionKind::Directory.badge(), "[dir]");
        assert_eq!(CompletionKind::File.badge(), "[file]");
        assert_eq!(CompletionKind::Variable.badge(), "[var]");
        assert_eq!(CompletionKind::Alias.badge(), "[alias]");
        assert_eq!(CompletionKind::Function.badge(), "[fn]");
    }

    #[test]
    fn draw_and_erase_menu_output_proper_ansi_sequences() {
        let items = vec![
            CompletionItem {
                replacement: "cargo".to_string(),
                display: "cargo".to_string(),
                kind: CompletionKind::Binary,
            },
            CompletionItem {
                replacement: "cd".to_string(),
                display: "cd".to_string(),
                kind: CompletionKind::Builtin,
            },
        ];

        let mut buf = Vec::new();
        let layout = MenuLayout {
            width: 30,
            height: 2,
            menu_col: 0,
            cursor_row: 10,
            cursor_col: 5,
        };
        draw_menu(&mut buf, &items, 0, layout).unwrap();
        let rendered = String::from_utf8_lossy(&buf);

        assert!(rendered.contains("┌"));
        assert!(rendered.contains("▶ cargo"));
        assert!(rendered.contains("[bin]"));
        assert!(rendered.contains("cd"));
        assert!(rendered.contains("[builtin]"));
        assert!(rendered.contains("└"));

        let mut erase_buf = Vec::new();
        erase_menu(&mut erase_buf, 4, 10, 5).unwrap();
        assert!(!erase_buf.is_empty());
    }

    #[test]
    fn box_borders_and_rows_align_to_exact_width() {
        let items = vec![
            CompletionItem {
                replacement: "cargo".to_string(),
                display: "cargo".to_string(),
                kind: CompletionKind::Binary,
            },
            CompletionItem {
                replacement: "cd".to_string(),
                display: "cd".to_string(),
                kind: CompletionKind::Builtin,
            },
        ];

        let width = 30;
        let mut buf = Vec::new();
        let layout = MenuLayout {
            width,
            height: 2,
            menu_col: 0,
            cursor_row: 10,
            cursor_col: 5,
        };
        draw_menu(&mut buf, &items, 0, layout).unwrap();
        let rendered = String::from_utf8_lossy(&buf);

        // Top and bottom borders must have exactly width (30 chars)
        assert!(rendered.contains("┌────────────────────────────┐"));
        assert!(rendered.contains("└────────────────────────────┘"));

        // Inner item content must be exactly width - 2 (28 chars)
        assert!(rendered.contains(" ▶ cargo              [bin] "));
        assert!(rendered.contains("   cd             "));
        assert!(rendered.contains("[builtin]\x1b[0m "));
    }

    #[test]
    fn handles_navigation_key_events_correctly() {
        // Next: Down, Tab (no shift), Ctrl-N
        assert_eq!(
            handle_key_event(KeyCode::Down, KeyModifiers::NONE),
            MenuAction::Next
        );
        assert_eq!(
            handle_key_event(KeyCode::Tab, KeyModifiers::NONE),
            MenuAction::Next
        );
        assert_eq!(
            handle_key_event(KeyCode::Char('n'), KeyModifiers::CONTROL),
            MenuAction::Next
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
        assert_eq!(
            handle_key_event(KeyCode::Right, KeyModifiers::NONE),
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

    #[test]
    fn filter_completion_items_narrows_candidates_dynamically() {
        let items = vec![
            CompletionItem {
                replacement: "cargo".to_string(),
                display: "cargo".to_string(),
                kind: CompletionKind::Binary,
            },
            CompletionItem {
                replacement: "cat".to_string(),
                display: "cat".to_string(),
                kind: CompletionKind::Binary,
            },
            CompletionItem {
                replacement: "cd".to_string(),
                display: "cd".to_string(),
                kind: CompletionKind::Builtin,
            },
        ];

        // Empty query keeps all items
        assert_eq!(filter_completion_items(&items, "").len(), 3);

        // "ca" keeps cargo and cat
        let ca_matches = filter_completion_items(&items, "ca");
        assert_eq!(ca_matches.len(), 2);
        assert_eq!(ca_matches[0].replacement, "cargo");
        assert_eq!(ca_matches[1].replacement, "cat");

        // "car" keeps cargo
        let car_matches = filter_completion_items(&items, "car");
        assert_eq!(car_matches.len(), 1);
        assert_eq!(car_matches[0].replacement, "cargo");

        // "card" matches nothing
        assert_eq!(filter_completion_items(&items, "card").len(), 0);

        // Case-insensitive query matches
        let upper_matches = filter_completion_items(&items, "CD");
        assert_eq!(upper_matches.len(), 1);
        assert_eq!(upper_matches[0].replacement, "cd");
    }
}
