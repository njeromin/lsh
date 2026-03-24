use std::{fs, io::stderr, path::PathBuf};

use crate::{debug, diagnostics, eval::Evaluator, lexer, parser::Parser, ui};

pub fn exec_file(evaluator: &mut Evaluator, path: PathBuf) {
    let mut cerr = stderr();

    match fs::read_to_string(&path) {
        Ok(input) => {
            let tokens =
                match lexer::lex_input_with_source(path.to_string_lossy().as_ref(), input.as_str())
                {
                    Ok(tokens) => tokens,
                    Err(diag) => {
                        diagnostics::print_diagnostic(&mut cerr, &diag);
                        return;
                    }
                };

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
