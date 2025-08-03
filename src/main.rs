pub mod lexer;
pub mod parser;

fn main() {
    let content = include_str!("../example.txt");

    let tokens = lexer::lex_input(content);

    for token in tokens {
        println!("{token}");
    }
}
