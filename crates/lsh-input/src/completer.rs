/// A completion candidate proposed by a `Completer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The text to insert into the buffer if this candidate is accepted.
    pub replacement: String,
    /// The display text shown in the completion menu (defaults to `replacement`).
    pub display: String,
    /// Optional description text (e.g. documentation or flag help).
    pub description: Option<String>,
    /// Whether a trailing space should be appended when this candidate is committed.
    pub append_space: bool,
}

impl Candidate {
    /// Create a new candidate with the given replacement text.
    pub fn new(replacement: impl Into<String>) -> Self {
        let rep = replacement.into();
        Self {
            display: rep.clone(),
            replacement: rep,
            description: None,
            append_space: false,
        }
    }

    /// Set custom display text for the candidate.
    pub fn with_display(mut self, display: impl Into<String>) -> Self {
        self.display = display.into();
        self
    }

    /// Set an optional description for the candidate.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set whether a trailing space should be appended upon acceptance.
    pub fn with_append_space(mut self, append_space: bool) -> Self {
        self.append_space = append_space;
        self
    }
}

impl From<&str> for Candidate {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Candidate {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// A trait for providing completion candidates given the current line and cursor position.
pub trait Completer: Send + Sync {
    /// Returns the start byte offset of the token being completed in `line`,
    /// and a vector of candidate completions.
    fn complete(&self, line: &str, cursor: usize) -> (usize, Vec<Candidate>);
}

// Blanket implementation allowing closures to act as completers.
impl<F> Completer for F
where
    F: Fn(&str, usize) -> (usize, Vec<Candidate>) + Send + Sync,
{
    fn complete(&self, line: &str, cursor: usize) -> (usize, Vec<Candidate>) {
        self(line, cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_builder() {
        let c = Candidate::new("cargo")
            .with_display("cargo (bin)")
            .with_description("Rust package manager")
            .with_append_space(true);

        assert_eq!(c.replacement, "cargo");
        assert_eq!(c.display, "cargo (bin)");
        assert_eq!(c.description.as_deref(), Some("Rust package manager"));
        assert!(c.append_space);
    }

    #[test]
    fn test_closure_completer() {
        let completer = |line: &str, cursor: usize| {
            if line == "ca" && cursor == 2 {
                (
                    0,
                    vec![
                        Candidate::new("cargo").with_append_space(true),
                        Candidate::new("cat").with_append_space(true),
                    ],
                )
            } else {
                (cursor, Vec::new())
            }
        };

        let (start, items) = completer.complete("ca", 2);
        assert_eq!(start, 0);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].display, "cargo");
    }
}
