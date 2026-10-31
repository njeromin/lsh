use std::io;

use crossterm::{
    queue,
    style::{self, PrintStyledContent, Stylize},
};

pub mod editor;
pub mod highlight;
pub mod menu;
pub mod osc;
pub mod signals;

pub fn print_error<W: io::Write>(writer: &mut W, err_msg: &str) {
    queue!(
        writer,
        PrintStyledContent(err_msg.red()),
        style::Print("\n")
    )
    .expect("Failed to print error");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_error_writes_red_styled_text() {
        let mut buf = Vec::new();
        print_error(&mut buf, "test error message");
        let s = String::from_utf8_lossy(&buf);
        assert!(s.contains("test error message"));
        assert!(s.ends_with('\n'));
    }
}

