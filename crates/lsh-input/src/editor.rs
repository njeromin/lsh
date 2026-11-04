use std::io::{self, BufWriter, Stdout, Write};

use crate::buffer::LineBuffer;
use crate::clipboard::{Clipboard, default_clipboard};
use crate::completer::Completer;
use crate::edit_mode::{EditMode, ViMode, ViOperator};
use crate::formatter::{StyledText, TextFormatter};
use crate::hinter::{Hinter, default_next_hint_word};
use crate::history::History;
use crate::keymap::{EditorAction, Keymap};
pub use crate::layout::{ScreenLayout, compute_screen_layout};
use crate::menu::{Menu, MenuOutcome};
use crate::prompt::Prompt;
use crate::validator::{ValidationResult, Validator};
use crossterm::{
    cursor::{self, MoveToColumn},
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    queue,
    style::{ContentStyle, Print, Stylize},
    terminal::{self, Clear, ClearType},
};

/// Normalises pasted text by converting CRLF and CR line endings to standard LF (`\n`),
/// preserving valid whitespace (`\n`, `\t`) and printable Unicode characters,
/// and stripping non-printable control characters to prevent escape sequence injection.
pub fn normalise_paste(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                result.push('\n');
            }
            '\n' | '\t' => result.push(c),
            c if c.is_control() => {} // Strip control characters / escape injections
            c => result.push(c),
        }
    }

    result
}

/// RAII guard to ensure terminal raw mode is disabled when exiting `read_line`,
/// even in the case of errors or panics.
struct RawModeGuard {
    enhanced_keyboard: bool,
}

impl RawModeGuard {
    fn new<W: Write>(writer: &mut W) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let _ = queue!(writer, EnableBracketedPaste);
        let enhanced_keyboard = matches!(terminal::supports_keyboard_enhancement(), Ok(true));
        if enhanced_keyboard {
            let _ = queue!(
                writer,
                event::PushKeyboardEnhancementFlags(
                    event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                )
            );
        }
        let _ = writer.flush();
        Ok(Self { enhanced_keyboard })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        let _ = queue!(stdout, cursor::SetCursorStyle::DefaultUserShape);
        let _ = queue!(stdout, DisableBracketedPaste);
        if self.enhanced_keyboard {
            let _ = queue!(stdout, event::PopKeyboardEnhancementFlags);
        }
        let _ = stdout.flush();
        let _ = terminal::disable_raw_mode();
    }
}

pub struct Editor<W: Write = BufWriter<Stdout>> {
    writer: W,
    keymap: Keymap,
    edit_mode: EditMode,
    vi_mode: ViMode,
    cursor_style: cursor::SetCursorStyle,
    formatter: Option<Box<dyn TextFormatter>>,
    validator: Option<Box<dyn Validator>>,
    hinter: Option<Box<dyn Hinter>>,
    hint_style: ContentStyle,
    completer: Option<Box<dyn Completer>>,
    menu: Option<Box<dyn Menu>>,
    history: Option<Box<dyn History>>,
    clipboard: Option<Box<dyn Clipboard>>,
    indent_size: usize,
    terminal_width: Option<usize>,
}

impl Editor<BufWriter<Stdout>> {
    /// Create a new editor writing to standard output wrapped in a `BufWriter` with default keybindings.
    pub fn new() -> Self {
        Self {
            writer: BufWriter::new(io::stdout()),
            keymap: Keymap::default(),
            edit_mode: EditMode::Emacs,
            vi_mode: ViMode::Insert,
            cursor_style: cursor::SetCursorStyle::BlinkingBar,
            formatter: None,
            validator: None,
            hinter: None,
            hint_style: ContentStyle::new().dark_grey(),
            completer: None,
            menu: None,
            history: None,
            clipboard: Some(default_clipboard()),
            indent_size: 4,
            terminal_width: None,
        }
    }
}

impl<W: Write> Editor<W> {
    /// Create an editor with a custom writer (e.g. `stderr`, `Vec<u8>`, or a custom buffer).
    pub fn new_with_writer(writer: W) -> Self {
        Self {
            writer,
            keymap: Keymap::default(),
            edit_mode: EditMode::Emacs,
            vi_mode: ViMode::Insert,
            cursor_style: cursor::SetCursorStyle::BlinkingBar,
            formatter: None,
            validator: None,
            hinter: None,
            hint_style: ContentStyle::new().dark_grey(),
            completer: None,
            menu: None,
            history: None,
            clipboard: Some(default_clipboard()),
            indent_size: 4,
            terminal_width: None,
        }
    }

    /// Alias for `new_with_writer`.
    #[deprecated(note = "Use `Editor::new_with_writer` instead")]
    pub fn with_writer(writer: W) -> Self {
        Self::new_with_writer(writer)
    }

    /// Set a custom hinter for providing inline ghost text suggestions.
    pub fn with_hinter(mut self, hinter: impl Hinter + 'static) -> Self {
        self.hinter = Some(Box::new(hinter));
        self
    }

    /// Dynamically set or clear the hinter.
    pub fn set_hinter(&mut self, hinter: Option<Box<dyn Hinter>>) {
        self.hinter = hinter;
    }

    /// Immutably borrow the active hinter, if any.
    pub fn hinter(&self) -> Option<&dyn Hinter> {
        self.hinter.as_deref()
    }

    /// Set the styling for inline ghost text hints (defaults to dark grey / dim).
    pub fn with_hint_style(mut self, style: ContentStyle) -> Self {
        self.hint_style = style;
        self
    }

    /// Get the current styling for inline ghost text hints.
    pub fn hint_style(&self) -> ContentStyle {
        self.hint_style
    }

    /// Set a custom completer for tab completion.
    pub fn with_completer(mut self, completer: impl Completer + 'static) -> Self {
        self.completer = Some(Box::new(completer));
        self
    }

    /// Dynamically set or clear the completer.
    pub fn set_completer(&mut self, completer: Option<Box<dyn Completer>>) {
        self.completer = completer;
    }

    /// Immutably borrow the active completer, if any.
    pub fn completer(&self) -> Option<&dyn Completer> {
        self.completer.as_deref()
    }

    /// Set a custom completion menu.
    pub fn with_menu(mut self, menu: impl Menu + 'static) -> Self {
        self.menu = Some(Box::new(menu));
        self
    }

    /// Dynamically set or clear the completion menu.
    pub fn set_menu(&mut self, menu: Option<Box<dyn Menu>>) {
        self.menu = menu;
    }

    /// Immutably borrow the active completion menu, if any.
    pub fn menu(&self) -> Option<&dyn Menu> {
        self.menu.as_deref()
    }

    /// Mutably borrow the active completion menu, if any.
    pub fn menu_mut(&mut self) -> Option<&mut (dyn Menu + 'static)> {
        self.menu.as_deref_mut()
    }

    /// Set a custom command history.
    pub fn with_history(mut self, history: impl History + 'static) -> Self {
        self.history = Some(Box::new(history));
        self
    }

    /// Dynamically set or clear the command history.
    pub fn set_history(&mut self, history: Option<Box<dyn History>>) {
        self.history = history;
    }

    /// Immutably borrow the active command history, if any.
    pub fn history(&self) -> Option<&dyn History> {
        self.history.as_deref()
    }

    /// Mutably borrow the active command history, if any.
    pub fn history_mut(&mut self) -> Option<&mut (dyn History + 'static)> {
        self.history.as_deref_mut()
    }

    /// Set a custom clipboard provider.
    pub fn with_clipboard(mut self, clipboard: impl Clipboard + 'static) -> Self {
        self.clipboard = Some(Box::new(clipboard));
        self
    }

    /// Dynamically set or clear the clipboard provider.
    pub fn set_clipboard(&mut self, clipboard: Option<Box<dyn Clipboard>>) {
        self.clipboard = clipboard;
    }

    /// Immutably borrow the active clipboard, if any.
    pub fn clipboard(&self) -> Option<&dyn Clipboard> {
        self.clipboard.as_deref()
    }

    /// Mutably borrow the active clipboard, if any.
    pub fn clipboard_mut(&mut self) -> Option<&mut (dyn Clipboard + 'static)> {
        self.clipboard.as_deref_mut()
    }

    /// Set the editor edit mode (Emacs or Vi).
    pub fn with_edit_mode(mut self, mode: EditMode) -> Self {
        self.edit_mode = mode;
        if mode == EditMode::Vi {
            self.keymap = Keymap::default_vi_insert();
            self.vi_mode = ViMode::Insert;
        } else {
            self.keymap = Keymap::default_emacs();
        }
        self
    }

    /// Dynamically update the editor edit mode (Emacs or Vi).
    pub fn set_edit_mode(&mut self, mode: EditMode) {
        self.edit_mode = mode;
        if mode == EditMode::Vi {
            self.keymap = Keymap::default_vi_insert();
            self.vi_mode = ViMode::Insert;
        } else {
            self.keymap = Keymap::default_emacs();
        }
    }

    /// Immutably borrow the active edit mode.
    pub fn edit_mode(&self) -> EditMode {
        self.edit_mode
    }

    /// Get the current Vi mode (Insert or Normal).
    pub fn vi_mode(&self) -> ViMode {
        self.vi_mode
    }

    /// Set the cursor style used for insert/editing mode (defaults to `cursor::SetCursorStyle::BlinkingBar`).
    pub fn with_cursor_style(mut self, style: cursor::SetCursorStyle) -> Self {
        self.cursor_style = style;
        self
    }

    /// Dynamically update the cursor style used for insert/editing mode.
    pub fn set_cursor_style(&mut self, style: cursor::SetCursorStyle) {
        self.cursor_style = style;
    }

    /// Get the current cursor style.
    pub fn cursor_style(&self) -> cursor::SetCursorStyle {
        self.cursor_style
    }

    /// Set an explicit terminal column width override (useful for testing or fixed-width rendering).
    pub fn with_terminal_width(mut self, width: usize) -> Self {
        self.terminal_width = Some(width);
        self
    }

    /// Dynamically set or clear the terminal width override.
    pub fn set_terminal_width(&mut self, width: Option<usize>) {
        self.terminal_width = width;
    }

    /// Returns the effective terminal column width (queries crossterm if not overridden).
    pub fn terminal_width(&self) -> usize {
        self.terminal_width
            .unwrap_or_else(|| terminal::size().map(|(w, _)| w as usize).unwrap_or(80))
            .max(1)
    }

    /// Set indentation size in spaces for auto-indent and smart dedent (defaults to 4).
    pub fn with_indent_size(mut self, size: usize) -> Self {
        self.indent_size = size;
        self
    }

    /// Get the current indentation size in spaces.
    pub fn indent_size(&self) -> usize {
        self.indent_size
    }

    /// Set the indentation size dynamically.
    pub fn set_indent_size(&mut self, size: usize) {
        self.indent_size = size;
    }

    /// Set a custom keymap.
    pub fn with_keymap(mut self, keymap: Keymap) -> Self {
        self.keymap = keymap;
        self
    }

    /// Set a custom text formatter.
    pub fn with_formatter(mut self, formatter: impl TextFormatter + 'static) -> Self {
        self.formatter = Some(Box::new(formatter));
        self
    }

    /// Dynamically set or clear the text formatter.
    pub fn set_formatter(&mut self, formatter: Option<Box<dyn TextFormatter>>) {
        self.formatter = formatter;
    }

    /// Immutably borrow the active formatter, if any.
    pub fn formatter(&self) -> Option<&dyn TextFormatter> {
        self.formatter.as_deref()
    }

    /// Set a custom syntax/multiline validator.
    pub fn with_validator(mut self, validator: impl Validator + 'static) -> Self {
        self.validator = Some(Box::new(validator));
        self
    }

    /// Dynamically set or clear the validator.
    pub fn set_validator(&mut self, validator: Option<Box<dyn Validator>>) {
        self.validator = validator;
    }

    /// Immutably borrow the active validator, if any.
    pub fn validator(&self) -> Option<&dyn Validator> {
        self.validator.as_deref()
    }

    /// Immutably borrow the active keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Mutably borrow the active keymap to edit bindings at runtime.
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }

    /// Dynamically update the active keymap.
    pub fn set_keymap(&mut self, keymap: Keymap) {
        self.keymap = keymap;
    }

    /// Read a line of input with the given prompt.
    pub fn read_line(&mut self, prompt: impl Prompt) -> io::Result<Signal> {
        let mut line_buffer = LineBuffer::new();
        if let Some(menu) = &mut self.menu {
            menu.dismiss(&mut line_buffer);
        }

        let mut history_index: Option<usize> = None;
        let mut saved_buffer: Option<String> = None;
        let mut history_prefix: Option<String> = None;
        let mut vi_operator: Option<ViOperator> = None;
        let mut undo_stack: Vec<(String, usize)> = Vec::new();
        let mut redo_stack: Vec<(String, usize)> = Vec::new();
        let mut in_insert_session = false;

        macro_rules! save_undo {
            () => {
                undo_stack.push((line_buffer.as_str().to_string(), line_buffer.cursor()));
                redo_stack.clear();
            };
        }

        let _guard = RawModeGuard::new(&mut self.writer)?;

        match self.edit_mode {
            EditMode::Emacs => {
                let _ = queue!(self.writer, self.cursor_style);
            }
            EditMode::Vi => {
                self.vi_mode = ViMode::Insert;
                self.keymap = Keymap::default_vi_insert();
                let _ = queue!(self.writer, self.cursor_style);
            }
        }

        // Initial render
        let (layout, mut current_hint) = self.render_line(&prompt, &line_buffer, 0)?;
        let mut previous_cursor_row = layout.cursor_row;

        loop {
            let event = event::read()?;

            match event {
                Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                }) => {
                    // Ignore key release events to avoid duplicate execution
                    if kind == KeyEventKind::Release {
                        continue;
                    }

                    // Delegate to active menu first if menu is open
                    if let Some(menu) = &mut self.menu
                        && menu.is_active()
                    {
                        let outcome = menu.handle_key(
                            code,
                            modifiers,
                            &mut line_buffer,
                            self.completer.as_deref(),
                        );
                        match outcome {
                            MenuOutcome::Consumed
                            | MenuOutcome::Accepted
                            | MenuOutcome::Dismissed => {
                                let (layout, hint) =
                                    self.render_line(&prompt, &line_buffer, previous_cursor_row)?;
                                previous_cursor_row = layout.cursor_row;
                                current_hint = hint;
                                continue;
                            }
                            MenuOutcome::PassThrough => {
                                // Menu dismissed; fall through to normal editor handling
                            }
                        }
                    }

                    let mut changed = false;

                    // If an operator is pending in Vi mode, handle it
                    if let Some(op) = vi_operator.take() {
                        in_insert_session = false;
                        match op {
                            ViOperator::ReplaceChar => {
                                if !(matches!(code, KeyCode::Esc)
                                    || (code == KeyCode::Char('[')
                                        && modifiers.contains(KeyModifiers::CONTROL)))
                                    && let (
                                        KeyCode::Char(c),
                                        KeyModifiers::NONE | KeyModifiers::SHIFT,
                                    ) = (code, modifiers)
                                {
                                    save_undo!();
                                    changed = line_buffer.replace_char_at_cursor(c);
                                }
                            }
                            ViOperator::Delete => {
                                match code {
                                    KeyCode::Char('d') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_line() {
                                            if let Some(cb) = &mut self.clipboard {
                                                cb.set(&cut);
                                            }
                                            changed = true;
                                        }
                                    }
                                    KeyCode::Char('w') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_word_right() {
                                            if let Some(cb) = &mut self.clipboard {
                                                cb.set(&cut);
                                            }
                                            if line_buffer.cursor() > 0
                                                && line_buffer.cursor() >= line_buffer.len()
                                            {
                                                line_buffer.move_left();
                                            }
                                            changed = true;
                                        }
                                    }
                                    KeyCode::Char('b') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_word_left() {
                                            if let Some(cb) = &mut self.clipboard {
                                                cb.set(&cut);
                                            }
                                            changed = true;
                                        }
                                    }
                                    KeyCode::Char('e') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_to_word_end() {
                                            if let Some(cb) = &mut self.clipboard {
                                                cb.set(&cut);
                                            }
                                            changed = true;
                                        }
                                    }
                                    KeyCode::Char('$') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_to_end() {
                                            if let Some(cb) = &mut self.clipboard {
                                                cb.set(&cut);
                                            }
                                            if line_buffer.cursor() > 0
                                                && line_buffer.cursor() >= line_buffer.len()
                                            {
                                                line_buffer.move_left();
                                            }
                                            changed = true;
                                        }
                                    }
                                    KeyCode::Char('0') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_to_start() {
                                            if let Some(cb) = &mut self.clipboard {
                                                cb.set(&cut);
                                            }
                                            changed = true;
                                        }
                                    }
                                    _ => {} // Cancelled
                                }
                            }
                            ViOperator::Change => {
                                let mut enter_insert = false;
                                match code {
                                    KeyCode::Char('c') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_line()
                                            && let Some(cb) = &mut self.clipboard
                                        {
                                            cb.set(&cut);
                                        }
                                        enter_insert = true;
                                    }
                                    KeyCode::Char('w' | 'e') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_to_word_end()
                                            && let Some(cb) = &mut self.clipboard
                                        {
                                            cb.set(&cut);
                                        }
                                        enter_insert = true;
                                    }
                                    KeyCode::Char('b') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_word_left()
                                            && let Some(cb) = &mut self.clipboard
                                        {
                                            cb.set(&cut);
                                        }
                                        enter_insert = true;
                                    }
                                    KeyCode::Char('$') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_to_end()
                                            && let Some(cb) = &mut self.clipboard
                                        {
                                            cb.set(&cut);
                                        }
                                        enter_insert = true;
                                    }
                                    KeyCode::Char('0') => {
                                        save_undo!();
                                        if let Some(cut) = line_buffer.cut_to_start()
                                            && let Some(cb) = &mut self.clipboard
                                        {
                                            cb.set(&cut);
                                        }
                                        enter_insert = true;
                                    }
                                    _ => {} // Cancelled
                                }
                                if enter_insert {
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                    changed = true;
                                }
                            }
                            ViOperator::Yank => {
                                match code {
                                    KeyCode::Char('y') => {
                                        if let Some(cb) = &mut self.clipboard
                                            && let Some(text) = line_buffer.yank_line()
                                        {
                                            cb.set(&text);
                                        }
                                    }
                                    KeyCode::Char('w') => {
                                        if let Some(cb) = &mut self.clipboard
                                            && let Some(text) = line_buffer.yank_word()
                                        {
                                            cb.set(&text);
                                        }
                                    }
                                    KeyCode::Char('e') => {
                                        if let Some(cb) = &mut self.clipboard
                                            && let Some(text) = line_buffer.yank_to_word_end()
                                        {
                                            cb.set(&text);
                                        }
                                    }
                                    KeyCode::Char('$') => {
                                        if let Some(cb) = &mut self.clipboard
                                            && let Some(text) = line_buffer.yank_to_end()
                                        {
                                            cb.set(&text);
                                        }
                                    }
                                    KeyCode::Char('0') => {
                                        if let Some(cb) = &mut self.clipboard
                                            && line_buffer.cursor() > 0
                                        {
                                            cb.set(&line_buffer.as_str()[..line_buffer.cursor()]);
                                        }
                                    }
                                    _ => {} // Cancelled
                                }
                            }
                        }
                    } else if let Some(action) = self.keymap.get(code, modifiers) {
                        match action {
                            EditorAction::Complete => {
                                if let Some(menu) = &mut self.menu {
                                    menu.activate(&mut line_buffer, self.completer.as_deref());
                                    changed = true;
                                } else if let Some(completer) = &self.completer {
                                    let (start, candidates) = completer
                                        .complete(line_buffer.as_str(), line_buffer.cursor());
                                    if candidates.len() == 1 {
                                        let c = &candidates[0];
                                        let mut rep = c.replacement.clone();
                                        if c.append_space && !rep.ends_with(' ') {
                                            rep.push(' ');
                                        }
                                        line_buffer
                                            .replace_range(start..line_buffer.cursor(), &rep);
                                        line_buffer.set_cursor(start + rep.len());
                                        changed = true;
                                    }
                                }
                            }
                            EditorAction::CompletePrevious => {
                                // If a menu is active, handle_key already dealt with reverse cycling
                            }
                            EditorAction::Submit => {
                                let is_complete = match &self.validator {
                                    Some(v) => matches!(
                                        v.validate(line_buffer.as_str()),
                                        ValidationResult::Complete
                                    ),
                                    None => true,
                                };

                                if is_complete {
                                    if let Some(menu) = &mut self.menu {
                                        menu.dismiss(&mut line_buffer);
                                    }
                                    // Cleanly erase any ghost text / menu before moving to next line
                                    let (final_layout, _) = self.render_line_internal(
                                        &prompt,
                                        &line_buffer,
                                        previous_cursor_row,
                                        false,
                                    )?;
                                    let rows_to_move_down = final_layout
                                        .end_row
                                        .saturating_sub(final_layout.cursor_row);
                                    if rows_to_move_down > 0 {
                                        queue!(
                                            self.writer,
                                            cursor::MoveDown(rows_to_move_down as u16)
                                        )?;
                                    }
                                    queue!(self.writer, Print("\r\n"))?;
                                    self.writer.flush()?;
                                    let line = line_buffer.into_string();
                                    if let Some(history) = &mut self.history {
                                        history.append(&line);
                                    }
                                    return Ok(Signal::Success(line));
                                } else {
                                    // Incomplete input -> query validator if we should indent
                                    let should_indent = self.validator.as_ref().is_some_and(|v| {
                                        v.should_indent(line_buffer.current_line_prefix())
                                    });
                                    line_buffer.insert_newline_preserve_indent(
                                        should_indent,
                                        self.indent_size,
                                    );
                                    changed = true;
                                }
                            }
                            EditorAction::InsertNewline => {
                                if let Some(menu) = &mut self.menu {
                                    menu.dismiss(&mut line_buffer);
                                }
                                let should_indent = self.validator.as_ref().is_some_and(|v| {
                                    v.should_indent(line_buffer.current_line_prefix())
                                });
                                line_buffer.insert_newline_preserve_indent(
                                    should_indent,
                                    self.indent_size,
                                );
                                changed = true;
                            }
                            EditorAction::Interrupt => {
                                if let Some(menu) = &mut self.menu {
                                    menu.dismiss(&mut line_buffer);
                                }
                                // Cleanly erase any ghost text / menu before printing ^C
                                let (final_layout, _) = self.render_line_internal(
                                    &prompt,
                                    &line_buffer,
                                    previous_cursor_row,
                                    false,
                                )?;
                                let rows_to_move_down =
                                    final_layout.end_row.saturating_sub(final_layout.cursor_row);
                                if rows_to_move_down > 0 {
                                    queue!(
                                        self.writer,
                                        cursor::MoveDown(rows_to_move_down as u16)
                                    )?;
                                }
                                queue!(self.writer, Print("^C\r\n"))?;
                                self.writer.flush()?;
                                return Ok(Signal::CtrlC);
                            }
                            EditorAction::Eof if line_buffer.is_empty() => {
                                queue!(self.writer, Print("\r\n"))?;
                                self.writer.flush()?;
                                return Ok(Signal::CtrlD);
                            }
                            EditorAction::AcceptGhostText => {
                                if let Some(hint) = &current_hint {
                                    line_buffer.insert_str(hint);
                                    changed = true;
                                }
                            }
                            EditorAction::AcceptGhostTextWord => {
                                if let Some(hint) = &current_hint {
                                    let word = self.hinter.as_ref().map_or_else(
                                        || default_next_hint_word(hint),
                                        |h| h.next_hint_word(hint),
                                    );
                                    if !word.is_empty() {
                                        line_buffer.insert_str(word);
                                        changed = true;
                                    }
                                }
                            }
                            EditorAction::MoveLeft => {
                                in_insert_session = false;
                                changed = line_buffer.move_left();
                            }
                            EditorAction::MoveRight => {
                                in_insert_session = false;
                                if self.edit_mode == EditMode::Vi && self.vi_mode == ViMode::Normal
                                {
                                    if line_buffer.cursor() + 1 < line_buffer.len() {
                                        changed = line_buffer.move_right();
                                    }
                                } else if !line_buffer.move_right() {
                                    // At end of line: accept ghost text if present
                                    if let Some(hint) = &current_hint {
                                        line_buffer.insert_str(hint);
                                        changed = true;
                                    }
                                } else {
                                    changed = true;
                                }
                            }
                            EditorAction::MoveUp => {
                                in_insert_session = false;
                                if line_buffer.move_up() {
                                    changed = true;
                                } else if let Some(history) = &self.history
                                    && !history.is_empty()
                                {
                                    let hist_len = history.len();
                                    let pfx = history_prefix.get_or_insert_with(|| {
                                        saved_buffer = Some(line_buffer.as_str().to_string());
                                        line_buffer.as_str().to_string()
                                    });

                                    let search_start = history_index.unwrap_or(hist_len);
                                    if search_start > 0 {
                                        for i in (0..search_start).rev() {
                                            if let Some(entry) = history.get(i)
                                                && entry.starts_with(pfx.as_str())
                                            {
                                                history_index = Some(i);
                                                line_buffer
                                                    .replace_range(0..line_buffer.len(), &entry);
                                                line_buffer.set_cursor(entry.len());
                                                changed = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            EditorAction::MoveDown => {
                                in_insert_session = false;
                                if line_buffer.move_down() {
                                    changed = true;
                                } else if let Some(history) = &self.history
                                    && let Some(curr_idx) = history_index
                                {
                                    let hist_len = history.len();
                                    let pfx = history_prefix.as_deref().unwrap_or("");
                                    let mut next_entry = None;

                                    for i in (curr_idx + 1)..hist_len {
                                        if let Some(entry) = history.get(i)
                                            && entry.starts_with(pfx)
                                        {
                                            next_entry = Some((i, entry));
                                            break;
                                        }
                                    }

                                    if let Some((idx, entry)) = next_entry {
                                        history_index = Some(idx);
                                        line_buffer.replace_range(0..line_buffer.len(), &entry);
                                        line_buffer.set_cursor(entry.len());
                                        changed = true;
                                    } else {
                                        history_index = None;
                                        history_prefix = None;
                                        let orig = saved_buffer.take().unwrap_or_default();
                                        line_buffer.replace_range(0..line_buffer.len(), &orig);
                                        line_buffer.set_cursor(orig.len());
                                        changed = true;
                                    }
                                }
                            }
                            EditorAction::MoveWordLeft => {
                                in_insert_session = false;
                                changed = line_buffer.move_word_left();
                            }
                            EditorAction::MoveWordRight => {
                                in_insert_session = false;
                                if !line_buffer.move_word_right() {
                                    // At end of line: accept next hint word if present
                                    if let Some(hint) = &current_hint {
                                        let word = self.hinter.as_ref().map_or_else(
                                            || default_next_hint_word(hint),
                                            |h| h.next_hint_word(hint),
                                        );
                                        if !word.is_empty() {
                                            line_buffer.insert_str(word);
                                            changed = true;
                                        }
                                    }
                                } else {
                                    if self.edit_mode == EditMode::Vi
                                        && self.vi_mode == ViMode::Normal
                                        && line_buffer.cursor() > 0
                                        && line_buffer.cursor() == line_buffer.len()
                                    {
                                        line_buffer.move_left();
                                    }
                                    changed = true;
                                }
                            }
                            EditorAction::MoveWordEnd => {
                                in_insert_session = false;
                                changed = line_buffer.move_word_end();
                            }
                            EditorAction::MoveHome => {
                                in_insert_session = false;
                                changed = line_buffer.move_home();
                            }
                            EditorAction::MoveToFirstNonWhitespace => {
                                in_insert_session = false;
                                changed = line_buffer.move_to_first_non_whitespace();
                            }
                            EditorAction::MoveEnd => {
                                in_insert_session = false;
                                if !line_buffer.move_end() {
                                    // Already at end: accept ghost text if present
                                    if let Some(hint) = &current_hint {
                                        line_buffer.insert_str(hint);
                                        changed = true;
                                    }
                                } else {
                                    if self.edit_mode == EditMode::Vi
                                        && self.vi_mode == ViMode::Normal
                                        && line_buffer.cursor() > 0
                                        && line_buffer.cursor() == line_buffer.len()
                                    {
                                        line_buffer.move_left();
                                    }
                                    changed = true;
                                }
                            }
                            EditorAction::Backspace => {
                                if !in_insert_session {
                                    save_undo!();
                                    in_insert_session = true;
                                }
                                changed =
                                    line_buffer.delete_prev_grapheme_or_dedent(self.indent_size);
                            }
                            EditorAction::Delete => {
                                if !in_insert_session {
                                    save_undo!();
                                    in_insert_session = true;
                                }
                                changed = line_buffer.delete_next_grapheme();
                            }
                            EditorAction::DeleteWordLeft | EditorAction::CutWordLeft => {
                                save_undo!();
                                in_insert_session = false;
                                if let Some(cut) = line_buffer.cut_word_left() {
                                    if let Some(cb) = &mut self.clipboard {
                                        cb.set(&cut);
                                    }
                                    changed = true;
                                }
                            }
                            EditorAction::DeleteWordRight | EditorAction::CutWordRight => {
                                save_undo!();
                                in_insert_session = false;
                                if let Some(cut) = line_buffer.cut_word_right() {
                                    if let Some(cb) = &mut self.clipboard {
                                        cb.set(&cut);
                                    }
                                    changed = true;
                                }
                            }
                            EditorAction::ClearToStart | EditorAction::CutToStart => {
                                save_undo!();
                                in_insert_session = false;
                                if let Some(cut) = line_buffer.cut_to_start() {
                                    if let Some(cb) = &mut self.clipboard {
                                        cb.set(&cut);
                                    }
                                    changed = true;
                                }
                            }
                            EditorAction::ClearToEnd | EditorAction::CutToEnd => {
                                save_undo!();
                                in_insert_session = false;
                                if let Some(cut) = line_buffer.cut_to_end() {
                                    if let Some(cb) = &mut self.clipboard {
                                        cb.set(&cut);
                                    }
                                    changed = true;
                                }
                            }
                            EditorAction::CutChar => {
                                save_undo!();
                                in_insert_session = false;
                                if let Some(cut) = line_buffer.cut_char_under_cursor() {
                                    if let Some(cb) = &mut self.clipboard {
                                        cb.set(&cut);
                                    }
                                    changed = true;
                                }
                            }
                            EditorAction::CutCharBefore => {
                                save_undo!();
                                in_insert_session = false;
                                if let Some(cut) = line_buffer.cut_char_before_cursor() {
                                    if let Some(cb) = &mut self.clipboard {
                                        cb.set(&cut);
                                    }
                                    changed = true;
                                }
                            }
                            EditorAction::Yank => {
                                if let Some(cb) = &mut self.clipboard
                                    && let Some(text) = cb.get()
                                {
                                    let normalised = normalise_paste(&text);
                                    if !normalised.is_empty() {
                                        save_undo!();
                                        in_insert_session = false;
                                        line_buffer.insert_str(&normalised);
                                        changed = true;
                                    }
                                }
                            }
                            EditorAction::PasteAfter => {
                                if let Some(cb) = &mut self.clipboard
                                    && let Some(text) = cb.get()
                                {
                                    let normalised = normalise_paste(&text);
                                    if !normalised.is_empty() {
                                        save_undo!();
                                        in_insert_session = false;
                                        line_buffer.paste_after(&normalised);
                                        changed = true;
                                    }
                                }
                            }
                            EditorAction::PasteBefore => {
                                if let Some(cb) = &mut self.clipboard
                                    && let Some(text) = cb.get()
                                {
                                    let normalised = normalise_paste(&text);
                                    if !normalised.is_empty() {
                                        save_undo!();
                                        in_insert_session = false;
                                        line_buffer.paste_before(&normalised);
                                        changed = true;
                                    }
                                }
                            }
                            EditorAction::YankLine => {
                                if let Some(cb) = &mut self.clipboard
                                    && let Some(text) = line_buffer.yank_line()
                                {
                                    cb.set(&text);
                                }
                            }
                            EditorAction::ToggleCase => {
                                save_undo!();
                                in_insert_session = false;
                                changed = line_buffer.toggle_case_at_cursor();
                            }
                            EditorAction::Undo => {
                                in_insert_session = false;
                                if let Some((saved_content, saved_cursor)) = undo_stack.pop() {
                                    redo_stack.push((
                                        line_buffer.as_str().to_string(),
                                        line_buffer.cursor(),
                                    ));
                                    line_buffer.replace_range(0..line_buffer.len(), &saved_content);
                                    line_buffer.set_cursor(saved_cursor.min(line_buffer.len()));
                                    changed = true;
                                }
                            }
                            EditorAction::Redo => {
                                in_insert_session = false;
                                if let Some((saved_content, saved_cursor)) = redo_stack.pop() {
                                    undo_stack.push((
                                        line_buffer.as_str().to_string(),
                                        line_buffer.cursor(),
                                    ));
                                    line_buffer.replace_range(0..line_buffer.len(), &saved_content);
                                    line_buffer.set_cursor(saved_cursor.min(line_buffer.len()));
                                    changed = true;
                                }
                            }
                            EditorAction::SwitchToNormalMode => {
                                in_insert_session = false;
                                if self.edit_mode == EditMode::Vi && self.vi_mode != ViMode::Normal
                                {
                                    self.vi_mode = ViMode::Normal;
                                    self.keymap = Keymap::default_vi_normal();
                                    if line_buffer.cursor() > 0 {
                                        line_buffer.move_left();
                                    }
                                    let _ =
                                        queue!(self.writer, cursor::SetCursorStyle::SteadyBlock);
                                    changed = true;
                                }
                            }
                            EditorAction::SwitchToInsertMode => {
                                if self.edit_mode == EditMode::Vi && self.vi_mode != ViMode::Insert
                                {
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                    changed = true;
                                }
                            }
                            EditorAction::Append => {
                                if self.edit_mode == EditMode::Vi {
                                    if line_buffer.cursor() < line_buffer.len() {
                                        line_buffer.move_right();
                                    }
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                    changed = true;
                                }
                            }
                            EditorAction::AppendEnd => {
                                if self.edit_mode == EditMode::Vi {
                                    line_buffer.move_end();
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                    changed = true;
                                }
                            }
                            EditorAction::InsertAtFirstNonWhitespace => {
                                if self.edit_mode == EditMode::Vi {
                                    line_buffer.move_to_first_non_whitespace();
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                    changed = true;
                                }
                            }
                            EditorAction::OpenBelow => {
                                if self.edit_mode == EditMode::Vi {
                                    save_undo!();
                                    line_buffer.move_end();
                                    let should_indent = self.validator.as_ref().is_some_and(|v| {
                                        v.should_indent(line_buffer.current_line_prefix())
                                    });
                                    line_buffer.insert_newline_preserve_indent(
                                        should_indent,
                                        self.indent_size,
                                    );
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                    changed = true;
                                }
                            }
                            EditorAction::OpenAbove => {
                                if self.edit_mode == EditMode::Vi {
                                    save_undo!();
                                    line_buffer.move_home();
                                    line_buffer.insert_char('\n');
                                    line_buffer.move_up();
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                    changed = true;
                                }
                            }
                            EditorAction::SubstituteChar => {
                                save_undo!();
                                if let Some(cut) = line_buffer.cut_char_under_cursor()
                                    && let Some(cb) = &mut self.clipboard
                                {
                                    cb.set(&cut);
                                }
                                if self.edit_mode == EditMode::Vi {
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                }
                                changed = true;
                            }
                            EditorAction::SubstituteLine => {
                                save_undo!();
                                if let Some(cut) = line_buffer.cut_line()
                                    && let Some(cb) = &mut self.clipboard
                                {
                                    cb.set(&cut);
                                }
                                if self.edit_mode == EditMode::Vi {
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                }
                                changed = true;
                            }
                            EditorAction::ChangeToEnd => {
                                save_undo!();
                                if let Some(cut) = line_buffer.cut_to_end()
                                    && let Some(cb) = &mut self.clipboard
                                {
                                    cb.set(&cut);
                                }
                                if self.edit_mode == EditMode::Vi {
                                    self.vi_mode = ViMode::Insert;
                                    self.keymap = Keymap::default_vi_insert();
                                    let _ = queue!(self.writer, self.cursor_style);
                                }
                                changed = true;
                            }
                            EditorAction::ViDeleteOperator => {
                                vi_operator = Some(ViOperator::Delete);
                            }
                            EditorAction::ViChangeOperator => {
                                vi_operator = Some(ViOperator::Change);
                            }
                            EditorAction::ViYankOperator => {
                                vi_operator = Some(ViOperator::Yank);
                            }
                            EditorAction::ViReplaceCharOperator => {
                                vi_operator = Some(ViOperator::ReplaceChar);
                            }
                            EditorAction::ClearLine => {
                                save_undo!();
                                in_insert_session = false;
                                changed = line_buffer.clear();
                            }
                            EditorAction::ClearScreen => {
                                queue!(self.writer, Clear(ClearType::All), cursor::MoveTo(0, 0),)?;
                                let (layout, hint) = self.render_line(&prompt, &line_buffer, 0)?;
                                previous_cursor_row = layout.cursor_row;
                                current_hint = hint;
                                continue;
                            }
                            EditorAction::Ignore | EditorAction::Eof => {}
                        }
                    } else if (self.edit_mode != EditMode::Vi || self.vi_mode == ViMode::Insert)
                        && let (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) =
                            (code, modifiers)
                    {
                        if !in_insert_session {
                            save_undo!();
                            in_insert_session = true;
                        }
                        // Unbound printable characters are inserted into the buffer
                        let should_dedent = self
                            .validator
                            .as_ref()
                            .is_some_and(|v| v.should_dedent_on_char(c));
                        if should_dedent {
                            line_buffer.dedent_line(self.indent_size);
                        }
                        line_buffer.insert_char(c);
                        if self.edit_mode != EditMode::Vi && (c == ' ' || c == '\t' || c == '\n') {
                            in_insert_session = false;
                        }
                        changed = true;
                    }

                    // Reset history navigation if buffer was modified by an action other than up/down
                    if changed
                        && !matches!(
                            self.keymap.get(code, modifiers),
                            Some(EditorAction::MoveUp | EditorAction::MoveDown)
                        )
                    {
                        history_index = None;
                        saved_buffer = None;
                        history_prefix = None;
                    }

                    // Only redraw if the line buffer or cursor actually changed
                    if changed {
                        let (layout, hint) =
                            self.render_line(&prompt, &line_buffer, previous_cursor_row)?;
                        previous_cursor_row = layout.cursor_row;
                        current_hint = hint;
                    }
                }

                // Bracketed paste event
                Event::Paste(text) => {
                    if let Some(menu) = &mut self.menu {
                        menu.dismiss(&mut line_buffer);
                    }
                    save_undo!();
                    in_insert_session = false;
                    let normalised = normalise_paste(&text);
                    if !normalised.is_empty() {
                        line_buffer.insert_str(&normalised);
                        let (layout, hint) =
                            self.render_line(&prompt, &line_buffer, previous_cursor_row)?;
                        previous_cursor_row = layout.cursor_row;
                        current_hint = hint;
                    }
                }

                // Terminal resize -> re-render line to adjust layout
                Event::Resize(..) => {
                    let (layout, hint) =
                        self.render_line(&prompt, &line_buffer, previous_cursor_row)?;
                    previous_cursor_row = layout.cursor_row;
                    current_hint = hint;
                }

                _ => {}
            }
        }
    }

    /// Redraws the prompt, buffer (including soft-wrapped and multiline rows),
    /// optional inline ghost text hint, and positions the cursor.
    pub fn render_line(
        &mut self,
        prompt: &impl Prompt,
        line_buffer: &LineBuffer,
        previous_cursor_row: usize,
    ) -> io::Result<(ScreenLayout, Option<String>)> {
        self.render_line_internal(prompt, line_buffer, previous_cursor_row, true)
    }

    fn render_line_internal(
        &mut self,
        prompt: &impl Prompt,
        line_buffer: &LineBuffer,
        previous_cursor_row: usize,
        show_ghost_text: bool,
    ) -> io::Result<(ScreenLayout, Option<String>)> {
        let term_cols = self.terminal_width();
        let prompt_text = prompt.render();
        let prompt_stripped = crate::prompt::strip_ansi(&prompt_text);
        let indicator_text = prompt.render_multiline_indicator();
        let indicator_stripped = crate::prompt::strip_ansi(&indicator_text);

        // Query ghost text hint if cursor is at the end of the buffer, content is non-empty, and menu is not active
        let is_menu_active = show_ghost_text && self.menu.as_ref().is_some_and(|m| m.is_active());
        let ghost_text = if is_menu_active
            || !show_ghost_text
            || (self.edit_mode == EditMode::Vi && self.vi_mode == ViMode::Normal)
        {
            None
        } else if let Some(hinter) = &self.hinter {
            if line_buffer.cursor() == line_buffer.len() && !line_buffer.is_empty() {
                hinter
                    .hint(line_buffer.as_str(), line_buffer.cursor())
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        } else {
            None
        };

        let layout = compute_screen_layout(
            &prompt_stripped,
            &indicator_stripped,
            line_buffer,
            ghost_text.as_deref(),
            term_cols,
        );

        // Move up to row 0 if we were previously on a lower row
        if previous_cursor_row > 0 {
            queue!(self.writer, cursor::MoveUp(previous_cursor_row as u16))?;
        }

        // Clear from row 0 downwards and print the prompt with CRLF newlines
        let prompt_render = prompt_text.replace("\r\n", "\n").replace('\n', "\r\n");
        let indicator_render = indicator_text.replace("\r\n", "\n").replace('\n', "\r\n");

        queue!(
            self.writer,
            cursor::Hide,
            MoveToColumn(0),
            Clear(ClearType::FromCursorDown),
            Print(&prompt_render),
        )?;

        let styled_text = match &self.formatter {
            Some(f) => f.format(line_buffer.as_str()),
            None => StyledText::plain(line_buffer.as_str()),
        };

        // Render spans, inserting indicator on newlines
        for (style, text) in &styled_text {
            let mut parts = text.split('\n').peekable();
            while let Some(part) = parts.next() {
                if !part.is_empty() {
                    queue!(
                        self.writer,
                        crossterm::style::PrintStyledContent(style.apply(part))
                    )?;
                }
                if parts.peek().is_some() {
                    queue!(
                        self.writer,
                        crossterm::style::ResetColor,
                        Print("\r\n"),
                        Print(&indicator_render),
                    )?;
                }
            }
        }

        // Render inline ghost text hint if present
        if let Some(ref hint) = ghost_text {
            queue!(
                self.writer,
                crossterm::style::PrintStyledContent(self.hint_style.apply(hint))
            )?;
        }

        queue!(self.writer, crossterm::style::ResetColor)?;

        // 5. Render completion menu lines beneath the buffer if menu is active
        let mut menu_rows = 0;
        if is_menu_active && let Some(menu) = &self.menu {
            let lines = menu.render_lines(term_cols, layout.cursor_col);
            for line in &lines {
                queue!(self.writer, Print("\r\n"), Print(line))?;
            }
            menu_rows = lines.len();
        }

        queue!(self.writer, crossterm::style::ResetColor)?;

        // Move cursor from the end of the rendered content back to layout.cursor_row
        let total_end_row = layout.end_row + menu_rows;
        let rows_to_move_up = total_end_row.saturating_sub(layout.cursor_row);
        if rows_to_move_up > 0 {
            queue!(self.writer, cursor::MoveUp(rows_to_move_up as u16))?;
        }

        // Position cursor horizontally at layout.cursor_col
        queue!(
            self.writer,
            MoveToColumn(layout.cursor_col as u16),
            cursor::Show,
        )?;
        self.writer.flush()?;

        let mut effective_layout = layout;
        effective_layout.end_row = total_end_row;
        Ok((effective_layout, ghost_text))
    }

    /// Immutably borrow the underlying writer.
    pub fn writer(&self) -> &W {
        &self.writer
    }

    /// Mutably borrow the underlying writer.
    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Consume the editor and return the underlying writer.
    pub fn into_writer(self) -> W {
        self.writer
    }
}

impl Default for Editor<BufWriter<Stdout>> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// The user typed a command and pressed Enter.
    Success(String),

    /// The user pressed Ctrl+C.
    /// In a shell, this means: cancel the current input, print a newline,
    /// and show a fresh prompt (do not execute anything).
    CtrlC,

    /// The user pressed Ctrl+D on an empty line.
    /// In Unix shells, this signals EOF (End of File), meaning "exit the shell".
    CtrlD,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completer::Candidate;
    use crate::keymap::KeyCombo;

    #[test]
    fn test_editor_creation_default() {
        let editor = Editor::new();
        assert!(
            editor
                .keymap()
                .contains((KeyCode::Enter, KeyModifiers::NONE))
        );
    }

    #[test]
    fn test_editor_with_custom_writer_and_keymap() {
        let buffer = Vec::new();
        let mut keymap = Keymap::empty();
        keymap.bind(KeyCombo::ctrl('q'), EditorAction::Interrupt);

        let mut editor = Editor::new_with_writer(buffer).with_keymap(keymap);
        assert!(editor.keymap().contains(KeyCombo::ctrl('q')));
        assert!(!editor.keymap().contains(KeyCode::Enter));

        editor
            .keymap_mut()
            .bind(KeyCode::Enter, EditorAction::Submit);
        assert!(editor.keymap().contains(KeyCode::Enter));
    }

    #[test]
    fn test_render_line_to_buffer() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer);
        let line_buf = LineBuffer::with_content("echo 1".to_string());
        let (layout, _) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 8); // "> " (2) + "echo 1" (6)
        let out = editor.into_writer();
        assert!(!out.is_empty());
    }

    #[test]
    fn test_render_line_with_formatter() {
        let buffer = Vec::new();
        let mut editor =
            Editor::new_with_writer(buffer).with_formatter(|s: &str| format!("***{}***", s));
        let line_buf = LineBuffer::with_content("secret".to_string());
        let (layout, _) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 0);
        let out = String::from_utf8(editor.into_writer()).unwrap();
        assert!(out.contains("***secret***"));
    }

    #[test]
    fn test_render_multiline() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer);
        let line_buf = LineBuffer::with_content("first\nsecond".to_string());
        let (layout, _) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        let out = String::from_utf8(editor.into_writer()).unwrap();
        // Should contain continuation indicator "… "
        assert!(out.contains("… "));
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.end_row, 1);
    }

    #[test]
    fn test_render_line_with_soft_wrap_override() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer).with_terminal_width(10);
        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("123456789"); // 2 + 9 = 11 cols in 10-col terminal

        let (layout, _) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(layout.cursor_row, 1);
        assert_eq!(layout.cursor_col, 1);
        assert_eq!(layout.end_row, 1);
        assert_eq!(layout.end_col, 1);
    }

    #[test]
    fn test_render_line_with_ghost_text() {
        let buffer = Vec::new();
        let mut editor =
            Editor::new_with_writer(buffer).with_hinter(|line: &str, cursor: usize| {
                if line == "git c" && cursor == 5 {
                    Some("heckout".to_string())
                } else {
                    None
                }
            });

        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("git c");

        let (layout, hint) = editor.render_line(&"> ", &line_buf, 0).unwrap();
        assert_eq!(hint.as_deref(), Some("heckout"));
        // Prompt is "> " (2) + "git c" (5) = 7
        assert_eq!(layout.cursor_col, 7);
        assert_eq!(layout.cursor_row, 0);
        // Total printed text is "> git checkout" (2 + 12 = 14)
        assert_eq!(layout.end_col, 14);
        assert_eq!(layout.end_row, 0);

        let out = String::from_utf8(editor.into_writer()).unwrap();
        assert!(out.contains("heckout"));
    }

    #[test]
    fn test_render_line_suppresses_ghost_text_when_disabled_or_vi_normal() {
        let buffer = Vec::new();
        let mut editor =
            Editor::new_with_writer(buffer).with_hinter(|line: &str, cursor: usize| {
                if line == "git c" && cursor == 5 {
                    Some("heckout".to_string())
                } else {
                    None
                }
            });

        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("git c");

        // 1. When show_ghost_text is false (e.g. on submit or interrupt), ghost text is suppressed
        let (layout, hint) = editor
            .render_line_internal(&"> ", &line_buf, 0, false)
            .unwrap();
        assert_eq!(hint, None);
        assert_eq!(layout.cursor_col, 7);
        assert_eq!(layout.end_col, 7);

        // 2. In Vi Normal mode, ghost text is also suppressed even when show_ghost_text is true
        editor.set_edit_mode(EditMode::Vi);
        editor.vi_mode = ViMode::Normal;
        let (layout_vi, hint_vi) = editor
            .render_line_internal(&"> ", &line_buf, 0, true)
            .unwrap();
        assert_eq!(hint_vi, None);
        assert_eq!(layout_vi.end_col, 7);
    }

    #[test]
    fn test_normalise_paste_crlf_and_cr() {
        let input = "line1\r\nline2\rline3\nline4";
        let normalised = normalise_paste(input);
        assert_eq!(normalised, "line1\nline2\nline3\nline4");
    }

    #[test]
    fn test_normalise_paste_preserves_tabs_and_strips_control_chars() {
        let input = "hello\tworld\x00\x1b[31mred\x1b[0m\x08test";
        let normalised = normalise_paste(input);
        // \x00, \x1b, and \x08 are stripped; \t is retained
        assert_eq!(normalised, "hello\tworld[31mred[0mtest");
    }

    #[test]
    fn test_normalise_paste_multiline_code_block() {
        let input = "def foo():\r\n    return 42\r\n";
        let normalised = normalise_paste(input);
        assert_eq!(normalised, "def foo():\n    return 42\n");
    }

    #[test]
    fn test_editor_with_completer_and_menu() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer)
            .with_completer(|line: &str, cursor: usize| {
                if line == "car" && cursor == 3 {
                    (0, vec![Candidate::new("cargo")])
                } else {
                    (cursor, Vec::new())
                }
            })
            .with_menu(crate::menu::BasicMenu::default());

        assert!(editor.completer().is_some());
        assert!(editor.menu().is_some());

        let (start, items) = editor.completer().unwrap().complete("car", 3);
        assert_eq!(start, 0);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].display, "cargo");

        editor.set_menu(None);
        assert!(editor.menu().is_none());
        editor.set_completer(None);
        assert!(editor.completer().is_none());
    }

    #[test]
    fn test_render_line_with_active_completion_menu() {
        let buffer = Vec::new();
        let mut menu = crate::menu::BasicMenu::default();

        let mut line_buf = LineBuffer::new();
        line_buf.insert_str("ca");

        let completer = |_line: &str, _cursor: usize| {
            (
                0,
                vec![
                    Candidate::new("cargo").with_description("Rust package manager"),
                    Candidate::new("cat").with_description("Concatenate files"),
                ],
            )
        };

        menu.activate(&mut line_buf, Some(&completer));
        assert!(menu.is_active());

        let mut editor = Editor::new_with_writer(buffer)
            .with_terminal_width(80)
            .with_hinter(|_line: &str, _cursor: usize| Some("ghost-hint".to_string()))
            .with_menu(menu);

        let (layout, hint) = editor.render_line(&"> ", &line_buf, 0).unwrap();

        // Ghost text MUST be suppressed when completion menu is active
        assert_eq!(hint, None);

        // Layout cursor is at row 0, col 4 ("> " (2) + "ca" (2))
        assert_eq!(layout.cursor_row, 0);
        assert_eq!(layout.cursor_col, 4);

        // Layout end_row includes menu lines: 1 (buffer row 0) + 4 (top border + 2 items + bottom border) = 4
        assert_eq!(layout.end_row, 4);

        let out = String::from_utf8(editor.into_writer()).unwrap();
        // Menu borders and items are drawn in the rendered output
        assert!(out.contains("┌"));
        assert!(out.contains("└"));
        assert!(out.contains("cargo"));
        assert!(out.contains("cat"));
        // Ghost text is NOT drawn
        assert!(!out.contains("ghost-hint"));
    }

    #[test]
    fn test_editor_history_builder() {
        let mut hist = crate::history::DefaultHistory::new(10);
        hist.append("echo 1");
        hist.append("echo 2");

        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer).with_history(hist);
        assert!(editor.history().is_some());
        assert_eq!(editor.history().unwrap().len(), 2);
        assert_eq!(editor.history().unwrap().get(0).as_deref(), Some("echo 1"));
        assert_eq!(editor.history().unwrap().get(1).as_deref(), Some("echo 2"));

        editor.set_history(None);
        assert!(editor.history().is_none());
    }

    #[test]
    fn test_render_line_multiline_prompt_redraws_in_place() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer).with_terminal_width(80);
        let prompt = "\n\x1b[1;34m~/dev/lsh\x1b[0m (main)\n\x1b[35m❯ \x1b[0m";

        let mut line_buf = LineBuffer::new();
        let (layout1, _) = editor.render_line(&prompt, &line_buf, 0).unwrap();
        assert_eq!(layout1.cursor_row, 2);
        assert_eq!(layout1.cursor_col, 2);

        // Render second keystroke with previous_cursor_row = 2
        line_buf.insert_char('c');
        let (layout2, _) = editor
            .render_line(&prompt, &line_buf, layout1.cursor_row)
            .unwrap();
        assert_eq!(layout2.cursor_row, 2);
        assert_eq!(layout2.cursor_col, 3);

        let out = String::from_utf8(editor.into_writer()).unwrap();
        // Prompt newlines should be rendered with \r\n for raw mode compatibility
        assert!(out.contains("\r\n"));
        // Second render must emit MoveUp(2) to return to row 0 of the prompt
        assert!(out.contains("\x1b[2A"));
    }

    #[test]
    fn test_editor_clipboard_cut_and_yank() {
        use crate::clipboard::InMemoryClipboard;

        let buffer = Vec::new();
        let cb = InMemoryClipboard::new();
        let mut editor = Editor::new_with_writer(buffer).with_clipboard(cb);

        assert!(editor.clipboard().is_some());

        // Check mutable borrow and value storage
        editor.clipboard_mut().unwrap().set("copied text");
        assert_eq!(
            editor.clipboard_mut().unwrap().get().as_deref(),
            Some("copied text")
        );

        editor.set_clipboard(None);
        assert!(editor.clipboard().is_none());
    }

    #[test]
    fn test_editor_vi_mode_configuration_and_keymaps() {
        let buffer = Vec::new();
        let mut editor = Editor::new_with_writer(buffer);
        assert_eq!(editor.edit_mode(), EditMode::Emacs);
        assert_eq!(editor.vi_mode(), ViMode::Insert);

        // Switch to Vi mode
        editor.set_edit_mode(EditMode::Vi);
        assert_eq!(editor.edit_mode(), EditMode::Vi);
        assert_eq!(editor.vi_mode(), ViMode::Insert);

        // In Vi insert mode, Esc is mapped to SwitchToNormalMode
        assert_eq!(
            editor.keymap().get(KeyCode::Esc, KeyModifiers::NONE),
            Some(&EditorAction::SwitchToNormalMode)
        );

        // Check normal mode keybindings
        let normal_km = Keymap::default_vi_normal();
        assert_eq!(
            normal_km.get(KeyCode::Char('i'), KeyModifiers::NONE),
            Some(&EditorAction::SwitchToInsertMode)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('a'), KeyModifiers::NONE),
            Some(&EditorAction::Append)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('A'), KeyModifiers::NONE),
            Some(&EditorAction::AppendEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('I'), KeyModifiers::NONE),
            Some(&EditorAction::InsertAtFirstNonWhitespace)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('o'), KeyModifiers::NONE),
            Some(&EditorAction::OpenBelow)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('O'), KeyModifiers::NONE),
            Some(&EditorAction::OpenAbove)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('s'), KeyModifiers::NONE),
            Some(&EditorAction::SubstituteChar)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('S'), KeyModifiers::NONE),
            Some(&EditorAction::SubstituteLine)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('C'), KeyModifiers::NONE),
            Some(&EditorAction::ChangeToEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('d'), KeyModifiers::NONE),
            Some(&EditorAction::ViDeleteOperator)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('c'), KeyModifiers::NONE),
            Some(&EditorAction::ViChangeOperator)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('y'), KeyModifiers::NONE),
            Some(&EditorAction::ViYankOperator)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('r'), KeyModifiers::NONE),
            Some(&EditorAction::ViReplaceCharOperator)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('x'), KeyModifiers::NONE),
            Some(&EditorAction::CutChar)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('X'), KeyModifiers::NONE),
            Some(&EditorAction::CutCharBefore)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('D'), KeyModifiers::NONE),
            Some(&EditorAction::CutToEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('~'), KeyModifiers::NONE),
            Some(&EditorAction::ToggleCase)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('u'), KeyModifiers::NONE),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('p'), KeyModifiers::NONE),
            Some(&EditorAction::PasteAfter)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('P'), KeyModifiers::NONE),
            Some(&EditorAction::PasteBefore)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('w'), KeyModifiers::NONE),
            Some(&EditorAction::MoveWordRight)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('b'), KeyModifiers::NONE),
            Some(&EditorAction::MoveWordLeft)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('e'), KeyModifiers::NONE),
            Some(&EditorAction::MoveWordEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('0'), KeyModifiers::NONE),
            Some(&EditorAction::MoveHome)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('^'), KeyModifiers::NONE),
            Some(&EditorAction::MoveToFirstNonWhitespace)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('$'), KeyModifiers::NONE),
            Some(&EditorAction::MoveEnd)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('u'), KeyModifiers::NONE),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            normal_km.get(KeyCode::Char('r'), KeyModifiers::CONTROL),
            Some(&EditorAction::Redo)
        );

        // Switch back to Emacs
        editor.set_edit_mode(EditMode::Emacs);
        assert_eq!(editor.edit_mode(), EditMode::Emacs);

        // Emacs keybindings for Undo and Redo
        assert_eq!(
            editor
                .keymap()
                .get(KeyCode::Char('_'), KeyModifiers::CONTROL),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            editor
                .keymap()
                .get(KeyCode::Char('z'), KeyModifiers::CONTROL),
            Some(&EditorAction::Undo)
        );
        assert_eq!(
            editor.keymap().get(KeyCode::Char('r'), KeyModifiers::ALT),
            Some(&EditorAction::Redo)
        );
    }

    #[test]
    fn test_editor_cursor_style_configuration() {
        let mut editor = Editor::new();
        assert_eq!(editor.cursor_style(), cursor::SetCursorStyle::BlinkingBar);

        editor.set_cursor_style(cursor::SetCursorStyle::SteadyBar);
        assert_eq!(editor.cursor_style(), cursor::SetCursorStyle::SteadyBar);

        let editor2 = Editor::new_with_writer(Vec::<u8>::new())
            .with_cursor_style(cursor::SetCursorStyle::SteadyBlock);
        assert_eq!(editor2.cursor_style(), cursor::SetCursorStyle::SteadyBlock);
    }
}
