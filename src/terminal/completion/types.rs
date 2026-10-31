use std::collections::HashMap;

use crate::terminal::menu::{CompletionItem, CompletionKind};

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
}
