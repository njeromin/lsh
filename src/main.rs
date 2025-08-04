use std::io::{self, Write};

use crate::parser::Parser;

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

        if input == "q\n" {
            return;
        }

        let tokens = lexer::lex_input(&input);

        for token in &tokens {
            println!("{token}");
        }

        let mut parser = Parser::new(tokens);

        let prog = parser.parse_program();

        println!("Parsing done");
    }
}
