use std::fmt;

pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::String(str) => str.to_owned(),
                Self::Number(num) => num.to_string(),
                Self::Bool(bool) => bool.to_string(),
                Self::Null => "null".to_string(),
            }
        )
    }
}

impl Value {
    fn get_type_display_name(&self) -> &str {
        match self {
            Self::String(_) => "string",
            Self::Number(_) => "number",
            Self::Bool(_) => "bool",
            Self::Null => "null",
        }
    }
}
