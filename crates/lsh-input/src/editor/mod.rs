use std::io::{self, BufWriter, Stdout, Write};

use crate::clipboard::{Clipboard, default_clipboard};
use crate::completer::Completer;
use crate::edit_mode::{EditMode, ViMode};
use crate::formatter::TextFormatter;
use crate::hinter::Hinter;
use crate::history::History;
use crate::keymap::Keymap;
pub use crate::layout::{ScreenLayout, compute_screen_layout};
use crate::menu::{Menu, MenuOutcome};
use crate::prompt::Prompt;
use crate::validator::Validator;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    queue,
    style::{ContentStyle, Stylize},
    terminal,
};

mod actions;
mod paste;
mod raw_mode;
mod render;
mod session;
mod signal;
mod vi;

#[cfg(test)]
mod tests;

pub use paste::normalise_paste;
pub use signal::Signal;
use actions::ActionResult;
use raw_mode::RawModeGuard;
use session::EditorSession;

pub struct Editor<W: Write = BufWriter<Stdout>> {
    writer: W,
    keymap: Keymap,
    vi_insert_keymap: Option<Keymap>,
    vi_normal_keymap: Option<Keymap>,
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
            vi_insert_keymap: None,
            vi_normal_keymap: None,
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
            vi_insert_keymap: None,
            vi_normal_keymap: None,
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

    fn vi_insert_keymap(&self) -> Keymap {
        self.vi_insert_keymap
            .clone()
            .unwrap_or_else(Keymap::default_vi_insert)
    }

    fn vi_normal_keymap(&self) -> Keymap {
        self.vi_normal_keymap
            .clone()
            .unwrap_or_else(Keymap::default_vi_normal)
    }

    /// Set the indentation size dynamically.
    pub fn set_indent_size(&mut self, size: usize) {
        self.indent_size = size;
    }

    /// Set a custom keymap.
    pub fn with_keymap(mut self, keymap: Keymap) -> Self {
        if self.edit_mode == EditMode::Vi {
            self.vi_insert_keymap = Some(keymap.clone());
        }
        self.keymap = keymap;
        self
    }

    /// Set a custom keymap for Vi normal mode.
    pub fn with_vi_normal_keymap(mut self, keymap: Keymap) -> Self {
        self.vi_normal_keymap = Some(keymap);
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
        if self.edit_mode == EditMode::Vi {
            self.vi_insert_keymap = Some(keymap.clone());
        }
        self.keymap = keymap;
    }

    /// Dynamically set or clear the Vi normal mode keymap.
    pub fn set_vi_normal_keymap(&mut self, keymap: Option<Keymap>) {
        self.vi_normal_keymap = keymap;
    }

    /// Read a line of input with the given prompt.
    pub fn read_line(&mut self, prompt: impl Prompt) -> io::Result<Signal> {
        let mut session = EditorSession::new();
        if let Some(menu) = &mut self.menu {
            menu.dismiss(&mut session.line_buffer);
        }

        let _guard = RawModeGuard::new(&mut self.writer)?;

        match self.edit_mode {
            EditMode::Emacs => {
                let _ = queue!(self.writer, self.cursor_style);
            }
            EditMode::Vi => {
                self.vi_mode = ViMode::Insert;
                self.keymap = self.vi_insert_keymap();
                let _ = queue!(self.writer, self.cursor_style);
            }
        }

        // Initial render
        let (layout, mut current_hint) = self.render_line(&prompt, &session.line_buffer, 0)?;
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
                            &mut session.line_buffer,
                            self.completer.as_deref(),
                        );
                        match outcome {
                            MenuOutcome::Consumed
                            | MenuOutcome::Accepted
                            | MenuOutcome::Dismissed => {
                                let (layout, hint) =
                                    self.render_line(&prompt, &session.line_buffer, previous_cursor_row)?;
                                previous_cursor_row = layout.cursor_row;
                                current_hint = hint;
                                continue;
                            }
                            MenuOutcome::PassThrough => {
                                // Menu dismissed; fall through to normal editor handling
                            }
                        }
                    }

                    // If an operator is pending in Vi mode, handle it
                    let mut operator_handled = false;
                    let mut changed = false;

                    if let Some(op) = session.vi_operator.take()
                        && let Some(op_changed) =
                            self.handle_vi_operator(op, code, modifiers, &mut session)
                    {
                        operator_handled = true;
                        changed = op_changed;
                    }

                    if !operator_handled {
                        let action = self.keymap.get(code, modifiers).copied();
                        let outcome = if let Some(action) = action {
                            self.handle_action(
                                action,
                                &prompt,
                                &mut session,
                                &mut previous_cursor_row,
                                &current_hint,
                            )?
                        } else if (self.edit_mode != EditMode::Vi || self.vi_mode == ViMode::Insert)
                            && let (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) =
                                (code, modifiers)
                        {
                            self.handle_char_insert(c, &mut session)
                        } else {
                            ActionResult::Noop
                        };

                        match outcome {
                            ActionResult::Signal(sig) => return Ok(sig),
                            ActionResult::Changed => changed = true,
                            ActionResult::Noop => {}
                        }
                    }

                    // Only redraw if the line buffer or cursor actually changed
                    if changed {
                        let (layout, hint) =
                            self.render_line(&prompt, &session.line_buffer, previous_cursor_row)?;
                        previous_cursor_row = layout.cursor_row;
                        current_hint = hint;
                    }
                }

                // Bracketed paste event
                Event::Paste(text) => {
                    if let Some(menu) = &mut self.menu {
                        menu.dismiss(&mut session.line_buffer);
                    }
                    session.save_undo();
                    session.in_insert_session = false;
                    let normalised = normalise_paste(&text);
                    if !normalised.is_empty() {
                        session.line_buffer.insert_str(&normalised);
                        let (layout, hint) =
                            self.render_line(&prompt, &session.line_buffer, previous_cursor_row)?;
                        previous_cursor_row = layout.cursor_row;
                        current_hint = hint;
                    }
                }

                // Terminal resize -> re-render line to adjust layout
                Event::Resize(..) => {
                    let (layout, hint) =
                        self.render_line(&prompt, &session.line_buffer, previous_cursor_row)?;
                    previous_cursor_row = layout.cursor_row;
                    current_hint = hint;
                }

                _ => {}
            }
        }
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
