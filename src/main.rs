pub mod lexer;

fn main() {
    let content = include_str!("../example.txt");

    let tokens = lexer::lex_input(content);

    for token in tokens {
        println!("{token}");
    }
}
