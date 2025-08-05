use std::fmt;

pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
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
            }
        )
    }
}
