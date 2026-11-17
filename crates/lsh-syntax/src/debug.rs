use std::{env, sync::LazyLock};

use crate::{
    lexer::token::Token,
    parser::ast::{Expr, Program, Statement},
};

static DEBUG_LEXER: LazyLock<bool> = LazyLock::new(|| env_flag("DEBUG_LEXER"));
static DEBUG_PARSER: LazyLock<bool> = LazyLock::new(|| env_flag("DEBUG_PARSER"));

fn env_flag(name: &str) -> bool {
    matches!(env::var(name).as_deref(), Ok("1" | "true"))
}

pub fn print_lexer(tokens: &[Token<'_>]) {
    if !*DEBUG_LEXER {
        return;
    }

    for token in tokens {
        println!("{token}");
    }
}

pub fn print_parser(prog: &Program) {
    if !*DEBUG_PARSER {
        return;
    }

    prog.body.iter().for_each(|stmt| {
        print_parser_statement(stmt);
    });
}

fn print_parser_statement(stmt: &Statement) {
    match stmt {
        Statement::Expr(expr) => {
            print!("Expr: ");
            print_parser_expr(expr);
        }
        Statement::Command(command_expr) => {
            println!("Command({command_expr:?})");
        }
        Statement::Let { name, value } => {
            print!("Let: {name} = ");
            print_parser_expr(value);
        }
        Statement::Assign { target, value } => {
            print!("Assign: ");
            print_parser_expr(target);
            print!(" = ");
            print_parser_expr(value);
        }
        Statement::CompoundAssign { target, op, value } => {
            print!("CompoundAssign({op:?}): ");
            print_parser_expr(target);
            print!(" = ");
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
        Statement::While { condition, body } => {
            println!("---- Start while ----");
            print_parser_expr(condition);
            body.iter().for_each(|s| {
                print_parser_statement(s);
            });
            println!("---- End while ----");
        }
        Statement::For {
            variable,
            iterable,
            body,
        } => {
            println!("---- Start for ----");
            println!("Variable: {variable}");
            print_parser_expr(iterable);
            body.iter().for_each(|s| {
                print_parser_statement(s);
            });
            println!("---- End for ----");
        }
        Statement::Break => {
            println!("Break");
        }
        Statement::Continue => {
            println!("Continue");
        }
        Statement::FunctionDeclaration { target, params, .. } => {
            print!("Fn: ");
            print_parser_expr(target);
            println!("({})", params.join(", "));
        }
        Statement::Return(expr) => {
            print!("Return: ");
            if let Some(e) = expr {
                print_parser_expr(e);
            }
            println!();
        }
        Statement::Block(stmts) => {
            println!("---- Start block ----");

            stmts.iter().for_each(|s| {
                print_parser_statement(s);
            });

            println!("----   End block ----");
        }
    }
}

fn print_parser_expr(expr: &Expr) {
    match expr {
        Expr::String(str) | Expr::QuotedString(str) => {
            println!("String({str})");
        }
        Expr::InterpolatedString(parts) => {
            println!("InterpolatedString({parts:?})");
        }
        Expr::Number(num) => {
            println!("Number({num})");
        }
        Expr::Bool(b) => {
            println!("Bool({b})");
        }
        Expr::Object(map) => {
            println!("Object({map:?})");
        }
        Expr::Array(elements) => {
            println!("Array({elements:?})");
        }
        Expr::Variable(var) => {
            println!("Variable({var})");
        }
        Expr::Unary { op, expr } => {
            println!("Unary Expression({op:?} {expr:?})");
        }
        Expr::Binary { left, op, right } => {
            println!("Binary Expression({left:?} {op:?} {right:?})");
        }
        Expr::Index { object, index } => {
            println!("Index({object:?}[{index:?}])");
        }
        Expr::Word(word) => {
            println!("Word({word})");
        }
        Expr::Call { target, args } => {
            println!("Call({target:?}, {} args)", args.len());
        }
        Expr::TypeOf(expr) => {
            println!("TypeOf({expr:?})");
        }
        Expr::CommandSub(expr) => {
            println!("CommandSub({expr:?})");
        }
        Expr::CommandSubCommand(command_expr) => {
            println!("CommandSubCommand({command_expr:?})");
        }
        Expr::AnonymousFunction { params, body } => {
            println!("AnonymousFunction({params:?}, {} statements)", body.len());
        }
    }
}
