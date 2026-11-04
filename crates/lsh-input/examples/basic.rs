use std::borrow::Cow;

use lsh_input::{Editor, EditorAction, KeyCombo, Keymap, Prompt, Signal};

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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[lsh-input basic example]");
    println!("  - Dynamic ANSI prompt");
    println!("  - Standard Emacs editing: Ctrl+A (home), Ctrl+E (end)");
    println!("  - Word navigation: Alt+Left / Alt+B (word left), Alt+Right / Alt+F (word right)");
    println!("  - Word deletion: Ctrl+W / Alt+Backspace, Alt+D (delete word forward)");
    println!("  - Line deletion: Ctrl+U (delete to start), Ctrl+K (delete to end)");
    println!("  - Custom binding: Ctrl+L (clear entire line)");
    println!("  - Press Ctrl+C to cancel current line, Ctrl+D or type 'exit' to quit.\n");

    // 1. Setup custom keymap (extending standard Emacs bindings)
    let mut keymap = Keymap::default();
    keymap.bind(KeyCombo::ctrl('l'), EditorAction::ClearLine);

    // 2. Initialize the editor
    let mut editor = Editor::new().with_keymap(keymap);
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
                    println!("You typed: {trimmed}");
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
