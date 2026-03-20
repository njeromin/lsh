use std::{collections::HashMap, fmt};

#[derive(Clone)]
pub enum ValueType {
    String,
    Number,
    Bool,
    Object,
    Null,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Number => write!(f, "number"),
            Self::Bool => write!(f, "bool"),
            Self::Object => write!(f, "object"),
            Self::Null => write!(f, "null"),
        }
    }
}

#[derive(Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Object(HashMap<String, Value>),
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(str) => write!(f, "{str}"),
            Self::Number(num) => write!(f, "{num}"),
            Self::Bool(bool) => write!(f, "{bool}"),
            Self::Object(map) => {
                // TODO: Proper recursive object printing.
                write!(f, "{{\n")?;

                for (key, value) in map {
                    write!(f, "  {}: {}\n", key, value.to_string())?;
                }

                write!(f, "}}\n")
            }
            Self::Null => write!(f, ""),
        }
    }
}

impl Value {
    pub fn get_type(&self) -> ValueType {
        match self {
            Self::String(_) => ValueType::String,
            Self::Number(_) => ValueType::Number,
            Self::Bool(_) => ValueType::Bool,
            Self::Object(_) => ValueType::Object,
            Self::Null => ValueType::Null,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0,
            Self::String(s) => !s.is_empty(),
            Self::Object(map) => !map.is_empty(),
            Self::Null => false,
        }
    }

    pub fn cast(&self, target: ValueType) -> Self {
        match target {
            ValueType::String => Self::String(self.to_string()),
            ValueType::Number => match self {
                Self::String(s) => s.parse::<f64>().map_or(Self::Null, Self::Number),
                Self::Number(n) => Self::Number(*n),
                Self::Bool(b) => Self::Number(if *b { 1.0 } else { 0.0 }),
                Self::Object(_) => Self::Null,
                Self::Null => Self::Null,
            },
            ValueType::Bool => Self::Bool(self.is_truthy()),
            ValueType::Object => match self {
                Self::Object(map) => Self::Object(map.clone()),
                _ => Self::Object(HashMap::new()),
            },
            ValueType::Null => Self::Null,
        }
    }

    pub fn check_equality(&self, other: &Self) -> bool {
        // Check nulls before casting.
        if let Self::Null = self {
            if let Self::Null = other {
                return true;
            } else {
                return false;
            }
        }

        let other_cast = other.cast(self.get_type());

        match (self, &other_cast) {
            (Self::String(s1), Self::String(s2)) => s1 == s2,
            (Self::Number(n1), Self::Number(n2)) => n1 == n2,
            (Self::Bool(b1), Self::Bool(b2)) => b1 == b2,
            (Self::Null, Self::Null) => true,
            _ => false,
        }
    }
}
