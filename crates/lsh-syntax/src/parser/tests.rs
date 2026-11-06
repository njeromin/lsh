use crate::{diagnostics::DiagnosticKind, lexer::lex_input};

use super::{
    Parser,
    ast::{BinaryOp, CommandExpr, Expr, Redirect, Statement},
};

fn parse_program(input: &str) -> Vec<Statement> {
    let tokens = lex_input(0, input).expect("lexer should succeed in parser test");
    let mut parser = Parser::new(0, tokens);
    parser
        .parse_program()
        .expect("parser should succeed in parser test")
        .body
}

fn assert_command_identifier(identifier: &Expr, expected: &str) {
    assert!(
        matches!(identifier, Expr::String(actual) | Expr::QuotedString(actual) if actual == expected)
            || matches!(identifier, Expr::Word(actual) if actual == expected),
        "Expected command identifier {expected:?}, got {identifier:?}"
    );
}

fn extract_call_parts(statement: &Statement) -> (&Expr, &Vec<Expr>) {
    match statement {
        Statement::Command(CommandExpr::Call {
            identifier, args, ..
        }) => (identifier, args),
        Statement::Expr(Expr::Call { target, args }) => (target.as_ref(), args),
        _ => panic!("Expected call-like statement"),
    }
}

#[test]
fn returns_parse_diagnostic_for_invalid_let_identifier() {
    let tokens = lex_input(0, "let = 1").expect("lexer should succeed");
    let mut parser = Parser::new(0, tokens);

    let err = parser
        .parse_program()
        .expect_err("parser should return parse diagnostic");
    assert!(matches!(err.kind, DiagnosticKind::ParseError));
    assert!(err.message.contains("Expected identifier after 'let'."));
}

#[test]
fn returns_parse_diagnostic_for_unclosed_command_sub_expr() {
    let tokens = lex_input(0, "echo $(1 + 2").expect("lexer should succeed");
    let mut parser = Parser::new(0, tokens);

    let err = parser
        .parse_program()
        .expect_err("parser should return parse diagnostic");
    assert!(matches!(err.kind, DiagnosticKind::ParseError));
    assert!(err.message.contains("Expected ')' after expression"));
}

#[test]
fn returns_parse_diagnostic_for_missing_redirect_target() {
    let tokens = lex_input(0, "echo hi ->").expect("lexer should succeed");
    let mut parser = Parser::new(0, tokens);

    let err = parser
        .parse_program()
        .expect_err("parser should return parse diagnostic");
    assert!(matches!(err.kind, DiagnosticKind::ParseError));
    assert!(err.message.contains("Expected redirect target after '->'."));
}

#[test]
fn returns_parse_diagnostic_with_hint_for_assignment_in_expression() {
    let tokens = lex_input(0, "if 1 = 1 {}").expect("lexer should succeed");
    let mut parser = Parser::new(0, tokens);

    let err = parser
        .parse_program()
        .expect_err("parser should return parse diagnostic");
    assert!(matches!(err.kind, DiagnosticKind::ParseError));
    assert!(err.message.contains("Unexpected '=' in expression."));
    assert!(matches!(
        err.hint.as_deref(),
        Some("If you meant comparison, use '=='.")
    ));
}

#[test]
fn parses_regular_command_call() {
    let body = parse_program("echo hello world");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
    assert!(matches!(&args[1], Expr::String(s) if s == "world"));
}

#[test]
fn parses_pipeline_command() {
    let body = parse_program("cat file.txt | grep txt");

    let Statement::Command(CommandExpr::Pipeline(stages)) = &body[0] else {
        panic!("Expected command pipeline statement");
    };

    assert_eq!(stages.len(), 2);

    let CommandExpr::Call {
        identifier: first_id,
        args: first_args,
        ..
    } = &stages[0]
    else {
        panic!("Expected first pipeline stage to be a call");
    };
    assert_command_identifier(first_id, "cat");
    assert!(matches!(&first_args[0], Expr::String(s) if s == "file.txt"));

    let CommandExpr::Call {
        identifier: second_id,
        args: second_args,
        ..
    } = &stages[1]
    else {
        panic!("Expected second pipeline stage to be a call");
    };
    assert_command_identifier(second_id, "grep");
    assert!(matches!(&second_args[0], Expr::String(s) if s == "txt"));
}

#[test]
fn parses_and_command_chain() {
    let body = parse_program("echo ok && echo done");

    let Statement::Command(CommandExpr::And { left, right }) = &body[0] else {
        panic!("Expected command and-chain");
    };

    let CommandExpr::Call {
        identifier: left_id,
        ..
    } = left.as_ref()
    else {
        panic!("Expected left side of and-chain to be a command call");
    };
    let CommandExpr::Call {
        identifier: right_id,
        ..
    } = right.as_ref()
    else {
        panic!("Expected right side of and-chain to be a command call");
    };

    assert_command_identifier(left_id, "echo");
    assert_command_identifier(right_id, "echo");
}

#[test]
fn parses_command_with_env_prefix_assignment() {
    let body = parse_program("ENV_VAR=1 echo hello");

    let Statement::Command(CommandExpr::Call {
        env_assignments,
        identifier,
        args,
        ..
    }) = &body[0]
    else {
        panic!("Expected command call statement with env prefix");
    };

    assert_eq!(env_assignments.len(), 1);
    assert!(matches!(
        &env_assignments[0],
        (name, Expr::Number(n)) if name == "ENV_VAR" && (*n - 1.0).abs() < f64::EPSILON
    ));
    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
}

#[test]
fn parses_command_with_multiple_env_prefix_assignments() {
    let body = parse_program("A=1 B=2 echo ok");

    let Statement::Command(CommandExpr::Call {
        env_assignments,
        identifier,
        args,
        ..
    }) = &body[0]
    else {
        panic!("Expected command call statement with env prefix");
    };

    assert_eq!(env_assignments.len(), 2);
    assert!(matches!(&env_assignments[0], (name, Expr::Number(_)) if name == "A"));
    assert!(matches!(&env_assignments[1], (name, Expr::Number(_)) if name == "B"));
    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "ok"));
}

#[test]
fn parses_command_with_env_prefix_variable_value() {
    let body = parse_program("ENV_VAR=$HOME echo ok");

    let Statement::Command(CommandExpr::Call {
        env_assignments,
        identifier,
        args,
        ..
    }) = &body[0]
    else {
        panic!("Expected command call statement with env prefix");
    };

    assert_eq!(env_assignments.len(), 1);
    assert!(matches!(
        &env_assignments[0],
        (name, Expr::Variable(var)) if name == "ENV_VAR" && var == "HOME"
    ));
    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "ok"));
}

#[test]
fn parses_or_command_chain() {
    let body = parse_program("missing || echo fallback");

    let Statement::Command(CommandExpr::Or { left, right }) = &body[0] else {
        panic!("Expected command or-chain");
    };

    let CommandExpr::Call {
        identifier: left_id,
        ..
    } = left.as_ref()
    else {
        panic!("Expected left side of or-chain to be a command call");
    };
    let CommandExpr::Call {
        identifier: right_id,
        ..
    } = right.as_ref()
    else {
        panic!("Expected right side of or-chain to be a command call");
    };

    assert_command_identifier(left_id, "missing");
    assert_command_identifier(right_id, "echo");
}

#[test]
fn parses_pipeline_before_and_chain() {
    let body = parse_program("cat file.txt | grep txt && echo done");

    let Statement::Command(CommandExpr::And { left, right }) = &body[0] else {
        panic!("Expected top-level and-chain");
    };

    let CommandExpr::Pipeline(stages) = left.as_ref() else {
        panic!("Expected left side to be parsed as pipeline");
    };
    assert_eq!(stages.len(), 2);

    let CommandExpr::Call { identifier, .. } = right.as_ref() else {
        panic!("Expected right side to be a command call");
    };
    assert_command_identifier(identifier, "echo");
}

#[test]
fn parses_command_with_colon_path_argument() {
    let body = parse_program("cat /sys/bus/pci/devices/0000:64:00.0/power/runtime_status");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "cat");
    assert_eq!(args.len(), 1);
    assert!(
        matches!(&args[0], Expr::String(s) if s == "/sys/bus/pci/devices/0000:64:00.0/power/runtime_status")
    );
}

#[test]
fn parses_command_with_command_substitution_argument() {
    let body = parse_program("echo $(cat /etc/hosts)");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);

    let Expr::CommandSubCommand(sub_command) = &args[0] else {
        panic!("Expected command substitution command argument");
    };

    let CommandExpr::Call {
        identifier: sub_id,
        args: sub_args,
        ..
    } = sub_command.as_ref()
    else {
        panic!("Expected command substitution to contain a command call");
    };
    assert_command_identifier(sub_id, "cat");
    assert!(matches!(&sub_args[0], Expr::String(s) if s == "/etc/hosts"));
}

#[test]
fn parses_member_access_on_command_substitution_in_expr_call_arg() {
    let body = parse_program("echo $(cat /etc/hosts).out");

    let Statement::Expr(Expr::Call { target, args }) = &body[0] else {
        panic!("Expected expression call statement");
    };

    assert!(matches!(target.as_ref(), Expr::Word(word) if word == "echo"));
    assert_eq!(args.len(), 1);

    let Expr::Index { object, index } = &args[0] else {
        panic!("Expected member access on command substitution");
    };
    assert!(matches!(index.as_ref(), Expr::String(s) if s == "out"));

    let Expr::CommandSubCommand(sub_command) = object.as_ref() else {
        panic!("Expected command substitution expression");
    };
    let CommandExpr::Call {
        identifier: sub_id,
        args: sub_args,
        ..
    } = sub_command.as_ref()
    else {
        panic!("Expected command substitution to contain a command call");
    };
    assert_command_identifier(sub_id, "cat");
    assert!(matches!(&sub_args[0], Expr::String(s) if s == "/etc/hosts"));
}

#[test]
fn parses_member_access_on_command_sub_with_following_arg() {
    let body = parse_program("echo $(fmt-cwd $PWD).out $git_info");

    let (identifier, args) = extract_call_parts(&body[0]);
    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 2);

    let Expr::Index { object, index } = &args[0] else {
        panic!("Expected first arg to be member access on command substitution");
    };
    assert!(matches!(index.as_ref(), Expr::String(s) if s == "out"));
    assert!(matches!(object.as_ref(), Expr::CommandSubCommand(_)));
    assert!(matches!(&args[1], Expr::Variable(name) if name == "git_info"));
}

#[test]
fn parses_command_with_dotted_path_argument() {
    let body = parse_program("source config/config.lsh");

    let (identifier, args) = extract_call_parts(&body[0]);
    assert_command_identifier(identifier, "source");
    assert_eq!(args.len(), 1);
    assert!(
        matches!(&args[0], Expr::String(s) if s == "config/config.lsh")
            || matches!(
                &args[0],
                Expr::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
    );
}

#[test]
fn parses_command_with_spaced_dot_argument() {
    let body = parse_program("git add .");

    let (identifier, args) = extract_call_parts(&body[0]);
    assert_command_identifier(identifier, "git");
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[0], Expr::String(s) if s == "add"));
    assert!(matches!(&args[1], Expr::String(s) if s == "."));
}

#[test]
fn parses_command_with_contiguous_dot_path_argument() {
    let body = parse_program("ls ./examples/");

    let (identifier, args) = extract_call_parts(&body[0]);
    assert_command_identifier(identifier, "ls");
    assert_eq!(args.len(), 1);
    assert!(
        matches!(&args[0], Expr::String(s) if s == "./examples/")
            || matches!(
                &args[0],
                Expr::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
    );
}

#[test]
fn parses_command_with_contiguous_path_identifier() {
    let body = parse_program("./examples/anonymous-functions.lsh");

    let (identifier, args) = extract_call_parts(&body[0]);
    assert_eq!(args.len(), 0);
    assert!(
        matches!(identifier, Expr::String(s) if s == "./examples/anonymous-functions.lsh")
            || matches!(
                identifier,
                Expr::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
    );
}

#[test]
fn parses_command_with_stdout_redirection() {
    let body = parse_program("echo hello -> out.txt");

    let Statement::Command(CommandExpr::Call {
        identifier,
        args,
        stdout_redirect,
        ..
    }) = &body[0]
    else {
        panic!("Expected command call statement with redirection");
    };

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
    assert!(matches!(
        stdout_redirect,
        Some(Redirect {
            target: Expr::String(s),
            append: false
        }) if s == "out.txt"
    ));
}

#[test]
fn parses_command_with_stdout_append_redirection_thin_arrow() {
    let body = parse_program("echo hello ->> out.txt");

    let Statement::Command(CommandExpr::Call {
        identifier,
        args,
        stdout_redirect,
        ..
    }) = &body[0]
    else {
        panic!("Expected command call statement with append redirection");
    };

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
    assert!(matches!(
        stdout_redirect,
        Some(Redirect {
            target: Expr::String(s),
            append: true
        }) if s == "out.txt"
    ));
}

#[test]
fn double_gt_is_not_redirection() {
    let body = parse_program("echo hello >> out.txt");
    let Statement::Command(CommandExpr::Call {
        identifier,
        args,
        stdout_redirect,
        ..
    }) = &body[0]
    else {
        panic!("Expected command call statement");
    };

    assert_command_identifier(identifier, "echo");
    assert!(stdout_redirect.is_none());
    assert_eq!(args.len(), 3);
}

#[test]
fn parses_pipeline_with_last_stage_stdout_redirection() {
    let body = parse_program("echo hello | cat -> out.txt");

    let Statement::Command(CommandExpr::Pipeline(stages)) = &body[0] else {
        panic!("Expected command pipeline statement");
    };
    assert_eq!(stages.len(), 2);

    let CommandExpr::Call {
        stdout_redirect: first_redirect,
        ..
    } = &stages[0]
    else {
        panic!("Expected first stage call");
    };
    assert!(first_redirect.is_none());

    let CommandExpr::Call {
        identifier,
        stdout_redirect,
        ..
    } = &stages[1]
    else {
        panic!("Expected second stage call");
    };
    assert_command_identifier(identifier, "cat");
    assert!(matches!(
        stdout_redirect,
        Some(Redirect {
            target: Expr::String(s),
            append: false
        }) if s == "out.txt"
    ));
}

#[test]
fn parses_object_literal_with_equals_separator() {
    let body = parse_program("let x = { key = value }");

    let Statement::Let { name, value } = &body[0] else {
        panic!("Expected let statement");
    };
    assert_eq!(name, "x");

    let Expr::Object(map) = value else {
        panic!("Expected object literal expression");
    };
    assert!(matches!(map.get("key"), Some(Expr::Word(value)) if value == "value"));
}

#[test]
fn parses_command_with_comma_arguments_without_panicking() {
    let body = parse_program("echo hello, world");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 2);
    assert!(matches!(
        &args[0],
        Expr::Binary {
            op: BinaryOp::Add,
            left,
            right
        } if matches!(left.as_ref(), Expr::String(s) if s == "hello")
            && matches!(right.as_ref(), Expr::String(s) if s == ",")
    ));
    assert!(matches!(&args[1], Expr::String(s) if s == "world"));
}

#[test]
fn parses_command_with_dash_argument_without_panicking() {
    let body = parse_program("cd -");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "cd");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "-"));
}

#[test]
fn parses_command_with_url_argument() {
    let body = parse_program("echo https://example.com");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(
        matches!(&args[0], Expr::String(s) if s == "https://example.com")
            || matches!(
                &args[0],
                Expr::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
    );
}

#[test]
fn parses_command_with_key_value_argument_tokens() {
    let body = parse_program("echo key=value");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "key=value"));
}

#[test]
fn parses_command_with_multiple_equals_in_argument() {
    let body = parse_program("echo key=value=next");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "key=value=next"));
}

#[test]
fn parses_command_with_unspaced_star_word_argument() {
    let body = parse_program("echo a*b");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "a*b"));
}

#[test]
fn parses_command_with_variable_path_suffix_as_single_argument() {
    let body = parse_program("echo $HOME/.config");

    let (identifier, args) = extract_call_parts(&body[0]);

    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(
        &args[0],
        Expr::Binary {
            op: BinaryOp::Add,
            left,
            right
        } if matches!(left.as_ref(), Expr::Variable(name) if name == "HOME")
            && matches!(right.as_ref(), Expr::String(s) if s == "/.config")
    ));
}

#[test]
fn parses_string_concatenation_in_assignment_expression() {
    let body = parse_program("let x = \"a\" + \"b\" + \"c\"");

    let Statement::Let { value, .. } = &body[0] else {
        panic!("Expected let statement");
    };

    let Expr::Binary {
        op: BinaryOp::Add, ..
    } = value
    else {
        panic!("Expected top-level binary add expression");
    };
}

#[test]
fn parses_function_declaration_target_as_variable() {
    let body = parse_program("fn add(a, b) { return $a + $b; }");

    let Statement::FunctionDeclaration { target, params, .. } = &body[0] else {
        panic!("Expected function declaration");
    };

    assert!(matches!(target, Expr::Variable(name) if name == "add"));
    assert_eq!(params, &vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn parses_block_with_trailing_separator_before_right_curly() {
    let body = parse_program("if true {\n    echo ok\n}\n");

    let Statement::If { then_branch, .. } = &body[0] else {
        panic!("Expected if statement");
    };
    assert_eq!(then_branch.len(), 1);
    let (identifier, _args) = extract_call_parts(&then_branch[0]);
    assert_command_identifier(identifier, "echo");
}

#[test]
fn parses_program_with_only_separators_as_empty() {
    let body = parse_program("\n;\n;;\n");
    assert!(body.is_empty());
}

#[test]
fn parses_multiline_commands_with_command_substitution_separately() {
    let body = parse_program("echo \"first\" $(add 2 3).out\necho \"second\" $(add 10 5).out");
    assert_eq!(body.len(), 2);
}

#[test]
fn parses_multiline_commands_with_inline_comments_separately() {
    let body = parse_program("echo hello # first comment\necho world # second comment");
    assert_eq!(body.len(), 2);
}

#[test]
fn parses_command_with_multiple_command_substitutions() {
    let body = parse_program("echo $(add 1 2).out $(add 3 4).out");
    assert_eq!(body.len(), 1);
    let (_, args) = extract_call_parts(&body[0]);
    assert_eq!(args.len(), 2);
}

#[test]
fn parses_nested_objects_without_trailing_commas() {
    let input = r#"let obj = {
            outer_fn = () => {
                return 42
            },
            nested = {
                inner_fn = () => {
                    return "hello"
                }
            }
        }"#;
    let body = parse_program(input);
    assert_eq!(body.len(), 1);
}

#[test]
fn parses_array_literals_empty_and_populated() {
    let input = r#"let empty = [];
        let items = ["cwd", "git", 42, true];
        let nested = [[1, 2], ["a", "b"]];
        let trailing = [
            "one",
            "two",
        ];"#;
    let body = parse_program(input);
    assert_eq!(body.len(), 4);

    if let Statement::Let {
        value: Expr::Array(elements),
        ..
    } = &body[1]
    {
        assert_eq!(elements.len(), 4);
    } else {
        panic!("Expected Expr::Array for items");
    }

    if let Statement::Let {
        value: Expr::Array(elements),
        ..
    } = &body[3]
    {
        assert_eq!(elements.len(), 2);
    } else {
        panic!("Expected Expr::Array for trailing");
    }
}

#[test]
fn parses_find_command_with_braces_and_escaped_semicolon() {
    let input = "find . -exec echo {} \\;";
    let body = parse_program(input);
    assert_eq!(body.len(), 1);
    if let Statement::Command(CommandExpr::Call {
        identifier: Expr::String(cmd),
        args,
        ..
    }) = &body[0]
    {
        assert_eq!(cmd, "find");
        assert_eq!(args.len(), 5);
        assert!(matches!(&args[0], Expr::String(s) if s == "."));
        assert!(matches!(&args[1], Expr::String(s) if s == "-exec"));
        assert!(matches!(&args[2], Expr::String(s) if s == "echo"));
        assert!(
            matches!(&args[3], Expr::Binary { op: BinaryOp::Add, left, right }
                if matches!(left.as_ref(), Expr::String(s) if s == "{")
                && matches!(right.as_ref(), Expr::String(s) if s == "}"))
        );
        assert!(matches!(&args[4], Expr::String(s) if s == ";"));
    } else {
        panic!("Expected CommandExpr::Call");
    }
}

#[test]
fn parses_command_with_escaped_spaces() {
    let input = "cat My\\ File.txt";
    let body = parse_program(input);
    assert_eq!(body.len(), 1);
    let (identifier, args) = extract_call_parts(&body[0]);
    assert_command_identifier(identifier, "cat");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::String(s) if s == "My File.txt"));
}

#[test]
fn parses_line_continuation_in_commands_and_expressions() {
    let input = "echo hello \\\n world;\nlet x = 10 \\\n + 20;";
    let body = parse_program(input);
    assert_eq!(body.len(), 2);
    let (identifier, args) = extract_call_parts(&body[0]);
    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[0], Expr::String(s) if s == "hello"));
    assert!(matches!(&args[1], Expr::String(s) if s == "world"));

    if let Statement::Let { value, .. } = &body[1] {
        assert!(matches!(
            value,
            Expr::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
    } else {
        panic!("Expected let statement");
    }
}

#[test]
fn parses_block_containing_command_with_braces() {
    let input = "if true { echo {} }";
    let body = parse_program(input);
    assert_eq!(body.len(), 1);
    if let Statement::If {
        then_branch: if_body,
        ..
    } = &body[0]
    {
        assert_eq!(if_body.len(), 1);
        if let Statement::Command(CommandExpr::Call { args, .. }) = &if_body[0] {
            assert_eq!(args.len(), 1);
        } else {
            panic!("Expected command inside block");
        }
    } else {
        panic!("Expected If statement");
    }
}

#[test]
fn parses_interpolated_string_with_variable() {
    let body = parse_program(r#"let msg = "Hello $name!""#);
    let Statement::Let { value, .. } = &body[0] else {
        panic!("Expected let statement");
    };
    let Expr::InterpolatedString(parts) = value else {
        panic!("Expected InterpolatedString, got {value:?}");
    };
    assert_eq!(parts.len(), 3);
    assert!(matches!(&parts[0], Expr::QuotedString(s) if s == "Hello "));
    assert!(matches!(&parts[1], Expr::Variable(v) if v == "name"));
    assert!(matches!(&parts[2], Expr::QuotedString(s) if s == "!"));
}

#[test]
fn parses_interpolated_string_with_expression() {
    let body = parse_program(r#"let msg = "Result: ${count + 1}""#);
    let Statement::Let { value, .. } = &body[0] else {
        panic!("Expected let statement");
    };
    let Expr::InterpolatedString(parts) = value else {
        panic!("Expected InterpolatedString, got {value:?}");
    };
    assert_eq!(parts.len(), 2);
    assert!(matches!(&parts[0], Expr::QuotedString(s) if s == "Result: "));
    assert!(matches!(
        &parts[1],
        Expr::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn parses_interpolated_string_with_command_substitution() {
    let body = parse_program(r#"let msg = "Branch: $(git branch)""#);
    let Statement::Let { value, .. } = &body[0] else {
        panic!("Expected let statement");
    };
    let Expr::InterpolatedString(parts) = value else {
        panic!("Expected InterpolatedString, got {value:?}");
    };
    assert_eq!(parts.len(), 2);
    assert!(matches!(&parts[0], Expr::QuotedString(s) if s == "Branch: "));
    assert!(matches!(&parts[1], Expr::CommandSubCommand(_)));
}

#[test]
fn parses_interpolated_string_with_escaped_dollar() {
    let body = parse_program(r#"let msg = "Price: \$100""#);
    let Statement::Let { value, .. } = &body[0] else {
        panic!("Expected let statement");
    };
    assert!(matches!(value, Expr::QuotedString(s) if s == "Price: $100"));
}

#[test]
fn parses_single_quoted_string_as_literal_unexpanded() {
    let body = parse_program("let msg = 'Hello $name $(cmd) ${1+2}'");
    let Statement::Let { value, .. } = &body[0] else {
        panic!("Expected let statement");
    };
    assert!(matches!(value, Expr::QuotedString(s) if s == "Hello $name $(cmd) ${1+2}"));
}

#[test]
fn parses_interpolated_string_in_command_arguments() {
    let body = parse_program(r#"echo "Hello $name""#);
    let (identifier, args) = extract_call_parts(&body[0]);
    assert_command_identifier(identifier, "echo");
    assert_eq!(args.len(), 1);
    assert!(matches!(&args[0], Expr::InterpolatedString(parts) if parts.len() == 2));
}

#[test]
fn parses_for_loop_and_break_continue() {
    let body = parse_program("for item in [1, 2, 3] { if $item == 2 { break } else { continue } }");
    let Statement::For {
        variable,
        iterable,
        body: loop_body,
    } = &body[0]
    else {
        panic!("Expected for loop statement");
    };
    assert_eq!(variable, "item");
    assert!(matches!(iterable, Expr::Array(arr) if arr.len() == 3));
    assert_eq!(loop_body.len(), 1);

    let Statement::If {
        then_branch,
        else_branch,
        ..
    } = &loop_body[0]
    else {
        panic!("Expected if statement in for body");
    };
    assert!(matches!(&then_branch[0], Statement::Break));
    assert!(matches!(
        &else_branch.as_ref().unwrap()[0],
        Statement::Continue
    ));
}

#[test]
fn parses_for_loop_with_parens_and_variable_sigil() {
    let body = parse_program("for ($item in $items) { echo $item }");
    let Statement::For {
        variable,
        iterable,
        body: loop_body,
    } = &body[0]
    else {
        panic!("Expected for loop statement");
    };
    assert_eq!(variable, "item");
    assert!(matches!(iterable, Expr::Variable(v) if v == "items"));
    assert_eq!(loop_body.len(), 1);
}

#[test]
fn parses_background_commands() {
    let body = parse_program("sleep 10 &");
    assert_eq!(body.len(), 1);
    let Statement::Command(CommandExpr::Background(inner)) = &body[0] else {
        panic!("Expected CommandExpr::Background");
    };
    assert!(matches!(inner.as_ref(), CommandExpr::Call { .. }));

    let body2 = parse_program("sleep 1 & sleep 2 &");
    assert_eq!(body2.len(), 2);
    assert!(matches!(
        &body2[0],
        Statement::Command(CommandExpr::Background(_))
    ));
    assert!(matches!(
        &body2[1],
        Statement::Command(CommandExpr::Background(_))
    ));

    let body3 = parse_program("cat foo | grep bar &");
    assert_eq!(body3.len(), 1);
    let Statement::Command(CommandExpr::Background(inner)) = &body3[0] else {
        panic!("Expected CommandExpr::Background");
    };
    assert!(matches!(inner.as_ref(), CommandExpr::Pipeline(_)));
}
