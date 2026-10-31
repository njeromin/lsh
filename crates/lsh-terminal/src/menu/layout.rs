use super::types::CompletionItem;

pub const MIN_VISIBLE_ITEMS: usize = 3;

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
    nerd_icons: bool,
) -> (usize, usize) {
    let icon_pad = if nerd_icons { 2 } else { 0 };
    let max_item_len = items
        .iter()
        .map(|it| {
            let desc_len = it
                .description
                .as_ref()
                .map(|d| d.chars().count() + 2)
                .unwrap_or(0);
            it.display.chars().count() + icon_pad + it.kind.badge().chars().count() + desc_len + 7
        })
        .max()
        .unwrap_or(20);

    let max_allowed = (term_cols as usize).saturating_sub(4).max(24);
    let width = max_item_len.clamp(26, max_allowed);
    let height = items.len().min(max_visible);

    (width, height)
}

pub fn calculate_max_visible_height(
    cursor_row: u16,
    term_rows: u16,
    max_visible: usize,
    total_items: usize,
) -> (usize, u16) {
    let min_items = MIN_VISIBLE_ITEMS.min(max_visible).min(total_items).max(1);
    let min_needed_rows = (min_items as u16) + 2;
    let space_below = term_rows.saturating_sub(cursor_row + 1);

    if space_below >= min_needed_rows {
        let max_items = (space_below - 2) as usize;
        (max_visible.min(max_items).max(min_items), 0)
    } else {
        let needed_scroll = (min_needed_rows - space_below).min(cursor_row);
        (min_items, needed_scroll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::types::{CompletionItem, CompletionKind};

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
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cat", "cat", CompletionKind::Binary),
        ];

        let (width, height) = calculate_menu_dimensions(&items, 80, 7, false);
        assert_eq!(height, 2);
        assert!(width >= 26);
        assert!(width <= 76);

        // With nerd icons, width accommodates the 2-char icon prefix
        let (width_icons, _) = calculate_menu_dimensions(&items, 80, 7, true);
        assert!(width_icons >= width);
    }

    #[test]
    fn calculate_max_visible_height_fits_space_below_without_scrolling() {
        // cursor at row 10 in 24-row terminal (space_below = 13 >= 5)
        assert_eq!(calculate_max_visible_height(10, 24, 7, 10), (7, 0));
        // cursor at row 16 in 24-row terminal (space_below = 7 >= 5)
        // max_items = 7 - 2 = 5
        assert_eq!(calculate_max_visible_height(16, 24, 7, 10), (5, 0));
        // cursor at row 17 in 24-row terminal (space_below = 6 >= 5)
        // max_items = 6 - 2 = 4
        assert_eq!(calculate_max_visible_height(17, 24, 7, 10), (4, 0));
        // cursor at row 18 in 24-row terminal (space_below = 5 >= 5)
        // max_items = 5 - 2 = 3
        assert_eq!(calculate_max_visible_height(18, 24, 7, 10), (3, 0));
    }

    #[test]
    fn calculate_max_visible_height_ensures_at_least_three_items_at_terminal_edge() {
        // cursor at row 19 in 24-row terminal (space_below = 4 < 5)
        assert_eq!(calculate_max_visible_height(19, 24, 7, 10), (3, 1));
        // cursor at row 20 in 24-row terminal (space_below = 3 < 5)
        assert_eq!(calculate_max_visible_height(20, 24, 7, 10), (3, 2));
        // cursor at row 21 in 24-row terminal (space_below = 2 < 5)
        assert_eq!(calculate_max_visible_height(21, 24, 7, 10), (3, 3));
        // cursor at row 22 in 24-row terminal (space_below = 1 < 5)
        assert_eq!(calculate_max_visible_height(22, 24, 7, 10), (3, 4));
        // cursor at row 23 (bottom row) in 24-row terminal (space_below = 0 < 5)
        assert_eq!(calculate_max_visible_height(23, 24, 7, 10), (3, 5));
    }

    #[test]
    fn calculate_max_visible_height_adapts_to_fewer_total_items() {
        // Only 2 items in total: min_items = 2, min_needed_rows = 4
        // At bottom row 23 (space_below = 0), only scrolls 4 rows for 2 items
        assert_eq!(calculate_max_visible_height(23, 24, 7, 2), (2, 4));
        // With space_below = 4 (row 19), fits 2 items with 0 scroll
        assert_eq!(calculate_max_visible_height(19, 24, 7, 2), (2, 0));
    }
}
