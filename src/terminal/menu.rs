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
    pub match_indices: Vec<usize>,
}

impl CompletionItem {
    pub fn new(
        replacement: impl Into<String>,
        display: impl Into<String>,
        kind: CompletionKind,
    ) -> Self {
        Self {
            replacement: replacement.into(),
            display: display.into(),
            kind,
            match_indices: Vec::new(),
        }
    }

    pub fn nerd_icon(&self) -> (&'static str, &'static str) {
        match self.kind {
            CompletionKind::Directory => ("\u{f115}", "\x1b[1;34m"), //  Bold Blue
            CompletionKind::Binary => ("\u{f489}", "\x1b[32m"),      //  Green
            CompletionKind::Builtin => ("\u{f0ad}", "\x1b[35m"),     //  Magenta
            CompletionKind::Alias => ("\u{f0c1}", "\x1b[36m"),       //  Cyan
            CompletionKind::Function => ("\u{f0295}", "\x1b[34m"),   // 󰊕 Blue
            CompletionKind::Variable => ("\u{eb5d}", "\x1b[33m"),    //  Yellow
            CompletionKind::File => Self::file_nerd_icon(&self.display),
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == CompletionKind::Directory
            || self.replacement.ends_with('/')
            || self.display.ends_with('/')
    }

    fn file_nerd_icon(filename: &str) -> (&'static str, &'static str) {
        let name_lower = filename.to_lowercase();
        // Exact names first
        if name_lower == "cargo.toml" || name_lower == "cargo.lock" {
            return ("\u{e7a8}", "\x1b[38;5;208m"); //  Rust orange
        }
        if name_lower.starts_with(".git") {
            return ("\u{e702}", "\x1b[31m"); //  Git red
        }
        if name_lower == "license" || name_lower.starts_with("license.") {
            return ("\u{e60a}", "\x1b[33m"); //  Yellow
        }

        // By extension
        if let Some(ext) = name_lower.rsplit('.').next() {
            match ext {
                "rs" => ("\u{e7a8}", "\x1b[38;5;208m"), //  Rust orange
                "toml" | "yaml" | "yml" | "json" | "ini" | "conf" => ("\u{e615}", "\x1b[33m"), //  Config yellow
                "md" | "markdown" => ("\u{f48a}", "\x1b[36m"), //  Markdown cyan
                "sh" | "bash" | "zsh" | "lsh" => ("\u{f489}", "\x1b[32m"), //  Terminal green
                "lock" => ("\u{f023}", "\x1b[38;5;245m"), //  Lock gray
                "py" => ("\u{e606}", "\x1b[34m"), //  Python blue
                "js" | "mjs" | "cjs" => ("\u{e60c}", "\x1b[33m"), //  JS yellow
                "ts" => ("\u{e628}", "\x1b[34m"), //  TS blue
                "html" | "css" | "scss" => ("\u{e60e}", "\x1b[35m"), //  Web purple
                "c" | "h" | "cpp" | "hpp" | "cc" => ("\u{e61d}", "\x1b[34m"), //  C/C++ blue
                "go" => ("\u{e627}", "\x1b[36m"), //  Go cyan
                "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" => ("\u{f1c5}", "\x1b[35m"), //  Image purple
                "mp3" | "wav" | "ogg" | "mp4" | "mkv" | "avi" => ("\u{f1c7}", "\x1b[35m"), //  Media purple
                "zip" | "tar" | "gz" | "xz" | "7z" | "bz2" => ("\u{f1c6}", "\x1b[31m"), //  Archive red
                "txt" | "log" | "csv" => ("\u{f15c}", "\x1b[37m"), //  Text white
                _ => ("\u{f15b}", "\x1b[37m"), //  File white
            }
        } else {
            ("\u{f15b}", "\x1b[37m") //  File white
        }
    }
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
    nerd_icons: bool,
) -> (usize, usize) {
    let icon_pad = if nerd_icons { 2 } else { 0 };
    let max_item_len = items
        .iter()
        .map(|it| it.display.chars().count() + icon_pad + it.kind.badge().chars().count() + 6)
        .max()
        .unwrap_or(20);

    let max_allowed = (term_cols as usize).saturating_sub(4).max(24);
    let width = max_item_len.clamp(26, max_allowed);
    let height = items.len().min(max_visible);

    (width, height)
}

pub const MIN_VISIBLE_ITEMS: usize = 3;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuLayout {
    pub width: usize,
    pub height: usize,
    pub menu_col: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub nerd_icons: bool,
}

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

        let used_len = 1 + 2 + icon_len + visible_name_len + badge.chars().count() + 1;
        let pad_len = inner_space.saturating_sub(used_len);
        let pad = " ".repeat(pad_len);

        let mut name_formatted = String::new();
        for (i, &c) in visible_chars.iter().enumerate() {
            if item.match_indices.contains(&i) {
                if is_selected {
                    name_formatted.push_str("\x1b[1;4;33m");
                    name_formatted.push(c);
                    name_formatted.push_str("\x1b[0m\x1b[7m");
                } else {
                    name_formatted.push_str("\x1b[1;36m");
                    name_formatted.push(c);
                    name_formatted.push_str("\x1b[0m");
                }
            } else {
                name_formatted.push(c);
            }
        }
        if should_truncate {
            name_formatted.push('…');
        }

        queue!(writer, cursor::MoveTo(layout.menu_col, item_start_row + r as u16))?;

        if is_selected {
            queue!(
                writer,
                style::Print(format!(
                    "\x1b[38;5;240m│\x1b[7m {marker}{icon_str}{name_formatted}{pad}{badge} \x1b[0m\x1b[38;5;240m│\x1b[0m"
                ))
            )?;
        } else {
            let badge_colored = format!("{}{badge}\x1b[0m", item.kind.color());
            let icon_colored = if icon_str.is_empty() {
                String::new()
            } else {
                format!("{icon_color}{icon_str}\x1b[0m")
            };
            queue!(
                writer,
                style::Print(format!(
                    "\x1b[38;5;240m│\x1b[0m {marker}{icon_colored}{name_formatted}{pad}{badge_colored} \x1b[38;5;240m│\x1b[0m"
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
    Tab,
    Slash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatch {
    pub score: i64,
    pub indices: Vec<usize>,
}

fn is_word_boundary(cand: &[char], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    let prev = cand[idx - 1];
    prev == '_'
        || prev == '-'
        || prev == '.'
        || prev == '/'
        || prev == '\\'
        || prev == ' '
        || prev == ':'
        || (!prev.is_uppercase() && cand[idx].is_uppercase())
}

pub fn fuzzy_match(candidate: &str, query: &str) -> Option<FuzzyMatch> {
    if query.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            indices: Vec::new(),
        });
    }

    let cand_chars: Vec<char> = candidate.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();

    if query_chars.len() > cand_chars.len() {
        return None;
    }

    let mut indices: Vec<usize> = Vec::with_capacity(query_chars.len());
    let mut search_from = 0;

    for &qc in &query_chars {
        let mut best_pos = None;

        for pos in search_from..cand_chars.len() {
            if cand_chars[pos].to_lowercase().eq(qc.to_lowercase()) {
                // Consecutive match to previous index takes highest priority
                if indices.last().is_some_and(|&prev| pos == prev + 1) {
                    best_pos = Some(pos);
                    break;
                }
                // Word boundary match takes priority over non-boundary match
                let is_boundary = is_word_boundary(&cand_chars, pos);
                if is_boundary {
                    best_pos = Some(pos);
                    break;
                } else if best_pos.is_none() {
                    best_pos = Some(pos);
                }
            }
        }

        match best_pos {
            Some(pos) => {
                indices.push(pos);
                search_from = pos + 1;
            }
            None => return None,
        }
    }

    let mut score: i64 = 0;
    let mut prev_idx: Option<usize> = None;

    for (q_idx, &c_idx) in indices.iter().enumerate() {
        score += 10;

        if c_idx == 0 {
            score += 35;
        } else if is_word_boundary(&cand_chars, c_idx) {
            score += 25;
        }

        if let Some(prev) = prev_idx {
            if c_idx == prev + 1 {
                score += 15;
            } else {
                let gap = c_idx - prev - 1;
                score -= (gap as i64).min(10);
            }
        }

        if cand_chars[c_idx] == query_chars[q_idx] {
            score += 5;
        }

        prev_idx = Some(c_idx);
    }

    score -= (cand_chars.len() as i64).min(20);

    Some(FuzzyMatch { score, indices })
}

pub fn filter_completion_items(items: &[CompletionItem], query: &str) -> Vec<CompletionItem> {
    if query.is_empty() {
        return items
            .iter()
            .map(|it| {
                let mut c = it.clone();
                c.match_indices.clear();
                c
            })
            .collect();
    }

    let mut scored: Vec<(CompletionItem, i64)> = items
        .iter()
        .filter_map(|it| {
            if let Some(m) = fuzzy_match(&it.display, query) {
                let mut item = it.clone();
                item.match_indices = m.indices;
                Some((item, m.score))
            } else if let Some(m) = fuzzy_match(&it.replacement, query) {
                let mut item = it.clone();
                item.match_indices = Vec::new();
                Some((item, m.score))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by_key(|b| std::cmp::Reverse(b.1));

    scored.into_iter().map(|(item, _)| item).collect()
}

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
        let mut root_items = items.to_vec();
        let mut active_items = items.to_vec();
        let mut selected = 0;
        let mut cur_col = cursor_col;

        struct DescentLevel {
            prefix: String,
            items: Vec<CompletionItem>,
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
                })) if kind != KeyEventKind::Release => {
                    match handle_key_event(code, modifiers) {
                        MenuAction::Next => {
                            if !active_items.is_empty() {
                                selected = (selected + 1) % active_items.len();
                                let (l, _) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
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
                                let (l, _) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::PageDown => {
                            if !active_items.is_empty() {
                                let (l, _) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                                selected = (selected + l.height).min(active_items.len() - 1);
                                let (l, _) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::PageUp => {
                            if !active_items.is_empty() {
                                let (l, _) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                                selected = selected.saturating_sub(l.height);
                                let (l, _) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::First => {
                            if !active_items.is_empty() {
                                selected = 0;
                                let (l, _) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::Last => {
                            if !active_items.is_empty() {
                                selected = active_items.len() - 1;
                                let (l, _) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            }
                        }
                        MenuAction::Accept => {
                            break if !active_items.is_empty() {
                                MenuResult::Selected(active_items[selected].clone())
                            } else {
                                MenuResult::Typed(format!("{current_prefix}{filter_suffix}"))
                            };
                        }
                        MenuAction::Tab | MenuAction::Slash
                            if !active_items.is_empty() && active_items[selected].is_dir() =>
                        {
                            let item = &active_items[selected];
                            let next_items = on_descend(&item.replacement);
                            if !next_items.is_empty() {
                                descent_stack.push(DescentLevel {
                                    prefix: current_prefix.clone(),
                                    items: root_items.clone(),
                                });

                                let _ = erase_menu(
                                    &mut cout,
                                    max_needed_rows,
                                    cursor_row,
                                    cur_col + filter_suffix.chars().count() as u16,
                                );

                                let old_len = current_prefix.chars().count() + filter_suffix.chars().count();
                                let new_prefix = item.replacement.clone();
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
                                root_items = next_items.clone();
                                active_items = next_items;
                                selected = 0;

                                let (l, needed) = make_layout(&active_items, cur_col, 0);
                                max_needed_rows = max_needed_rows.max(needed);
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            } else {
                                break MenuResult::Selected(item.clone());
                            }
                        }
                        MenuAction::Tab => {
                            break if !active_items.is_empty() {
                                MenuResult::Selected(active_items[selected].clone())
                            } else {
                                MenuResult::Typed(format!("{current_prefix}{filter_suffix}"))
                            };
                        }
                        MenuAction::Slash => {
                            filter_suffix.push('/');
                            let new_items = filter_completion_items(&root_items, &filter_suffix);

                            if new_items.is_empty() {
                                break MenuResult::Typed(format!("{current_prefix}{filter_suffix}"));
                            }

                            let char_col = cur_col + (filter_suffix.chars().count() - 1) as u16;
                            let _ = queue!(cout, cursor::MoveTo(char_col, cursor_row), style::Print('/'));

                            let _ = erase_menu(
                                &mut cout,
                                max_needed_rows,
                                cursor_row,
                                cur_col + filter_suffix.chars().count() as u16,
                            );

                            active_items = new_items;
                            selected = selected.min(active_items.len() - 1);

                            let (l, needed) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            max_needed_rows = max_needed_rows.max(needed);
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                        MenuAction::Dismiss => {
                            break MenuResult::Cancelled;
                        }
                        MenuAction::Char(c) => {
                            filter_suffix.push(c);
                            let new_items = filter_completion_items(&root_items, &filter_suffix);

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

                            let (l, needed) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                            max_needed_rows = max_needed_rows.max(needed);
                            let _ = draw_menu(&mut cout, &active_items, selected, l);
                        }
                        MenuAction::Backspace => {
                            if !filter_suffix.is_empty() {
                                let char_col =
                                    cur_col + filter_suffix.chars().count() as u16 - 1;
                                let _ = queue!(
                                    cout,
                                    cursor::MoveTo(char_col, cursor_row),
                                    style::Print(' '),
                                    cursor::MoveTo(char_col, cursor_row),
                                );

                                filter_suffix.pop();
                                active_items = filter_completion_items(&root_items, &filter_suffix);
                                selected = selected.min(active_items.len().saturating_sub(1));

                                let _ = erase_menu(
                                    &mut cout,
                                    max_needed_rows,
                                    cursor_row,
                                    cur_col + filter_suffix.chars().count() as u16,
                                );

                                let (l, needed) = make_layout(&active_items, cur_col, filter_suffix.chars().count());
                                max_needed_rows = max_needed_rows.max(needed);
                                let _ = draw_menu(&mut cout, &active_items, selected, l);
                            } else if let Some(parent) = descent_stack.pop() {
                                let _ = erase_menu(
                                    &mut cout,
                                    max_needed_rows,
                                    cursor_row,
                                    cur_col,
                                );

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
                    }
                }
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
    fn item_nerd_icons_match_kind_and_extensions() {
        let dir = CompletionItem::new("src", "src", CompletionKind::Directory);
        assert_eq!(dir.nerd_icon().0, "\u{f115}"); // 

        let bin = CompletionItem::new("git", "git", CompletionKind::Binary);
        assert_eq!(bin.nerd_icon().0, "\u{f489}"); // 

        let rust_file = CompletionItem::new("main.rs", "main.rs", CompletionKind::File);
        assert_eq!(rust_file.nerd_icon().0, "\u{e7a8}"); // 

        let toml_file = CompletionItem::new("Cargo.toml", "Cargo.toml", CompletionKind::File);
        assert_eq!(toml_file.nerd_icon().0, "\u{e7a8}"); // 

        let md_file = CompletionItem::new("README.md", "README.md", CompletionKind::File);
        assert_eq!(md_file.nerd_icon().0, "\u{f48a}"); // 

        let plain_file = CompletionItem::new("data.bin", "data.bin", CompletionKind::File);
        assert_eq!(plain_file.nerd_icon().0, "\u{f15b}"); // 
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
    fn completion_item_is_dir_identifies_directories() {
        let dir_item = CompletionItem::new("src", "src", CompletionKind::Directory);
        assert!(dir_item.is_dir());

        let slash_item = CompletionItem::new("scripts/", "scripts/", CompletionKind::File);
        assert!(slash_item.is_dir());

        let file_item = CompletionItem::new("main.rs", "main.rs", CompletionKind::File);
        assert!(!file_item.is_dir());

        let bin_item = CompletionItem::new("cargo", "cargo", CompletionKind::Binary);
        assert!(!bin_item.is_dir());
    }

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

    #[test]
    fn filter_completion_items_narrows_candidates_dynamically() {
        let items = vec![
            CompletionItem::new("cargo", "cargo", CompletionKind::Binary),
            CompletionItem::new("cat", "cat", CompletionKind::Binary),
            CompletionItem::new("cd", "cd", CompletionKind::Builtin),
        ];

        // Empty query keeps all items
        assert_eq!(filter_completion_items(&items, "").len(), 3);

        // "ca" keeps cargo and cat (cat scores higher due to shorter length)
        let ca_matches = filter_completion_items(&items, "ca");
        assert_eq!(ca_matches.len(), 2);
        assert_eq!(ca_matches[0].replacement, "cat");
        assert_eq!(ca_matches[1].replacement, "cargo");

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

    #[test]
    fn fuzzy_match_detects_subsequences_and_word_boundaries() {
        // "ct" matches Cargo.toml ('C' at 0, 't' at 6)
        let ct_cargo = fuzzy_match("Cargo.toml", "ct").unwrap();
        assert_eq!(ct_cargo.indices, vec![0, 6]);

        // "ct" matches cat ('c' at 0, 't' at 2)
        let ct_cat = fuzzy_match("cat", "ct").unwrap();
        assert_eq!(ct_cat.indices, vec![0, 2]);

        // "dco" matches docker-compose ('d' at 0, 'c' at 7, 'o' at 8)
        let dco = fuzzy_match("docker-compose", "dco").unwrap();
        assert_eq!(dco.indices, vec![0, 7, 8]);

        // "mrs" matches menu.rs ('m' at 0, 'r' at 5, 's' at 6)
        let mrs = fuzzy_match("menu.rs", "mrs").unwrap();
        assert_eq!(mrs.indices, vec![0, 5, 6]);

        // Non-matching query
        assert!(fuzzy_match("cargo", "xyz").is_none());
    }

    #[test]
    fn fuzzy_match_ranks_exact_prefix_higher_than_subsequence() {
        let prefix_match = fuzzy_match("cargo", "car").unwrap();
        let subseq_match = fuzzy_match("clear", "car").unwrap();
        assert!(prefix_match.score > subseq_match.score);
    }

    #[test]
    fn filter_completion_items_supports_fuzzy_subsequences() {
        let items = vec![
            CompletionItem::new("Cargo.toml", "Cargo.toml", CompletionKind::File),
            CompletionItem::new("cat", "cat", CompletionKind::Binary),
            CompletionItem::new("docker-compose", "docker-compose", CompletionKind::Binary),
        ];

        // "ct" matches both Cargo.toml and cat
        let ct_matches = filter_completion_items(&items, "ct");
        assert_eq!(ct_matches.len(), 2);
        assert!(ct_matches[0].match_indices.len() == 2);

        // "dco" matches docker-compose
        let dco_matches = filter_completion_items(&items, "dco");
        assert_eq!(dco_matches.len(), 1);
        assert_eq!(dco_matches[0].display, "docker-compose");
        assert_eq!(dco_matches[0].match_indices, vec![0, 7, 8]);
    }
}
