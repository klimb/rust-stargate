use super::super::super::scripting::{Expression, Value};
use std::collections::HashSet;

pub fn handle_set_methods(
    method: &str,
    mut set: HashSet<Value>,
    args: &[Expression],
    eval_fn: &mut dyn FnMut(Expression) -> Result<Value, String>,
) -> Result<Value, String> {
    match method {
        // Core methods
        "insert" | "add" => {
            if args.len() != 1 {
                return Err(format!("insert() expects 1 argument, got {}", args.len()));
            }
            let value = eval_fn(args[0].clone())?;
            set.insert(value);
            Ok(Value::Set(set))
        }
        "remove" => {
            if args.len() != 1 {
                return Err(format!("remove() expects 1 argument, got {}", args.len()));
            }
            let value = eval_fn(args[0].clone())?;
            set.remove(&value);
            Ok(Value::Set(set))
        }
        "contains" => {
            if args.len() != 1 {
                return Err(format!("contains() expects 1 argument, got {}", args.len()));
            }
            let value = eval_fn(args[0].clone())?;
            Ok(Value::Bool(set.contains(&value)))
        }
        "size" | "len" => {
            if !args.is_empty() {
                return Err(format!("size() expects 0 arguments, got {}", args.len()));
            }
            let len = set.len();
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
            Ok(Value::Bool(set.is_empty()))
        }
        "clear" => {
            if !args.is_empty() {
                return Err(format!("clear() expects 0 arguments, got {}", args.len()));
            }
            set.clear();
            Ok(Value::Set(set))
        }
        // Set operations
        "union" => {
            if args.len() != 1 {
                return Err(format!("union() expects 1 argument, got {}", args.len()));
            }
            let other = eval_fn(args[0].clone())?;
            match other {
                Value::Set(other_set) => {
                    let result: HashSet<_> = set.union(&other_set).cloned().collect();
                    Ok(Value::Set(result))
                }
                _ => Err("union() expects a set argument".to_string())
            }
        }
        "intersection" => {
            if args.len() != 1 {
                return Err(format!("intersection() expects 1 argument, got {}", args.len()));
            }
            let other = eval_fn(args[0].clone())?;
            match other {
                Value::Set(other_set) => {
                    let result: HashSet<_> = set.intersection(&other_set).cloned().collect();
                    Ok(Value::Set(result))
                }
                _ => Err("intersection() expects a set argument".to_string())
            }
        }
        "difference" => {
            if args.len() != 1 {
                return Err(format!("difference() expects 1 argument, got {}", args.len()));
            }
            let other = eval_fn(args[0].clone())?;
            match other {
                Value::Set(other_set) => {
                    let result: HashSet<_> = set.difference(&other_set).cloned().collect();
                    Ok(Value::Set(result))
                }
                _ => Err("difference() expects a set argument".to_string())
            }
        }
        "symmetric_difference" => {
            if args.len() != 1 {
                return Err(format!("symmetric_difference() expects 1 argument, got {}", args.len()));
            }
            let other = eval_fn(args[0].clone())?;
            match other {
                Value::Set(other_set) => {
                    let result: HashSet<_> = set.symmetric_difference(&other_set).cloned().collect();
                    Ok(Value::Set(result))
                }
                _ => Err("symmetric_difference() expects a set argument".to_string())
            }
        }
        "is_subset" => {
            if args.len() != 1 {
                return Err(format!("is_subset() expects 1 argument, got {}", args.len()));
            }
            let other = eval_fn(args[0].clone())?;
            match other {
                Value::Set(other_set) => {
                    Ok(Value::Bool(set.is_subset(&other_set)))
                }
                _ => Err("is_subset() expects a set argument".to_string())
            }
        }
        "is_superset" => {
            if args.len() != 1 {
                return Err(format!("is_superset() expects 1 argument, got {}", args.len()));
            }
            let other = eval_fn(args[0].clone())?;
            match other {
                Value::Set(other_set) => {
                    Ok(Value::Bool(set.is_superset(&other_set)))
                }
                _ => Err("is_superset() expects a set argument".to_string())
            }
        }
        "is_disjoint" => {
            if args.len() != 1 {
                return Err(format!("is_disjoint() expects 1 argument, got {}", args.len()));
            }
            let other = eval_fn(args[0].clone())?;
            match other {
                Value::Set(other_set) => {
                    Ok(Value::Bool(set.is_disjoint(&other_set)))
                }
                _ => Err("is_disjoint() expects a set argument".to_string())
            }
        }
        // Convert to list (sorted)
        "to_list" => {
            if !args.is_empty() {
                return Err(format!("to_list() expects 0 arguments, got {}", args.len()));
            }
            let mut items: Vec<_> = set.into_iter().collect();
            items.sort_by_key(|v| v.to_string());
            Ok(Value::List(items))
        }
        _ => Err(format!("Unknown set method: {}", method))
    }
}
