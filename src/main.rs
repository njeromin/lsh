use std::{
    env::{self, current_dir},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use crossterm::{cursor, queue};
use rustyline::{DefaultEditor, error::ReadlineError};

use crate::{
    eval::{Evaluator, exec::exec_file, value::Value},
    parser::Parser,
    ui::informer,
};

pub mod builtins;
pub mod debug;
pub mod eval;
pub mod lexer;
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
        .unwrap_or_else(|| DEFAULT_PROMPT)
}

fn interactive_session(eval: &mut Evaluator, config_dir: &Path) -> rustyline::Result<()> {
    let mut cout = io::stdout();

    if let Err(e) = queue!(cout, cursor::SetCursorStyle::BlinkingBar) {
        informer::print_error(&mut cout, e.to_string().as_str());
    }

    let mut editor = DefaultEditor::new()?;
    if editor
        .load_history(&config_dir.join("history.txt"))
        .is_err()
    {
        println!("No previous history.");
    }

    loop {
        print!("\n");

        let mut input = String::new();

        let readline = editor.readline(get_prompt(eval));

        match readline {
            Ok(line) => {
                editor.add_history_entry(line.as_str())?;
                input = line;
            }
            Err(e) => match e {
                ReadlineError::Interrupted => continue,
                _ => {
                    informer::print_error(&mut cout, e.to_string().as_str());
                }
            },
        }

        // If input is empty, skip to next iteration of loop as there won't be anything to execute
        // in the current one.
        if input.is_empty() {
            continue;
        }

        let tokens = lexer::lex_input(input.trim());

        debug::print_lexer(&tokens);

        let mut parser = Parser::new(tokens);
        let prog = parser.parse_program();

        debug::print_parser(&prog);

        eval.evaluate(prog);
    }

    #[cfg(feature = "with-file-history")]
    rl.save_history("history.txt");
}

fn main() {
    let mut eval = Evaluator::new().with_globals();

    let mut args = env::args();
    args.next();

    let current_dir = current_dir().expect("Failed to get current directory");

    let config_dir = Path::new(&current_dir).join("config/");
    let config_entrypoint = config_dir.join("config.lsh");

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
    } else {
        if let Err(e) = interactive_session(&mut eval, &config_dir) {
            informer::print_error(&mut io::stderr(), e.to_string().as_str());
        }
    }
}
