use std::{env, io, path::PathBuf};

use crate::{eval::Evaluator, paths, ui::informer};

pub fn cd<R: io::Read, W: io::Write, WE: io::Write>(
    _: &mut Evaluator,
    io: &mut lsh::IoCtx<R, W, WE>,
    args: Vec<String>,
) {
    if args.len() > 1 {
        informer::print_error(&mut io.out, "Too many arguments.");
        return;
    }

    if let Some(arg) = args.first() {
        let path = paths::expand_path(PathBuf::from(arg));

        env::set_current_dir(&path).unwrap_or_else(|_| {
            informer::print_error(
                &mut io.out,
                format!("Failed to change directory to '{arg}'.").as_str(),
            );
        });
    } else {
        informer::print_error(&mut io.out, "An argument is required.");
    }
}
