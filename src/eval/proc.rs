use std::{io, process::Command};

use crate::ui::informer;

pub fn spawn_proc(cmd: &str, args: Vec<String>) {
    let maybe_cmd = Command::new(cmd).args(args).spawn();

    match maybe_cmd {
        Ok(mut cmd) => match cmd.wait() {
            Ok(_) => {}
            Err(e) => informer::print_error(&mut io::stdout(), e.to_string().as_str()),
        },
        Err(e) => informer::print_error(&mut io::stdout(), e.to_string().as_str()),
    }
}
