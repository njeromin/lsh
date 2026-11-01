use std::{fs, io::stderr, path::PathBuf};

use lsh_syntax::{SourceMap, debug, lexer, parser::Parser, print_diagnostic, print_error};

use crate::eval::{EvalOutcome, Evaluator};

pub fn exec_file_outcome(evaluator: &mut Evaluator, path: PathBuf) -> Result<EvalOutcome, String> {
    let mut cerr = stderr();

    match fs::read_to_string(&path) {
        Ok(input) => {
            let mut source_map = SourceMap::default();
            let src_id = source_map.insert(path.to_string_lossy().as_ref(), input.as_str());

            let tokens = match lexer::lex_input(src_id, input.as_str()) {
                Ok(tokens) => tokens,
                Err(diag) => {
                    print_diagnostic(&mut cerr, &diag, &source_map);
                    return Err(diag.message);
                }
            };

            debug::print_lexer(&tokens);

            let mut parser = Parser::new(src_id, tokens);
            let prog = match parser.parse_program() {
                Ok(prog) => prog,
                Err(diag) => {
                    print_diagnostic(&mut cerr, &diag, &source_map);
                    return Err(diag.message);
                }
            };

            debug::print_parser(&prog);

            Ok(evaluator.evaluate_program(prog))
        }
        Err(e) => {
            let msg = e.to_string();
            print_error(&mut cerr, msg.as_str());
            Err(msg)
        }
    }
}

pub fn exec_file(evaluator: &mut Evaluator, path: PathBuf) -> i32 {
    exec_file_outcome(evaluator, path)
        .map(|outcome| outcome.last_status)
        .unwrap_or(1)
}
