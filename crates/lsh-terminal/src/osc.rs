use std::{
    borrow::Cow,
    fmt::Write as _,
    io,
    io::{IsTerminal, Write},
    path::Path,
};

pub struct Osc133PromptHighlighter {
    enabled: bool,
}

impl Osc133PromptHighlighter {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn reset_prompt_marks(&self) {}

    pub fn format_prompt<'a>(&self, prompt: &'a str) -> Cow<'a, str> {
        if !self.enabled {
            return Cow::Borrowed(prompt);
        }

        let (leading_newlines, rest) = match prompt.find(|c| c != '\n' && c != '\r') {
            Some(idx) => prompt.split_at(idx),
            None => ("", prompt),
        };

        if rest.contains('\n') {
            let multiline_marked = rest.replace('\n', "\n\x1b]133;A;k=s\x07");
            Cow::Owned(format!(
                "{leading_newlines}\x1b]133;A\x07{multiline_marked}\x1b]133;B\x07"
            ))
        } else {
            Cow::Owned(format!(
                "{leading_newlines}\x1b]133;A\x07{rest}\x1b]133;B\x07"
            ))
        }
    }
}

pub fn enabled() -> bool {
    io::stdout().is_terminal()
}

pub fn write_133(marker: &str) {
    if !enabled() {
        return;
    }

    let mut stdout = io::stdout();
    let _ = write!(stdout, "\x1b]133;{marker}\x07");
    let _ = stdout.flush();
}

/// Percent-encodes a path according to RFC 8089 URI requirements for file URIs.
pub fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for &byte in path.as_bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(byte as char);
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

/// Formats an OSC 7 working directory reporting escape sequence.
pub fn format_7_cwd(path: &Path, hostname: &str) -> Option<String> {
    let abs_path = std::path::absolute(path).ok()?;
    let path_str = abs_path.to_str()?;
    let encoded_path = percent_encode_path(path_str);
    let mut clean_hostname: String = hostname.chars().filter(|c| !c.is_control()).collect();
    if clean_hostname.is_empty() {
        clean_hostname = "localhost".to_string();
    }
    Some(format!("\x1b]7;file://{clean_hostname}{encoded_path}\x07"))
}

pub fn write_7_cwd(path: &Path) {
    if !enabled() {
        return;
    }

    let hostname = lsh_sys::get_hostname().unwrap_or_else(|| "localhost".to_string());
    write_7_cwd_with_host(path, &hostname);
}

pub fn write_7_cwd_with_host(path: &Path, hostname: &str) {
    if !enabled() {
        return;
    }

    let Some(sequence) = format_7_cwd(path, hostname) else {
        return;
    };

    let mut stdout = io::stdout();
    let _ = write!(stdout, "{sequence}");
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_prompt_wrapped_with_osc133_marks() {
        let highlighter = Osc133PromptHighlighter::new(true);
        let prompt = "❯ ";
        let formatted = highlighter.format_prompt(prompt);
        assert_eq!(formatted, "\x1b]133;A\x07❯ \x1b]133;B\x07");
    }

    #[test]
    fn leading_newlines_emitted_before_osc133_a_mark() {
        let highlighter = Osc133PromptHighlighter::new(true);
        let prompt = "\n/path/to/cwd\n❯ ";
        let formatted = highlighter.format_prompt(prompt);
        assert_eq!(
            formatted,
            "\n\x1b]133;A\x07/path/to/cwd\n\x1b]133;A;k=s\x07❯ \x1b]133;B\x07"
        );
    }

    #[test]
    fn disabled_highlighter_returns_borrowed_prompt_unchanged() {
        let highlighter = Osc133PromptHighlighter::new(false);
        let prompt = "\n❯ ";
        let formatted = highlighter.format_prompt(prompt);
        assert_eq!(formatted, prompt);
    }

    #[test]
    fn test_format_7_cwd() {
        let path = Path::new("/tmp/test_dir");
        let formatted = format_7_cwd(path, "myhost").unwrap();
        assert_eq!(formatted, "\x1b]7;file://myhost/tmp/test_dir\x07");
    }

    #[test]
    fn test_format_7_cwd_with_spaces_and_special_chars() {
        let path = Path::new("/tmp/my dir#special");
        let formatted = format_7_cwd(path, "myhost").unwrap();
        assert_eq!(
            formatted,
            "\x1b]7;file://myhost/tmp/my%20dir%23special\x07"
        );
    }

    #[test]
    fn test_format_7_cwd_empty_hostname_defaults_to_localhost() {
        let path = Path::new("/tmp/test");
        let formatted = format_7_cwd(path, "").unwrap();
        assert_eq!(formatted, "\x1b]7;file://localhost/tmp/test\x07");
    }
}
