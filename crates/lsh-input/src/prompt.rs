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

pub fn printable_width(s: &str) -> usize {
    let mut width = 0;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.next() {
                // CSI escape: \x1b[ ... [A-Za-z]
                Some('[') => {
                    for next in chars.by_ref() {
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                // OSC escape: \x1b] ... \x07 (or \x1b\)
                Some(']') => {
                    for next in chars.by_ref() {
                        if next == '\x07' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else {
            // Not sure if its a good idea to just unwrap or 1.
            width += unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
        }
    }

    width
}
