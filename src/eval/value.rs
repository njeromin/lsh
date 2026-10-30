use std::{collections::HashMap, fmt};

use crate::parser::ast::Statement;

#[derive(Clone)]
pub enum ValueType {
    String,
    Number,
    Bool,
    Object,
    Function,
    Null,
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Number => write!(f, "number"),
            Self::Bool => write!(f, "bool"),
            Self::Object => write!(f, "object"),
            Self::Function => write!(f, "function"),
            Self::Null => write!(f, "null"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub params: Vec<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Object(HashMap<String, Value>),
    Function(FunctionValue),
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
                writeln!(f, "{{")?;

                for (key, value) in map {
                    writeln!(f, "  {}: {:?}", key, value.to_string())?;
                }

                writeln!(f, "}}")
            }
            Self::Function(func) => write!(
                f,
                "function({}): {}",
                func.params.join(", "),
                func.body.len()
            ),
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
            Self::Function(_) => ValueType::Function,
            Self::Null => ValueType::Null,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(n) => *n != 0.0,
            Self::String(s) => !s.is_empty(),
            Self::Object(map) => !map.is_empty(),
            Self::Function(_) => true,
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
                Self::Function(_) => Self::Null,
                Self::Null => Self::Null,
            },
            ValueType::Bool => Self::Bool(self.is_truthy()),
            ValueType::Object => match self {
                Self::Object(map) => Self::Object(map.clone()),
                _ => Self::Object(HashMap::new()),
            },
            ValueType::Function => match self {
                Self::Function(func) => Self::Function(func.clone()),
                _ => Self::Null,
            },
            ValueType::Null => Self::Null,
        }
    }

    pub fn check_equality(&self, other: &Self) -> bool {
        match (self, other) {
            // Null equality: null equals null, and symmetrically equals empty string.
            (Self::Null, Self::Null) => true,
            (Self::Null, Self::String(s)) | (Self::String(s), Self::Null) => s.is_empty(),
            (Self::Null, _) | (_, Self::Null) => false,

            // Same-type primitives
            (Self::Bool(b1), Self::Bool(b2)) => b1 == b2,
            (Self::Number(n1), Self::Number(n2)) => {
                if n1.is_nan() || n2.is_nan() {
                    false
                } else {
                    n1 == n2
                }
            }
            (Self::String(s1), Self::String(s2)) => s1 == s2,

            // Deep object equality
            (Self::Object(m1), Self::Object(m2)) => {
                m1.len() == m2.len()
                    && m1
                        .iter()
                        .all(|(k, v1)| m2.get(k).is_some_and(|v2| v1.check_equality(v2)))
            }

            // Cross-type: Number and String
            (Self::Number(n), Self::String(s)) | (Self::String(s), Self::Number(n)) => {
                if n.is_nan() {
                    false
                } else {
                    s.parse::<f64>().is_ok_and(|parsed| !parsed.is_nan() && n == &parsed)
                }
            }

            // Cross-type: Bool and Number
            (Self::Bool(b), Self::Number(n)) | (Self::Number(n), Self::Bool(b)) => {
                *n == if *b { 1.0 } else { 0.0 }
            }

            // Cross-type: Bool and String
            (Self::Bool(b), Self::String(s)) | (Self::String(s), Self::Bool(b)) => {
                (if *b { "true" } else { "false" }) == s
            }

            _ => false,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.check_equality(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_equality_is_symmetric() {
        assert_eq!(Value::Null, Value::Null);
        assert_eq!(Value::Null, Value::String(String::new()));
        assert_eq!(Value::String(String::new()), Value::Null);

        assert_ne!(Value::Null, Value::String("foo".to_string()));
        assert_ne!(Value::String("foo".to_string()), Value::Null);
        assert_ne!(Value::Null, Value::Number(0.0));
        assert_ne!(Value::Number(0.0), Value::Null);
        assert_ne!(Value::Null, Value::Bool(false));
        assert_ne!(Value::Bool(false), Value::Null);
    }

    #[test]
    fn primitive_equality() {
        assert_eq!(Value::Number(42.0), Value::Number(42.0));
        assert_ne!(Value::Number(42.0), Value::Number(43.0));
        assert_ne!(Value::Number(f64::NAN), Value::Number(f64::NAN));

        assert_eq!(Value::String("abc".to_string()), Value::String("abc".to_string()));
        assert_ne!(Value::String("abc".to_string()), Value::String("xyz".to_string()));

        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Bool(false), Value::Bool(false));
        assert_ne!(Value::Bool(true), Value::Bool(false));
    }

    #[test]
    fn cross_type_equality_is_symmetric() {
        // Number and String
        assert_eq!(Value::Number(42.0), Value::String("42".to_string()));
        assert_eq!(Value::String("42".to_string()), Value::Number(42.0));
        assert_eq!(Value::Number(42.0), Value::String("42.0".to_string()));
        assert_eq!(Value::String("42.0".to_string()), Value::Number(42.0));
        assert_ne!(Value::Number(42.0), Value::String("foo".to_string()));
        assert_ne!(Value::String("foo".to_string()), Value::Number(42.0));

        // Bool and Number
        assert_eq!(Value::Bool(true), Value::Number(1.0));
        assert_eq!(Value::Number(1.0), Value::Bool(true));
        assert_eq!(Value::Bool(false), Value::Number(0.0));
        assert_eq!(Value::Number(0.0), Value::Bool(false));
        assert_ne!(Value::Bool(true), Value::Number(0.0));
        assert_ne!(Value::Number(0.0), Value::Bool(true));

        // Bool and String
        assert_eq!(Value::Bool(true), Value::String("true".to_string()));
        assert_eq!(Value::String("true".to_string()), Value::Bool(true));
        assert_eq!(Value::Bool(false), Value::String("false".to_string()));
        assert_eq!(Value::String("false".to_string()), Value::Bool(false));
        assert_ne!(Value::Bool(false), Value::String("true".to_string()));
        assert_ne!(Value::Bool(true), Value::String("foo".to_string()));
    }

    #[test]
    fn object_deep_equality() {
        let mut obj1 = HashMap::new();
        obj1.insert("a".to_string(), Value::Number(1.0));
        obj1.insert("b".to_string(), Value::String("hello".to_string()));

        let mut obj2 = HashMap::new();
        obj2.insert("b".to_string(), Value::String("hello".to_string()));
        obj2.insert("a".to_string(), Value::Number(1.0));

        assert_eq!(Value::Object(obj1.clone()), Value::Object(obj2));

        let mut obj3 = HashMap::new();
        obj3.insert("a".to_string(), Value::Number(1.0));
        obj3.insert("b".to_string(), Value::String("different".to_string()));

        assert_ne!(Value::Object(obj1.clone()), Value::Object(obj3));

        let empty = HashMap::new();
        assert_eq!(Value::Object(empty.clone()), Value::Object(empty));
        assert_ne!(Value::Object(obj1), Value::Number(1.0));
    }
}
