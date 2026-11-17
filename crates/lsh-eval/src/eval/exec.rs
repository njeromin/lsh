use std::{fs, io::stderr, path::PathBuf};

use lsh_syntax::{debug, lexer, parser::Parser, print_diagnostic, print_error};

use crate::eval::{EvalOutcome, Evaluator};

pub fn exec_file_outcome_scoped(
    evaluator: &mut Evaluator,
    path: PathBuf,
    scope_indices: &mut Vec<usize>,
) -> Result<EvalOutcome, String> {
    let mut cerr = stderr();

    match fs::read_to_string(&path) {
        Ok(input) => {
            let src_id = evaluator
                .source_map
                .insert(path.to_string_lossy().as_ref(), input.as_str());

            let tokens = match lexer::lex_input(src_id, input.as_str()) {
                Ok(tokens) => tokens,
                Err(diag) => {
                    print_diagnostic(&mut cerr, &diag, &evaluator.source_map);
                    let _ = evaluator.source_map.remove(src_id);
                    return Err(diag.message);
                }
            };

            debug::print_lexer(&tokens);

            let mut parser = Parser::new(src_id, tokens);
            let prog = match parser.parse_program() {
                Ok(prog) => prog,
                Err(diag) => {
                    print_diagnostic(&mut cerr, &diag, &evaluator.source_map);
                    let _ = evaluator.source_map.remove(src_id);
                    return Err(diag.message);
                }
            };

            debug::print_parser(&prog);

            evaluator.last_error_span = None;
            let outcome = evaluator
                .try_evaluate_statements(&prog.body, scope_indices)
                .map_err(|err| {
                    evaluator.report_runtime_error(&err);
                    err.to_string()
                })?;

            Ok(outcome)
        }
        Err(e) => {
            let msg = e.to_string();
            print_error(&mut cerr, msg.as_str());
            Err(msg)
        }
    }
}

pub fn exec_file_outcome(evaluator: &mut Evaluator, path: PathBuf) -> Result<EvalOutcome, String> {
    exec_file_outcome_scoped(evaluator, path, &mut vec![0])
}

pub fn exec_file(evaluator: &mut Evaluator, path: PathBuf) -> i32 {
    exec_file_outcome(evaluator, path)
        .map(|outcome| outcome.last_status)
        .unwrap_or(1)
}
