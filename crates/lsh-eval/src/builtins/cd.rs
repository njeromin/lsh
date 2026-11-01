use std::{
    env, io,
    path::{Component, Path, PathBuf},
};

use lsh_syntax::{paths, print_error};

use crate::{
    eval::{Evaluator, value::Value},
    io::IoCtx,
};

fn normalize_logical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let is_absolute = path.is_absolute();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped = normalized.pop();
                if !popped && !is_absolute {
                    normalized.push(component.as_os_str());
                }
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }

    // Never allow an absolute path normalization to lose its root.
    if is_absolute && !normalized.is_absolute() {
        return PathBuf::from("/");
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

pub fn cd<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if args.len() > 1 {
        print_error(&mut io.err, "Too many arguments.");
        return 1;
    }

    if let Some(arg) = args.first() {
        let current_logical_pwd = if let Some(Value::String(pwd)) = eval.get_var(0, "PWD") {
            PathBuf::from(pwd)
        } else {
            env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        };

        let expanded_arg = paths::expand_tilde(PathBuf::from(arg));
        let requested_path = if expanded_arg.is_absolute() {
            expanded_arg
        } else {
            current_logical_pwd.join(expanded_arg)
        };
        let logical_target = normalize_logical_path(&requested_path);

        if env::set_current_dir(&logical_target).is_err() {
            print_error(
                &mut io.err,
                format!("Failed to change directory to '{arg}'.").as_str(),
            );
            return 1;
        }

        eval.push_var(
            0,
            String::from("PWD"),
            Value::String(logical_target.to_string_lossy().into_owned()),
        );
        eval.trigger_chpwd(
            &current_logical_pwd.to_string_lossy(),
            &logical_target.to_string_lossy(),
        );
        0
    } else {
        print_error(&mut io.err, "An argument is required.");
        1
    }
}
