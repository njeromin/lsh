/// Result of input validation when Enter is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationResult {
    /// Command is complete and ready to execute.
    Complete,
    /// Command is incomplete (unclosed brackets, quotes, trailing delimiters); insert newline.
    Incomplete,
}

/// A trait for validating input completeness and providing indentation hints.
pub trait Validator: Send + Sync {
    /// Validates the input line to check if it is complete or incomplete.
    fn validate(&self, line: &str) -> ValidationResult;

    /// Checks whether pressing Enter after `line_prefix` (the text on the current line
    /// up to the cursor) should trigger an additional level of indentation.
    /// Default implementation returns `false`.
    fn should_indent(&self, _line_prefix: &str) -> bool {
        false
    }

    /// Checks whether typing `c` at the start of an indented line should automatically dedent
    /// by one indentation level (e.g., closing delimiters like `}`, `]`, `)`).
    /// Default implementation returns `false`.
    fn should_dedent_on_char(&self, _c: char) -> bool {
        false
    }
}

// Blanket implementation allowing closures `Fn(&str) -> ValidationResult` to act as validators.
impl<F> Validator for F
where
    F: Fn(&str) -> ValidationResult + Send + Sync,
{
    fn validate(&self, line: &str) -> ValidationResult {
        self(line)
    }
}

/// A standard validator that detects unclosed quotes, brackets, braces, and trailing backslashes.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultValidator;

impl Validator for DefaultValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        if line.ends_with('\\') {
            return ValidationResult::Incomplete;
        }

        let mut in_single_quote = false;
        let mut in_double_quote = false;
        let mut escaped = false;
        let mut paren_depth: usize = 0;
        let mut bracket_depth: usize = 0;
        let mut brace_depth: usize = 0;

        for c in line.chars() {
            if escaped {
                escaped = false;
                continue;
            }
            if c == '\\' && !in_single_quote {
                escaped = true;
                continue;
            }
            if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
                continue;
            }
            if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
                continue;
            }

            if in_single_quote || in_double_quote {
                continue;
            }

            match c {
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                '[' => bracket_depth += 1,
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                _ => {}
            }
        }

        if in_single_quote
            || in_double_quote
            || paren_depth > 0
            || bracket_depth > 0
            || brace_depth > 0
        {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }

    fn should_indent(&self, line_prefix: &str) -> bool {
        let trimmed = line_prefix.trim_end();
        trimmed.ends_with('{')
            || trimmed.ends_with('(')
            || trimmed.ends_with('[')
            || trimmed.ends_with(':')
    }

    fn should_dedent_on_char(&self, c: char) -> bool {
        matches!(c, '}' | ']' | ')')
    }
}

/// A configurable validator that detects completion and indentation rules based on custom delimiters.
#[derive(Debug, Clone)]
pub struct DelimiterValidator {
    indent_suffixes: Vec<String>,
    dedent_chars: Vec<char>,
}

impl Default for DelimiterValidator {
    fn default() -> Self {
        Self {
            indent_suffixes: vec!["{".into(), "(".into(), "[".into(), ":".into()],
            dedent_chars: vec!['}', ')', ']'],
        }
    }
}

impl DelimiterValidator {
    /// Create a new `DelimiterValidator` with default delimiters (`{`, `(`, `[`, `:`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom line suffixes that trigger indentation on Enter (e.g. `["{", "then", "do"]`).
    pub fn with_indent_suffixes<I, S>(mut self, suffixes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.indent_suffixes = suffixes.into_iter().map(Into::into).collect();
        self
    }

    /// Set custom characters that trigger dedentation when typed at the start of an indented line.
    pub fn with_dedent_chars(mut self, chars: impl IntoIterator<Item = char>) -> Self {
        self.dedent_chars = chars.into_iter().collect();
        self
    }
}

impl Validator for DelimiterValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        DefaultValidator.validate(line)
    }

    fn should_indent(&self, line_prefix: &str) -> bool {
        let trimmed = line_prefix.trim_end();
        self.indent_suffixes
            .iter()
            .any(|suffix| trimmed.ends_with(suffix))
    }

    fn should_dedent_on_char(&self, c: char) -> bool {
        self.dedent_chars.contains(&c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_validator_complete() {
        let val = DefaultValidator;
        assert_eq!(val.validate("echo hello"), ValidationResult::Complete);
        assert_eq!(val.validate("fn foo() {}"), ValidationResult::Complete);
        assert_eq!(val.validate("echo 'hello'"), ValidationResult::Complete);
    }

    #[test]
    fn test_default_validator_incomplete() {
        let val = DefaultValidator;
        assert_eq!(val.validate("fn foo() {"), ValidationResult::Incomplete);
        assert_eq!(val.validate("echo 'unclosed"), ValidationResult::Incomplete);
        assert_eq!(val.validate("echo \"unclosed"), ValidationResult::Incomplete);
        assert_eq!(val.validate("echo [1, 2,"), ValidationResult::Incomplete);
        assert_eq!(val.validate("echo hello \\"), ValidationResult::Incomplete);
    }

    #[test]
    fn test_default_validator_indent_and_dedent() {
        let val = DefaultValidator;
        assert!(val.should_indent("fn foo() {"));
        assert!(val.should_indent("let arr = ["));
        assert!(val.should_indent("if True:"));
        assert!(!val.should_indent("let x = 10;"));

        assert!(val.should_dedent_on_char('}'));
        assert!(val.should_dedent_on_char(')'));
        assert!(val.should_dedent_on_char(']'));
        assert!(!val.should_dedent_on_char(';'));
        assert!(!val.should_dedent_on_char('a'));
    }

    #[test]
    fn test_custom_delimiter_validator() {
        let val = DelimiterValidator::new()
            .with_indent_suffixes(["then", "do"])
            .with_dedent_chars([';']);

        assert!(val.should_indent("if [ $x -eq 1 ]; then"));
        assert!(val.should_indent("for i in 1 2 3; do"));
        assert!(!val.should_indent("echo hello"));

        assert!(val.should_dedent_on_char(';'));
        assert!(!val.should_dedent_on_char('}'));
    }
}
