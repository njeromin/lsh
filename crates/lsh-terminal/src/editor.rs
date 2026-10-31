use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

use rustyline::{
    Context, Helper,
    completion::{Completer, FilenameCompleter, Pair},
    highlight::{CmdKind, Highlighter},
    hint::{Hinter, HistoryHinter},
    validate::Validator,
};

use lsh_completions::{CompletionItem, CompletionKind, CompletionRegistry, CompletionResolution};
use lsh_eval::builtins::BUILTIN_NAMES;

use crate::{
    highlight::SyntaxHighlighter,
    menu,
    options::CompletionType,
    osc::{self, Osc133PromptHighlighter},
};

#[derive(Debug, PartialEq, Eq)]
pub enum CompletionContext<'a> {
    Command { prefix: &'a str, start: usize },
    Variable { prefix: &'a str, start: usize },
    Path { command: &'a str },
}

pub fn extract_command_name(trimmed: &str) -> &str {
    let mut words = trimmed.split_whitespace();
    while let Some(word) = words.next() {
        if word.contains('=') && !word.starts_with("./") && !word.starts_with('/') {
            continue;
        }
        if (word == "builtin" || word == "command")
            && let Some(next) = words.next()
        {
            return next;
        }
        return word;
    }
    ""
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

fn split_path_argument(sub: &str) -> (usize, &str, &str) {
    let arg_start = sub
        .rfind(|c: char| c.is_whitespace() || matches!(c, '=' | '"' | '\''))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    let path_arg = &sub[arg_start..];

    if let Some(slash_idx) = path_arg.rfind('/') {
        let dir_part = &path_arg[..=slash_idx];
        let file_prefix = &path_arg[slash_idx + 1..];
        (arg_start, dir_part, file_prefix)
    } else {
        (arg_start, "", path_arg)
    }
}

pub fn parse_command_args(segment: &str) -> (Option<&str>, Vec<String>, &str, usize) {
    let mut tokens: Vec<(&str, usize)> = Vec::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut token_start: Option<usize> = None;

    let chars: Vec<(usize, char)> = segment.char_indices().collect();
    for &(idx, c) in &chars {
        if c == '\'' && !in_double {
            in_single = !in_single;
            if token_start.is_none() {
                token_start = Some(idx);
            }
        } else if c == '"' && !in_single {
            in_double = !in_double;
            if token_start.is_none() {
                token_start = Some(idx);
            }
        } else if c.is_whitespace() && !in_single && !in_double {
            if let Some(start) = token_start {
                tokens.push((&segment[start..idx], start));
                token_start = None;
            }
        } else if token_start.is_none() {
            token_start = Some(idx);
        }
    }

    let ends_with_ws = segment.ends_with(char::is_whitespace) && !in_single && !in_double;
    let (cur_word, cur_word_start) = if ends_with_ws {
        ("", segment.len())
    } else if let Some(start) = token_start {
        (&segment[start..], start)
    } else {
        ("", segment.len())
    };

    let mut words = tokens.into_iter();
    let mut cmd_name = None;

    while let Some((word, _)) = words.next() {
        let clean = word.trim_matches(|c| c == '"' || c == '\'');
        if clean.contains('=')
            && !clean.starts_with('-')
            && !clean.starts_with("./")
            && !clean.starts_with('/')
        {
            continue;
        }
        if (clean == "builtin" || clean == "command")
            && let Some((next, _)) = words.next()
        {
            cmd_name = Some(next.trim_matches(|c| c == '"' || c == '\''));
            break;
        }
        cmd_name = Some(clean);
        break;
    }

    let Some(cmd) = cmd_name else {
        return (None, Vec::new(), cur_word, cur_word_start);
    };

    let args: Vec<String> = words
        .map(|(w, _)| w.trim_matches(|c| c == '"' || c == '\'').to_string())
        .collect();

    (Some(cmd), args, cur_word, cur_word_start)
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
            return CompletionContext::Path { command: "" };
        }

        let segment_start_in_sub = sub.len() - segment.len();
        let start = segment_start_in_sub + leading_spaces;
        return CompletionContext::Command {
            prefix: trimmed,
            start,
        };
    }

    let command = extract_command_name(trimmed);
    CompletionContext::Path { command }
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
    pub completion_registry: Arc<RwLock<CompletionRegistry>>,
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
            completion_registry: Arc::new(RwLock::new(CompletionRegistry::with_defaults())),
        }
    }

    pub fn set_completion_registry(&mut self, registry: Arc<RwLock<CompletionRegistry>>) {
        self.completion_registry = registry;
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
                    map.insert(binary.clone(), CompletionKind::Binary);
                }

                for alias in &self.cached_aliases {
                    map.insert(alias.clone(), CompletionKind::Alias);
                }

                for &builtin in BUILTIN_NAMES {
                    map.insert(builtin.to_string(), CompletionKind::Builtin);
                }

                let all_items: Vec<CompletionItem> = map
                    .into_iter()
                    .map(|(name, kind)| CompletionItem::new(name.clone(), name, kind))
                    .collect();

                let items = menu::filter_completion_items(&all_items, prefix);

                Ok((start, items))
            }
            CompletionContext::Variable { prefix, start } => {
                let mut candidates = BTreeSet::new();

                for var in &self.cached_vars {
                    candidates.insert(var.clone());
                }

                for (key, _) in std::env::vars() {
                    candidates.insert(key);
                }

                let all_items: Vec<CompletionItem> = candidates
                    .into_iter()
                    .map(|name| {
                        CompletionItem::new(
                            format!("${name}"),
                            format!("${name}"),
                            CompletionKind::Variable,
                        )
                    })
                    .collect();

                let query = if prefix.starts_with('$') {
                    prefix.to_string()
                } else {
                    format!("${prefix}")
                };
                let items = menu::filter_completion_items(&all_items, &query);

                Ok((start, items))
            }
            CompletionContext::Path { command: _ } => {
                let sub = if pos <= line.len() {
                    &line[..pos]
                } else {
                    line
                };
                let segment = find_command_segment(sub);
                let segment_start_in_sub = sub.len() - segment.len();
                let (cmd_opt, args, cur_word, cur_word_start_in_segment) =
                    parse_command_args(segment);
                let abs_cur_word_start = segment_start_in_sub + cur_word_start_in_segment;

                let mut dirs_only_override = false;
                let mut exts_filter: Option<Vec<String>> = None;

                if let Some(cmd) = cmd_opt {
                    let reg = self.completion_registry.read().unwrap();
                    if let Some(resolution) = reg.resolve(cmd, &args, cur_word) {
                        match resolution {
                            CompletionResolution::Items(items) => {
                                let filtered = if cur_word.is_empty() {
                                    items
                                } else {
                                    menu::filter_completion_items(&items, cur_word)
                                };
                                return Ok((abs_cur_word_start, filtered));
                            }
                            CompletionResolution::DirectoriesOnly => {
                                dirs_only_override = true;
                            }
                            CompletionResolution::FileExtensions(exts) => {
                                exts_filter = Some(exts);
                            }
                        }
                    }
                }

                let (arg_start, dir_part, file_prefix) = split_path_argument(sub);
                let dir_line = format!("{}{}", &sub[..arg_start], dir_part);
                let dir_pos = dir_line.len();

                let (start, pairs) = self
                    .file_completer
                    .complete(&dir_line, dir_pos, ctx)
                    .unwrap_or((arg_start, Vec::new()));

                let base_cmd = cmd_opt.unwrap_or("").rsplit('/').next().unwrap_or("");
                let dirs_only = dirs_only_override || base_cmd == "cd" || base_cmd == "pushd";

                let all_items: Vec<CompletionItem> = pairs
                    .into_iter()
                    .filter_map(|p| {
                        let is_dir = p.replacement.ends_with('/')
                            || p.display.ends_with('/')
                            || Path::new(&p.replacement).is_dir();
                        if dirs_only && !is_dir {
                            return None;
                        }
                        if let Some(exts) = &exts_filter
                            && !is_dir
                        {
                            let p_ext = p.replacement.rsplit('.').next().unwrap_or("");
                            if !exts.iter().any(|e| e == p_ext) {
                                return None;
                            }
                        }
                        let kind = if is_dir {
                            CompletionKind::Directory
                        } else {
                            CompletionKind::File
                        };
                        Some(CompletionItem::new(p.replacement, p.display, kind))
                    })
                    .collect();

                let items = if file_prefix.is_empty() {
                    all_items
                } else {
                    menu::filter_completion_items(&all_items, file_prefix)
                };

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
            match menu::show_completion_menu(base_prefix, &items, 7, self.nerd_icons, on_descend) {
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
        // Try history-based ghost text first (highest relevance)
        if let Some(hint) = self.history_hinter.hint(line, pos, ctx) {
            return Some(hint);
        }

        // Fallback: single / top completion candidate when typing at the end of the line with a non-empty prefix
        if !line.is_empty()
            && pos == line.len()
            && let Ok((start, items)) = self.get_candidates(line, pos, ctx)
            && !items.is_empty()
        {
            let replacement = &items[0].replacement;
            let typed_prefix = &line[start..pos];
            if !typed_prefix.is_empty()
                && replacement.starts_with(typed_prefix)
                && replacement.len() > typed_prefix.len()
            {
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
            CompletionContext::Path { command: "ls" }
        );
        assert_eq!(
            classify_completion_context("cat src/m", 9),
            CompletionContext::Path { command: "cat" }
        );
        assert_eq!(
            classify_completion_context("./script", 8),
            CompletionContext::Path { command: "" }
        );
        assert_eq!(
            classify_completion_context("/bin/s", 6),
            CompletionContext::Path { command: "" }
        );
        assert_eq!(
            classify_completion_context("cd /tmp", 7),
            CompletionContext::Path { command: "cd" }
        );
        assert_eq!(
            classify_completion_context("FOO=bar pushd /tmp", 18),
            CompletionContext::Path { command: "pushd" }
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
            .complete("cat src/ed", 10, &ctx)
            .expect("completion failed");
        assert_eq!(start, 4);

        let names: Vec<String> = candidates.into_iter().map(|p| p.replacement).collect();
        assert!(names.iter().any(|n| n.contains("editor.rs")));
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

    #[test]
    fn does_not_hint_ghost_text_on_empty_prefix_or_after_quote() {
        use rustyline::history::DefaultHistory;

        let helper = LineEditorHelper::new();
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        // After a space (e.g. "cat ")
        assert_eq!(helper.hint("cat ", 4, &ctx), None);

        // After a closing quote (e.g. git commit -m 'feat: test')
        assert_eq!(helper.hint("git commit -m 'feat: test'", 26, &ctx), None);
    }

    #[test]
    fn completes_commands_with_direct_fuzzy_matching() {
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        helper.cached_binaries = vec!["cut".to_string(), "cat".to_string(), "echo".to_string()];

        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, items) = helper
            .get_candidates("ct", 2, &ctx)
            .expect("completion failed");
        assert_eq!(start, 0);
        assert!(items.len() >= 2);
        assert!(items.iter().any(|it| it.replacement == "cat"));
        assert!(items.iter().any(|it| it.replacement == "cut"));
        assert!(!items.iter().any(|it| it.replacement == "echo"));

        let cat_item = items.iter().find(|it| it.replacement == "cat").unwrap();
        assert_eq!(cat_item.match_indices, vec![0, 2]);
    }

    #[test]
    fn completes_single_fuzzy_match_directly() {
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        helper.cached_binaries = vec!["docker-compose".to_string(), "cargo".to_string()];

        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, candidates) = helper.complete("dco", 3, &ctx).expect("completion failed");
        assert_eq!(start, 0);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].replacement, "docker-compose");
    }

    #[test]
    fn completes_variables_with_direct_fuzzy_matching() {
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        helper.set_vars(vec!["CONFIG_PATH".to_string(), "OTHER_VAR".to_string()]);

        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, items) = helper
            .get_candidates("echo $cp", 8, &ctx)
            .expect("completion failed");
        assert_eq!(start, 5);
        assert!(items.iter().any(|it| it.replacement == "$CONFIG_PATH"));
        assert!(!items.iter().any(|it| it.replacement == "$OTHER_VAR"));
    }

    #[test]
    fn completes_paths_with_direct_fuzzy_matching() {
        use rustyline::history::DefaultHistory;

        let helper = LineEditorHelper::new();
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, items) = helper
            .get_candidates("cat ct", 6, &ctx)
            .expect("completion failed");
        assert_eq!(start, 4);
        assert!(items.iter().any(|it| it.replacement.contains("Cargo.toml")));
    }

    #[test]
    fn completes_directories_only_for_cd_and_pushd_commands() {
        use rustyline::history::DefaultHistory;

        let helper = LineEditorHelper::new();
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        // "cd " should only return directories, not regular files like Cargo.toml
        let (start, cd_items) = helper
            .get_candidates("cd ", 3, &ctx)
            .expect("completion failed");
        assert_eq!(start, 3);
        assert!(!cd_items.is_empty());
        assert!(
            cd_items
                .iter()
                .all(|it| it.kind == CompletionKind::Directory)
        );
        assert!(cd_items.iter().any(|it| it.replacement.contains("src")));
        assert!(
            !cd_items
                .iter()
                .any(|it| it.replacement.contains("Cargo.toml"))
        );

        // "pushd " should also only return directories
        let (_, pushd_items) = helper
            .get_candidates("pushd ", 6, &ctx)
            .expect("completion failed");
        assert!(
            pushd_items
                .iter()
                .all(|it| it.kind == CompletionKind::Directory)
        );
        assert!(
            !pushd_items
                .iter()
                .any(|it| it.replacement.contains("Cargo.toml"))
        );
    }

    #[test]
    fn completes_both_files_and_directories_for_general_commands() {
        use rustyline::history::DefaultHistory;

        let helper = LineEditorHelper::new();
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        // "cat " or "ls " should return both files and directories
        let (_, items) = helper
            .get_candidates("ls ", 3, &ctx)
            .expect("completion failed");
        assert!(items.iter().any(|it| it.kind == CompletionKind::Directory));
        assert!(items.iter().any(|it| it.replacement.contains("Cargo.toml")));
    }

    #[test]
    fn parse_command_args_extracts_command_subcommands_and_cur_word() {
        let (cmd, args, cur, start) = parse_command_args("git commit -m ");
        assert_eq!(cmd, Some("git"));
        assert_eq!(args, vec!["commit".to_string(), "-m".to_string()]);
        assert_eq!(cur, "");
        assert_eq!(start, 14);

        let (cmd, args, cur, start) = parse_command_args("git co");
        assert_eq!(cmd, Some("git"));
        assert_eq!(args, Vec::<String>::new());
        assert_eq!(cur, "co");
        assert_eq!(start, 4);

        let (cmd, args, cur, start) = parse_command_args("FOO=1 git 'checkout' feat/new");
        assert_eq!(cmd, Some("git"));
        assert_eq!(args, vec!["checkout".to_string()]);
        assert_eq!(cur, "feat/new");
        assert_eq!(start, 21);
    }

    #[test]
    fn completes_git_subcommands_and_flags() {
        use rustyline::history::DefaultHistory;

        let helper = LineEditorHelper::new();
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        // "git " with space at pos 4 should return git subcommands only, flags excluded
        let (start, items) = helper
            .get_candidates("git ", 4, &ctx)
            .expect("completion failed");
        assert_eq!(start, 4);
        assert!(
            items
                .iter()
                .any(|it| it.replacement == "commit" && it.kind == CompletionKind::Subcommand)
        );
        assert!(
            items
                .iter()
                .any(|it| it.replacement == "checkout" && it.kind == CompletionKind::Subcommand)
        );
        assert!(!items.iter().any(|it| it.replacement == "--version"));

        // "git -" at pos 5 should return flags only, subcommands excluded
        let (start, items) = helper
            .get_candidates("git -", 5, &ctx)
            .expect("completion failed");
        assert_eq!(start, 4);
        assert!(
            items
                .iter()
                .any(|it| it.replacement == "--version" && it.kind == CompletionKind::Flag)
        );
        assert!(!items.iter().any(|it| it.replacement == "commit"));

        // "git co" at pos 6 should filter to commit and checkout
        let (start, items) = helper
            .get_candidates("git co", 6, &ctx)
            .expect("completion failed");
        assert_eq!(start, 4);
        assert!(items.iter().any(|it| it.replacement == "commit"));
        assert!(items.iter().any(|it| it.replacement == "checkout"));
        assert!(!items.iter().any(|it| it.replacement == "branch"));
    }

    #[test]
    fn completes_git_commit_flags() {
        use rustyline::history::DefaultHistory;

        let helper = LineEditorHelper::new();
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        // "git commit -" at pos 12 should return commit flags
        let (start, items) = helper
            .get_candidates("git commit -", 12, &ctx)
            .expect("completion failed");
        assert_eq!(start, 11);
        assert!(
            items
                .iter()
                .any(|it| it.replacement == "-m" && it.kind == CompletionKind::Flag)
        );
        assert!(
            items
                .iter()
                .any(|it| it.replacement == "--amend" && it.kind == CompletionKind::Flag)
        );

        // "git commit " at pos 11 should fall back to file completion (e.g. Cargo.toml)
        let (_, items) = helper
            .get_candidates("git commit ", 11, &ctx)
            .expect("completion failed");
        assert!(items.iter().any(|it| it.replacement.contains("Cargo.toml")));
    }

    #[test]
    fn completes_cargo_subcommands() {
        use rustyline::history::DefaultHistory;

        let helper = LineEditorHelper::new();
        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, items) = helper
            .get_candidates("cargo ", 6, &ctx)
            .expect("completion failed");
        assert_eq!(start, 6);
        assert!(
            items
                .iter()
                .any(|it| it.replacement == "build" && it.kind == CompletionKind::Subcommand)
        );
        assert!(
            items
                .iter()
                .any(|it| it.replacement == "test" && it.kind == CompletionKind::Subcommand)
        );
    }

    #[test]
    fn completes_custom_registered_command() {
        use lsh_completions::{CandidateSpec, CompletionRule};
        use rustyline::history::DefaultHistory;

        let mut helper = LineEditorHelper::new();
        let reg = Arc::new(RwLock::new(CompletionRegistry::new()));
        let mut rule = CompletionRule::new();
        rule.candidates = vec![
            CandidateSpec::from_raw("start:Start server", CompletionKind::Value),
            CandidateSpec::from_raw("stop:Stop server", CompletionKind::Value),
        ];
        reg.write().unwrap().register("serverctl", rule);
        helper.set_completion_registry(reg);

        let history = DefaultHistory::new();
        let ctx = Context::new(&history);

        let (start, items) = helper
            .get_candidates("serverctl st", 12, &ctx)
            .expect("completion failed");
        assert_eq!(start, 10);
        assert!(items.iter().any(|it| it.replacement == "start"));
        assert!(items.iter().any(|it| it.replacement == "stop"));
    }
}
