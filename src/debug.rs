use std::env;

use crate::{
    lexer::token::Token,
    parser::ast::{Expr, Program, Statement},
};

pub fn get_debug_env(name: &str) -> bool {
    let var_name = "DEBUG_".to_string() + name;

    if let Ok(debug_var) = env::var(var_name) {
        return matches!(debug_var.as_str(), "1" | "true");
    }

    false
}

pub fn print_lexer(tokens: &[Token]) {
    if !get_debug_env("LEXER") {
        return;
    }

    for token in tokens {
        println!("{token}");
    }
}

pub fn print_parser(prog: &Program) {
    if !get_debug_env("PARSER") {
        return;
    }

    prog.body.iter().for_each(|stmt| {
        print_parser_statement(stmt);
    });
}

fn print_parser_statement(stmt: &Statement) {
    match stmt {
        Statement::Expr(expr) => print_parser_expr(expr),
        Statement::Let { name, value } => {
            print!("Let: {name} = ");
            print_parser_expr(value);
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            println!("---- Start if    ----");

            println!("Cond ->");

            print_parser_expr(condition);

            println!("Then ->");

            then_branch.iter().for_each(|s| {
                print_parser_statement(s);
            });

            if let Some(some_then_branch) = else_branch {
                println!("Else ->");

                some_then_branch.iter().for_each(|s| {
                    print_parser_statement(s);
                });
            } else {
                println!("No else branch");
            }

            println!("----   End if    ----");
        }
        Statement::Block(stmts) => {
            println!("---- Start block ----");

            stmts.iter().for_each(|s| {
                print_parser_statement(s);
            });

            println!("----   End block ----");
        }
        _ => println!("Other statement"),
    }
}

fn print_parser_expr(expr: &Expr) {
    match expr {
        Expr::String(str) => {
            println!("String({str})");
        }
        Expr::Number(num) => {
            println!("Number({num})");
        }
        Expr::Bool(b) => {
            println!("Bool({b})");
        }
        Expr::Variable(var) => {
            println!("Variable({var})");
        }
        Expr::Binary { left, op, right } => {
            println!("Binary Expression({left:?} {op:?} {right:?})");
        }
        Expr::Call { identifier, args } => {
            println!("Call({identifier}, {} args)", args.len());
        }
        Expr::Typeof(expr) => {
            println!("Typeof({expr:?})");
        }
    }
}
