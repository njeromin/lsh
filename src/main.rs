use std::{
    env::{self, current_dir},
    fs, io,
    path::{Path, PathBuf},
};

use crossterm::{cursor, execute, terminal::SetTitle};
use dirs::home_dir;
use lsh_eval::{Evaluator, Value, builtins::fmt_cwd::format_cwd, exec_file};
use lsh_input::{DefaultHistory, DefaultValidator, Editor, Signal};
use lsh_syntax::{
    SourceMap, debug, diagnostics::print_diagnostic, lexer, parser::Parser, print_error,
};
use lsh_sys::signals::{init_interactive_terminal, reset_interrupted};
use lsh_terminal::{
    CompletionType, EditMode, LshCompleter, LshHinter, LshPrompt, Options, SyntaxHighlighter, osc,
};

const DEFAULT_PROMPT: &str = "\x1b[35m›\x1b[0m ";

fn get_prompt(eval: &mut Evaluator) -> String {
    eval.trigger_pre_prompt();

    let before = eval
        .get_var(0, "prompt")
        .and_then(|v| match v {
            Value::Object(prompt) => prompt.read().ok().and_then(|p| p.get("before").cloned()),
            _ => None,
        })
        .and_then(|v| match v {
            Value::Function(f) => Some(f),
            _ => None,
        });

    let prompt_val = eval.get_var(0, "prompt").cloned();

    if let (Some(before_fn), Some(prompt_obj)) = (before, prompt_val.as_ref()) {
        let prompt_expr = lsh_syntax::ast::Expr::Variable("prompt".to_string());
        eval.exec_function_with_receiver(
            &before_fn,
            vec![],
            Some((prompt_expr, prompt_obj.clone())),
        );
    }

    let rendered = match prompt_val {
        Some(Value::String(s)) => s,
        Some(Value::Object(_)) | None => eval.render_prompt_segments(None, None, None),
        _ => String::new(),
    };

    if rendered.is_empty() {
        DEFAULT_PROMPT.to_string()
    } else {
        rendered
    }
}

fn interactive_session(
    eval: &mut Evaluator,
    src_map: &mut SourceMap,
    config_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    init_interactive_terminal();

    let mut cerr = io::stderr();

    if let Err(e) = execute!(io::stdout(), cursor::SetCursorStyle::BlinkingBar) {
        print_error(&mut cerr, e.to_string().as_str());
    }

    let history_path = config_dir.join("history.txt");
    let history = std::sync::Arc::new(std::sync::RwLock::new(DefaultHistory::with_file(
        1000,
        history_path,
    )?));

    let mut completer = LshCompleter::new();
    completer.set_completion_registry(std::sync::Arc::clone(&eval.completion_registry));

    let hinter = LshHinter::new(completer.clone()).with_history(std::sync::Arc::clone(&history));
    let validator = DefaultValidator;
    let highlighter = SyntaxHighlighter::new();
    let initial_menu = CompletionType::Menu.create_menu(&completer);

    let mut line_editor = Editor::new()
        .with_history(std::sync::Arc::clone(&history))
        .with_formatter(highlighter)
        .with_completer(completer.clone())
        .with_hinter(hinter)
        .with_validator(validator)
        .with_menu(initial_menu)
        .with_keymap(EditMode::Default.create_keymap());

    let mut last_applied_options: Option<Options> = None;

    loop {
        reset_interrupted();

        if let Some(opts) = eval.get_var(0, "options") {
            let opts = Options::from(opts);

            if last_applied_options.as_ref() != Some(&opts) {
                line_editor.set_keymap(opts.edit_mode.create_keymap());
                line_editor.set_menu(Some(opts.completion_type.create_menu(&completer)));
                completer.set_completion_type(opts.completion_type);
                completer.set_nerd_icons(opts.nerd_icons);
                last_applied_options = Some(opts);
            }
        }

        completer.reset_prompt_marks();
        let mut commands = eval.get_aliases().keys().cloned().collect::<Vec<_>>();
        commands.extend(eval.plugin_manager.list_commands());
        completer.set_aliases(commands);
        completer.set_vars(eval.get_var_names());
        completer.refresh_binaries_if_path_changed();

        let cwd_path = current_dir().expect("Failed to get CWD");
        let formatted_cwd = format_cwd(&cwd_path, &home_dir().expect("Failed to get home dir"));
        execute!(io::stdout(), SetTitle(formatted_cwd)).expect("Failed to set tab title");

        let prompt_str = get_prompt(eval);
        let prompt = LshPrompt::new(prompt_str);

        let input = match line_editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                line.to_owned()
            }
            Ok(Signal::CtrlC) => {
                reset_interrupted();
                continue;
            }
            Ok(Signal::CtrlD) => {
                println!();
                break Ok(());
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
        eval.trigger_pre_exec(&input);
        let status = eval.evaluate(prog);
        eval.trigger_post_exec(&input, status);
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
    eval.plugin_manager
        .search_paths
        .insert(0, config_dir.join("plugins"));

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
