use std::{
    env,
    io::{self, Write},
};

use which::which;

use crate::{eval::Evaluator, parser::Parser};

pub mod eval;
pub mod lexer;
pub mod parser;
pub mod paths;
pub mod ui;

fn main() {
    let mut debug: bool = false;

    if let Ok(debug_var) = env::var("DEBUG") {
        debug = matches!(debug_var.as_str(), "1");
    }

    let mut cout = io::stdout();
    let cin = io::stdin();

    let mut eval = Evaluator::new();

    // TODO: Make this an option.
    if which("eza").is_ok() {
        eval.set_alias(String::from("ls"), String::from("eza --icons"));
    }

    loop {
        ui::print_prompt(&mut cout);

        cout.flush().expect("Failed to flush");

        let mut input = String::new();

        cin.read_line(&mut input).expect("Failed to read input.");

        // If input is empty, skip all following steps as there won't be anything to execute.
        if input.is_empty() {
            continue;
        }

        let tokens = lexer::lex_input(input.trim());

        if debug {
            for token in &tokens {
                println!("{token}");
            }
        }

        let mut parser = Parser::new(tokens);

        let prog = parser.parse_program();

        if debug {
            println!("Parsing done");
        }

        eval.evaluate(prog);

        if debug {
            println!("Executing done");
        }
    }
}
