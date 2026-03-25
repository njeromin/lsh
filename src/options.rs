use std::collections::HashMap;

use crate::eval::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Default,
    Emacs,
    Vi,
}

impl ToString for EditMode {
    fn to_string(&self) -> String {
        match self {
            EditMode::Default => "default".to_string(),
            EditMode::Emacs => "emacs".to_string(),
            EditMode::Vi => "vi".to_string(),
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

impl Into<rustyline::EditMode> for EditMode {
    fn into(self) -> rustyline::EditMode {
        match self {
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

impl Options {
    pub fn default() -> Self {
        Self {
            tab_stop: 4,
            edit_mode: EditMode::Default,
        }
    }
}

impl Into<HashMap<String, Value>> for Options {
    fn into(self) -> HashMap<String, Value> {
        let mut map = HashMap::new();

        map.insert("tab_stop".to_string(), Value::Number(self.tab_stop as f64));
        map.insert(
            "edit_mode".to_string(),
            Value::String(self.edit_mode.to_string()),
        );

        map
    }
}

impl Into<Value> for Options {
    fn into(self) -> Value {
        Value::Object(self.into())
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
