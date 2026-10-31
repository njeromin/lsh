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
    let border_fill = layout.width.saturating_sub(border_title.len() + 2);
    queue!(
        writer,
        cursor::MoveTo(layout.menu_col, layout.cursor_row + 1),
        style::Print(format!(
            "\x1b[38;5;240m┌{border_title}{}\x1b[38;5;240m┐\x1b[0m",
            "─".repeat(border_fill)
        )),
    )?;

    // Item rows
    for (r, item) in items[start_idx..end_idx].iter().enumerate() {
        let item_idx = start_idx + r;
        let is_selected = item_idx == selected;
        let marker = if is_selected { "▶ " } else { "  " };

        let name = &item.display;
        let badge = item.kind.badge();
        let inner_space = layout.width.saturating_sub(2);
        let used_len = 2 + name.chars().count() + badge.len();
        let pad_len = inner_space.saturating_sub(used_len);
        let pad = " ".repeat(pad_len);

        queue!(writer, cursor::MoveTo(layout.menu_col, layout.cursor_row + 2 + r as u16))?;

        if is_selected {
            queue!(
                writer,
                style::Print(format!(
                    "\x1b[38;5;240m│\x1b[7m {marker}{name}{pad}{badge} \x1b[0m\x1b[38;5;240m│\x1b[0m"
                ))
            )?;
        } else {
            let badge_colored = format!("{}{badge}\x1b[0m", item.kind.color());
            queue!(
                writer,
                style::Print(format!(
                    "\x1b[38;5;240m│\x1b[0m {marker}{name}{pad}{badge_colored} \x1b[38;5;240m│\x1b[0m"
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
    let footer_fill = layout.width.saturating_sub(footer_title.len() + 2);
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

        _ => MenuAction::Dismiss,
    }
}

pub fn show_completion_menu(items: &[CompletionItem], max_visible: usize) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    if items.len() == 1 {
        return Some(0);
    }
    #[cfg(test)]
    {
        let _ = max_visible;
        Some(0)
    }
    #[cfg(not(test))]
    {
        if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
            return Some(0);
        }

        let mut cout = io::stdout();

        let (cursor_col, mut cursor_row) = cursor::position().unwrap_or((0, 0));
        let (term_cols, term_rows) = terminal::size().unwrap_or((80, 24));

        let (width, height) = calculate_menu_dimensions(items, term_cols, max_visible);
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

        let layout = MenuLayout {
            width,
            height,
            menu_col,
            cursor_row,
            cursor_col,
        };

        let mut selected = 0;
        let _ = execute!(cout, cursor::Hide);

        let _ = draw_menu(&mut cout, items, selected, layout);

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
                            selected = (selected + 1) % items.len();
                            let _ = draw_menu(&mut cout, items, selected, layout);
                        }
                        MenuAction::Previous => {
                            selected = if selected == 0 {
                                items.len() - 1
                            } else {
                                selected - 1
                            };
                            let _ = draw_menu(&mut cout, items, selected, layout);
                        }
                        MenuAction::PageDown => {
                            selected = (selected + height).min(items.len() - 1);
                            let _ = draw_menu(&mut cout, items, selected, layout);
                        }
                        MenuAction::PageUp => {
                            selected = selected.saturating_sub(height);
                            let _ = draw_menu(&mut cout, items, selected, layout);
                        }
                        MenuAction::First => {
                            selected = 0;
                            let _ = draw_menu(&mut cout, items, selected, layout);
                        }
                        MenuAction::Last => {
                            selected = items.len() - 1;
                            let _ = draw_menu(&mut cout, items, selected, layout);
                        }
                        MenuAction::Accept => {
                            break Some(selected);
                        }
                        MenuAction::Dismiss => {
                            break None;
                        }
                    }
                }
                Err(_) => break None,
                _ => {}
            }
        };

        let _ = erase_menu(&mut cout, needed_rows, cursor_row, cursor_col);
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
    }
}
