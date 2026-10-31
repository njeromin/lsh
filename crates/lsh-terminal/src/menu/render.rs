use std::io::{self, Write};

use crossterm::{cursor, queue, style, terminal};

use super::layout::calculate_viewport;
use super::types::{CompletionItem, MenuLayout};

pub fn draw_menu<W: Write>(
    writer: &mut W,
    items: &[CompletionItem],
    selected: usize,
    layout: MenuLayout,
) -> io::Result<()> {
    let (start_idx, end_idx) = calculate_viewport(items.len(), selected, layout.height);

    let top_row = layout.cursor_row + 1;
    let item_start_row = layout.cursor_row + 2;
    let bottom_row = layout.cursor_row + 2 + layout.height as u16;

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
        cursor::MoveTo(layout.menu_col, top_row),
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

        let (icon_glyph, icon_color) = if layout.nerd_icons {
            item.nerd_icon()
        } else {
            ("", "")
        };
        let icon_str = if icon_glyph.is_empty() {
            String::new()
        } else {
            format!("{icon_glyph} ")
        };
        let icon_len = icon_str.chars().count();

        // Visible inner layout: " " (1) + marker (2) + icon (icon_len) + name + pad + badge + " " (1)
        // Overhead without name: 1 + 2 + icon_len + 1 (min space before badge) + badge_len + 1 = 5 + icon_len + badge_len
        let overhead = 5 + icon_len + badge.chars().count();
        let max_name_len = inner_space.saturating_sub(overhead);

        let name_chars: Vec<char> = name.chars().collect();
        let should_truncate = name_chars.len() > max_name_len && max_name_len > 1;
        let visible_chars: &[char] = if should_truncate {
            &name_chars[..max_name_len - 1]
        } else {
            &name_chars[..]
        };
        let visible_name_len = visible_chars.len() + if should_truncate { 1 } else { 0 };

        let min_required = 1 + 2 + icon_len + visible_name_len + 1 + badge.chars().count() + 1;
        let remaining_for_desc = inner_space.saturating_sub(min_required);

        let (desc_str, desc_len) = if let Some(desc) = &item.description {
            if remaining_for_desc >= 6 {
                let avail = remaining_for_desc - 2;
                let desc_chars: Vec<char> = desc.chars().collect();
                if desc_chars.len() > avail {
                    let s: String = desc_chars[..avail.saturating_sub(1)].iter().collect();
                    (format!("  {s}…"), avail + 2)
                } else {
                    (format!("  {desc}"), desc_chars.len() + 2)
                }
            } else {
                (String::new(), 0)
            }
        } else {
            (String::new(), 0)
        };

        let used_len = 1 + 2 + icon_len + visible_name_len + desc_len + badge.chars().count() + 1;
        let pad_len = inner_space.saturating_sub(used_len);
        let pad = " ".repeat(pad_len);

        let mut name_formatted = String::new();
        if is_selected {
            for (i, &c) in visible_chars.iter().enumerate() {
                if item.match_indices.contains(&i) {
                    name_formatted.push_str("\x1b[1;4;33m");
                    name_formatted.push(c);
                    name_formatted.push_str("\x1b[0m\x1b[7m");
                } else {
                    name_formatted.push(c);
                }
            }
            if should_truncate {
                name_formatted.push('…');
            }
        } else if item.match_indices.is_empty() {
            for &c in visible_chars {
                name_formatted.push(c);
            }
            if should_truncate {
                name_formatted.push('…');
            }
        } else {
            let dim_grey = "\x1b[38;5;244m";
            let mut in_dim = false;

            for (i, &c) in visible_chars.iter().enumerate() {
                if item.match_indices.contains(&i) {
                    if in_dim {
                        name_formatted.push_str("\x1b[0m");
                        in_dim = false;
                    }
                    name_formatted.push_str("\x1b[1;36m");
                    name_formatted.push(c);
                    name_formatted.push_str("\x1b[0m");
                } else {
                    if !in_dim {
                        name_formatted.push_str(dim_grey);
                        in_dim = true;
                    }
                    name_formatted.push(c);
                }
            }
            if in_dim {
                name_formatted.push_str("\x1b[0m");
            }
            if should_truncate {
                name_formatted.push_str("\x1b[38;5;244m…\x1b[0m");
            }
        }

        queue!(
            writer,
            cursor::MoveTo(layout.menu_col, item_start_row + r as u16)
        )?;

        if is_selected {
            queue!(
                writer,
                style::Print(format!(
                    "\x1b[38;5;240m│\x1b[0m\x1b[7m {marker}{icon_str}{name_formatted}{desc_str}{pad}{badge} \x1b[0m\x1b[38;5;240m│\x1b[0m"
                ))
            )?;
        } else {
            let badge_colored = format!("{}{badge}\x1b[0m", item.kind.color());
            let icon_colored = if icon_str.is_empty() {
                String::new()
            } else {
                format!("{icon_color}{icon_str}\x1b[0m")
            };
            let desc_colored = if desc_str.is_empty() {
                String::new()
            } else {
                format!("\x1b[38;5;244m{desc_str}\x1b[0m")
            };
            queue!(
                writer,
                style::Print(format!(
                    "\x1b[38;5;240m│\x1b[0m {marker}{icon_colored}{name_formatted}{desc_colored}{pad}{badge_colored} \x1b[38;5;240m│\x1b[0m"
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
        cursor::MoveTo(layout.menu_col, bottom_row),
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
    for row in (cursor_row + 1)..=(cursor_row + needed_rows) {
        queue!(
            writer,
            cursor::MoveTo(0, row),
            terminal::Clear(terminal::ClearType::CurrentLine),
        )?;
    }
    queue!(writer, cursor::MoveTo(cursor_col, cursor_row), cursor::Show,)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::calculate_menu_dimensions;
    use crate::menu::types::{CompletionItem, CompletionKind, MenuLayout};

    #[test]
    fn draw_menu_renders_strictly_below_cursor_row() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        let cursor_row = 10;
        let layout = MenuLayout {
            width: 30,
            height: 2,
            menu_col: 0,
            cursor_row,
            cursor_col: 5,
            nerd_icons: false,
        };

        let mut buf = Vec::new();
        draw_menu(&mut buf, &items, 0, layout).unwrap();
        let rendered = String::from_utf8_lossy(&buf);

        // Top border is at cursor_row + 1 = 11 (VT100 12)
        // Items are at cursor_row + 2 = 12 (VT100 13) and cursor_row + 3 = 13 (VT100 14)
        // Bottom border is at cursor_row + 4 = 14 (VT100 15)
        assert!(rendered.contains("\x1b[12;1H"));
        assert!(rendered.contains("\x1b[15;1H"));
        // Prompt line (row 10, VT100 11) is never drawn over
        assert!(!rendered.contains("\x1b[11;1H┌"));
    }

    #[test]
    fn erase_menu_never_erases_cursor_row_or_above() {
        let mut buf = Vec::new();
        let cursor_row = 10;
        let needed_rows = 4;
        erase_menu(&mut buf, needed_rows, cursor_row, 5).unwrap();
        let rendered = String::from_utf8_lossy(&buf);

        // Erases rows (cursor_row + 1)..=(cursor_row + needed_rows) = 11..=14
        // In VT100 1-based indexing: rows 12..=15
        assert!(rendered.contains("\x1b[12;1H\x1b[2K"));
        assert!(rendered.contains("\x1b[15;1H\x1b[2K"));
        // Does NOT erase row 10 (prompt row, VT100 11) or any row above
        assert!(!rendered.contains("\x1b[11;1H\x1b[2K"));
        assert!(!rendered.contains("\x1b[10;1H\x1b[2K"));
    }

    #[test]
    fn draw_and_erase_menu_output_proper_ansi_sequences() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        let mut buf = Vec::new();
        let layout = MenuLayout {
            width: 30,
            height: 2,
            menu_col: 0,
            cursor_row: 10,
            cursor_col: 5,
            nerd_icons: false,
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
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        let width = 30;
        let mut buf = Vec::new();
        let layout = MenuLayout {
            width,
            height: 2,
            menu_col: 0,
            cursor_row: 10,
            cursor_col: 5,
            nerd_icons: false,
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
    fn box_borders_and_rows_align_with_nerd_icons() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        let width = 30;
        let mut buf = Vec::new();
        let layout = MenuLayout {
            width,
            height: 2,
            menu_col: 0,
            cursor_row: 10,
            cursor_col: 5,
            nerd_icons: true,
        };
        draw_menu(&mut buf, &items, 0, layout).unwrap();
        let rendered = String::from_utf8_lossy(&buf);

        // Top and bottom borders must have exactly width (30 chars)
        assert!(rendered.contains("┌────────────────────────────┐"));
        assert!(rendered.contains("└────────────────────────────┘"));

        // Selected row contains the binary icon \u{f489} () followed by a space
        assert!(rendered.contains(" ▶ \u{f489} cargo"));
        // Unselected row contains the builtin icon \u{f0ad} ()
        assert!(rendered.contains("\u{f0ad} \x1b[0mcd"));
        assert!(rendered.contains("[builtin]\x1b[0m "));
    }

    #[test]
    fn box_borders_and_rows_align_with_descriptions() {
        let items = vec![
            CompletionItem::new("commit", "commit", CompletionKind::Subcommand)
                .with_description("Record changes to repository"),
            CompletionItem::new("-m", "-m", CompletionKind::Flag)
                .with_description("Commit message"),
            CompletionItem::new("main", "main", CompletionKind::Value)
                .with_description("Main branch"),
        ];

        let (width, height) = calculate_menu_dimensions(&items, 100, 7, true);
        let layout = MenuLayout {
            width,
            height,
            menu_col: 0,
            cursor_row: 10,
            cursor_col: 0,
            nerd_icons: true,
        };

        let mut buf = Vec::new();
        draw_menu(&mut buf, &items, 0, layout).unwrap();
        let rendered = String::from_utf8_lossy(&buf);

        let lines: Vec<&str> = rendered.split("\x1b[0m\x1b[").collect();
        assert!(!lines.is_empty());
        assert!(rendered.contains("Record changes to repository"));
        assert!(rendered.contains("Commit message"));
    }

    #[test]
    fn draw_menu_dims_non_matching_characters_in_grey_on_unselected_items() {
        let mut item1 = CompletionItem::new("cargo", "cargo", CompletionKind::Binary);
        item1.match_indices = vec![0, 1]; // "ca"
        let mut item2 = CompletionItem::new("cat", "cat", CompletionKind::Binary);
        item2.match_indices = vec![0, 1]; // "ca"

        let items = vec![item1, item2];
        let layout = MenuLayout {
            width: 30,
            height: 2,
            menu_col: 0,
            cursor_row: 10,
            cursor_col: 5,
            nerd_icons: false,
        };

        let mut buf = Vec::new();
        draw_menu(&mut buf, &items, 0, layout).unwrap();
        let rendered = String::from_utf8_lossy(&buf);

        // Selected item (item 0) has white inverse video (\x1b[0m\x1b[7m) and yellow match (\x1b[1;4;33m)
        assert!(rendered.contains("\x1b[38;5;240m│\x1b[0m\x1b[7m"));
        assert!(rendered.contains("\x1b[1;4;33m"));

        // Unselected item (item 1, cat) has bold cyan (\x1b[1;36m) for "ca" and dim grey (\x1b[38;5;244m) for "t"
        assert!(rendered.contains("\x1b[1;36mc\x1b[0m\x1b[1;36ma\x1b[0m\x1b[38;5;244mt\x1b[0m"));
    }
}
