use std::ops::{Deref, DerefMut};

use crossterm::style::ContentStyle;

/// A collection of styled text segments representing formatted input.
///
/// Wraps a `Vec<(ContentStyle, String)>` and implements `Deref` to provide
/// direct slice/vector access alongside ergonomic helper methods.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyledText(pub Vec<(ContentStyle, String)>);

impl StyledText {
    /// Create a new, empty `StyledText`.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Push a styled span into the collection.
    pub fn push(&mut self, style: ContentStyle, text: impl Into<String>) {
        self.0.push((style, text.into()));
    }

    /// Create plain, unstyled text.
    pub fn plain(text: impl Into<String>) -> Self {
        let mut st = Self::new();
        st.push(ContentStyle::default(), text);
        st
    }

    /// Access the underlying slice of `(ContentStyle, String)` pairs.
    pub fn spans(&self) -> &[(ContentStyle, String)] {
        &self.0
    }
}

impl Deref for StyledText {
    type Target = [(ContentStyle, String)];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StyledText {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl IntoIterator for StyledText {
    type Item = (ContentStyle, String);
    type IntoIter = std::vec::IntoIter<(ContentStyle, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a StyledText {
    type Item = &'a (ContentStyle, String);
    type IntoIter = std::slice::Iter<'a, (ContentStyle, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

// Convert from raw Vec<(ContentStyle, String)>
impl From<Vec<(ContentStyle, String)>> for StyledText {
    fn from(vec: Vec<(ContentStyle, String)>) -> Self {
        Self(vec)
    }
}

// Convert into raw Vec<(ContentStyle, String)>
impl From<StyledText> for Vec<(ContentStyle, String)> {
    fn from(st: StyledText) -> Self {
        st.0
    }
}

// Convert from a string literal
impl From<&str> for StyledText {
    fn from(s: &str) -> Self {
        Self::plain(s)
    }
}

// Convert from an owned String
impl From<String> for StyledText {
    fn from(s: String) -> Self {
        Self::plain(s)
    }
}

/// A trait for formatting the raw input buffer before rendering to the terminal.
///
/// Implementations can return a `StyledText` collection with custom colors,
/// styles, error underlines, or masked text.
pub trait TextFormatter: Send + Sync {
    /// Formats the raw input line into styled text segments.
    fn format(&self, line: &str) -> StyledText;
}

// Blanket implementation allowing any closure returning a type that converts into `StyledText`
// (e.g. `StyledText`, `String`, `&str`, or `Vec<(ContentStyle, String)>`).
impl<F, T> TextFormatter for F
where
    F: Fn(&str) -> T + Send + Sync,
    T: Into<StyledText>,
{
    fn format(&self, line: &str) -> StyledText {
        self(line).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::style::{Color, Stylize};

    #[test]
    fn test_styled_text_push_and_deref() {
        let mut st = StyledText::new();
        st.push(ContentStyle::new().green(), "hello");
        st.push(ContentStyle::new().red(), " world");

        assert_eq!(st.len(), 2);
        assert_eq!(st[0].1, "hello");
        assert_eq!(st[1].1, " world");

        // Iterate over spans
        let joined: String = st.iter().map(|(_, s)| s.as_str()).collect();
        assert_eq!(joined, "hello world");
    }

    #[test]
    fn test_from_conversions() {
        let from_str: StyledText = "hello".into();
        assert_eq!(from_str.len(), 1);
        assert_eq!(from_str[0].1, "hello");

        let raw_vec = vec![(ContentStyle::default(), "test".to_string())];
        let from_vec: StyledText = raw_vec.into();
        assert_eq!(from_vec.len(), 1);
    }

    #[test]
    fn test_closure_formatter_with_string() {
        let formatter = |s: &str| format!("formatted: {s}");
        let res = formatter.format("cmd");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].1, "formatted: cmd");
    }

    #[test]
    fn test_closure_formatter_with_styled_text() {
        let formatter = |s: &str| {
            let mut st = StyledText::new();
            st.push(ContentStyle::new().yellow(), s);
            st
        };
        let res = formatter.format("123");
        assert_eq!(res[0].0.foreground_color, Some(Color::Yellow));
        assert_eq!(res[0].1, "123");
    }
}
