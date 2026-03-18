use std::{env, io, path::Path};

use crossterm::{
    cursor, queue,
    style::{self, Stylize},
};
use git2::Repository;

pub mod informer;

pub fn print_prompt<W: io::Write>(stdout: &mut W) {
    let cwd = env::current_dir().expect("Failed to get current working directory.");
    let cwd_str = fmt_cwd(&cwd);
    let branch = get_branch(stdout, &cwd);

    queue!(
        stdout,
        style::Print("\n"),
        style::PrintStyledContent(cwd_str.blue()),
    )
    .expect("Failed to print prompt level 1.");

    // Print decorations for prompt.
    if let Some(some_branch) = branch {
        queue!(
            stdout,
            cursor::MoveRight(1),
            style::PrintStyledContent(some_branch.green())
        )
        .expect("Failed to print prompt level 1 decoration.");
    }

    queue!(
        stdout,
        style::Print("\n"),
        style::PrintStyledContent("› ".magenta())
    )
    .expect("Failed to print prompt level 2.")
}

// TODO: Look over, might be a better way to implement.
fn get_branch<W: io::Write>(stdout: &mut W, cwd: &Path) -> Option<String> {
    if let Ok(repo) = Repository::open(cwd) {
        match repo.head() {
            Ok(head) => {
                return head.name().map(|branch_name| {
                    branch_name
                        .strip_prefix("refs/heads/")
                        .unwrap_or(branch_name)
                        .to_string()
                });
            }
            Err(e) => {
                informer::print_error(
                    stdout,
                    format!("Failed to get head of repository: {e}").as_str(),
                );
            }
        }
    }

    None
}

fn fmt_cwd(cwd: &Path) -> String {
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
