use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, RwLock},
};

use lsh_syntax::ast::Statement;

use crate::eval::Evaluator;

pub type NativeFn = Arc<dyn Fn(&mut Evaluator, Vec<Value>, Option<Value>) -> Value + Send + Sync>;

#[derive(Clone)]
pub enum FunctionKind {
    Ast,
    Native { name: String, func: NativeFn },
}

#[derive(Clone)]
pub struct FunctionValue {
    pub params: Vec<String>,
    pub body: Vec<Statement>,
    pub kind: FunctionKind,
}

impl fmt::Debug for FunctionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FunctionKind::Ast => f
                .debug_struct("FunctionValue::Ast")
                .field("params", &self.params)
                .field("body_len", &self.body.len())
                .finish(),
            FunctionKind::Native { name, .. } => f
                .debug_struct("FunctionValue::Native")
                .field("name", name)
                .finish(),
        }
    }
}

impl FunctionValue {
    pub fn ast(params: Vec<String>, body: Vec<Statement>) -> Self {
        Self {
            params,
            body,
            kind: FunctionKind::Ast,
        }
    }

    pub fn native<F>(name: impl Into<String>, func: F) -> Self
    where
        F: Fn(&mut Evaluator, Vec<Value>, Option<Value>) -> Value + Send + Sync + 'static,
    {
        Self {
            params: vec![],
            body: vec![],
            kind: FunctionKind::Native {
                name: name.into(),
                func: Arc::new(func),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Object(Arc<RwLock<HashMap<String, Value>>>),
    Function(Arc<FunctionValue>),
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

                if let Ok(guard) = map.read() {
                    for (key, value) in guard.iter() {
                        writeln!(f, "  {}: {:?}", key, value.to_string())?;
                    }
                }

                writeln!(f, "}}")
            }
            Self::Function(func) => match &func.kind {
                FunctionKind::Ast => write!(
                    f,
                    "function({}): {}",
                    func.params.join(", "),
                    func.body.len()
                ),
                FunctionKind::Native { name, .. } => write!(f, "function [native {name}]"),
            },
            Self::Null => write!(f, ""),
        }
    }
}

impl Value {
    pub fn object(map: HashMap<String, Value>) -> Self {
        Self::Object(Arc::new(RwLock::new(map)))
    }

    pub fn function(func: FunctionValue) -> Self {
        Self::Function(Arc::new(func))
    }

    pub fn as_object(&self) -> Option<&Arc<RwLock<HashMap<String, Value>>>> {
        match self {
            Self::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_function(&self) -> Option<&Arc<FunctionValue>> {
        match self {
            Self::Function(func) => Some(func),
            _ => None,
        }
    }

    pub fn get_property(&self, key: &str) -> Option<Value> {
        match self {
            Self::Object(map) => map.read().ok().and_then(|m| m.get(key).cloned()),
            _ => None,
        }
    }

    pub fn set_property(&self, key: impl Into<String>, value: Value) -> bool {
        match self {
            Self::Object(map) => {
                if let Ok(mut m) = map.write() {
                    m.insert(key.into(), value);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

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
            Self::Object(map) => map.read().map(|m| !m.is_empty()).unwrap_or(true),
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
                Self::Object(map) => Self::Object(Arc::clone(map)),
                _ => Self::object(HashMap::new()),
            },
            ValueType::Function => match self {
                Self::Function(func) => Self::Function(Arc::clone(func)),
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

            // Object equality: reference identity or deep structural equality
            (Self::Object(m1), Self::Object(m2)) => {
                if Arc::ptr_eq(m1, m2) {
                    true
                } else {
                    let map1 = match m1.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    let map2 = match m2.read() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    map1.len() == map2.len()
                        && map1
                            .iter()
                            .all(|(k, v1)| map2.get(k).is_some_and(|v2| v1.check_equality(v2)))
                }
            }

            // Function equality: reference identity
            (Self::Function(f1), Self::Function(f2)) => Arc::ptr_eq(f1, f2),

            // Cross-type: Number and String
            (Self::Number(n), Self::String(s)) | (Self::String(s), Self::Number(n)) => {
                if n.is_nan() {
                    false
                } else {
                    s.parse::<f64>()
                        .is_ok_and(|parsed| !parsed.is_nan() && n == &parsed)
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

impl From<HashMap<String, Value>> for Value {
    fn from(map: HashMap<String, Value>) -> Self {
        Self::object(map)
    }
}

impl From<Arc<RwLock<HashMap<String, Value>>>> for Value {
    fn from(arc: Arc<RwLock<HashMap<String, Value>>>) -> Self {
        Self::Object(arc)
    }
}

impl From<FunctionValue> for Value {
    fn from(func: FunctionValue) -> Self {
        Self::function(func)
    }
}

impl From<Arc<FunctionValue>> for Value {
    fn from(arc: Arc<FunctionValue>) -> Self {
        Self::Function(arc)
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

        assert_eq!(
            Value::String("abc".to_string()),
            Value::String("abc".to_string())
        );
        assert_ne!(
            Value::String("abc".to_string()),
            Value::String("xyz".to_string())
        );

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

        assert_eq!(Value::object(obj1.clone()), Value::object(obj2));

        let mut obj3 = HashMap::new();
        obj3.insert("a".to_string(), Value::Number(1.0));
        obj3.insert("b".to_string(), Value::String("different".to_string()));

        assert_ne!(Value::object(obj1.clone()), Value::object(obj3));

        let empty = HashMap::new();
        assert_eq!(Value::object(empty.clone()), Value::object(empty));
        assert_ne!(Value::object(obj1), Value::Number(1.0));
    }

    #[test]
    fn object_reference_sharing() {
        let obj = Value::object(HashMap::from([("x".to_string(), Value::Number(1.0))]));
        let obj_ref = obj.clone();

        assert_eq!(obj, obj_ref);

        if let Value::Object(map) = &obj {
            map.write()
                .unwrap()
                .insert("x".to_string(), Value::Number(2.0));
        }

        if let Value::Object(map) = &obj_ref {
            assert_eq!(map.read().unwrap().get("x"), Some(&Value::Number(2.0)));
        }
    }

    #[test]
    fn function_reference_equality() {
        let f1 = Value::function(FunctionValue::ast(vec!["a".to_string()], vec![]));
        let f1_clone = f1.clone();
        let f2 = Value::function(FunctionValue::ast(vec!["a".to_string()], vec![]));

        assert_eq!(f1, f1_clone);
        assert_ne!(f1, f2);
    }
}
