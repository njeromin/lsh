/// Normalises pasted text by converting CRLF and CR line endings to standard LF (`\n`),
/// preserving valid whitespace (`\n`, `\t`) and printable Unicode characters,
/// and stripping non-printable control characters to prevent escape sequence injection.
pub fn normalise_paste(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                result.push('\n');
            }
            '\n' | '\t' => result.push(c),
            c if c.is_control() => {} // Strip control characters / escape injections
            c => result.push(c),
        }
    }

    result
}
