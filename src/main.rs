use std::{
    env::{self, current_dir},
    fs, io,
    path::{Path, PathBuf},
};

use crossterm::{cursor, queue};
use lsh_eval::{Evaluator, Value, exec_file};
use lsh_syntax::{
    SourceMap, debug, diagnostics::print_diagnostic, lexer, parser::Parser, print_error,
};
use lsh_sys::signals::{init_interactive_terminal, reset_interrupted};
use lsh_terminal::{LineEditorHelper, Options, osc};
use rustyline::{
    CompletionType, Config, Editor, config::Configurer, error::ReadlineError,
    history::DefaultHistory,
};

const DEFAULT_PROMPT: &str = "\x1b[35m›\x1b[0m ";

fn get_prompt(eval: &mut Evaluator) -> &str {
    let before = eval
        .get_var(0, "prompt")
        .and_then(|v| match v {
            Value::Object(prompt) => prompt.get("before"),
            _ => None,
        })
        .and_then(|v| match v {
            Value::Function(f) => Some(f.clone()),
            _ => None,
        });

    if let Some(before_fn) = before {
        eval.exec_function(&before_fn, vec![]);
    }

    eval.get_var(0, "prompt")
        .and_then(|v| match v {
            Value::Object(prompt) => prompt.get("text"),
            _ => None,
        })
        .and_then(|v| match v {
            Value::String(text) => Some(text.as_str()),
            _ => None,
        })
        .unwrap_or(DEFAULT_PROMPT)
}

fn interactive_session(
    eval: &mut Evaluator,
    src_map: &mut SourceMap,
    config_dir: &Path,
) -> rustyline::Result<()> {
    init_interactive_terminal();

    let mut cout = io::stdout();
    let mut cerr = io::stderr();

    if let Err(e) = queue!(cout, cursor::SetCursorStyle::BlinkingBar) {
        print_error(&mut cerr, e.to_string().as_str());
    }

    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();
    let mut editor = Editor::<LineEditorHelper, DefaultHistory>::with_config(config)?;
    let mut helper = LineEditorHelper::new();
    helper.set_completion_registry(std::sync::Arc::clone(&eval.completion_registry));
    editor.set_helper(Some(helper));
    let _ = editor.load_history(&config_dir.join("history.txt"));
    let mut last_applied_options: Option<Options> = None;

    loop {
        reset_interrupted();

        if let Some(opts) = eval.get_var(0, "options").and_then(|v| match v {
            Value::Object(opts) => Some(opts),
            _ => None,
        }) {
            let opts = Options::from(opts);

            if last_applied_options.as_ref() != Some(&opts) {
                editor.set_tab_stop(opts.tab_stop);
                editor.set_edit_mode(opts.edit_mode.into());
                editor.set_completion_type(opts.completion_type.into());
                if let Some(helper) = editor.helper_mut() {
                    helper.set_completion_type(opts.completion_type);
                    helper.set_nerd_icons(opts.nerd_icons);
                }
                last_applied_options = Some(opts);
            }
        }

        osc::write_133("A");
        if let Some(helper) = editor.helper_mut() {
            helper.reset_prompt_marks();
            let aliases = eval.get_aliases().keys().cloned().collect();
            helper.set_aliases(aliases);
            helper.set_vars(eval.get_var_names());
        }

        let input = match editor.readline(get_prompt(eval)) {
            Ok(line) => {
                editor.add_history_entry(&line)?;

                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                line.to_owned()
            }
            Err(ReadlineError::Interrupted) => {
                reset_interrupted();
                continue;
            }
            Err(e) => {
                print_error(&mut cerr, e.to_string().as_str());
                continue;
            }
        };

        let src_id = src_map.insert("<input>", &input);

        let tokens = match lexer::lex_input(src_id, &input) {
            Ok(tokens) => tokens,
            Err(diag) => {
                print_diagnostic(&mut cerr, &diag, src_map);
                osc::write_133("D;1");
                let _ = src_map.remove(src_id);
                continue;
            }
        };

        debug::print_lexer(&tokens);

        let mut parser = Parser::new(src_id, tokens);
        let prog = match parser.parse_program() {
            Ok(prog) => prog,
            Err(diag) => {
                print_diagnostic(&mut cerr, &diag, src_map);
                osc::write_133("D;1");
                let _ = src_map.remove(src_id);
                continue;
            }
        };

        debug::print_parser(&prog);

        osc::write_133("C");
        let status = eval.evaluate(prog);
        osc::write_133(&format!("D;{status}"));
        let _ = src_map.remove(src_id);
    }
}

fn main() {
    let mut src_map = SourceMap::default();
    let mut eval = Evaluator::new().with_globals();

    let mut args = env::args();
    args.next(); // Skip the program name.

    // Handle initial config.
    let config_dir = if let Ok(cfg_path) = env::var("LSH_CONFIG_DIR") {
        PathBuf::from(cfg_path)
    } else {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("lsh/")
        } else {
            let current_dir = current_dir().expect("Failed to get current directory");
            Path::new(&current_dir).join("config/")
        }
    };

    let config_entrypoint = config_dir.join("config.lsh");

    // Evaluate config if it exists.
    if fs::exists(&config_entrypoint).expect("Failed to check config file existence") {
        exec_file(&mut eval, config_entrypoint);
    }

    let lib_path = config_dir.join("lib/");
    eval.set_var(
        0,
        "lib_path",
        Value::String(lib_path.to_string_lossy().into_owned()),
    )
    .expect("Failed to set library path");

    if let Some(file_path) = args.next() {
        // Push command line arguments as into global scope.
        for (i, arg) in args.enumerate() {
            eval.push_var(0, i.to_string(), Value::String(arg.to_owned()));
        }

        let code = exec_file(&mut eval, PathBuf::from(file_path));
        std::process::exit(code);
    } else if let Err(e) = interactive_session(&mut eval, &mut src_map, &config_dir) {
        print_error(&mut io::stderr(), e.to_string().as_str());
        std::process::exit(1);
    }
}
