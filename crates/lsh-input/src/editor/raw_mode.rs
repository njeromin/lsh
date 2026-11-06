use crossterm::{
    cursor,
    event::{self, DisableBracketedPaste, EnableBracketedPaste},
    queue, terminal,
};
use std::io::{self, Write};

/// RAII guard to ensure terminal raw mode is disabled when exiting `read_line`,
/// even in the case of errors or panics.
pub(crate) struct RawModeGuard {
    enhanced_keyboard: bool,
}

impl RawModeGuard {
    pub(crate) fn new<W: Write>(writer: &mut W) -> io::Result<Self> {
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
