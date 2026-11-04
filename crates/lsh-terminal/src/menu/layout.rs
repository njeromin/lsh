use lsh_completions::CompletionItem;

/// Calculates the visible viewport `(start_idx, end_idx)` when scrolling through completion items.
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

/// Computes the optimal (width, height) for the dropdown menu based on contents and terminal width.
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

#[cfg(test)]
mod tests {
    use super::*;
    use lsh_completions::CompletionKind;

    #[test]
    fn viewport_within_bounds_when_items_fit() {
        assert_eq!(calculate_viewport(5, 0, 7), (0, 5));
        assert_eq!(calculate_viewport(5, 4, 7), (0, 5));
    }

    #[test]
    fn viewport_scrolls_centered_when_items_exceed_limit() {
        assert_eq!(calculate_viewport(20, 0, 7), (0, 7));
        assert_eq!(calculate_viewport(20, 3, 7), (0, 7));
        assert_eq!(calculate_viewport(20, 4, 7), (1, 8));
        assert_eq!(calculate_viewport(20, 10, 7), (7, 14));
        assert_eq!(calculate_viewport(20, 18, 7), (13, 20));
        assert_eq!(calculate_viewport(20, 19, 7), (13, 20));
    }

    #[test]
    fn menu_dimensions_scale_to_content_and_terminal() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        let (width, height) = calculate_menu_dimensions(&items, 80, 7, false);
        assert_eq!(height, 2);
        assert!((26..=80).contains(&width));
    }
}
