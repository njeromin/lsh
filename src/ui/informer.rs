use std::io::{Stdout, Write};

use crossterm::{
    cursor, queue,
    style::{PrintStyledContent, Stylize},
};

pub fn print_error(stdout: &mut Stdout, err_msg: &str) {
    queue!(
        stdout,
        PrintStyledContent(err_msg.red()),
        cursor::MoveToNextLine(1)
    )
    .expect("Failed to print error");
}
