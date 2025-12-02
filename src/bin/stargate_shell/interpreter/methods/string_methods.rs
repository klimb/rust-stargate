use crate::stargate_shell::scripting::*;

pub fn handle_string_methods(
    method: &str,
    string_value: String,
    args: &[Expression],
    eval_fn: &mut dyn FnMut(Expression) -> Result<Value, String>,
) -> Result<Value, String> {
    match method {
        "trim" => {
            if !args.is_empty() {
                return Err(format!("trim() takes no arguments, got {}", args.len()));
            }
            Ok(Value::String(string_value.trim().to_string()))
        }
        "trim_start" | "trim_left" => {
            if !args.is_empty() {
                return Err(format!("trim_start() takes no arguments, got {}", args.len()));
            }
            Ok(Value::String(string_value.trim_start().to_string()))
        }
        "trim_end" | "trim_right" => {
            if !args.is_empty() {
                return Err(format!("trim_end() takes no arguments, got {}", args.len()));
            }
            Ok(Value::String(string_value.trim_end().to_string()))
        }
        "to_uppercase" | "upper" => {
            if !args.is_empty() {
                return Err(format!("to_uppercase() takes no arguments, got {}", args.len()));
            }
            Ok(Value::String(string_value.to_uppercase()))
        }
        "to_lowercase" | "lower" => {
            if !args.is_empty() {
                return Err(format!("to_lowercase() takes no arguments, got {}", args.len()));
            }
            Ok(Value::String(string_value.to_lowercase()))
        }
        "length" | "len" => {
            if !args.is_empty() {
                return Err(format!("length() takes no arguments, got {}", args.len()));
            }
            Ok(Value::Number(string_value.len() as f64))
        }
        "contains" => {
            if args.len() != 1 {
                return Err(format!("contains() expects 1 argument, got {}", args.len()));
            }
            let search = eval_fn(args[0].clone())?.to_string();
            Ok(Value::Bool(string_value.contains(&search)))
        }
        "starts_with" => {
            if args.len() != 1 {
                return Err(format!("starts_with() expects 1 argument, got {}", args.len()));
            }
            let prefix = eval_fn(args[0].clone())?.to_string();
            Ok(Value::Bool(string_value.starts_with(&prefix)))
        }
        "ends_with" => {
            if args.len() != 1 {
                return Err(format!("ends_with() expects 1 argument, got {}", args.len()));
            }
            let suffix = eval_fn(args[0].clone())?.to_string();
            Ok(Value::Bool(string_value.ends_with(&suffix)))
        }
        "replace" => {
            if args.len() != 2 {
                return Err(format!("replace() expects 2 arguments (from, to), got {}", args.len()));
            }
            let from = eval_fn(args[0].clone())?.to_string();
            let to = eval_fn(args[1].clone())?.to_string();
            Ok(Value::String(string_value.replace(&from, &to)))
        }
        "split" => {
            if args.len() != 1 {
                return Err(format!("split() expects 1 argument (delimiter), got {}", args.len()));
            }
            let delimiter = eval_fn(args[0].clone())?.to_string();
            let parts: Vec<serde_json::Value> = string_value
                .split(&delimiter)
                .map(|s| serde_json::Value::String(s.to_string()))
                .collect();
            Ok(Value::Object(serde_json::Value::Array(parts)))
        }
        "slice" => {
            if args.len() != 2 {
                return Err(format!("slice() expects 2 arguments (start, end), got {}", args.len()));
            }
            let start = eval_fn(args[0].clone())?.to_number() as usize;
            let end = eval_fn(args[1].clone())?.to_number() as usize;
            
            let chars: Vec<char> = string_value.chars().collect();
            if start > chars.len() || end > chars.len() || start > end {
                return Err(format!("slice indices out of bounds: start={}, end={}, length={}", start, end, chars.len()));
            }
            
            let sliced: String = chars[start..end].iter().collect();
            Ok(Value::String(sliced))
        }
        _ => Err(format!("Unknown string method: {}", method))
    }
}
