use std::{fs, io::stderr, path::PathBuf};

use crate::{debug, eval::Evaluator, lexer, parser::Parser, ui};

pub fn exec_file(evaluator: &mut Evaluator, path: PathBuf) {
    let mut cerr = stderr();

    match fs::read_to_string(path) {
        Ok(input) => {
            let tokens = lexer::lex_input(input.as_str());

            debug::print_lexer(&tokens);

            let mut parser = Parser::new(tokens);
            let prog = parser.parse_program();

            debug::print_parser(&prog);

            evaluator.evaluate(prog);
        }
        Err(e) => {
            ui::print_error(&mut cerr, e.to_string().as_str());
        }
    }
}
