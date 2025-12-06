use serde_json;
use std::hash::{Hash, Hasher};
use super::ast::Expression;

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    SmallInt(i32),
    Number(f64),
    Bool(bool),
    None,
    Object(serde_json::Value),
    Instance {
        class_name: String,
        fields: std::collections::HashMap<String, Value>,
    },
    List(Vec<Value>),
    Dict(std::collections::HashMap<Value, Value>),
    Set(std::collections::HashSet<Value>),
    Closure {
        params: Vec<String>,
        body: Box<Expression>,
    },
}

impl Value {
    pub fn to_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::SmallInt(i) => i.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::None => "none".to_string(),
            Value::Object(obj) => obj.to_string(),
            Value::Instance { class_name, .. } => format!("<{} instance>", class_name),
            Value::List(items) => {
                let items_str: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                format!("[{}]", items_str.join(", "))
            }
            Value::Dict(map) => {
                let mut pairs: Vec<String> = map.iter()
                    .map(|(k, v)| format!("{}: {}", k.to_string(), v.to_string()))
                    .collect();
                pairs.sort();
                format!("{{{}}}", pairs.join(", "))
            }
            Value::Set(items) => {
                let mut items_str: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                items_str.sort();
                format!("{{{}}}", items_str.join(", "))
            }
            Value::Closure { params, .. } => {
                format!("<closure |{}|>", params.join(", "))
            }
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::SmallInt(i) => *i != 0,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::None => false,
            Value::Object(_) => true,
            Value::Instance { .. } => true,
            Value::List(items) => !items.is_empty(),
            Value::Dict(map) => !map.is_empty(),
            Value::Set(items) => !items.is_empty(),
            Value::Closure { .. } => true,
        }
    }

    pub fn to_number(&self) -> f64 {
        match self {
            Value::SmallInt(i) => *i as f64,
            Value::Number(n) => *n,
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
            Value::String(s) => s.parse().unwrap_or(0.0),
            Value::None => 0.0,
            Value::Object(_) => 0.0,
            Value::Instance { .. } => 0.0,
            Value::List(items) => items.len() as f64,
            Value::Dict(map) => map.len() as f64,
            Value::Set(items) => items.len() as f64,
            Value::Closure { .. } => 0.0,
        }
    }
}

// Manual implementation of PartialEq for Value (handles f64 comparison)
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::String(a), Value::String(b)) => a == b,
            (Value::SmallInt(a), Value::SmallInt(b)) => a == b,
            (Value::SmallInt(a), Value::Number(b)) => (*a as f64) == *b,
            (Value::Number(a), Value::SmallInt(b)) => *a == (*b as f64),
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::None, Value::None) => true,
            (Value::Object(a), Value::Object(b)) => a == b,
            (Value::Instance { class_name: c1, fields: f1 }, Value::Instance { class_name: c2, fields: f2 }) => {
                c1 == c2 && f1 == f2
            }
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Dict(a), Value::Dict(b)) => a == b,
            (Value::Set(a), Value::Set(b)) => a == b,
            (Value::Closure { params: p1, body: b1 }, Value::Closure { params: p2, body: b2 }) => {
                p1 == p2 && format!("{:?}", b1) == format!("{:?}", b2)
            }
            _ => false,
        }
    }
}

impl Eq for Value {}

// Manual implementation of Hash for Value (handles f64 hashing)
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::String(s) => {
                0u8.hash(state);
                s.hash(state);
            }
            Value::SmallInt(i) => {
                1u8.hash(state);
                i.hash(state);
            }
            Value::Number(n) => {
                2u8.hash(state);
                n.to_bits().hash(state);
            }
            Value::Bool(b) => {
                3u8.hash(state);
                b.hash(state);
            }
            Value::None => {
                4u8.hash(state);
            }
            Value::Object(obj) => {
                5u8.hash(state);
                obj.to_string().hash(state);
            }
            Value::Instance { class_name, fields } => {
                6u8.hash(state);
                class_name.hash(state);
                // Hash fields in a deterministic order
                let mut pairs: Vec<_> = fields.iter().collect();
                pairs.sort_by_key(|(k, _)| k.as_str());
                for (k, v) in pairs {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Value::List(items) => {
                7u8.hash(state);
                for item in items {
                    item.hash(state);
                }
            }
            Value::Dict(map) => {
                8u8.hash(state);
                // Hash dict entries in a deterministic order
                let mut pairs: Vec<_> = map.iter().collect();
                pairs.sort_by_key(|(k, _)| k.to_string());
                for (k, v) in pairs {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Value::Set(items) => {
                9u8.hash(state);
                // Hash set items in a deterministic order
                let mut items_vec: Vec<_> = items.iter().collect();
                items_vec.sort_by_key(|v| v.to_string());
                for item in items_vec {
                    item.hash(state);
                }
            }
            Value::Closure { params, body } => {
                10u8.hash(state);
                for param in params {
                    param.hash(state);
                }
                // Hash the debug representation of the body
                format!("{:?}", body).hash(state);
            }
        }
    }
}
