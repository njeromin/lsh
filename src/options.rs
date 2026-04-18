use std::collections::HashMap;
use std::fmt;

use crate::eval::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Default,
    Emacs,
    Vi,
}

impl fmt::Display for EditMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditMode::Default => write!(f, "default"),
            EditMode::Emacs => write!(f, "emacs"),
            EditMode::Vi => write!(f, "vi"),
        }
    }
}

impl From<&str> for EditMode {
    fn from(s: &str) -> Self {
        match s {
            "default" => EditMode::Default,
            "emacs" => EditMode::Emacs,
            "vi" => EditMode::Vi,
            _ => EditMode::Default,
        }
    }
}

impl From<EditMode> for rustyline::EditMode {
    fn from(value: EditMode) -> Self {
        match value {
            EditMode::Default => rustyline::EditMode::Vi,
            EditMode::Emacs => rustyline::EditMode::Emacs,
            EditMode::Vi => rustyline::EditMode::Vi,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub tab_stop: u8,
    pub edit_mode: EditMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            tab_stop: 4,
            edit_mode: EditMode::Default,
        }
    }
}

impl From<Options> for HashMap<String, Value> {
    fn from(value: Options) -> Self {
        let mut map = HashMap::new();

        map.insert("tab_stop".to_string(), Value::Number(value.tab_stop as f64));
        map.insert(
            "edit_mode".to_string(),
            Value::String(value.edit_mode.to_string()),
        );

        map
    }
}

impl From<Options> for Value {
    fn from(value: Options) -> Self {
        Value::Object(value.into())
    }
}

impl From<&HashMap<String, Value>> for Options {
    fn from(map: &HashMap<String, Value>) -> Self {
        let tab_stop = map
            .get("tab_stop")
            .and_then(|v| match v {
                Value::Number(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(4.0) as u8;

        let edit_mode = map
            .get("edit_mode")
            .and_then(|v| match v {
                Value::String(s) => Some(EditMode::from(s.as_str())),
                _ => None,
            })
            .unwrap_or(EditMode::Default);

        Self {
            tab_stop,
            edit_mode,
        }
    }
}
