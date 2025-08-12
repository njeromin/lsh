use std::fmt;

#[derive(Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

impl ToString for Value {
    fn to_string(&self) -> String {
        match self {
            Self::String(str) => str.to_owned(),
            Self::Number(num) => num.to_string(),
            Self::Bool(bool) => bool.to_string(),
            Self::Null => "Null".to_string(),
        }
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
