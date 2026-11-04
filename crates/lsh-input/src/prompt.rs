use std::borrow::Cow;

pub trait Prompt {
    /// Renders the primary left prompt.
    fn render(&self) -> Cow<'_, str>;

    /// Renders the continuation indicator for subsequent lines in multiline input.
    fn render_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("… ")
    }
}

impl Prompt for &str {
    fn render(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }
}

impl Prompt for String {
    fn render(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }
}

impl<T: Prompt + ?Sized> Prompt for &T {
    fn render(&self) -> Cow<'_, str> {
        (**self).render()
    }

    fn render_multiline_indicator(&self) -> Cow<'_, str> {
        (**self).render_multiline_indicator()
    }
}

/// Strips ANSI CSI and OSC escape sequences from a string.
pub fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.next() {
                // CSI escape: \x1b[ ... [@-~]
                Some('[') => {
                    for next in chars.by_ref() {
                        if ('\x40'..='\x7e').contains(&next) {
                            break;
                        }
                    }
                }
                // OSC escape: \x1b] ... \x07 (BEL) or \x1b\ (ST)
                Some(']') => {
                    while let Some(next) = chars.next() {
                        if next == '\x07' {
                            break;
                        }
                        if next == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Character set selection: \x1b(X or \x1b)X
                Some('(') | Some(')') => {
                    chars.next();
                }
                _ => {}
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Computes the visual printable width on screen of a string, ignoring ANSI escape codes.
pub fn printable_width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(strip_ansi(s).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_csi_colors() {
        let text = "\x1b[1;34mhello\x1b[0m \x1b[32mworld\x1b[0m";
        assert_eq!(strip_ansi(text), "hello world");
        assert_eq!(printable_width(text), 11);
    }

    #[test]
    fn test_strip_ansi_osc_133_and_multiline() {
        let prompt = "\n\x1b]133;A\x07\x1b[1;34m~/dev/lsh\x1b[0m (main)\n\x1b]133;A;k=s\x07\x1b[35m❯ \x1b[0m\x1b]133;B\x07";
        let stripped = strip_ansi(prompt);
        assert_eq!(stripped, "\n~/dev/lsh (main)\n❯ ");
    }

    #[test]
    fn test_strip_ansi_osc_st_terminator() {
        let text = "\x1b]0;Title\x1b\\prompt> ";
        assert_eq!(strip_ansi(text), "prompt> ");
    }

    #[test]
    fn test_strip_ansi_charset_reset() {
        let text = "\x1b(Bhello";
        assert_eq!(strip_ansi(text), "hello");
    }
}
