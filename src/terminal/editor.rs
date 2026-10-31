use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rustyline::{
    Context, Helper,
    completion::{Completer, FilenameCompleter, Pair},
    highlight::{CmdKind, Highlighter},
    hint::{Hinter, HistoryHinter},
    validate::Validator,
};

use crate::options::CompletionType;
use crate::terminal::{
    highlight::SyntaxHighlighter,
    menu::{self, CompletionItem, CompletionKind},
    osc::{self, Osc133PromptHighlighter},
};

#[derive(Debug, PartialEq, Eq)]
pub enum CompletionContext<'a> {
    Command { prefix: &'a str, start: usize },
    Variable { prefix: &'a str, start: usize },
    Path,
}

pub fn find_command_segment(sub: &str) -> &str {
    let mut last_sep = 0;
    let mut in_single = false;
    let mut in_double = false;
    let bytes = sub.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'|' | b';' if !in_single && !in_double => {
                if bytes[i] == b'|' && i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    i += 1;
                }
                last_sep = i + 1;
            }
            b'&' if !in_single && !in_double && i + 1 < bytes.len() && bytes[i + 1] == b'&' => {
                i += 1;
                last_sep = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    &sub[last_sep..]
}

pub fn classify_completion_context(line: &str, pos: usize) -> CompletionContext<'_> {
    let sub = if pos <= line.len() {
        &line[..pos]
    } else {
        line
    };

    // Check for variable completion: find the current word start.
    let word_start = sub
        .rfind(|c: char| c.is_whitespace() || matches!(c, '=' | '"' | '\''))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let current_word = &sub[word_start..];

    if let Some(var_prefix) = current_word.strip_prefix('$') {
        return CompletionContext::Variable {
            prefix: var_prefix,
            start: word_start,
        };
    }

    // Check for command position in current command segment.
    let segment = find_command_segment(sub);
    let leading_spaces = segment.len() - segment.trim_start().len();
    let trimmed = segment.trim_start();

    // If there is no whitespace in the current command segment, cursor is at command position
    // (unless typing an explicit path such as ./script, /bin/sh, or ../foo).
    if !trimmed.contains(char::is_whitespace) {
        if trimmed.starts_with("./") || trimmed.starts_with('/') || trimmed.starts_with("../") {
            return CompletionContext::Path;
        }

        let segment_start_in_sub = sub.len() - segment.len();
        let start = segment_start_in_sub + leading_spaces;
        return CompletionContext::Command {
            prefix: trimmed,
            start,
        };
    }

    CompletionContext::Path
}

pub fn scan_path_binaries() -> Vec<String> {
    let mut binaries = BTreeSet::new();

    if let Some(path_val) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_val) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type()
                        && (file_type.is_file() || file_type.is_symlink())
                        && let Ok(name) = entry.file_name().into_string()
                    {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(meta) = entry.metadata()
                                && meta.permissions().mode() & 0o111 != 0
                            {
                                binaries.insert(name);
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            binaries.insert(name);
                        }
                    }
                }
            }
        }
    }

    binaries.into_iter().collect()
}

pub struct LineEditorHelper {
    osc133: Osc133PromptHighlighter,
    syntax: SyntaxHighlighter,
    file_completer: FilenameCompleter,
    history_hinter: HistoryHinter,
    cached_binaries: Vec<String>,
    cached_aliases: Vec<String>,
    cached_vars: Vec<String>,
    pub completion_type: CompletionType,
    pub nerd_icons: bool,
}

impl Default for LineEditorHelper {
    fn default() -> Self {
        Self::new()
    }
}

impl LineEditorHelper {
    pub fn new() -> Self {
        Self {
            osc133: Osc133PromptHighlighter::new(osc::enabled()),
            syntax: SyntaxHighlighter::new(),
            file_completer: FilenameCompleter::new(),
            history_hinter: HistoryHinter::new(),
            cached_binaries: scan_path_binaries(),
            cached_aliases: Vec::new(),
            cached_vars: Vec::new(),
            completion_type: CompletionType::Menu,
            nerd_icons: false,
        }
    }

    pub fn set_completion_type(&mut self, completion_type: CompletionType) {
        self.completion_type = completion_type;
    }

    pub fn set_nerd_icons(&mut self, nerd_icons: bool) {
        self.nerd_icons = nerd_icons;
    }

    pub fn set_aliases(&mut self, aliases: Vec<String>) {
        self.cached_aliases = aliases;
    }

    pub fn set_vars(&mut self, vars: Vec<String>) {
        self.cached_vars = vars;
    }

    pub fn reset_prompt_marks(&self) {
        self.osc133.reset_prompt_marks();
    }

    pub fn get_candidates(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<CompletionItem>)> {
        match classify_completion_context(line, pos) {
            CompletionContext::Command { prefix, start } => {
                let mut map = BTreeMap::new();

                for binary in &self.cached_binaries {
                    if binary.starts_with(prefix) {
                        map.insert(binary.clone(), CompletionKind::Binary);
                    }
                }

                for alias in &self.cached_aliases {
                    if alias.starts_with(prefix) {
                        map.insert(alias.clone(), CompletionKind::Alias);
                    }
                }

                for &builtin in crate::builtins::BUILTIN_NAMES {
                    if builtin.starts_with(prefix) {
                        map.insert(builtin.to_string(), CompletionKind::Builtin);
                    }
                }

                let items = map
                    .into_iter()
                    .map(|(name, kind)| CompletionItem::new(name.clone(), name, kind))
                    .collect();

                Ok((start, items))
            }
            CompletionContext::Variable { prefix, start } => {
                let mut candidates = BTreeSet::new();

                for var in &self.cached_vars {
                    if var.starts_with(prefix) {
                        candidates.insert(format!("${var}"));
                    }
                }

                for (key, _) in std::env::vars() {
                    if key.starts_with(prefix) {
                        candidates.insert(format!("${key}"));
                    }
                }

                let items = candidates
                    .into_iter()
                    .map(|c| CompletionItem::new(c.clone(), c, CompletionKind::Variable))
                    .collect();

                Ok((start, items))
            }
            CompletionContext::Path => {
                let (start, pairs) = self.file_completer.complete(line, pos, ctx)?;
                let items = pairs
                    .into_iter()
                    .map(|p| {
                        let is_dir = p.replacement.ends_with('/')
                            || p.display.ends_with('/')
                            || Path::new(&p.replacement).is_dir();
                        let kind = if is_dir {
                            CompletionKind::Directory
                        } else {
                            CompletionKind::File
                        };
                        CompletionItem::new(p.replacement, p.display, kind)
                    })
                    .collect();
                Ok((start, items))
            }
        }
    }
}

impl Completer for LineEditorHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let (start, items) = self.get_candidates(line, pos, ctx)?;
        if items.is_empty() {
            return Ok((start, Vec::new()));
        }

        if self.completion_type == CompletionType::Menu && items.len() > 1 {
            let base_prefix = &line[start..pos];
            let on_descend = |replacement: &str| -> Vec<CompletionItem> {
                let new_line = format!("{}{}", &line[..start], replacement);
                let new_pos = start + replacement.len();
                self.get_candidates(&new_line, new_pos, ctx)
                    .map(|(_, items)| items)
                    .unwrap_or_default()
            };
            match menu::show_completion_menu(
                base_prefix,
                &items,
                7,
                self.nerd_icons,
                on_descend,
            ) {
                menu::MenuResult::Selected(item) => Ok((
                    start,
                    vec![Pair {
                        display: item.display,
                        replacement: item.replacement,
                    }],
                )),
                menu::MenuResult::Typed(typed) => Ok((
                    start,
                    vec![Pair {
                        display: typed.clone(),
                        replacement: typed,
                    }],
                )),
                menu::MenuResult::Cancelled => Ok((start, Vec::new())),
            }
        } else {
            let pairs = items
                .into_iter()
                .map(|item| Pair {
                    display: item.display,
                    replacement: item.replacement,
                })
                .collect();
            Ok((start, pairs))
        }
    }
}

impl Hinter for LineEditorHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        // 1. Try history-based ghost text first (highest relevance)
        if let Some(hint) = self.history_hinter.hint(line, pos, ctx) {
            return Some(hint);
        }

        // 2. Fallback: single / top completion candidate when typing at the end of the line
        if !line.is_empty()
            && pos == line.len()
            && let Ok((start, items)) = self.get_candidates(line, pos, ctx)
            && !items.is_empty()
        {
            let replacement = &items[0].replacement;
            let typed_prefix = &line[start..pos];
            if replacement.starts_with(typed_prefix) && replacement.len() > typed_prefix.len() {
                return Some(replacement[typed_prefix.len()..].to_string());
            }
        }

        None
    }
}

impl Validator for LineEditorHelper {}

impl Highlighter for LineEditorHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.syntax.highlight(line, pos)
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        self.osc133.highlight_prompt(prompt, default)
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[38;5;242m{hint}\x1b[0m"))
    }

    fn highlight_char(&self, line: &str, pos: usize, kind: CmdKind) -> bool {
        self.syntax.highlight_char(line, pos, kind)
    }
}

impl Helper for LineEditorHelper {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_initial_command_context() {
        assert_eq!(
            classify_completion_context("gi", 2),
            CompletionContext::Command {
                prefix: "gi",
                start: 0,
            }
        );
        assert_eq!(
            classify_completion_context("  c", 3),
            CompletionContext::Command {
                prefix: "c",
                start: 2,
            }
        );
        assert_eq!(
            classify_completion_context("", 0),
            CompletionContext::Command {
                prefix: "",
                start: 0,
            }
        );
    }

    #[test]
    fn classifies_pipeline_and_chained_command_context() {
        assert_eq!(
            classify_completion_context("cat foo | gr", 12),
            CompletionContext::Command {
                prefix: "gr",
                start: 10,
            }
        );
        assert_eq!(
            classify_completion_context("echo ok && ca", 13),
            CompletionContext::Command {
                prefix: "ca",
                start: 11,
            }
        );
        assert_eq!(
            classify_completion_context("false || cd", 11),
            CompletionContext::Command {
                prefix: "cd",
                start: 9,
            }
        );
        assert_eq!(
            classify_completion_context("echo a; ls", 10),
            CompletionContext::Command {
                prefix: "ls",
                start: 8,
            }
        );
    }

    #[test]
    fn classifies_path_context_after_command() {
        assert_eq!(
            classify_completion_context("ls ", 3),
            CompletionContext::Path
        );
        assert_eq!(
            classify_completion_context("cat src/m", 9),
            CompletionContext::Path
        );
        assert_eq!(
            classify_completion_context("./script", 8),
            CompletionContext::Path
        );
        assert_eq!(
            classify_completion_context("/bin/s", 6),
            CompletionContext::Path
        );
    }

    #[test]
    fn classifies_variable_context() {
        assert_eq!(
            classify_completion_context("echo $P", 7),
            CompletionContext::Variable {
                prefix: "P",
                start: 5,
            }
        );
        assert_eq!(
            classify_completion_context("VAR=$HO", 7),
            CompletionContext::Variable {
                prefix: "HO",
                start: 4,
            }
        );
    }

    #[test]
    fn formats_ghost_text_with_dimmed_ansi_style() {
        let helper = LineEditorHelper::new();
        let hint = helper.highlight_hint("eckout main");
        assert_eq!(hint, "\x1b[38;5;242meckout main\x1b[0m");
    }

    #[test]
    fn completes_commands_including_builtins_aliases_and_binaries() {
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        helper.set_completion_type(CompletionType::List);
        helper.cached_binaries = vec!["cargo".to_string(), "cat".to_string()];
        helper.set_aliases(vec!["co".to_string()]);

        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, candidates) = helper.complete("c", 1, &ctx).expect("completion failed");
        assert_eq!(start, 0);

        let names: Vec<String> = candidates.into_iter().map(|p| p.replacement).collect();
        assert!(names.contains(&"cd".to_string())); // Builtin
        assert!(names.contains(&"co".to_string())); // Alias
        assert!(names.contains(&"cargo".to_string())); // Binary
        assert!(names.contains(&"cat".to_string())); // Binary
    }

    #[test]
    fn candidates_have_correct_category_badges() {
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        helper.cached_binaries = vec!["cargo".to_string()];
        helper.set_aliases(vec!["co".to_string()]);

        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, items) = helper
            .get_candidates("c", 1, &ctx)
            .expect("completion failed");
        assert_eq!(start, 0);

        let builtin_item = items.iter().find(|it| it.replacement == "cd").unwrap();
        assert_eq!(builtin_item.kind, CompletionKind::Builtin);

        let alias_item = items.iter().find(|it| it.replacement == "co").unwrap();
        assert_eq!(alias_item.kind, CompletionKind::Alias);

        let bin_item = items.iter().find(|it| it.replacement == "cargo").unwrap();
        assert_eq!(bin_item.kind, CompletionKind::Binary);
    }

    #[test]
    fn completes_with_menu_type_returns_single_selected_item() {
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        helper.cached_binaries = vec!["cargo".to_string(), "cat".to_string()];

        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, candidates) = helper.complete("c", 1, &ctx).expect("completion failed");
        assert_eq!(start, 0);
        // In non-interactive test environment, menu resolves to 1 candidate
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn completes_variables_with_dollar_prefix() {
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        helper.set_vars(vec!["MY_SHELL_VAR".to_string()]);

        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, candidates) = helper
            .complete("echo $MY_SHELL", 14, &ctx)
            .expect("completion failed");
        assert_eq!(start, 5);

        let names: Vec<String> = candidates.into_iter().map(|p| p.replacement).collect();
        assert_eq!(names, vec!["$MY_SHELL_VAR".to_string()]);
    }

    #[test]
    fn completes_files_in_path_context() {
        use rustyline::history::DefaultHistory;

        let helper = LineEditorHelper::new();
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, candidates) = helper
            .complete("cat src/ex", 10, &ctx)
            .expect("completion failed");
        assert_eq!(start, 4);

        let names: Vec<String> = candidates.into_iter().map(|p| p.replacement).collect();
        assert!(names.iter().any(|n| n.contains("expand.rs")));
    }

    #[test]
    fn hints_ghost_text_from_history() {
        use rustyline::history::{DefaultHistory, History};

        let helper = LineEditorHelper::new();
        let mut history = DefaultHistory::new();
        history.add("git checkout main").unwrap();
        let ctx = Context::new(&history);

        let hint = helper.hint("git ch", 6, &ctx);
        assert_eq!(hint, Some("eckout main".to_string()));
    }

    #[test]
    fn hints_ghost_text_fallback_from_top_completion() {
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        helper.cached_binaries = vec!["unique_command".to_string()];
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let hint = helper.hint("uniq", 4, &ctx);
        assert_eq!(hint, Some("ue_command".to_string()));
    }
}
