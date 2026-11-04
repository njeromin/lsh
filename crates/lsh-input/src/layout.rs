use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::buffer::LineBuffer;

/// Screen layout coordinates for the cursor and the rendered text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenLayout {
    /// Visual row of the cursor relative to the first row of the prompt (0-indexed).
    pub cursor_row: usize,
    /// Visual column of the cursor (0-indexed).
    pub cursor_col: usize,
    /// Visual row where rendered text ends (relative to the first row of the prompt).
    pub end_row: usize,
    /// Visual column where rendered text ends.
    pub end_col: usize,
}

fn resolve_cursor_wrap(
    (col, row): (usize, usize),
    is_at_buffer_end: bool,
    term_cols: usize,
) -> (usize, usize) {
    if col >= term_cols {
        if is_at_buffer_end {
            (term_cols.saturating_sub(1), row)
        } else {
            (0, row + 1)
        }
    } else {
        (col, row)
    }
}

/// Computes the visual screen layout (soft-wrapped rows and cursor coordinates)
/// given the prompt, multiline indicator, line buffer, optional ghost text hint, and terminal column width.
pub fn compute_screen_layout(
    prompt_stripped: &str,
    indicator_stripped: &str,
    line_buffer: &LineBuffer,
    ghost_text: Option<&str>,
    term_cols: usize,
) -> ScreenLayout {
    let term_cols = term_cols.max(1);
    let mut current_row: usize = 0;
    let mut current_col: usize = 0;
    let mut cursor_screen: Option<(usize, usize)> = None;

    let target_cursor_byte = line_buffer.cursor();
    let mut current_byte_offset: usize = 0;

    let lines: Vec<&str> = line_buffer.as_str().split('\n').collect();

    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx > 0 {
            current_row += 1;
            current_col = 0;
        }

        let prefix = if line_idx == 0 {
            prompt_stripped
        } else {
            indicator_stripped
        };

        // Advance through prefix graphemes, properly tracking newlines in multiline prompts
        for grapheme in prefix.graphemes(true) {
            if grapheme == "\n" || grapheme == "\r\n" {
                current_row += 1;
                current_col = 0;
            } else if grapheme == "\r" {
                current_col = 0;
            } else {
                let w = UnicodeWidthStr::width(grapheme);
                if current_col >= term_cols {
                    current_row += 1;
                    current_col = 0;
                }
                if current_col > 0 && current_col + w > term_cols {
                    current_row += 1;
                    current_col = 0;
                }
                current_col += w;
            }
        }

        // Check if cursor sits at the beginning of this line's content
        if cursor_screen.is_none() && current_byte_offset == target_cursor_byte {
            let is_at_end = current_byte_offset == line_buffer.len();
            cursor_screen = Some(resolve_cursor_wrap(
                (current_col, current_row),
                is_at_end,
                term_cols,
            ));
        }

        // Advance through line content graphemes
        for grapheme in line.graphemes(true) {
            let w = UnicodeWidthStr::width(grapheme);
            if current_col >= term_cols {
                current_row += 1;
                current_col = 0;
            }
            if current_col > 0 && current_col + w > term_cols {
                current_row += 1;
                current_col = 0;
            }
            current_col += w;
            current_byte_offset += grapheme.len();

            if cursor_screen.is_none() && current_byte_offset == target_cursor_byte {
                let is_at_end = current_byte_offset == line_buffer.len();
                cursor_screen = Some(resolve_cursor_wrap(
                    (current_col, current_row),
                    is_at_end,
                    term_cols,
                ));
            }
        }

        // Account for '\n' byte between lines
        if line_idx + 1 < lines.len() {
            current_byte_offset += 1; // for '\n'
        }
    }

    // If ghost text is active, advance through its graphemes to calculate the true visual end position
    if let Some(hint) = ghost_text {
        for grapheme in hint.graphemes(true) {
            if grapheme == "\n" || grapheme == "\r\n" {
                current_row += 1;
                current_col = 0;
            } else if grapheme == "\r" {
                current_col = 0;
            } else {
                let w = UnicodeWidthStr::width(grapheme);
                if current_col >= term_cols {
                    current_row += 1;
                    current_col = 0;
                }
                if current_col > 0 && current_col + w > term_cols {
                    current_row += 1;
                    current_col = 0;
                }
                current_col += w;
            }
        }
    }

    let end_col = if current_col >= term_cols {
        term_cols.saturating_sub(1)
    } else {
        current_col
    };
    let end_row = current_row;

    let (cursor_col, cursor_row) = cursor_screen.unwrap_or((end_col, end_row));

    ScreenLayout {
        cursor_row,
        cursor_col,
        end_row,
        end_col,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::LineBuffer;

    #[test]
    fn test_compute_screen_layout_soft_wrap() {
        // Terminal of width 10 columns
        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("12345678"); // 8 chars. Prompt is "> " (2 chars). Total = 10 chars.

        // At end of row (10 chars in 10-col terminal)
        let layout = compute_screen_layout("> ", "… ", &line_buf, None, 10);
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 9); // sits at right margin
        assert_eq!(layout.end_row, 0);
        assert_eq!(layout.end_col, 9);

        // Type 1 more character -> wraps to row 1, col 1
        line_buf.insert_char('9');
        let layout = compute_screen_layout("> ", "… ", &line_buf, None, 10);
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.cursor_col, 1);
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 1);

        // Move cursor back to beginning: cursor sits on row 0, col 2 (right after prompt)
        line_buf.move_home();
        let layout = compute_screen_layout("> ", "… ", &line_buf, None, 10);
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 2);
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 1);
    }

    #[test]
    fn test_compute_screen_layout_wide_characters() {
        // Terminal of width 10 columns
        let mut line_buf = LineBuffer::new();
        // Prompt is "> " (2 cols)
        // Insert 7 single-width chars -> col reaches 9 (1 col left)
        line_buf.insert_str("1234567");

        let layout = compute_screen_layout("> ", "… ", &line_buf, None, 10);
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 9);

        // Insert double-width emoji 🦀 (width 2). It cannot fit in col 9, so it wraps to row 1!
        line_buf.insert_str("🦀");
        let layout = compute_screen_layout("> ", "… ", &line_buf, None, 10);
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.cursor_col, 2); // on row 1, occupies cols 0 and 1, cursor at 2
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 2);
    }

    #[test]
    fn test_ghost_text_wrapping() {
        // Terminal width 10 columns
        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("git c"); // Prompt "> " (2) + "git c" (5) = 7 cols on row 0

        // Hint "heckout" (7 cols) will overflow row 0 (which has 3 cols left) and wrap onto row 1!
        let layout = compute_screen_layout("> ", "… ", &line_buf, Some("heckout"), 10);
        // Cursor stays at end of typed text: col 7, row 0
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 7);
        // Ghost text continues and wraps to row 1!
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 4); // "kout" (4 cols on row 1)
    }

    #[test]
    fn test_compute_screen_layout_multiline_prompt() {
        let prompt = "\n~/dev/lsh (main)\n❯ ";
        let mut line_buf = LineBuffer::new();

        let layout = compute_screen_layout(prompt, "… ", &line_buf, None, 80);
        // Prompt has:
        // row 0: "" (from leading newline)
        // row 1: "~/dev/lsh (main)" (length 16)
        // row 2: "❯ " (length 2)
        // Therefore, the cursor must sit at row 2, col 2!
        assert_eq!(layout.cursor_row, 2);
        assert_eq!(layout.cursor_col, 2);
        assert_eq!(layout.end_row, 2);
        assert_eq!(layout.end_col, 2);

        // Type "cargo build"
        line_buf.insert_str("cargo build");
        let layout = compute_screen_layout(prompt, "… ", &line_buf, None, 80);
        assert_eq!(layout.cursor_row, 2);
        assert_eq!(layout.cursor_col, 13); // 2 + 11 = 13
        assert_eq!(layout.end_row, 2);
        assert_eq!(layout.end_col, 13);
    }
}
