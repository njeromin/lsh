use std::io::{self, Write};

use crate::{eval::Evaluator, parser::Parser};

pub mod eval;
pub mod lexer;
pub mod parser;

fn main() {
    let mut cout = io::stdout();
    let cin = io::stdin();

    loop {
        println!("\n~");
        print!("› ");
        cout.flush().expect("Failed to flush");

        let mut input = String::new();

        cin.read_line(&mut input).expect("Failed to read input.");

        if input.is_empty() {
            continue;
        }

        let tokens = lexer::lex_input(input.trim());

        for token in &tokens {
            println!("{token}");
        }

        let mut parser = Parser::new(tokens);

        let prog = parser.parse_program();

        println!("Parsing done");

        let mut executor = Evaluator::new();

        executor.execute(prog);

        println!("Executing done");
    }
}
