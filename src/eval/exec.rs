use std::{fs, io::stderr, path::PathBuf};

use crate::{debug, diagnostics::source_map::SourceMap, eval::Evaluator, lexer, parser::Parser, ui};

pub fn exec_file(evaluator: &mut Evaluator, path: PathBuf) -> i32 {
    let mut cerr = stderr();

    match fs::read_to_string(&path) {
        Ok(input) => {
            let mut source_map = SourceMap::default();
            let src_id = source_map.insert(path.to_string_lossy().as_ref(), input.as_str());

            let tokens = match lexer::lex_input(src_id, input.as_str()) {
                Ok(tokens) => tokens,
                Err(diag) => {
                    crate::diagnostics::print_diagnostic(&mut cerr, &diag, &source_map);
                    return 1;
                }
            };

            debug::print_lexer(&tokens);

            let mut parser = Parser::new(src_id, tokens);
            let prog = match parser.parse_program() {
                Ok(prog) => prog,
                Err(diag) => {
                    crate::diagnostics::print_diagnostic(&mut cerr, &diag, &source_map);
                    return 1;
                }
            };

            debug::print_parser(&prog);

            evaluator.evaluate(prog)
        }
        Err(e) => {
            ui::print_error(&mut cerr, e.to_string().as_str());
            1
        }
    }
}
