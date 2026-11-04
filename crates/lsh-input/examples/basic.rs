use std::borrow::Cow;

use crossterm::style::{ContentStyle, Stylize};
use lsh_input::{
    BasicMenu, Candidate, DefaultValidator, Editor, EditorAction, KeyCombo, Keymap, Prompt, Signal,
    StyledText,
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
    println!(
        "  - Tab Completion: Press Tab to trigger completion or open the interactive dropdown menu"
    );
    println!(
        "  - Menu Navigation: Tab / Down cycles next, Shift+Tab / Up cycles previous, Enter commits, Esc cancels"
    );
    println!(
        "  - Inline Ghost Text: Type ahead of known commands to see ghost text autosuggestions"
    );
    println!(
        "  - Accept Hint: Right Arrow / End / Ctrl+F accepts full hint; Alt+Right / Alt+F accepts next word"
    );
    println!(
        "  - Bracketed Paste: Paste multiline code or commands safely without premature execution"
    );
    println!(
        "  - Soft Wrapping: Text wraps cleanly across terminal margins without line corruption"
    );
    println!(
        "  - Multiline Input: unclosed {{, (, [, quotes, or trailing \\ automatically insert newlines!"
    );
    println!(
        "  - Explicit Newline: Shift+Enter or Alt+Enter (or Ctrl+J) inserts a newline at any time"
    );
    println!(
        "  - Vertical Navigation: Up/Down arrow moves between lines (and aligns visual columns)"
    );
    println!("  - Live StyledText TextFormatter: keywords in bold green, numbers in yellow");
    println!("  - Standard Emacs editing: Ctrl+A (home), Ctrl+E (end), Ctrl+B/F (left/right)");
    println!("  - Word navigation: Alt+Left / Alt+B (word left), Alt+Right / Alt+F (word right)");
    println!(
        "  - Word deletion: Ctrl+Backspace / Ctrl+W / Alt+Backspace, Alt+D (delete word forward)"
    );
    println!("  - Line deletion: Ctrl+U (delete to start), Ctrl+K (delete to end)");
    println!("  - Custom binding: Ctrl+L (clear entire line)");
    println!("  - Press Ctrl+C to cancel current line, Ctrl+D or type 'exit' to quit.\n");

    let mut keymap = Keymap::default();
    keymap.bind(KeyCombo::ctrl('l'), EditorAction::ClearLine);

    let mut editor = Editor::new()
        .with_keymap(keymap)
        .with_validator(DefaultValidator)
        .with_menu(BasicMenu::default())
        .with_completer(|line: &str, cursor: usize| {
            let prefix = &line[..cursor];
            let token_start = prefix.rfind(char::is_whitespace).map_or(0, |i| i + 1);
            let token = &prefix[token_start..];

            let candidates = vec![
                Candidate::new("cargo")
                    .with_display("cargo (bin)")
                    .with_description("Rust package manager")
                    .with_append_space(true),
                Candidate::new("cat")
                    .with_display("cat (bin)")
                    .with_description("Concatenate files")
                    .with_append_space(true),
                Candidate::new("cd")
                    .with_display("cd (builtin)")
                    .with_description("Change directory")
                    .with_append_space(true),
                Candidate::new("git")
                    .with_display("git (bin)")
                    .with_description("Distributed revision control")
                    .with_append_space(true),
                Candidate::new("build")
                    .with_display("build (subcommand)")
                    .with_description("Compile local package")
                    .with_append_space(true),
                Candidate::new("check")
                    .with_display("check (subcommand)")
                    .with_description("Check code for errors without code generation")
                    .with_append_space(true),
                Candidate::new("clippy")
                    .with_display("clippy (subcommand)")
                    .with_description("Catch common mistakes")
                    .with_append_space(true),
                Candidate::new("status")
                    .with_display("status (subcommand)")
                    .with_description("Show working tree status")
                    .with_append_space(true),
                Candidate::new("commit")
                    .with_display("commit (subcommand)")
                    .with_description("Record changes to repository")
                    .with_append_space(true),
                Candidate::new("checkout")
                    .with_display("checkout (subcommand)")
                    .with_description("Switch branches")
                    .with_append_space(true),
                Candidate::new("Cargo.toml")
                    .with_display("Cargo.toml (file)")
                    .with_description("Cargo manifest")
                    .with_append_space(true),
                Candidate::new("README.md")
                    .with_display("README.md (file)")
                    .with_description("Project readme")
                    .with_append_space(true),
                Candidate::new("src/")
                    .with_display("src/ (dir)")
                    .with_description("Source code directory"),
                Candidate::new("--release")
                    .with_display("--release (flag)")
                    .with_description("Build artifacts in release mode")
                    .with_append_space(true),
                Candidate::new("--workspace")
                    .with_display("--workspace (flag)")
                    .with_description("Operate on all workspace members")
                    .with_append_space(true),
            ];

            let filtered = candidates
                .into_iter()
                .filter(|c| c.display.starts_with(token) || c.replacement.starts_with(token))
                .collect();

            (token_start, filtered)
        })
        .with_hinter(|line: &str, _cursor: usize| {
            let suggestions = [
                "git status",
                "git commit -m 'feat: initial commit'",
                "git checkout -b main",
                "cargo build --release",
                "cargo test --workspace",
                "echo 'hello world'",
            ];
            for s in suggestions {
                if s.starts_with(line) && s.len() > line.len() {
                    return Some(s[line.len()..].to_string());
                }
            }
            None
        })
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
