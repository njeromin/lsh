use std::fmt;

#[derive(Clone, Copy)]
pub enum ValueType {
    String,
    Number,
    Bool,
    Null,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Number => write!(f, "number"),
            Self::Bool => write!(f, "bool"),
            Self::Null => write!(f, "null"),
        }
    }
}

#[derive(Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(str) => write!(f, "{str}"),
            Self::Number(num) => write!(f, "{num}"),
            Self::Bool(bool) => write!(f, "{bool}"),
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
            Self::Null => ValueType::Null,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Null => false,
        }
    }

    pub fn cast(&self, target: ValueType) -> Self {
        match target {
            ValueType::String => Self::String(self.to_string()),
            ValueType::Number => match self {
                Self::String(s) => s.parse::<f64>().map_or(Self::Null, Self::Number),
                Self::Number(n) => Self::Number(*n),
                Self::Bool(b) => Self::Number(if *b { 1.0 } else { 0.0 }),
                Self::Null => Self::Null,
            },
            ValueType::Bool => Self::Bool(self.is_truthy()),
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
