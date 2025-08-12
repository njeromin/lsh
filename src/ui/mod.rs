use std::{env, io::Stdout, path::PathBuf};

use crossterm::{
    cursor, queue,
    style::{self, Stylize},
};

pub mod informer;

pub fn print_prompt(stdout: &mut Stdout) {
    let cwd = env::current_dir().expect("Failed to get current working directory.");
    let cwd_str = fmt_cwd(cwd);

    queue!(
        stdout,
        cursor::MoveToNextLine(1),
        style::PrintStyledContent(cwd_str.cyan()),
        cursor::MoveToNextLine(1),
        style::PrintStyledContent("› ".magenta())
    )
    .expect("Failed to print prompt");
}

fn fmt_cwd(cwd: PathBuf) -> String {
    let home_dir = dirs::home_dir().expect("Failed to get home directory.");

    // TODO: Look at this again what have I written.
    if let Ok(stripped_cwd) = cwd.strip_prefix(home_dir) {
        let stripped_cwd_str = stripped_cwd
            .to_str()
            .expect("Failed to convert stripped current working directory to string.");

        let mut tilded_value = String::from("~");

        if !stripped_cwd_str.is_empty() {
            tilded_value.push('/');
            tilded_value.push_str(stripped_cwd_str);
        }

        return tilded_value;
    }

    let cwd_str = cwd
        .to_str()
        .expect("Failed to convert current working directory to string.");

    cwd_str.to_owned()
}
