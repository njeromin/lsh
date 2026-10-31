use std::collections::HashMap;

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
        if name_lower == "cargo.toml" || name_lower == "cargo.lock" {
            return ("\u{e7a8}", "\x1b[38;5;208m");
        }
        if name_lower.starts_with(".git") {
            return ("\u{e702}", "\x1b[31m");
        }
        if name_lower == "license" || name_lower.starts_with("license.") {
            return ("\u{e60a}", "\x1b[33m");
        }

        if let Some(ext) = name_lower.rsplit('.').next() {
            match ext {
                "rs" => ("\u{e7a8}", "\x1b[38;5;208m"),
                "toml" | "yaml" | "yml" | "json" | "ini" | "conf" => ("\u{e615}", "\x1b[33m"),
                "md" | "markdown" => ("\u{f48a}", "\x1b[36m"),
                "sh" | "bash" | "zsh" | "lsh" => ("\u{f489}", "\x1b[32m"),
                "lock" => ("\u{f023}", "\x1b[38;5;245m"),
                "py" => ("\u{e606}", "\x1b[34m"),
                "js" | "mjs" | "cjs" => ("\u{e60c}", "\x1b[33m"),
                "ts" => ("\u{e628}", "\x1b[34m"),
                "html" | "css" | "scss" => ("\u{e60e}", "\x1b[35m"),
                "c" | "h" | "cpp" | "hpp" | "cc" => ("\u{e61d}", "\x1b[34m"),
                "go" => ("\u{e627}", "\x1b[36m"),
                "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico" => ("\u{f1c5}", "\x1b[35m"),
                "mp3" | "wav" | "ogg" | "mp4" | "mkv" | "avi" => ("\u{f1c7}", "\x1b[35m"),
                "zip" | "tar" | "gz" | "xz" | "7z" | "bz2" => ("\u{f1c6}", "\x1b[31m"),
                "txt" | "log" | "csv" => ("\u{f15c}", "\x1b[37m"),
                _ => ("\u{f15b}", "\x1b[37m"),
            }
        } else {
            ("\u{f15b}", "\x1b[37m")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateSpec {
    pub replacement: String,
    pub display: String,
    pub description: Option<String>,
    pub kind: CompletionKind,
}

impl CandidateSpec {
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
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn from_raw(raw: &str, default_kind: CompletionKind) -> Self {
        let (spec, desc) = match raw.split_once(':') {
            Some((s, d)) => (s, Some(d.to_string())),
            None => (raw, None),
        };
        let kind = if spec.starts_with('-') {
            CompletionKind::Flag
        } else {
            default_kind
        };
        Self {
            replacement: spec.to_string(),
            display: spec.to_string(),
            description: desc,
            kind,
        }
    }

    pub fn to_completion_item(&self) -> CompletionItem {
        let mut it = CompletionItem::new(&self.replacement, &self.display, self.kind);
        it.description = self.description.clone();
        it
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionRule {
    pub candidates: Vec<CandidateSpec>,
    pub subcommands: HashMap<String, CompletionRule>,
    pub exec: Option<String>,
    pub dirs_only: bool,
    pub file_extensions: Vec<String>,
}

impl CompletionRule {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionResolution {
    Items(Vec<CompletionItem>),
    DirectoriesOnly,
    FileExtensions(Vec<String>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_candidate_spec() {
        let flag = CandidateSpec::from_raw("-m:Commit message", CompletionKind::Subcommand);
        assert_eq!(flag.kind, CompletionKind::Flag);
        assert_eq!(flag.replacement, "-m");
        assert_eq!(flag.description, Some("Commit message".to_string()));

        let subcmd = CandidateSpec::from_raw("commit:Record changes", CompletionKind::Subcommand);
        assert_eq!(subcmd.kind, CompletionKind::Subcommand);
        assert_eq!(subcmd.replacement, "commit");
        assert_eq!(subcmd.description, Some("Record changes".to_string()));
    }

    #[test]
    fn item_is_dir_checks() {
        let item = CompletionItem::new("src/", "src/", CompletionKind::File);
        assert!(item.is_dir());
        let item2 = CompletionItem::new("src", "src", CompletionKind::Directory);
        assert!(item2.is_dir());
        let item3 = CompletionItem::new("Cargo.toml", "Cargo.toml", CompletionKind::File);
        assert!(!item3.is_dir());
    }
}
