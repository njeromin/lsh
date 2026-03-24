use std::{
    io,
    path::{Path, PathBuf},
};

use crate::{
    eval::{Evaluator, value::Value},
    paths, ui,
};

fn format_cwd(path: &Path) -> String {
    let Some(home_dir) = dirs::home_dir() else {
        return path.to_string_lossy().into_owned();
    };

    if let Ok(stripped_cwd) = path.strip_prefix(home_dir) {
        let stripped = stripped_cwd.to_string_lossy();
        if stripped.is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", stripped)
        }
    } else {
        path.to_string_lossy().into_owned()
    }
}

pub fn fmt_cwd<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut lsh::IoCtx<R, W, WE>,
    args: Vec<String>,
) {
    if args.len() > 1 {
        ui::print_error(&mut io.err, "Too many arguments.");
        return;
    }

    let path = if let Some(path_arg) = args.first() {
        paths::expand_tilde(PathBuf::from(path_arg))
    } else if let Some(Value::String(pwd)) = eval.get_var(0, "PWD") {
        PathBuf::from(pwd)
    } else {
        PathBuf::from(".")
    };

    let formatted = format_cwd(path.as_path());
    write!(&mut io.out, "{formatted}").expect("Failed to write formatted cwd");
}
