use std::{
    io::{self, Write},
    path::Path,
};

use crate::{eval::Evaluator, parser::Parser};

pub mod eval;
pub mod lexer;
pub mod parser;
pub mod ui;

const DEBUG: bool = false;

fn main() {
    let mut cout = io::stdout();
    let cin = io::stdin();

    let mut eval = Evaluator::new();
    let cwd = Path::new("~");

    loop {
        ui::print_prompt(
            &mut cout,
            cwd.to_str().expect("Failed to convert path to str."),
        );

        cout.flush().expect("Failed to flush");

        let mut input = String::new();

        cin.read_line(&mut input).expect("Failed to read input.");

        // If input is empty, skip all following steps.
        if input.is_empty() {
            continue;
        }

        let tokens = lexer::lex_input(input.trim());

        if DEBUG {
            for token in &tokens {
                println!("{token}");
            }
        }

        let mut parser = Parser::new(tokens);

        let prog = parser.parse_program();

        if DEBUG {
            println!("Parsing done");
        }

        eval.evaluate(prog);

        if DEBUG {
            println!("Executing done");
        }
    }
}
