#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Builtin,
    Alias,
    Function,
    Binary,
    Variable,
    File,
    Directory,
    Subcommand,
    Flag,
    Value,
}

impl CompletionKind {
    pub fn badge(&self) -> &'static str {
        match self {
            CompletionKind::Builtin => "[builtin]",
            CompletionKind::Alias => "[alias]",
            CompletionKind::Function => "[fn]",
            CompletionKind::Binary => "[bin]",
            CompletionKind::Variable => "[var]",
            CompletionKind::File => "[file]",
            CompletionKind::Directory => "[dir]",
            CompletionKind::Subcommand => "[subcmd]",
            CompletionKind::Flag => "[flag]",
            CompletionKind::Value => "[val]",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            CompletionKind::Builtin => "\x1b[35m",     // Magenta
            CompletionKind::Alias => "\x1b[36m",       // Cyan
            CompletionKind::Function => "\x1b[34m",    // Blue
            CompletionKind::Binary => "\x1b[32m",      // Green
            CompletionKind::Variable => "\x1b[33m",    // Yellow
            CompletionKind::File => "\x1b[37m",        // White
            CompletionKind::Directory => "\x1b[1;34m", // Bold Blue
            CompletionKind::Subcommand => "\x1b[36m",  // Cyan
            CompletionKind::Flag => "\x1b[33m",        // Yellow
            CompletionKind::Value => "\x1b[32m",       // Green
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub replacement: String,
    pub display: String,
    pub description: Option<String>,
    pub kind: CompletionKind,
    pub match_indices: Vec<usize>,
}

impl CompletionItem {
    pub fn new(
        replacement: impl Into<String>,
        display: impl Into<String>,
        kind: CompletionKind,
    ) -> Self {
        Self {
            replacement: replacement.into(),
            display: display.into(),
            description: None,
            kind,
            match_indices: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn nerd_icon(&self) -> (&'static str, &'static str) {
        match self.kind {
            CompletionKind::Directory => ("\u{f115}", "\x1b[1;34m"), //  Bold Blue
            CompletionKind::Binary => ("\u{f489}", "\x1b[32m"),      //  Green
            CompletionKind::Builtin => ("\u{f0ad}", "\x1b[35m"),     //  Magenta
            CompletionKind::Alias => ("\u{f0c1}", "\x1b[36m"),       //  Cyan
            CompletionKind::Function => ("\u{f0295}", "\x1b[34m"),   // 󰊕 Blue
            CompletionKind::Variable => ("\u{eb5d}", "\x1b[33m"),    //  Yellow
            CompletionKind::Subcommand => ("\u{f126}", "\x1b[36m"),  //  Cyan
            CompletionKind::Flag => ("\u{f024}", "\x1b[33m"),        //  Yellow
            CompletionKind::Value => ("\u{f02b}", "\x1b[32m"),       //  Green
            CompletionKind::File => Self::file_nerd_icon(&self.display),
        }
    }

    pub fn is_dir(&self) -> bool {
        self.kind == CompletionKind::Directory
            || self.replacement.ends_with('/')
            || self.display.ends_with('/')
    }

    fn file_nerd_icon(filename: &str) -> (&'static str, &'static str) {
        let name_lower = filename.to_lowercase();
        // Exact names first
        if name_lower == "cargo.toml" || name_lower == "cargo.lock" {
            return ("\u{e7a8}", "\x1b[38;5;208m"); //  Rust orange
        }
        if name_lower.starts_with(".git") {
            return ("\u{e702}", "\x1b[31m"); //  Git red
        }
        if name_lower == "license" || name_lower.starts_with("license.") {
            return ("\u{e60a}", "\x1b[33m"); //  Yellow
        }

        // By extension
        if let Some(ext) = name_lower.rsplit('.').next() {
            match ext {
                "rs" => ("\u{e7a8}", "\x1b[38;5;208m"), //  Rust orange
                "toml" | "yaml" | "yml" | "json" | "ini" | "conf" => ("\u{e615}", "\x1b[33m"), //  Config yellow
                "md" | "markdown" => ("\u{f48a}", "\x1b[36m"), //  Markdown cyan
                "sh" | "bash" | "zsh" | "lsh" => ("\u{f489}", "\x1b[32m"), //  Terminal green
                "lock" => ("\u{f023}", "\x1b[38;5;245m"),      //  Lock gray
                "py" => ("\u{e606}", "\x1b[34m"),              //  Python blue
                "js" | "mjs" | "cjs" => ("\u{e60c}", "\x1b[33m"), //  JS yellow
                "ts" => ("\u{e628}", "\x1b[34m"),              //  TS blue
                "html" | "css" | "scss" => ("\u{e60e}", "\x1b[35m"), //  Web purple
                "c" | "h" | "cpp" | "hpp" | "cc" => ("\u{e61d}", "\x1b[34m"), //  C/C++ blue
                "go" => ("\u{e627}", "\x1b[36m"),              //  Go cyan
                "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" => ("\u{f1c5}", "\x1b[35m"), //  Image purple
                "mp3" | "wav" | "ogg" | "mp4" | "mkv" | "avi" => ("\u{f1c7}", "\x1b[35m"), //  Media purple
                "zip" | "tar" | "gz" | "xz" | "7z" | "bz2" => ("\u{f1c6}", "\x1b[31m"), //  Archive red
                "txt" | "log" | "csv" => ("\u{f15c}", "\x1b[37m"), //  Text white
                _ => ("\u{f15b}", "\x1b[37m"),                     //  File white
            }
        } else {
            ("\u{f15b}", "\x1b[37m") //  File white
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuLayout {
    pub width: usize,
    pub height: usize,
    pub menu_col: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub nerd_icons: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuResult {
    /// Accepted a candidate from the dropdown list
    Selected(CompletionItem),
    /// Typed past all matches or committed typed text
    Typed(String),
    /// Cancelled without accepting (e.g. Esc or Ctrl-C)
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Next,
    Previous,
    PageDown,
    PageUp,
    First,
    Last,
    Accept,
    Dismiss,
    Char(char),
    Backspace,
    Tab,
    Slash,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_badges_and_colors() {
        assert_eq!(CompletionKind::Builtin.badge(), "[builtin]");
        assert_eq!(CompletionKind::Binary.badge(), "[bin]");
        assert_eq!(CompletionKind::Directory.badge(), "[dir]");
        assert_eq!(CompletionKind::File.badge(), "[file]");
        assert_eq!(CompletionKind::Variable.badge(), "[var]");
        assert_eq!(CompletionKind::Alias.badge(), "[alias]");
        assert_eq!(CompletionKind::Function.badge(), "[fn]");
        assert_eq!(CompletionKind::Subcommand.badge(), "[subcmd]");
        assert_eq!(CompletionKind::Flag.badge(), "[flag]");
        assert_eq!(CompletionKind::Value.badge(), "[val]");
    }

    #[test]
    fn item_nerd_icons_match_kind_and_extensions() {
        let dir = CompletionItem::new("src", "src", CompletionKind::Directory);
        assert_eq!(dir.nerd_icon().0, "\u{f115}"); // 

        let bin = CompletionItem::new("git", "git", CompletionKind::Binary);
        assert_eq!(bin.nerd_icon().0, "\u{f489}"); // 

        let rust_file = CompletionItem::new("main.rs", "main.rs", CompletionKind::File);
        assert_eq!(rust_file.nerd_icon().0, "\u{e7a8}"); // 

        let toml_file = CompletionItem::new("Cargo.toml", "Cargo.toml", CompletionKind::File);
        assert_eq!(toml_file.nerd_icon().0, "\u{e7a8}"); // 

        let md_file = CompletionItem::new("README.md", "README.md", CompletionKind::File);
        assert_eq!(md_file.nerd_icon().0, "\u{f48a}"); // 

        let plain_file = CompletionItem::new("data.bin", "data.bin", CompletionKind::File);
        assert_eq!(plain_file.nerd_icon().0, "\u{f15b}"); // 
    }

    #[test]
    fn completion_item_is_dir_identifies_directories() {
        let dir_item = CompletionItem::new("src", "src", CompletionKind::Directory);
        assert!(dir_item.is_dir());

        let slash_item = CompletionItem::new("scripts/", "scripts/", CompletionKind::File);
        assert!(slash_item.is_dir());

        let file_item = CompletionItem::new("main.rs", "main.rs", CompletionKind::File);
        assert!(!file_item.is_dir());

        let bin_item = CompletionItem::new("cargo", "cargo", CompletionKind::Binary);
        assert!(!bin_item.is_dir());
    }
}
