use std::{fs, io::stdout, path::PathBuf};

use crate::{debug, eval::Evaluator, lexer, parser::Parser, ui::informer};

pub fn exec_file(evaluator: &mut Evaluator, path: PathBuf) {
    let mut cout = stdout();

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
            informer::print_error(&mut cout, e.to_string().as_str());
        }
    }
}
