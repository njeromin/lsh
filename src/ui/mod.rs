use std::{io::Stdout, path::Path};

use crossterm::{
    cursor, queue,
    style::{self, Stylize},
};

pub mod informer;

pub fn print_prompt(stdout: &mut Stdout, cwd_str: &str) {
    queue!(
        stdout,
        cursor::MoveToNextLine(1),
        style::PrintStyledContent(cwd_str.cyan()),
        cursor::MoveToNextLine(1),
        style::PrintStyledContent("› ".magenta())
    )
    .expect("Failed to print prompt");
}
