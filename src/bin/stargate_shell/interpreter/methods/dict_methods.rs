// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use super::super::super::scripting::{Expression, Value};
use std::collections::HashMap;

pub fn handle_dict_methods(
    method: &str,
    mut map: HashMap<Value, Value>,
    args: &[Expression],
    eval_fn: &mut dyn FnMut(Expression) -> Result<Value, String>,
) -> Result<Value, String> {
    match method {
        // Core access methods
        "get" => {
            if args.len() != 1 {
                return Err(format!("get() expects 1 argument, got {}", args.len()));
            }
            let key = eval_fn(args[0].clone())?;
            Ok(map.get(&key).cloned().unwrap_or(Value::None))
        }
        "get_or" => {
            if args.len() != 2 {
                return Err(format!("get_or() expects 2 arguments (key, default), got {}", args.len()));
            }
            let key = eval_fn(args[0].clone())?;
            if let Some(value) = map.get(&key) {
                Ok(value.clone())
            } else {
                eval_fn(args[1].clone())
            }
        }
        "insert" => {
            if args.len() != 2 {
                return Err(format!("insert() expects 2 arguments (key, value), got {}", args.len()));
            }
            let key = eval_fn(args[0].clone())?;
            let value = eval_fn(args[1].clone())?;
            let old_value = map.insert(key, value);
            Ok(old_value.unwrap_or(Value::None))
        }
        "remove" => {
            if args.len() != 1 {
                return Err(format!("remove() expects 1 argument, got {}", args.len()));
            }
            let key = eval_fn(args[0].clone())?;
            let removed = map.remove(&key);
            Ok(removed.unwrap_or(Value::None))
        }
        "contains_key" => {
            if args.len() != 1 {
                return Err(format!("contains_key() expects 1 argument, got {}", args.len()));
            }
            let key = eval_fn(args[0].clone())?;
            Ok(Value::Bool(map.contains_key(&key)))
        }
        // Collection methods
        "keys" => {
            if !args.is_empty() {
                return Err(format!("keys() expects 0 arguments, got {}", args.len()));
            }
            let mut keys: Vec<Value> = map.keys().cloned().collect();
            keys.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
            Ok(Value::List(keys))
        }
        "values" => {
            if !args.is_empty() {
                return Err(format!("values() expects 0 arguments, got {}", args.len()));
            }
            let values: Vec<Value> = map.values().cloned().collect();
            Ok(Value::List(values))
        }
        "entries" | "iter" => {
            if !args.is_empty() {
                return Err(format!("entries() expects 0 arguments, got {}", args.len()));
            }
            let entries: Vec<Value> = map.iter()
                .map(|(k, v)| Value::List(vec![k.clone(), v.clone()]))
                .collect();
            Ok(Value::List(entries))
        }
        // Size methods
        "len" | "size" => {
            if !args.is_empty() {
                return Err(format!("len() expects 0 arguments, got {}", args.len()));
            }
            let len = map.len();
            if len <= i32::MAX as usize {
                Ok(Value::SmallInt(len as i32))
            } else {
                Ok(Value::Number(len as f64))
            }
        }
        "is_empty" => {
            if !args.is_empty() {
                return Err(format!("is_empty() expects 0 arguments, got {}", args.len()));
            }
            Ok(Value::Bool(map.is_empty()))
        }
        // Mutation methods
        "clear" => {
            if !args.is_empty() {
                return Err(format!("clear() expects 0 arguments, got {}", args.len()));
            }
            map.clear();
            Ok(Value::Dict(map))
        }
        "retain" => {
            if args.len() != 1 {
                return Err(format!("retain() expects 1 argument (predicate function), got {}", args.len()));
            }
            Err("retain() not yet implemented - requires closure support".to_string())
        }
        // Capacity/optimization methods
        "reserve" => {
            if args.len() != 1 {
                return Err(format!("reserve() expects 1 argument, got {}", args.len()));
            }
            Ok(Value::Dict(map))
        }
        "shrink_to_fit" => {
            if !args.is_empty() {
                return Err(format!("shrink_to_fit() expects 0 arguments, got {}", args.len()));
            }
            Ok(Value::Dict(map))
        }
        // Deprecated aliases for compatibility
        "set" => {
            if args.len() != 2 {
                return Err(format!("set() expects 2 arguments, got {}", args.len()));
            }
            let key = eval_fn(args[0].clone())?;
            let value = eval_fn(args[1].clone())?;
            map.insert(key, value);
            Ok(Value::Dict(map))
        }
        "has_key" => {
            if args.len() != 1 {
                return Err(format!("has_key() expects 1 argument, got {}", args.len()));
            }
            let key = eval_fn(args[0].clone())?;
            Ok(Value::Bool(map.contains_key(&key)))
        }
        _ => Err(format!("Unknown dict method: {}", method))
    }
}
