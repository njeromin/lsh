
use super::*;
use crate::{IoCtx, IoStreamMode, JobState};
use lsh_syntax::{
    ast::{CommandExpr, Expr, Statement},
    lexer,
    parser::Parser,
};
use lsh_sys::signals::STATUS_SIGINT;
use std::{io, sync::atomic::AtomicBool};

fn eval_str(code: &str) -> i32 {
    let tokens = lexer::lex_input(0, code).expect("lex error");
    let mut parser = Parser::new(0, tokens);
    let prog = parser.parse_program().expect("parse error");
    let mut eval = Evaluator::new()
        .with_globals()
        .with_interrupt_flag(Arc::new(AtomicBool::new(false)));
    eval.evaluate(prog)
}

#[test]
fn cd_failure_returns_non_zero_and_stops_and_chain() {
    let status = eval_str("cd /nonexistent_directory_12345 && cd /");
    assert_eq!(status, 1);
}

#[test]
fn pipeline_streaming_returns_last_stage_exit_code() {
    let status = eval_str("echo 'test stream' | cat");
    assert_eq!(status, 0);
}

#[test]
fn path_assignment_updates_process_environment() {
    let orig_path = env::var("PATH").unwrap_or_default();
    let temp_dir = std::env::temp_dir().join("lsh_test_path_sync");
    let _ = std::fs::create_dir_all(&temp_dir);

    let code = format!(r#"$PATH = "{}:" + $PATH"#, temp_dir.display());
    let (eval, status) = eval_with_evaluator(&code);
    assert_eq!(status, 0);

    let current_env_path = env::var("PATH").unwrap_or_default();
    assert!(current_env_path.starts_with(&format!("{}:", temp_dir.display())));
    assert_eq!(
        eval.get_var(0, "PATH").unwrap().to_string(),
        current_env_path
    );

    // Restore original path
    unsafe {
        env::set_var("PATH", orig_path);
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn eval_var(code: &str, var_name: &str) -> Option<Value> {
    let (eval, _) = eval_with_evaluator(code);
    eval.scopes
        .first()
        .and_then(|s| s.vars.get(var_name).cloned())
}

fn eval_with_evaluator(code: &str) -> (Evaluator, i32) {
    let tokens = lexer::lex_input(0, code).expect("lex error");
    let mut parser = Parser::new(0, tokens);
    let prog = parser.parse_program().expect("parse error");
    let mut eval = Evaluator::new()
        .with_globals()
        .with_interrupt_flag(Arc::new(AtomicBool::new(false)));
    let status = eval.evaluate(prog);
    (eval, status)
}

#[test]
fn signal_interrupt_aborts_evaluation_with_status_130() {
    let code = r#"
            let reached_first = 0
            let reached_second = 0
            $reached_first = 1
            $reached_second = 2
        "#;

    let tokens = lexer::lex_input(0, code).expect("lex error");
    let mut parser = Parser::new(0, tokens);
    let prog = parser.parse_program().expect("parse error");
    let flag = Arc::new(AtomicBool::new(true));
    let mut eval = Evaluator::new().with_globals().with_interrupt_flag(flag);
    let status = eval.evaluate(prog);

    assert_eq!(status, STATUS_SIGINT);
    assert_eq!(
        eval.scopes
            .first()
            .and_then(|s| s.vars.get("reached_second")),
        None
    );
}

#[test]
fn signal_interrupt_aborts_while_loop_with_status_130() {
    let code = r#"
            let count = 0
            while true {
                $count = 1
            }
        "#;

    let tokens = lexer::lex_input(0, code).expect("lex error");
    let mut parser = Parser::new(0, tokens);
    let prog = parser.parse_program().expect("parse error");
    let flag = Arc::new(AtomicBool::new(true));
    let mut eval = Evaluator::new().with_globals().with_interrupt_flag(flag);
    let status = eval.evaluate(prog);

    assert_eq!(status, STATUS_SIGINT);
}

#[test]
fn command_sub_on_user_function_returns_object_with_out_and_code() {
    let code = r#"
            fn add(a, b) {
                return $a + $b
            }
            let res = $(add 2 3)
            let out_val = $(add 2 3).out
            let code_val = $(add 2 3).code
        "#;
    assert_eq!(
        eval_var(code, "out_val"),
        Some(Value::String("5".to_string()))
    );
    assert_eq!(eval_var(code, "code_val"), Some(Value::Number(0.0)));
}

#[test]
fn string_multiplication_is_commutative_and_safe() {
    let code = r#"
            let a = "ab" * 3
            let b = 3 * "ab"
            let zero = "ab" * 0
            let neg = "ab" * -2
            let huge = "ab" * 10000000
        "#;
    assert_eq!(
        eval_var(code, "a"),
        Some(Value::String("ababab".to_string()))
    );
    assert_eq!(
        eval_var(code, "b"),
        Some(Value::String("ababab".to_string()))
    );
    assert_eq!(eval_var(code, "zero"), Some(Value::String(String::new())));
    assert_eq!(eval_var(code, "neg"), Some(Value::String(String::new())));
    assert_eq!(eval_var(code, "huge"), Some(Value::Null));
}

#[test]
fn division_by_zero_returns_null() {
    let code = r#"
            let ok = 10 / 2
            let zero = 10 / 0
        "#;
    assert_eq!(eval_var(code, "ok"), Some(Value::Number(5.0)));
    assert_eq!(eval_var(code, "zero"), Some(Value::Null));
}

#[test]
fn null_equality_in_evaluator_conditions() {
    let code = r#"
            let matched_null_first = 0
            let matched_empty_first = 0
            if $missing == "" {
                $matched_null_first = 1
            }
            if "" == $missing {
                $matched_empty_first = 1
            }
        "#;
    assert_eq!(
        eval_var(code, "matched_null_first"),
        Some(Value::Number(1.0))
    );
    assert_eq!(
        eval_var(code, "matched_empty_first"),
        Some(Value::Number(1.0))
    );
}

#[test]
fn build_command_args_expands_unquoted_wildcards_and_preserves_quotes() {
    let mut eval = Evaluator::new().with_interrupt_flag(Arc::new(AtomicBool::new(false)));
    let mut scopes = vec![0];

    // Unquoted wildcard: echo src/*.rs
    let unquoted_code = "echo src/*.rs";
    let tokens = lexer::lex_input(0, unquoted_code).unwrap();
    let mut parser = Parser::new(0, tokens);
    let prog = parser.parse_program().unwrap();
    let call_args = match &prog.body[0] {
        Statement::Command(CommandExpr::Call { args, .. }) => args,
        Statement::Expr(Expr::Call { args, .. }) => args,
        _ => panic!("expected call-like statement"),
    };
    let args = eval.build_command_args(vec![], call_args, &mut scopes);
    assert!(args.contains(&"src/lib.rs".to_string()));
    assert!(args.iter().any(|a| a.ends_with(".rs")));

    // Quoted wildcard: echo "src/*.rs"
    let quoted_code = "echo \"src/*.rs\"";
    let tokens = lexer::lex_input(0, quoted_code).unwrap();
    let mut parser = Parser::new(0, tokens);
    let prog = parser.parse_program().unwrap();
    let call_args = match &prog.body[0] {
        Statement::Command(CommandExpr::Call { args, .. }) => args,
        Statement::Expr(Expr::Call { args, .. }) => args,
        _ => panic!("expected call-like statement"),
    };
    let args = eval.build_command_args(vec![], call_args, &mut scopes);
    assert_eq!(args, vec!["src/*.rs".to_string()]);

    // Nonexistent wildcard fallback
    let fallback_code = "echo nonexistent_*.xyz";
    let tokens = lexer::lex_input(0, fallback_code).unwrap();
    let mut parser = Parser::new(0, tokens);
    let prog = parser.parse_program().unwrap();
    let call_args = match &prog.body[0] {
        Statement::Command(CommandExpr::Call { args, .. }) => args,
        Statement::Expr(Expr::Call { args, .. }) => args,
        _ => panic!("expected call-like statement"),
    };
    let args = eval.build_command_args(vec![], call_args, &mut scopes);
    assert_eq!(args, vec!["nonexistent_*.xyz".to_string()]);
}

#[test]
fn method_call_binds_this_and_reads_properties() {
    let res = eval_var(
        r#"
            let person = {
                name = "Alice",
                greet = () => {
                    return "Hello, " + $this.name
                }
            }
            let msg = $person.greet()
            "#,
        "msg",
    );
    assert_eq!(res, Some(Value::String("Hello, Alice".to_string())));
}

#[test]
fn method_call_mutates_this_and_persists_to_receiver() {
    let res = eval_var(
        r#"
            let counter = {
                count = 0,
                inc = () => {
                    $this.count = $this.count + 1
                    return $this.count
                }
            }
            let first = $counter.inc()
            let second = $counter.inc()
            let final_count = $counter.count
            "#,
        "final_count",
    );
    assert_eq!(res, Some(Value::Number(2.0)));
}

#[test]
fn command_style_method_call_binds_this_and_args() {
    let res = eval_var(
        r#"
            let greeter = {
                prefix = "Hi",
                greet = (target) => {
                    return $this.prefix + " " + $target
                }
            }
            let msg = $(greeter.greet "Bob").out
            "#,
        "msg",
    );
    assert_eq!(res, Some(Value::String("Hi Bob".to_string())));
}

#[test]
fn nested_method_calls_on_this() {
    let res = eval_var(
        r#"
            let counter = {
                count = 0,
                inc = () => {
                    $this.count = $this.count + 1
                    return $this.count
                },
                add_two = () => {
                    $this.inc()
                    $this.inc()
                    return $this.count
                }
            }
            $counter.add_two()
            let final_count = $counter.count
            "#,
        "final_count",
    );
    assert_eq!(res, Some(Value::Number(2.0)));
}

#[test]
fn nested_object_method_call_updates_nested_property() {
    let res = eval_var(
        r#"
            let user = {
                stats = {
                    score = 10,
                    add = (pts) => {
                        $this.score = $this.score + $pts
                    }
                }
            }
            $user.stats.add 15
            let final_score = $user.stats.score
            "#,
        "final_score",
    );
    assert_eq!(res, Some(Value::Number(25.0)));
}

#[test]
fn fn_declaration_on_object_binds_this() {
    let res = eval_var(
        r#"
            let obj = { x = 42 }
            fn obj.get_x() {
                return $this.x
            }
            let val = $obj.get_x()
            "#,
        "val",
    );
    assert_eq!(res, Some(Value::Number(42.0)));
}

#[test]
fn object_variables_share_reference_mutations() {
    let res = eval_var(
        r#"
            let a = { count = 1 }
            let b = $a
            $b.count = 42
            let res = $a.count
            "#,
        "res",
    );
    assert_eq!(res, Some(Value::Number(42.0)));
}

#[test]
fn function_mutates_passed_object_reference() {
    let res = eval_var(
        r#"
            fn mutate_obj(target) {
                $target.modified = true
            }
            let my_obj = { modified = false }
            mutate_obj $my_obj
            let res = $my_obj.modified
            "#,
        "res",
    );
    assert_eq!(res, Some(Value::Bool(true)));
}

#[test]
fn object_reference_equality_in_evaluator() {
    let res = eval_var(
        r#"
            let a = { x = 1 }
            let b = $a
            let c = { x = 1 }
            let same_ref = $a == $b
            "#,
        "same_ref",
    );
    assert_eq!(res, Some(Value::Bool(true)));

    let res_deep = eval_var(
        r#"
            let a = { x = 1 }
            let c = { x = 1 }
            let deep_eq = $a == $c
            "#,
        "deep_eq",
    );
    assert_eq!(res_deep, Some(Value::Bool(true)));
}

fn write_test_plugin(plugin_name: &str, content: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let temp_dir = std::env::temp_dir().join(format!(
        "lsh_test_{}_{}_{}",
        plugin_name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let plugin_dir = temp_dir.join(plugin_name);
    let _ = std::fs::create_dir_all(&plugin_dir);
    let plugin_file = plugin_dir.join("main.lsh");
    std::fs::write(&plugin_file, content).unwrap();
    (temp_dir, plugin_file)
}

#[test]
fn plugin_return_and_command_execution_with_this() {
    let (temp_dir, _) = write_test_plugin(
        "test-plugin",
        r#"
            return {
                name = "test-plugin",
                version = "1.0.0",
                description = "Test plugin",
                prefix = ">>",
                commands = {
                    echo_prefixed = (msg) => {
                        return $this.prefix + " " + $msg
                    }
                }
            }
            "#,
    );

    let (mut eval, _) = eval_with_evaluator("");
    eval.plugin_manager.search_paths.insert(0, temp_dir.clone());
    let res = eval.load_plugin("test-plugin");
    assert_eq!(res, Ok("test-plugin".to_string()));

    let _ = eval.evaluate_statements(
        &lsh_syntax::parser::Parser::new(
            0,
            lsh_syntax::lexer::lex_input(0, "let out = $(echo_prefixed \"hello\").out").unwrap(),
        )
        .parse_program()
        .unwrap()
        .body,
        &mut vec![0],
    );

    let out = eval.scopes.first().and_then(|s| s.vars.get("out").cloned());
    assert_eq!(out, Some(Value::String(">> hello".to_string())));
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn plugin_lifecycle_hooks_mutate_plugin_state() {
    let (temp_dir, _) = write_test_plugin(
        "git-plugin",
        r#"
            return {
                name = "git-plugin",
                hooks = {
                    on_chpwd = (old_p, new_p) => {
                        $this.current_dir = $new_p
                    },
                    on_pre_prompt = () => {
                        $this.prompt_runs = 1
                    }
                },
                current_dir = "",
                prompt_runs = 0
            }
            "#,
    );

    let (mut eval, _) = eval_with_evaluator("");
    eval.plugin_manager.search_paths.insert(0, temp_dir.clone());
    assert_eq!(eval.load_plugin("git-plugin"), Ok("git-plugin".to_string()));

    eval.trigger_chpwd("/home/user", "/home/user/projects");
    eval.trigger_pre_prompt();

    let plugin = eval
        .plugin_manager
        .get("git-plugin")
        .expect("plugin not found");
    let current_dir = plugin.manifest.get_property("current_dir");
    let prompt_runs = plugin.manifest.get_property("prompt_runs");

    assert_eq!(
        current_dir,
        Some(Value::String("/home/user/projects".to_string()))
    );
    assert_eq!(prompt_runs, Some(Value::Number(1.0)));
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn plugin_unload_removes_commands_and_hooks() {
    let (temp_dir, _) = write_test_plugin(
        "temp-plugin",
        r#"
            return {
                name = "temp-plugin",
                commands = {
                    temp_cmd = () => "temp"
                }
            }
            "#,
    );

    let (mut eval, _) = eval_with_evaluator("");
    eval.plugin_manager.search_paths.insert(0, temp_dir.clone());
    assert_eq!(
        eval.load_plugin("temp-plugin"),
        Ok("temp-plugin".to_string())
    );

    assert!(eval.plugin_manager.get("temp-plugin").is_some());
    assert!(eval.plugin_manager.get_command("temp_cmd").is_some());

    assert!(eval.unload_plugin("temp-plugin"));
    assert!(eval.plugin_manager.get("temp-plugin").is_none());
    assert!(eval.plugin_manager.get_command("temp_cmd").is_none());
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn plugin_load_from_file_registers_manifest() {
    let (temp_dir, plugin_file) = write_test_plugin(
        "file-plugin",
        r#"
            return {
                name = "file-plugin",
                version = "2.0.0",
                commands = {
                    file_cmd = () => "from-file"
                }
            }
            "#,
    );

    let (mut eval, _) = eval_with_evaluator("");
    let load_res = eval.load_plugin(plugin_file.to_str().unwrap());
    assert_eq!(load_res, Ok("file-plugin".to_string()));

    let plugin = eval.plugin_manager.get("file-plugin");
    assert!(plugin.is_some());
    assert_eq!(plugin.unwrap().version.as_deref(), Some("2.0.0"));
    assert!(eval.plugin_manager.get_command("file_cmd").is_some());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn plugin_fails_to_load_if_not_returning_object() {
    let (temp_dir, plugin_file) = write_test_plugin(
        "invalid-plugin",
        r#"
            let plugin = {
                name = "invalid-plugin"
            }
            "#,
    );

    let (mut eval, _) = eval_with_evaluator("");
    let load_res = eval.load_plugin(plugin_file.to_str().unwrap());
    assert!(load_res.is_err());
    assert!(
        load_res
            .unwrap_err()
            .contains("must return an object manifest")
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn loads_git_prompt_plugin_from_config_directory() {
    let (mut eval, _) = eval_with_evaluator("");

    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("config")
        .join("plugins");

    eval.plugin_manager
        .search_paths
        .insert(0, manifest_path.clone());

    let load_res = eval.load_plugin("git-prompt");
    assert_eq!(load_res, Ok("git-prompt".to_string()));

    let prompt_obj = eval.get_var(0, "prompt").expect("prompt should exist");
    let segments = prompt_obj
        .get_property("segments")
        .expect("segments exists");

    if let Value::Array(arr) = segments {
        let guard = arr.read().unwrap();
        assert_eq!(guard.len(), 4);
    } else {
        panic!("Expected segments to be an array");
    }

    let rendered = eval.render_prompt_segments(None, None, None);
    assert!(!rendered.is_empty());

    // Test custom order and separator
    let custom_rendered = eval.render_prompt_segments(Some("git"), None, None);
    assert!(!custom_rendered.is_empty());

    // Test import function, importable methods, and segments array configuration
    let (mut script_eval, _) = eval_with_evaluator("");
    script_eval
        .plugin_manager
        .search_paths
        .insert(0, manifest_path);
    let script = r#"
        let git_prompt = import("git-prompt")
        $prompt.segments = ["\n", $fmt_cwd, $git_prompt.git, "\n", "❯ "]
        let out = $prompt.render()
        "#;
    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    let res = script_eval.evaluate_statements(&statements.body, &mut vec![0]);
    assert_eq!(res.flow, crate::eval::EvalFlow::Continue);
    let out = script_eval
        .get_var(0, "out")
        .expect("out exists")
        .to_string();
    assert!(out.contains("❯ "));
}

#[test]
fn prompt_segments_custom_registration_and_unload() {
    let (temp_dir, _) = write_test_plugin(
        "test-plugin",
        r#"
            return {
                name = "test-plugin",
                prompt_segments = {
                    alpha = () => {
                        return "ALPHA"
                    },
                    empty_seg = () => {
                        return ""
                    }
                }
            }
            "#,
    );

    let (mut eval, _) = eval_with_evaluator("");
    eval.plugin_manager.search_paths.insert(0, temp_dir.clone());
    assert_eq!(
        eval.load_plugin("test-plugin"),
        Ok("test-plugin".to_string())
    );

    // Explicitly register imported segments into $prompt.segments
    let script = r#"
        let p = import("test-plugin")
        $prompt.segments = $prompt.segments + [$p.alpha, $p.empty_seg]
        "#;
    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);

    let prompt_obj = eval.get_var(0, "prompt").expect("prompt exists");
    let segments = prompt_obj
        .get_property("segments")
        .expect("segments exists");
    if let Value::Array(arr) = segments {
        let guard = arr.read().unwrap();
        // Default 4 segments + 2 explicitly added = 6
        assert_eq!(guard.len(), 6);
    } else {
        panic!("Expected segments to be an array");
    }

    // empty_seg returns "" so it must be omitted
    let rendered = eval.render_prompt_segments(Some("alpha empty_seg"), None, None);
    assert_eq!(rendered, "ALPHA");

    // Custom separator
    let rendered_sep = eval.render_prompt_segments(Some("cwd alpha"), Some(" -> "), None);
    assert!(rendered_sep.contains(" -> ALPHA"));

    // Unload removes segments
    assert!(eval.unload_plugin("test-plugin"));
    let prompt_after_unload = eval.get_var(0, "prompt").expect("prompt exists");
    let segments_after = prompt_after_unload
        .get_property("segments")
        .expect("segments exists");
    if let Value::Array(arr) = segments_after {
        let guard = arr.read().unwrap();
        assert_eq!(guard.len(), 4);
    } else {
        panic!("Expected segments to be an array");
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn prompt_newline_segment_behavior() {
    let (mut eval, _) = eval_with_evaluator("");

    // 1. Leading newline is emitted cleanly without separator space after it
    let rendered = eval.render_prompt_segments(Some("newline cwd"), None, None);
    assert!(
        rendered.starts_with('\n'),
        "Leading newline should be emitted"
    );
    assert!(
        !rendered.starts_with("\n "),
        "Newline should not have separator space after it"
    );
    assert!(rendered.contains("\x1b[1;34m"));

    // 2. Omission of newline gives a compact prompt without newline
    let rendered_compact = eval.render_prompt_segments(Some("cwd"), None, None);
    assert!(!rendered_compact.starts_with('\n'));

    // 3. Middle newline is emitted cleanly for multi-line prompts
    let rendered_multiline = eval.render_prompt_segments(Some("cwd newline cwd"), None, None);
    assert!(rendered_multiline.contains('\n'));
    assert!(!rendered_multiline.contains(" \n"));
    assert!(!rendered_multiline.contains("\n "));
}

#[test]
fn native_array_literals_indexing_methods_and_operations() {
    let (mut eval, _) = eval_with_evaluator("");

    let script = r#"
        let arr = ["first", 20, "third"];
        let first = $arr[0];
        let last = $arr[-1];
        let len_prop = $arr.len;
        let len_call = $arr.len();
        
        $arr[0] = "updated_first";
        $arr.push("fourth");
        let popped = $arr.pop();
        let joined = $arr.join("-");
        let has_twenty = $arr.contains(20);
        let has_missing = $arr.contains("missing");

        let combined = [1, 2] + [3, 4];
        let appended = [1, 2] + 3;
        "#;

    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);

    assert_eq!(
        eval.get_var(0, "first"),
        Some(&Value::String("first".to_string()))
    );
    assert_eq!(
        eval.get_var(0, "last"),
        Some(&Value::String("third".to_string()))
    );
    assert_eq!(eval.get_var(0, "len_prop"), Some(&Value::Number(3.0)));
    assert_eq!(eval.get_var(0, "len_call"), Some(&Value::Number(3.0)));

    let arr_val = eval.get_var(0, "arr").expect("arr exists");
    if let Value::Array(a) = arr_val {
        let guard = a.read().unwrap();
        assert_eq!(guard[0], Value::String("updated_first".to_string()));
        assert_eq!(guard.len(), 3);
    } else {
        panic!("Expected Value::Array");
    }

    assert_eq!(
        eval.get_var(0, "popped"),
        Some(&Value::String("fourth".to_string()))
    );
    assert_eq!(
        eval.get_var(0, "joined"),
        Some(&Value::String("updated_first-20-third".to_string()))
    );
    assert_eq!(eval.get_var(0, "has_twenty"), Some(&Value::Bool(true)));
    assert_eq!(eval.get_var(0, "has_missing"), Some(&Value::Bool(false)));

    let combined_val = eval.get_var(0, "combined").expect("combined exists");
    assert_eq!(
        combined_val,
        &Value::array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::Number(4.0),
        ])
    );

    let appended_val = eval.get_var(0, "appended").expect("appended exists");
    assert_eq!(
        appended_val,
        &Value::array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ])
    );
}

#[test]
fn prompt_char_segment_and_customization() {
    let (mut eval, _) = eval_with_evaluator("");

    // Default char and symbol segments
    let char_rendered = eval.render_prompt_segments(Some("char"), None, None);
    assert!(char_rendered.contains("›"));

    let symbol_rendered = eval.render_prompt_segments(Some("symbol"), None, None);
    assert_eq!(char_rendered, symbol_rendered);

    // Customize char segment with $prompt.text
    let script = r#"
        $prompt.text = "❯ "
        let custom = $prompt.render("char")
        "#;
    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);
    let custom = eval.get_var(0, "custom").expect("custom exists");
    assert_eq!(custom.to_string(), "❯ ");

    // Configure prompt.segments directly with direct strings
    let script_direct = r#"
        $prompt.segments = ["\n", $fmt_cwd, "\n", "❯ "]
        let rendered_direct = $prompt.render()
        "#;
    let stmts_direct = Parser::new(0, lexer::lex_input(0, script_direct).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&stmts_direct.body, &mut vec![0]);
    let rendered_direct = eval
        .get_var(0, "rendered_direct")
        .expect("rendered_direct exists");
    assert!(rendered_direct.to_string().ends_with("❯ "));
}

#[test]
fn fmt_cwd_function_and_lambda_in_prompt_segments() {
    let (mut eval, _) = eval_with_evaluator("");

    let script = r#"
        let path = fmt_cwd()
        $prompt.segments = [
            "\n",
            () => { return "[" + fmt_cwd() + "]" },
            "\n",
            "❯ "
        ]
        let rendered = $prompt.render()
        "#;
    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);
    let path = eval.get_var(0, "path").expect("path exists");
    assert!(!path.to_string().is_empty());
    let rendered = eval.get_var(0, "rendered").expect("rendered exists");
    assert!(rendered.to_string().contains(&format!("[{path}]")));
    assert!(rendered.to_string().ends_with("❯ "));
}

#[test]
fn command_with_array_expansion_and_indexing() {
    let (mut eval, _) = eval_with_evaluator("");

    let script = r#"
        let words = ["one", "two", "three"];
        let out_full = $(echo $words).out;
        let out_idx = $(echo $words[0] $words[-1]).out;
        "#;

    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);

    let out_full = eval.get_var(0, "out_full").unwrap().to_string();
    let out_idx = eval.get_var(0, "out_idx").unwrap().to_string();
    assert_eq!(out_full.trim(), "one two three");
    assert_eq!(out_idx.trim(), "one three");

    let script_brackets = r#"
        let out_brackets = $(echo [0-9]).out;
        let out_compound = $(echo --flag=[val]).out;
        "#;
    let stmts_brackets = Parser::new(0, lexer::lex_input(0, script_brackets).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&stmts_brackets.body, &mut vec![0]);

    let out_brackets = eval.get_var(0, "out_brackets").unwrap().to_string();
    let out_compound = eval.get_var(0, "out_compound").unwrap().to_string();
    assert_eq!(out_brackets.trim(), "[0-9]");
    assert_eq!(out_compound.trim(), "--flag=[val]");
}

#[test]
fn command_with_escapes_and_line_continuations() {
    let (mut eval, _) = eval_with_evaluator("");

    let script = "let out_space = $(echo My\\ Documents).out;\n\
                      let out_escapes = $(echo {} \\; \\$NOT_EXPANDED).out;\n\
                      let out_continuation = $(echo line1 \\\n line2).out;";

    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);

    let out_space = eval.get_var(0, "out_space").unwrap().to_string();
    let out_escapes = eval.get_var(0, "out_escapes").unwrap().to_string();
    let out_continuation = eval.get_var(0, "out_continuation").unwrap().to_string();

    assert_eq!(out_space.trim(), "My Documents");
    assert_eq!(out_escapes.trim(), "{} ; $NOT_EXPANDED");
    assert_eq!(out_continuation.trim(), "line1 line2");
}

#[test]
fn evaluates_interpolated_string_with_variables_and_expressions() {
    let code = r#"
            let name = "world"
            let msg = "Hello $name!"
            let a = 10
            let b = 20
            let calc = "Sum: ${a + b}"
            let single = 'Raw $name unexpanded'
            let escaped = "Cost: \$100"
        "#;
    assert_eq!(
        eval_var(code, "msg"),
        Some(Value::String("Hello world!".to_string()))
    );
    assert_eq!(
        eval_var(code, "calc"),
        Some(Value::String("Sum: 30".to_string()))
    );
    assert_eq!(
        eval_var(code, "single"),
        Some(Value::String("Raw $name unexpanded".to_string()))
    );
    assert_eq!(
        eval_var(code, "escaped"),
        Some(Value::String("Cost: $100".to_string()))
    );
}

#[test]
fn evaluates_interpolated_string_with_dot_path_and_indexing() {
    let code = r#"
            let user = { name = "Alice", role = "admin" }
            let greeting = "Welcome ${user.name} (${$user.role})"
            let items = ["first", "second"]
            let item_msg = "Item 0: ${items[0]}"
        "#;
    assert_eq!(
        eval_var(code, "greeting"),
        Some(Value::String("Welcome Alice (admin)".to_string()))
    );
    assert_eq!(
        eval_var(code, "item_msg"),
        Some(Value::String("Item 0: first".to_string()))
    );
}

#[test]
fn evaluates_interpolated_string_with_command_sub() {
    let code = r#"
            let msg = "Output: $(echo hi)"
        "#;
    assert_eq!(
        eval_var(code, "msg"),
        Some(Value::String("Output: hi".to_string()))
    );
}

#[test]
fn evaluates_interpolated_string_in_command_arguments() {
    let (mut eval, _) = eval_with_evaluator("");
    let script = r#"
            let who = "LSH"
            let out = $(echo "Hello $who!").out
        "#;
    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);
    let out = eval.get_var(0, "out").unwrap().to_string();
    assert_eq!(out.trim(), "Hello LSH!");
}

#[test]
fn hooks_registry_lifecycle_and_methods() {
    let (mut eval, _) = eval_with_evaluator("");

    let script = r#"
        let chpwd_run = ""
        let pre_prompt_runs = 0

        $hooks.add("chpwd", (old_p, new_p) => {
            $chpwd_run = $old_p + " -> " + $new_p
        })

        $hooks.on("pre_prompt", () => {
            $pre_prompt_runs = $pre_prompt_runs + 1
        })

        let counts_before = $hooks.list()
        "#;
    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);

    eval.trigger_chpwd("/a", "/b");
    eval.trigger_pre_prompt();

    let chpwd_val = eval.get_var(0, "chpwd_run").unwrap().to_string();
    assert_eq!(chpwd_val, "/a -> /b");

    let pre_prompt_val = eval.get_var(0, "pre_prompt_runs").unwrap().clone();
    assert_eq!(pre_prompt_val, Value::Number(1.0));

    let counts = eval.get_var(0, "counts_before").unwrap().clone();
    assert_eq!(counts.get_property("chpwd"), Some(Value::Number(1.0)));
    assert_eq!(counts.get_property("pre_prompt"), Some(Value::Number(1.0)));

    // Clear pre_prompt hooks
    let script_clear = r#"
        $hooks.clear("pre_prompt")
        let counts_after = $hooks.list()
        "#;
    let stmts_clear = Parser::new(0, lexer::lex_input(0, script_clear).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&stmts_clear.body, &mut vec![0]);

    eval.trigger_pre_prompt();
    let pre_prompt_val2 = eval.get_var(0, "pre_prompt_runs").unwrap().clone();
    assert_eq!(pre_prompt_val2, Value::Number(1.0)); // unchanged

    let counts_after = eval.get_var(0, "counts_after").unwrap().clone();
    assert_eq!(counts_after.get_property("pre_prompt"), None);
}

#[test]
fn module_caching_and_scope_isolation() {
    let (temp_dir, module_file) = write_test_plugin(
        "cache-mod",
        r#"
            let internal_secret = 42
            return {
                name = "cache-mod",
                val = "loaded"
            }
            "#,
    );

    let (mut eval, _) = eval_with_evaluator("");
    let path_str = module_file.to_str().unwrap();

    // First import
    let val1 = eval.import_module(path_str).expect("first import succeeds");
    assert_eq!(
        val1.get_property("val"),
        Some(Value::String("loaded".to_string()))
    );

    // Check internal variable did not leak into global scope
    assert_eq!(eval.get_var(0, "internal_secret"), None);

    // Mutate the returned object
    if let Value::Object(map) = &val1 {
        map.write()
            .unwrap()
            .insert("mutated".to_string(), Value::Bool(true));
    }

    // Second import: returns cached instance with mutated property
    let val2 = eval
        .import_module(path_str)
        .expect("second import succeeds");
    assert_eq!(val2.get_property("mutated"), Some(Value::Bool(true)));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn property_access_method_binding_with_this() {
    let (mut eval, _) = eval_with_evaluator("");

    let script = r#"
        let counter = {
            count = 10,
            get_count = () => {
                return $this.count
            }
        }

        let extracted_fn = $counter.get_count
        let res = $extracted_fn()
        "#;
    let statements = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    eval.evaluate_statements(&statements.body, &mut vec![0]);

    let res = eval.get_var(0, "res").expect("res exists").clone();
    assert_eq!(res, Value::Number(10.0));
}

#[test]
fn test_time_segment_config_example() {
    let (mut eval, _) = eval_with_evaluator("");
    let script = r#"
        let time_segment = () => {
            return $palette.dim + $(date +%H:%M).out.trim()
        }
        $prompt.segments.push(time_segment)
        "#;
    let prog = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    let status = eval.evaluate(prog);
    assert_eq!(status, 0);

    let prompt_obj = eval.get_var(0, "prompt").expect("prompt exists");
    let segments = prompt_obj
        .get_property("segments")
        .expect("segments exists");
    if let Value::Array(arr) = segments {
        let guard = arr.read().unwrap();
        assert_eq!(guard.len(), 5);
    } else {
        panic!("Expected segments to be an array");
    }

    let rendered = eval.render_prompt_segments(None, None, None);
    // Ensure rendered output contains dim style and a colon from HH:MM
    assert!(rendered.contains(':'));
}

#[test]
fn test_string_methods() {
    let (mut eval, _) = eval_with_evaluator("");
    let script = r#"
        let s = "  hello world  "
        let trimmed = $s.trim()
        let upper = $trimmed.to_uppercase()
        let lower = $upper.to_lowercase()
        let s_len = $trimmed.len()
        let s_prop_len = $trimmed.len
        let parts = $trimmed.split(" ")
        let replaced = $trimmed.replace("world", "there")
        let starts = $trimmed.starts_with("hello")
        let ends = $trimmed.ends_with("world")
        let has_sub = $trimmed.contains("lo wo")
        let first_char = $trimmed[0]
        "#;
    let prog = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    let status = eval.evaluate(prog);
    assert_eq!(status, 0);

    assert_eq!(
        eval.get_var(0, "trimmed").unwrap().to_string(),
        "hello world"
    );
    assert_eq!(eval.get_var(0, "upper").unwrap().to_string(), "HELLO WORLD");
    assert_eq!(eval.get_var(0, "lower").unwrap().to_string(), "hello world");
    assert_eq!(eval.get_var(0, "s_len").unwrap(), &Value::Number(11.0));
    assert_eq!(eval.get_var(0, "s_prop_len").unwrap(), &Value::Number(11.0));
    assert_eq!(
        eval.get_var(0, "replaced").unwrap().to_string(),
        "hello there"
    );
    assert_eq!(eval.get_var(0, "starts").unwrap(), &Value::Bool(true));
    assert_eq!(eval.get_var(0, "ends").unwrap(), &Value::Bool(true));
    assert_eq!(eval.get_var(0, "has_sub").unwrap(), &Value::Bool(true));
    assert_eq!(eval.get_var(0, "first_char").unwrap().to_string(), "h");

    if let Value::Array(parts_arr) = eval.get_var(0, "parts").unwrap() {
        let guard = parts_arr.read().unwrap();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard[0].to_string(), "hello");
        assert_eq!(guard[1].to_string(), "world");
    } else {
        panic!("Expected parts to be an array");
    }
}

#[test]
fn test_command_result_methods() {
    let (mut eval, _) = eval_with_evaluator("");
    let script = r#"
        let ok_direct = $(true).is_ok()
        let err_direct = $(true).is_err()
        let false_ok = $(false).is_ok()
        let false_err = $(false).is_err()

        let res = $(echo "  world  ")
        let res_ok = $res.is_ok()
        let res_err = $res.is_err()
        let res_trimmed = $res.out.trim()

        let lines = $(printf "alpha\nbeta\ngamma\n").lines()
        let empty_lines = $(echo -n "").lines()
        "#;
    let prog = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    let status = eval.evaluate(prog);
    assert_eq!(status, 0);

    assert_eq!(eval.get_var(0, "ok_direct").unwrap(), &Value::Bool(true));
    assert_eq!(eval.get_var(0, "err_direct").unwrap(), &Value::Bool(false));
    assert_eq!(eval.get_var(0, "false_ok").unwrap(), &Value::Bool(false));
    assert_eq!(eval.get_var(0, "false_err").unwrap(), &Value::Bool(true));

    assert_eq!(eval.get_var(0, "res_ok").unwrap(), &Value::Bool(true));
    assert_eq!(eval.get_var(0, "res_err").unwrap(), &Value::Bool(false));
    assert_eq!(eval.get_var(0, "res_trimmed").unwrap().to_string(), "world");

    if let Value::Array(lines_arr) = eval.get_var(0, "lines").unwrap() {
        let guard = lines_arr.read().unwrap();
        assert_eq!(guard.len(), 3);
        assert_eq!(guard[0].to_string(), "alpha");
        assert_eq!(guard[1].to_string(), "beta");
        assert_eq!(guard[2].to_string(), "gamma");
    } else {
        panic!("Expected lines to be an array");
    }

    if let Value::Array(empty_arr) = eval.get_var(0, "empty_lines").unwrap() {
        let guard = empty_arr.read().unwrap();
        assert_eq!(guard.len(), 0);
    } else {
        panic!("Expected empty_lines to be an array");
    }
}

#[test]
fn test_for_loops_and_break_continue() {
    let (mut eval, _) = eval_with_evaluator("");
    let script = r#"
        let sum = 0
        for item in [1, 2, 3, 4] {
            $sum = $sum + $item
        }

        let paren_sum = 0
        for (x in [10, 20]) {
            $paren_sum = $paren_sum + $x
        }

        let break_sum = 0
        for n in [1, 2, 3, 4, 5] {
            if $n == 3 {
                break
            }
            $break_sum = $break_sum + $n
        }

        let continue_sum = 0
        for n in [1, 2, 3, 4] {
            if $n == 2 {
                continue
            }
            $continue_sum = $continue_sum + $n
        }

        let str_chars = []
        for ch in "abc" {
            $str_chars.push($ch)
        }

        let cmd_lines = []
        for line in $(printf "foo\nbar\n") {
            $cmd_lines.push($line)
        }

        fn find_first_even(list) {
            for x in list {
                if $x == 4 {
                    return $x
                }
            }
            return 0
        }
        let even = find_first_even([1, 3, 4, 5])
        "#;
    let prog = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    let status = eval.evaluate(prog);
    assert_eq!(status, 0);

    assert_eq!(eval.get_var(0, "sum").unwrap(), &Value::Number(10.0));
    assert_eq!(eval.get_var(0, "paren_sum").unwrap(), &Value::Number(30.0));
    assert_eq!(eval.get_var(0, "break_sum").unwrap(), &Value::Number(3.0));
    assert_eq!(
        eval.get_var(0, "continue_sum").unwrap(),
        &Value::Number(8.0)
    );
    assert_eq!(eval.get_var(0, "even").unwrap(), &Value::Number(4.0));

    if let Value::Array(ch_arr) = eval.get_var(0, "str_chars").unwrap() {
        let guard = ch_arr.read().unwrap();
        assert_eq!(guard.len(), 3);
        assert_eq!(guard[0].to_string(), "a");
        assert_eq!(guard[1].to_string(), "b");
        assert_eq!(guard[2].to_string(), "c");
    } else {
        panic!("Expected str_chars to be array");
    }

    if let Value::Array(cmd_arr) = eval.get_var(0, "cmd_lines").unwrap() {
        let guard = cmd_arr.read().unwrap();
        assert_eq!(guard.len(), 2);
        assert_eq!(guard[0].to_string(), "foo");
        assert_eq!(guard[1].to_string(), "bar");
    } else {
        panic!("Expected cmd_lines to be array");
    }
}

#[test]
fn test_while_loops_and_break_continue_return() {
    let (mut eval, _) = eval_with_evaluator("");
    let script = r#"
        let count = 0
        let break_count = 0
        while $count < 10 {
            $count = $count + 1
            if $count == 5 {
                break
            }
            $break_count = $break_count + 1
        }

        let i = 0
        let continue_sum = 0
        while $i < 5 {
            $i = $i + 1
            if $i == 3 {
                continue
            }
            $continue_sum = $continue_sum + $i
        }

        fn find_while_threshold() {
            let n = 0
            while true {
                $n = $n + 1
                if $n == 7 {
                    return $n * 10
                }
            }
            return -1
        }
        let early_ret = find_while_threshold()
        "#;
    let prog = Parser::new(0, lexer::lex_input(0, script).unwrap())
        .parse_program()
        .unwrap();
    let status = eval.evaluate(prog);
    assert_eq!(status, 0);

    assert_eq!(eval.get_var(0, "count").unwrap(), &Value::Number(5.0));
    assert_eq!(eval.get_var(0, "break_count").unwrap(), &Value::Number(4.0));
    // i goes 1, 2, 3 (skipped), 4, 5 => continue_sum = 1 + 2 + 4 + 5 = 12
    assert_eq!(
        eval.get_var(0, "continue_sum").unwrap(),
        &Value::Number(12.0)
    );
    assert_eq!(eval.get_var(0, "early_ret").unwrap(), &Value::Number(70.0));
}

#[test]
fn test_jobs_and_disown_builtins() {
    let mut eval = Evaluator::new();
    eval.job_manager.add_job(
        1234,
        vec![1234],
        "sleep 10 &".to_string(),
        JobState::Running,
        None,
    );
    eval.job_manager.add_job(
        5678,
        vec![5678],
        "vim test.txt".to_string(),
        JobState::Stopped(20),
        None,
    );

    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut ctx = IoCtx {
        input: io::Cursor::new(Vec::new()),
        out: &mut out,
        err: &mut err,
        input_mode: IoStreamMode::Pipe,
        out_mode: IoStreamMode::Pipe,
        err_mode: IoStreamMode::Pipe,
    };

    let status = crate::builtins::dispatch(
        crate::builtins::BuiltinKind::Jobs,
        &mut eval,
        &mut ctx,
        vec![],
    );
    assert_eq!(status, 0);
    let out_str = String::from_utf8(out).unwrap();
    assert!(out_str.contains("sleep 10 &"));
    assert!(out_str.contains("vim test.txt"));

    // Test disown %1
    let mut out2 = Vec::new();
    let mut err2 = Vec::new();
    let mut ctx2 = IoCtx {
        input: io::Cursor::new(Vec::new()),
        out: &mut out2,
        err: &mut err2,
        input_mode: IoStreamMode::Pipe,
        out_mode: IoStreamMode::Pipe,
        err_mode: IoStreamMode::Pipe,
    };
    let status = crate::builtins::dispatch(
        crate::builtins::BuiltinKind::Disown,
        &mut eval,
        &mut ctx2,
        vec!["%1".to_string()],
    );
    assert_eq!(status, 0);
    assert!(eval.job_manager.get_job(1).is_none());
    assert!(eval.job_manager.get_job(2).is_some());
}

#[test]
fn test_exit_warns_on_stopped_jobs() {
    let mut eval = Evaluator::new();
    eval.job_manager.add_job(
        9999,
        vec![9999],
        "vim file.txt".to_string(),
        JobState::Stopped(20),
        None,
    );

    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut ctx = IoCtx {
        input: io::Cursor::new(Vec::new()),
        out: &mut out,
        err: &mut err,
        input_mode: IoStreamMode::Pipe,
        out_mode: IoStreamMode::Pipe,
        err_mode: IoStreamMode::Pipe,
    };

    // First exit attempt should warn and fail
    let code = crate::builtins::dispatch(
        crate::builtins::BuiltinKind::Exit,
        &mut eval,
        &mut ctx,
        vec![],
    );
    assert_eq!(code, 1);
    assert!(eval.warned_stopped_jobs);
    let err_str = String::from_utf8(err).unwrap();
    assert!(err_str.contains("There are stopped jobs"));
}

#[test]
fn test_kill_builtin() {
    let mut eval = Evaluator::new();
    eval.job_manager.add_job(
        8888,
        vec![8888],
        "sleep 100".to_string(),
        JobState::Stopped(20),
        None,
    );

    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut ctx = IoCtx {
        input: io::Cursor::new(Vec::new()),
        out: &mut out,
        err: &mut err,
        input_mode: IoStreamMode::Pipe,
        out_mode: IoStreamMode::Pipe,
        err_mode: IoStreamMode::Pipe,
    };

    // Test kill -l
    let code = crate::builtins::dispatch(
        crate::builtins::BuiltinKind::Kill,
        &mut eval,
        &mut ctx,
        vec!["-l".to_string()],
    );
    assert_eq!(code, 0);
    let out_str = String::from_utf8(out).unwrap();
    assert!(out_str.contains("SIGKILL"));
    assert!(out_str.contains("SIGTERM"));
}
