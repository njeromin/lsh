use std::{env, io};

use lsh_syntax::print_error;

use crate::{eval::Evaluator, io::IoCtx};

pub fn export<R: io::Read, W: io::Write, WE: io::Write>(
    eval: &mut Evaluator,
    io: &mut IoCtx<R, W, WE>,
    args: Vec<String>,
) -> i32 {
    if args.is_empty() {
        let mut vars: Vec<(String, String)> = env::vars().collect();
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in vars {
            let _ = writeln!(&mut io.out, "{k}={v}");
        }
        return 0;
    }

    if args.len() == 2 && !args[0].contains('=') {
        let key = &args[0];
        let val = &args[1];
        eval.export_var(key, val);
        return 0;
    }

    for arg in args {
        if let Some((key, val)) = arg.split_once('=') {
            eval.export_var(key, val);
        } else if let Some(val) = eval.get_var(0, &arg) {
            eval.export_var(&arg, &val.to_string());
        } else if let Ok(val) = env::var(&arg) {
            eval.export_var(&arg, &val);
        } else {
            print_error(&mut io.err, &format!("Variable '{arg}' not found"));
            return 1;
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{IoCtx, IoStreamMode};
    use std::io::Cursor;

    #[test]
    fn test_export_set_and_read() {
        let mut eval = Evaluator::new();
        let mut out = Cursor::new(Vec::new());
        let mut err = Cursor::new(Vec::new());
        let mut r = Cursor::new(Vec::new());
        let mut io = IoCtx {
            input: &mut r,
            out: &mut out,
            err: &mut err,
            input_mode: IoStreamMode::Inherit,
            out_mode: IoStreamMode::Inherit,
            err_mode: IoStreamMode::Inherit,
        };

        let code = export(
            &mut eval,
            &mut io,
            vec!["TEST_EXPORT_KEY=TEST_EXPORT_VAL".to_string()],
        );
        assert_eq!(code, 0);
        assert_eq!(env::var("TEST_EXPORT_KEY").unwrap(), "TEST_EXPORT_VAL");
        assert_eq!(
            eval.get_var(0, "TEST_EXPORT_KEY").unwrap().to_string(),
            "TEST_EXPORT_VAL"
        );
        unsafe {
            env::remove_var("TEST_EXPORT_KEY");
        }
    }
}
