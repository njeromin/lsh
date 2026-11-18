use std::{collections::HashMap, fmt::Write};

use crate::eval::value::Value;

const MAX_JSON_DEPTH: usize = 256;
const MAX_STRINGIFY_DEPTH: usize = 64;

/// Parses an RFC 8259 JSON string into an `lsh` runtime [`Value`].
pub fn parse_json(input: &str) -> Result<Value, String> {
    let mut parser = JsonParser::new(input);
    parser.skip_whitespace();
    let value = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.pos < parser.bytes.len() {
        return Err(format!(
            "Unexpected trailing character '{}' at byte {}.",
            parser.peek_char().unwrap_or('?'),
            parser.pos
        ));
    }
    Ok(value)
}

/// Serializes an `lsh` runtime [`Value`] into a JSON string.
///
/// When `indent` is `Some(spaces)`, arrays and objects are formatted across multiple lines
/// using `spaces` spaces per nesting level. Object keys are emitted in sorted order.
pub fn stringify_json(value: &Value, indent: Option<usize>) -> String {
    let mut out = String::new();
    write_json(&mut out, value, indent, 0);
    out
}

struct JsonParser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), String> {
        match self.peek_byte() {
            Some(b) if b == expected => {
                self.pos += 1;
                Ok(())
            }
            Some(_) => Err(format!(
                "Expected '{}' at byte {}, found '{}'.",
                expected as char,
                self.pos,
                self.peek_char().unwrap_or('?')
            )),
            None => Err(format!(
                "Expected '{}' at byte {}, found end of input.",
                expected as char, self.pos
            )),
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_JSON_DEPTH {
            return Err("Maximum JSON nesting depth exceeded.".to_string());
        }

        self.skip_whitespace();
        let Some(b) = self.peek_byte() else {
            return Err("Unexpected end of JSON input.".to_string());
        };

        match b {
            b'{' => self.parse_object(depth + 1),
            b'[' => self.parse_array(depth + 1),
            b'"' => self.parse_string().map(Value::String),
            b't' => self.parse_literal(b"true", Value::Bool(true)),
            b'f' => self.parse_literal(b"false", Value::Bool(false)),
            b'n' => self.parse_literal(b"null", Value::Null),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => Err(format!(
                "Unexpected character '{}' at byte {}.",
                self.peek_char().unwrap_or('?'),
                self.pos
            )),
        }
    }

    fn parse_literal(&mut self, expected: &[u8], value: Value) -> Result<Value, String> {
        if self.bytes[self.pos..].starts_with(expected) {
            self.pos += expected.len();
            Ok(value)
        } else {
            Err(format!("Invalid JSON literal at byte {}.", self.pos))
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, String> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();

        let mut map = HashMap::new();
        if self.peek_byte() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::object(map));
        }

        loop {
            self.skip_whitespace();
            if self.peek_byte() != Some(b'"') {
                return Err(format!("Expected object key string at byte {}.", self.pos));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect_byte(b':')?;
            let val = self.parse_value(depth)?;
            map.insert(key, val);

            self.skip_whitespace();
            match self.peek_byte() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => {
                    return Err(format!(
                        "Expected ',' or '}}' in object at byte {}.",
                        self.pos
                    ));
                }
            }
        }

        Ok(Value::object(map))
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, String> {
        self.expect_byte(b'[')?;
        self.skip_whitespace();

        let mut items = Vec::new();
        if self.peek_byte() == Some(b']') {
            self.pos += 1;
            return Ok(Value::array(items));
        }

        loop {
            let val = self.parse_value(depth)?;
            items.push(val);

            self.skip_whitespace();
            match self.peek_byte() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                _ => {
                    return Err(format!(
                        "Expected ',' or ']' in array at byte {}.",
                        self.pos
                    ));
                }
            }
        }

        Ok(Value::array(items))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect_byte(b'"')?;
        let mut result = String::new();
        let mut literal_start = self.pos;

        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'"' {
                result.push_str(&self.src[literal_start..self.pos]);
                self.pos += 1;
                return Ok(result);
            }
            if b == b'\\' {
                result.push_str(&self.src[literal_start..self.pos]);
                self.pos += 1;
                let escaped = self.parse_escape()?;
                result.push(escaped);
                literal_start = self.pos;
                continue;
            }
            if b < 0x20 {
                return Err(format!(
                    "Unescaped control character in JSON string at byte {}.",
                    self.pos
                ));
            }
            let ch_len = self.src[self.pos..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
            self.pos += ch_len;
        }

        Err("Unterminated JSON string.".to_string())
    }

    fn parse_escape(&mut self) -> Result<char, String> {
        let Some(esc) = self.peek_byte() else {
            return Err("Unterminated escape sequence in JSON string.".to_string());
        };
        self.pos += 1;

        match esc {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{0008}'),
            b'f' => Ok('\u{000C}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => self.parse_unicode_escape(),
            other => Err(format!("Invalid JSON escape '\\{}'.", other as char)),
        }
    }

    fn parse_hex4(&mut self) -> Result<u16, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err("Incomplete \\uXXXX escape in JSON string.".to_string());
        }
        let hex = &self.src[self.pos..self.pos + 4];
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("Invalid hex digits in \\u{hex}."));
        }
        self.pos += 4;
        u16::from_str_radix(hex, 16).map_err(|_| format!("Invalid hex digits in \\u{hex}."))
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let code = self.parse_hex4()?;
        if (0xD800..=0xDBFF).contains(&code) {
            if !self.bytes[self.pos..].starts_with(b"\\u") {
                return Err("High surrogate not followed by low surrogate.".to_string());
            }
            self.pos += 2;
            let low = self.parse_hex4()?;
            if !(0xDC00..=0xDFFF).contains(&low) {
                return Err("Invalid low surrogate following high surrogate.".to_string());
            }
            let combined = 0x10000 + ((((code as u32) - 0xD800) << 10) | ((low as u32) - 0xDC00));
            return char::from_u32(combined)
                .ok_or_else(|| "Invalid Unicode code point from surrogate pair.".to_string());
        }

        if (0xDC00..=0xDFFF).contains(&code) {
            return Err("Unexpected unpaired low surrogate in JSON string.".to_string());
        }

        char::from_u32(code as u32)
            .ok_or_else(|| format!("Invalid Unicode code point U+{code:04X}."))
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek_byte() == Some(b'-') {
            self.pos += 1;
        }

        match self.peek_byte() {
            Some(b'0') => {
                self.pos += 1;
                if matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                    return Err(format!(
                        "Leading zeros are not allowed in JSON numbers at byte {start}."
                    ));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(format!("Invalid JSON number at byte {start}.")),
        }

        if self.peek_byte() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                return Err(format!(
                    "Expected digits after decimal point in JSON number at byte {}.",
                    self.pos
                ));
            }
            while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }

        if matches!(self.peek_byte(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek_byte(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                return Err(format!(
                    "Expected exponent digits in JSON number at byte {}.",
                    self.pos
                ));
            }
            while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }

        let num_str = &self.src[start..self.pos];
        let num = num_str
            .parse::<f64>()
            .map_err(|_| format!("Invalid JSON number '{num_str}'."))?;
        if !num.is_finite() {
            return Err(format!("JSON number '{num_str}' is out of range."));
        }

        Ok(Value::Number(num))
    }
}

fn write_json(out: &mut String, value: &Value, indent: Option<usize>, depth: usize) {
    if depth > MAX_STRINGIFY_DEPTH {
        out.push_str("null");
        return;
    }

    match value {
        Value::Null | Value::Function(_) => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if !n.is_finite() {
                out.push_str("null");
            } else if *n == 0.0 {
                out.push('0');
            } else if n.fract() == 0.0 && n.abs() < 1e15 {
                let _ = write!(out, "{}", *n as i64);
            } else {
                let _ = write!(out, "{n}");
            }
        }
        Value::String(s) => write_json_string(out, s),
        Value::Array(arr) => {
            let guard = match arr.read() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            if guard.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in guard.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if let Some(spaces) = indent {
                    out.push('\n');
                    push_indent(out, spaces * (depth + 1));
                }
                write_json(out, item, indent, depth + 1);
            }
            if let Some(spaces) = indent {
                out.push('\n');
                push_indent(out, spaces * depth);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let guard = match map.read() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            if guard.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut entries: Vec<(&String, &Value)> = guard.iter().collect();
            entries.sort_by_key(|(k1, _)| *k1);

            out.push('{');
            for (i, (key, val)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if let Some(spaces) = indent {
                    out.push('\n');
                    push_indent(out, spaces * (depth + 1));
                }
                write_json_string(out, key);
                if indent.is_some() {
                    out.push_str(": ");
                } else {
                    out.push(':');
                }
                write_json(out, val, indent, depth + 1);
            }
            if let Some(spaces) = indent {
                out.push('\n');
                push_indent(out, spaces * depth);
            }
            out.push('}');
        }
    }
}

fn push_indent(out: &mut String, count: usize) {
    for _ in 0..count {
        out.push(' ');
    }
}

fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
