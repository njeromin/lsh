use std::io::Stdout;

use crossterm::{
    queue,
    style::{PrintStyledContent, Stylize},
};

pub fn print_error(stdout: &mut Stdout, err_msg: &str) {
    queue!(stdout, PrintStyledContent(err_msg.red())).expect("Ts broken");
}
