/// A trait for providing inline ghost text (autosuggestions) ahead of the cursor.
pub trait Hinter: Send + Sync {
    /// Returns ghost text to display inline after `cursor`, or `None` if no suggestion is available.
    fn hint(&self, line: &str, cursor: usize) -> Option<String>;

    /// Extracts the next word or token from a hint string to support word-by-word acceptance (e.g. `Alt+Right`).
    fn next_hint_word<'a>(&self, hint: &'a str) -> &'a str {
        default_next_hint_word(hint)
    }
}

// Blanket implementation allowing closures `Fn(&str, usize) -> Option<String>` to act as hinters.
impl<F> Hinter for F
where
    F: Fn(&str, usize) -> Option<String> + Send + Sync,
{
    fn hint(&self, line: &str, cursor: usize) -> Option<String> {
        self(line, cursor)
    }
}

/// Extracts the first word (including any attached trailing space) from a ghost text hint.
pub fn default_next_hint_word(hint: &str) -> &str {
    if hint.is_empty() {
        return "";
    }

    let mut char_indices = hint.char_indices().peekable();

    // 1. Skip any initial whitespace (if the hint starts with a space)
    while let Some(&(_, c)) = char_indices.peek() {
        if c.is_whitespace() {
            char_indices.next();
        } else {
            break;
        }
    }

    // 2. Consume the non-whitespace word characters
    while let Some(&(_, c)) = char_indices.peek() {
        if !c.is_whitespace() {
            char_indices.next();
        } else {
            break;
        }
    }

    // 3. Consume any trailing whitespace immediately following the word
    while let Some(&(_, c)) = char_indices.peek() {
        if c.is_whitespace() {
            char_indices.next();
        } else {
            break;
        }
    }

    let end_byte = char_indices
        .peek()
        .map(|&(idx, _)| idx)
        .unwrap_or(hint.len());
    &hint[..end_byte]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_next_hint_word() {
        assert_eq!(default_next_hint_word(""), "");
        assert_eq!(default_next_hint_word("checkout -b main"), "checkout ");
        assert_eq!(default_next_hint_word("-b main"), "-b ");
        assert_eq!(default_next_hint_word("main"), "main");
        assert_eq!(default_next_hint_word("  hello  world"), "  hello  ");
    }

    #[test]
    fn test_closure_hinter() {
        let hinter = |line: &str, cursor: usize| {
            if line == "git c" && cursor == 5 {
                Some("heckout".to_string())
            } else {
                None
            }
        };

        assert_eq!(hinter.hint("git c", 5), Some("heckout".to_string()));
        assert_eq!(hinter.hint("git", 3), None);
        assert_eq!(hinter.next_hint_word("heckout"), "heckout");
    }
}
