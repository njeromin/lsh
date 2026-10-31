use std::io;

use crate::{
    eval::Evaluator,
    terminal::{
        self,
        completion::{CandidateSpec, CompletionRule},
        menu::CompletionKind,
    },
};

pub fn complete<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut lsh::IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    let subcmd = match args.first() {
        Some(s) => s.as_str(),
        None => {
            terminal::print_error(&mut io.err, "A subcommand (set/list/remove) is required.");
            return 1;
        }
    };

    match subcmd {
        "list" => {
            let registry = eval.completion_registry.read().unwrap();
            let entries = registry.list();
            for (path, rule) in entries {
                let mut parts = Vec::new();
                if rule.dirs_only {
                    parts.push("[dirs]".to_string());
                }
                for ext in &rule.file_extensions {
                    parts.push(format!("[ext: {ext}]"));
                }
                if let Some(cmd) = &rule.exec {
                    parts.push(format!("[exec: {cmd}]"));
                }
                for sub in rule.subcommands.keys() {
                    parts.push(sub.clone());
                }
                for cand in &rule.candidates {
                    if let Some(desc) = &cand.description {
                        parts.push(format!("{}:{}", cand.replacement, desc));
                    } else {
                        parts.push(cand.replacement.clone());
                    }
                }
                if !parts.is_empty() {
                    let _ = writeln!(&mut io.out, "{path}: {}", parts.join(", "));
                } else {
                    let _ = writeln!(&mut io.out, "{path}");
                }
            }
            0
        }
        "remove" => {
            let cmd_path = match args.get(1) {
                Some(p) => p.as_str(),
                None => {
                    terminal::print_error(&mut io.err, "A command name is required.");
                    return 1;
                }
            };
            let mut registry = eval.completion_registry.write().unwrap();
            if registry.remove(cmd_path) {
                0
            } else {
                terminal::print_error(
                    &mut io.err,
                    &format!("No completions found for '{cmd_path}'."),
                );
                1
            }
        }
        "set" => {
            let cmd_path = match args.get(1) {
                Some(p) => p.as_str(),
                None => {
                    terminal::print_error(&mut io.err, "A command name is required.");
                    return 1;
                }
            };

            let mut candidates = Vec::new();
            let mut exec = None;
            let mut dirs_only = false;
            let mut file_extensions = Vec::new();

            let mut idx = 2;
            while idx < args.len() {
                let arg = &args[idx];
                if arg == "--dirs" {
                    dirs_only = true;
                    idx += 1;
                } else if arg == "--exec" {
                    if let Some(exec_cmd) = args.get(idx + 1) {
                        exec = Some(exec_cmd.clone());
                        idx += 2;
                    } else {
                        terminal::print_error(
                            &mut io.err,
                            "A command string is required after --exec.",
                        );
                        return 1;
                    }
                } else if arg == "--ext" {
                    if let Some(ext) = args.get(idx + 1) {
                        file_extensions.push(ext.clone());
                        idx += 2;
                    } else {
                        terminal::print_error(
                            &mut io.err,
                            "A file extension is required after --ext.",
                        );
                        return 1;
                    }
                } else {
                    candidates.push(CandidateSpec::from_raw(arg, CompletionKind::Value));
                    idx += 1;
                }
            }

            let mut registry = eval.completion_registry.write().unwrap();
            let mut rule = CompletionRule::new();
            rule.candidates = candidates;
            rule.exec = exec;
            rule.dirs_only = dirs_only;
            rule.file_extensions = file_extensions;

            registry.register(cmd_path, rule);
            0
        }
        _ => {
            terminal::print_error(&mut io.err, "A subcommand (set/list/remove) is required.");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn complete_builtin_set_list_remove() {
        let mut eval = Evaluator::new();
        let mut out = Cursor::new(Vec::new());
        let mut err = Cursor::new(Vec::new());
        let mut r = Cursor::new(Vec::new());
        let mut io = lsh::IoCtx {
            input: &mut r,
            out: &mut out,
            err: &mut err,
            input_mode: lsh::IoStreamMode::Inherit,
            out_mode: lsh::IoStreamMode::Inherit,
            err_mode: lsh::IoStreamMode::Inherit,
        };

        // complete set foo alpha beta:Second
        let res = complete(
            &mut eval,
            &mut io,
            vec![
                "set".to_string(),
                "foo".to_string(),
                "alpha".to_string(),
                "beta:Second".to_string(),
            ],
        );
        assert_eq!(res, 0);

        // complete list
        let mut list_r = Cursor::new(Vec::new());
        let mut list_out = Cursor::new(Vec::new());
        let mut list_err = Cursor::new(Vec::new());
        let mut list_io = lsh::IoCtx {
            input: &mut list_r,
            out: &mut list_out,
            err: &mut list_err,
            input_mode: lsh::IoStreamMode::Inherit,
            out_mode: lsh::IoStreamMode::Inherit,
            err_mode: lsh::IoStreamMode::Inherit,
        };
        let res = complete(&mut eval, &mut list_io, vec!["list".to_string()]);
        assert_eq!(res, 0);
        let out_str = String::from_utf8_lossy(list_out.get_ref());
        assert!(out_str.contains("foo: alpha, beta:Second"));

        // complete remove foo
        let res = complete(
            &mut eval,
            &mut io,
            vec!["remove".to_string(), "foo".to_string()],
        );
        assert_eq!(res, 0);
    }
}
