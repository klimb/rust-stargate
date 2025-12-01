use super::super::super::scripting::{Expression, Value};

pub fn handle_list_methods(
    method: &str,
    mut list: Vec<Value>,
    args: &[Expression],
    eval_fn: &mut dyn FnMut(Expression) -> Result<Value, String>,
) -> Result<Value, String> {
    match method {
        "append" => {
            if args.len() != 1 {
                return Err(format!("append() expects 1 argument, got {}", args.len()));
            }
            let value = eval_fn(args[0].clone())?;
            list.push(value);
            Ok(Value::List(list))
        }
        "insert" => {
            if args.len() != 2 {
                return Err(format!("insert() expects 2 arguments, got {}", args.len()));
            }
            let index_value = eval_fn(args[0].clone())?;
            let value = eval_fn(args[1].clone())?;
            let idx = index_value.to_number() as usize;
            if idx > list.len() {
                return Err(format!("Index {} out of bounds for insert (list length: {})", idx, list.len()));
            }
            list.insert(idx, value);
            Ok(Value::List(list))
        }
        "remove" => {
            if args.len() != 1 {
                return Err(format!("remove() expects 1 argument, got {}", args.len()));
            }
            let index_value = eval_fn(args[0].clone())?;
            let idx = index_value.to_number() as i64;
            let actual_idx = if idx < 0 {
                (list.len() as i64 + idx) as usize
            } else {
                idx as usize
            };
            if actual_idx >= list.len() {
                return Err(format!("Index {} out of bounds (list length: {})", idx, list.len()));
            }
            list.remove(actual_idx);
            Ok(Value::List(list))
        }
        "size" => {
            if !args.is_empty() {
                return Err(format!("size() expects 0 arguments, got {}", args.len()));
            }
            Ok(Value::Number(list.len() as f64))
        }
        "pop" => {
            if !args.is_empty() {
                return Err(format!("pop() expects 0 arguments, got {}", args.len()));
            }
            if list.is_empty() {
                return Err("Cannot pop from empty list".to_string());
            }
            let value = list.pop().unwrap();
            Ok(value)
        }
        "clear" => {
            if !args.is_empty() {
                return Err(format!("clear() expects 0 arguments, got {}", args.len()));
            }
            list.clear();
            Ok(Value::List(list))
        }
        _ => Err(format!("Unknown list method: {}", method))
    }
}
