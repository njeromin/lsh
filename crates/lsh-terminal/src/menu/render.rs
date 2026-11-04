use lsh_completions::CompletionItem;

use super::layout::calculate_viewport;

/// Formats the visual box-bordered lines of the completion menu.
pub fn format_menu_lines(
    items: &[CompletionItem],
    selected: usize,
    width: usize,
    height: usize,
    nerd_icons: bool,
    left_pad: usize,
) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }

    let (start_idx, end_idx) = calculate_viewport(items.len(), selected, height);
    let indent = " ".repeat(left_pad);
    let mut lines = Vec::with_capacity((end_idx - start_idx) + 2);

    // Top border
    let border_title = if start_idx > 0 {
        format!("▲ {} more ", start_idx)
    } else {
        String::new()
    };
    let border_fill = width.saturating_sub(border_title.chars().count() + 2);
    lines.push(format!(
        "{indent}\x1b[38;5;240m┌{border_title}{}┐\x1b[0m",
        "─".repeat(border_fill)
    ));

    // Item rows
    let inner_space = width.saturating_sub(2);
    for (r, item) in items[start_idx..end_idx].iter().enumerate() {
        let item_idx = start_idx + r;
        let is_selected = item_idx == selected;
        let marker = if is_selected { "▶ " } else { "  " };

        let name = &item.display;
        let badge = item.kind.badge();

        let (icon_glyph, icon_color) = if nerd_icons {
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
        } else {
            for (i, &c) in visible_chars.iter().enumerate() {
                if item.match_indices.contains(&i) {
                    name_formatted.push_str("\x1b[1;36m");
                    name_formatted.push(c);
                    name_formatted.push_str("\x1b[0m");
                } else {
                    name_formatted.push_str("\x1b[38;5;244m");
                    name_formatted.push(c);
                    name_formatted.push_str("\x1b[0m");
                }
            }
            if should_truncate {
                name_formatted.push('…');
            }
        }

        let row_content = if is_selected {
            format!(
                "\x1b[7m {marker}{icon_str}{name_formatted}\x1b[0m\x1b[7m{desc_str}{pad}\x1b[2;7m{badge}\x1b[0m\x1b[7m \x1b[0m"
            )
        } else {
            let styled_icon = if !icon_str.is_empty() {
                format!("{icon_color}{icon_str}\x1b[0m")
            } else {
                String::new()
            };
            let styled_desc = if !desc_str.is_empty() {
                format!("\x1b[38;5;242m{desc_str}\x1b[0m")
            } else {
                String::new()
            };
            format!(
                " {marker}{styled_icon}{name_formatted}{styled_desc}{pad}\x1b[2m{badge}\x1b[0m "
            )
        };

        lines.push(format!(
            "{indent}\x1b[38;5;240m│\x1b[0m{row_content}\x1b[38;5;240m│\x1b[0m"
        ));
    }

    // Bottom border
    let remaining_after = items.len().saturating_sub(end_idx);
    let border_bottom = if remaining_after > 0 {
        format!("▼ {} more ", remaining_after)
    } else {
        String::new()
    };
    let border_fill = width.saturating_sub(border_bottom.chars().count() + 2);
    lines.push(format!(
        "{indent}\x1b[38;5;240m└{border_bottom}{}┘\x1b[0m",
        "─".repeat(border_fill)
    ));

    lines
}

pub use lsh_input::strip_ansi;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::calculate_menu_dimensions;
    use lsh_completions::{CompletionItem, CompletionKind};

    #[test]
    fn box_borders_and_rows_align_to_exact_width() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        let width = 30;
        let lines = format_menu_lines(&items, 0, width, 2, false, 0);
        let rendered = lines.join("\n");
        let clean = strip_ansi(&rendered);

        // Top and bottom borders must have exactly width (30 chars)
        assert!(clean.contains("┌────────────────────────────┐"));
        assert!(clean.contains("└────────────────────────────┘"));

        // Inner item content must align
        assert!(clean.contains("│ ▶ cargo              [bin] │"));
        assert!(clean.contains("│   cd             [builtin] │"));
    }

    #[test]
    fn box_borders_and_rows_align_with_nerd_icons() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        let width = 30;
        let lines = format_menu_lines(&items, 0, width, 2, true, 0);
        let rendered = lines.join("\n");
        let clean = strip_ansi(&rendered);

        assert!(clean.contains("┌────────────────────────────┐"));
        assert!(clean.contains("└────────────────────────────┘"));
        assert!(clean.contains("│ ▶ \u{f489} cargo"));
        assert!(clean.contains("│   \u{f0ad} cd"));
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
        let lines = format_menu_lines(&items, 0, width, height, true, 0);
        let rendered = lines.join("\n");

        assert!(rendered.contains("Record changes to repository"));
        assert!(rendered.contains("Commit message"));
    }

    #[test]
    fn format_menu_dims_non_matching_characters_in_grey_on_unselected_items() {
        let mut item1 = CompletionItem::new("cargo", "cargo", CompletionKind::Binary);
        item1.match_indices = vec![0, 1]; // "ca"
        let mut item2 = CompletionItem::new("cat", "cat", CompletionKind::Binary);
        item2.match_indices = vec![0, 1]; // "ca"

        let items = vec![item1, item2];
        let lines = format_menu_lines(&items, 0, 30, 2, false, 0);
        let rendered = lines.join("\n");

        assert!(rendered.contains("\x1b[38;5;240m│\x1b[0m\x1b[7m"));
        assert!(rendered.contains("\x1b[1;4;33m"));
        assert!(rendered.contains("\x1b[1;36mc\x1b[0m\x1b[1;36ma\x1b[0m\x1b[38;5;244mt\x1b[0m"));
    }

    #[test]
    fn format_menu_lines_returns_boxed_lines_with_indent() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        let lines = format_menu_lines(&items, 0, 30, 2, false, 4);
        assert_eq!(lines.len(), 4); // top border + 2 items + bottom border
        assert!(lines[0].starts_with("    "));
        assert!(strip_ansi(&lines[0]).ends_with("┌────────────────────────────┐"));
        assert!(strip_ansi(&lines[1]).contains("▶ cargo"));
        assert!(strip_ansi(&lines[2]).contains("  cd"));
        assert!(strip_ansi(&lines[3]).ends_with("└────────────────────────────┘"));
    }

    #[test]
    fn strip_ansi_removes_escape_codes() {
        let input = "\x1b[1;36mhello\x1b[0m \x1b[38;5;240mworld\x1b[0m";
        assert_eq!(strip_ansi(input), "hello world");
    }
}
