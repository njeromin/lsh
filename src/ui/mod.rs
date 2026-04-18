use std::io;

use crossterm::{
    queue,
    style::{self, PrintStyledContent, Stylize},
};

pub fn print_error<W: io::Write>(stdout: &mut W, err_msg: &str) {
    queue!(
        stdout,
        PrintStyledContent(err_msg.red()),
        style::Print("\n")
    )
    .expect("Failed to print error");
}
