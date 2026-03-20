use std::{
    env,
    io::{self, Write},
    path::PathBuf,
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

fn interactive_session(eval: &mut Evaluator) -> rustyline::Result<()> {
    let mut cout = io::stdout();

    if let Err(e) = queue!(cout, cursor::SetCursorStyle::BlinkingBar) {
        informer::print_error(&mut cout, e.to_string().as_str());
    }

    // TODO: Use rustyline
    let mut editor = DefaultEditor::new()?;
    // #[cfg(feature = "with-file-history")]
    // if rl.load_history("history.txt").is_err() {
    //     println!("No previous history.");
    // }

    loop {
        ui::print_prompt(&mut cout);

        cout.flush().expect("Failed to flush");

        let mut input = String::new();

        // if let Err(e) = cin.read_line(&mut input) {
        //     informer::print_error(&mut cout, e.to_string().as_str());
        //     return Ok(());
        // }

        let readline = editor.readline("› ");

        match readline {
            Ok(line) => {
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
}

fn main() {
    let mut eval = Evaluator::new().with_globals();

    let mut args = env::args();
    args.next();

    // TODO: Improve this
    if let Some(file_path) = args.next() {
        for (i, arg) in args.enumerate() {
            eval.push_var(0, i.to_string(), Value::String(arg.to_owned()));
        }

        exec_file(&mut eval, PathBuf::from(file_path));
    } else {
        if let Err(e) = interactive_session(&mut eval) {
            informer::print_error(&mut io::stderr(), e.to_string().as_str());
        }
    }
}
