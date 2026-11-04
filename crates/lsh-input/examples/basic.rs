use std::borrow::Cow;

use crossterm::style::{ContentStyle, Stylize};
use lsh_input::{
    DefaultValidator, Editor, EditorAction, KeyCombo, Keymap, Prompt, Signal, StyledText,
};

/// A custom dynamic prompt showing command count with ANSI colors.
struct FancyPrompt {
    counter: usize,
}

impl Prompt for FancyPrompt {
    fn render(&self) -> Cow<'_, str> {
        // Formats as: "[1] lsh-input> " with cyan counter and green label
        Cow::Owned(format!(
            "\x1b[36m[{}]\x1b[0m \x1b[32mlsh-input>\x1b[0m ",
            self.counter
        ))
    }

    fn render_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("\x1b[38;5;240m… \x1b[0m")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[lsh-input interactive demo]");
    println!("  - Dynamic ANSI prompt with multiline indicator");
    println!("  - Multiline Input: unclosed {{, (, [, quotes, or trailing \\ automatically insert newlines!");
    println!("  - Explicit Newline: Shift+Enter or Alt+Enter (or Ctrl+J) inserts a newline at any time");
    println!("  - Vertical Navigation: Up/Down arrow moves between lines (and aligns visual columns)");
    println!("  - Live StyledText TextFormatter: keywords in bold green, numbers in yellow");
    println!("  - Standard Emacs editing: Ctrl+A (home), Ctrl+E (end)");
    println!("  - Word navigation: Alt+Left / Alt+B (word left), Alt+Right / Alt+F (word right)");
    println!("  - Word deletion: Ctrl+Backspace / Ctrl+W / Alt+Backspace, Alt+D (delete word forward)");
    println!("  - Line deletion: Ctrl+U (delete to start), Ctrl+K (delete to end)");
    println!("  - Custom binding: Ctrl+L (clear entire line)");
    println!("  - Press Ctrl+C to cancel current line, Ctrl+D or type 'exit' to quit.\n");

    // 1. Setup custom keymap (extending standard Emacs bindings)
    let mut keymap = Keymap::default();
    keymap.bind(KeyCombo::ctrl('l'), EditorAction::ClearLine);

    // 2. Initialize the editor with custom keymap, validator, and live StyledText TextFormatter
    let mut editor = Editor::new()
        .with_keymap(keymap)
        .with_validator(DefaultValidator)
        .with_formatter(|line: &str| {
            let mut styled = StyledText::new();
            for word in line.split_inclusive(char::is_whitespace) {
                let trimmed = word.trim();
                if matches!(trimmed, "cargo" | "git" | "echo" | "let" | "cd" | "fn") {
                    styled.push(ContentStyle::new().green().bold(), word);
                } else if trimmed.parse::<f64>().is_ok() {
                    styled.push(ContentStyle::new().yellow(), word);
                } else {
                    styled.push(ContentStyle::default(), word);
                }
            }
            styled
        });

    let mut count = 1;

    // 3. Interactive loop
    loop {
        let prompt = FancyPrompt { counter: count };

        match editor.read_line(&prompt)? {
            Signal::Success(line) => {
                let trimmed = line.trim();
                if trimmed == "exit" || trimmed == "quit" {
                    println!("Goodbye!");
                    break;
                }
                if !trimmed.is_empty() {
                    println!("You entered ({} lines):\n{trimmed}", line.lines().count());
                    count += 1;
                }
            }
            Signal::CtrlC => {
                println!("(Interrupted with Ctrl+C)");
            }
            Signal::CtrlD => {
                println!("Exiting on EOF (Ctrl+D)...");
                break;
            }
        }
    }

    Ok(())
}
