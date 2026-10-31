use std::{
    env, io,
    path::{Path, PathBuf},
};

use crate::{
    eval::{Evaluator, value::Value},
    paths, terminal,
};

fn format_cwd(path: &Path, home_dir: &Path) -> String {
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

fn resolve_home_dir(eval: &Evaluator) -> Option<PathBuf> {
    if let Some(Value::String(home)) = eval.get_var(0, "HOME")
        && !home.is_empty()
    {
        return Some(PathBuf::from(home));
    }

    if let Ok(home) = env::var("HOME")
        && !home.is_empty()
    {
        return Some(PathBuf::from(home));
    }

    dirs::home_dir()
}

pub fn fmt_cwd<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut lsh::IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if args.len() > 1 {
        terminal::print_error(&mut io.err, "Too many arguments.");
        return 1;
    }

    let path = if let Some(path_arg) = args.first() {
        paths::expand_tilde(PathBuf::from(path_arg))
    } else if let Some(Value::String(pwd)) = eval.get_var(0, "PWD") {
        PathBuf::from(pwd)
    } else {
        PathBuf::from(".")
    };

    let formatted = if let Some(home_dir) = resolve_home_dir(eval) {
        format_cwd(path.as_path(), home_dir.as_path())
    } else {
        path.to_string_lossy().into_owned()
    };
    if let Err(e) = write!(&mut io.out, "{formatted}")
        && e.kind() != io::ErrorKind::BrokenPipe
    {
        terminal::print_error(&mut io.err, format!("Failed to write formatted cwd: {e}").as_str());
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::format_cwd;

    #[test]
    fn formats_home_path_as_tilde() {
        let formatted = format_cwd(Path::new("/home/njeromin"), Path::new("/home/njeromin"));
        assert_eq!(formatted, "~");
    }

    #[test]
    fn keeps_non_home_paths_unchanged() {
        let formatted = format_cwd(Path::new("/tmp"), Path::new("/home/njeromin"));
        assert_eq!(formatted, "/tmp");
    }
}
