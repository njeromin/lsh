use std::{
    env::{self, current_dir},
    fs, io,
    path::{Path, PathBuf},
};

use crossterm::{cursor, queue};
use rustyline::{DefaultEditor, config::Configurer, error::ReadlineError};

use crate::{
    diagnostics::source_map::SourceMap,
    eval::{Evaluator, exec::exec_file, value::Value},
    options::Options,
    parser::Parser,
};

pub mod builtins;
pub mod debug;
pub mod diagnostics;
pub mod eval;
pub mod expand;
pub mod lexer;
pub mod options;
pub mod parser;
pub mod paths;
pub mod ui;

const DEFAULT_PROMPT: &str = "\x1b[35m›\x1b[0m ";

fn get_prompt(eval: &mut Evaluator) -> &str {
    let before = eval
        .get_var(0, "prompt")
        .and_then(|v| match v {
            Value::Object(prompt) => prompt.get("before"),
            _ => None,
        })
        .and_then(|v| match v {
            Value::Function(f) => Some(f.clone()),
            _ => None,
        });

    if let Some(before_fn) = before {
        eval.exec_function(&before_fn, vec![]);
    }

    eval.get_var(0, "prompt")
        .and_then(|v| match v {
            Value::Object(prompt) => prompt.get("text"),
            _ => None,
        })
        .and_then(|v| match v {
            Value::String(text) => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or(DEFAULT_PROMPT)
}

fn interactive_session(
    eval: &mut Evaluator,
    src_map: &mut SourceMap,
    config_dir: &Path,
) -> rustyline::Result<()> {
    let mut cout = io::stdout();
    let mut cerr = io::stderr();

    if let Err(e) = queue!(cout, cursor::SetCursorStyle::BlinkingBar) {
        ui::print_error(&mut cerr, e.to_string().as_str());
    }

    let mut editor = DefaultEditor::new()?;
    let _ = editor.load_history(&config_dir.join("history.txt"));
    let mut last_applied_options: Option<Options> = None;

    loop {
        if let Some(opts) = eval.get_var(0, "options").and_then(|v| match v {
            Value::Object(opts) => Some(opts),
            _ => None,
        }) {
            let opts = Options::from(opts);

            if last_applied_options.as_ref() != Some(&opts) {
                editor.set_tab_stop(opts.tab_stop);
                editor.set_edit_mode(opts.edit_mode.into());
                last_applied_options = Some(opts);
            }
        }

        let input = match editor.readline(get_prompt(eval)) {
            Ok(line) => {
                editor.add_history_entry(&line)?;

                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                line.to_owned()
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(e) => {
                ui::print_error(&mut cerr, e.to_string().as_str());
                continue;
            }
        };

        let src_id = src_map.insert("<input>", &input);

        let tokens = match lexer::lex_input(src_id, &input) {
            Ok(tokens) => tokens,
            Err(diag) => {
                diagnostics::print_diagnostic(&mut cerr, &diag, src_map);
                continue;
            }
        };

        debug::print_lexer(&tokens);

        let mut parser = Parser::new(src_id, tokens);
        let prog = match parser.parse_program() {
            Ok(prog) => prog,
            Err(diag) => {
                diagnostics::print_diagnostic(&mut cerr, &diag, src_map);
                continue;
            }
        };

        debug::print_parser(&prog);

        eval.evaluate(prog);
    }
}

fn main() {
    let mut src_map = SourceMap::default();
    let mut eval = Evaluator::new().with_globals();

    let mut args = env::args();
    args.next(); // Skip the program name.

    // Handle initial config.
    let config_dir = if let Ok(cfg_path) = env::var("LSH_CONFIG_DIR") {
        PathBuf::from(cfg_path)
    } else {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("lsh/")
        } else {
            let current_dir = current_dir().expect("Failed to get current directory");
            Path::new(&current_dir).join("config/")
        }
    };

    let config_entrypoint = config_dir.join("config.lsh");

    // Run config.lsh.
    if fs::exists(&config_entrypoint).expect("Failed to check config file existence") {
        exec_file(&mut eval, config_entrypoint);
    }

    let lib_path = config_dir.join("lib/");
    eval.set_var(
        0,
        "lib_path",
        Value::String(lib_path.to_string_lossy().into_owned()),
    )
    .expect("Failed to set library path");

    // TODO: Improve this
    if let Some(file_path) = args.next() {
        for (i, arg) in args.enumerate() {
            eval.push_var(0, i.to_string(), Value::String(arg.to_owned()));
        }

        exec_file(&mut eval, PathBuf::from(file_path));
    } else if let Err(e) = interactive_session(&mut eval, &mut src_map, &config_dir) {
        ui::print_error(&mut io::stderr(), e.to_string().as_str());
    }
}
