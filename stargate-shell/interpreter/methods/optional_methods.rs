// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use super::super::super::scripting::{Expression, Value};

pub fn handle_optional_method(
    method: &str,
    obj_value: Value,
    args: &[Expression],
    eval_fn: &mut dyn FnMut(Expression) -> Result<Value, String>,
) -> Option<Result<Value, String>> {
    match method {
        "is_none" => {
            if !args.is_empty() {
                return Some(Err(format!("is_none() expects 0 arguments, got {}", args.len())));
            }
            Some(Ok(Value::Bool(matches!(obj_value, Value::None))))
        }
        "is_some" => {
            if !args.is_empty() {
                return Some(Err(format!("is_some() expects 0 arguments, got {}", args.len())));
            }
            Some(Ok(Value::Bool(!matches!(obj_value, Value::None))))
        }
        "unwrap" => {
            if !args.is_empty() {
                return Some(Err(format!("unwrap() expects 0 arguments, got {}", args.len())));
            }
            Some(match obj_value {
                Value::None => Err("Called unwrap() on a none value".to_string()),
                val => Ok(val),
            })
        }
        "unwrap_or" => {
            if args.len() != 1 {
                return Some(Err(format!("unwrap_or() expects 1 argument, got {}", args.len())));
            }
            Some(match obj_value {
                Value::None => eval_fn(args[0].clone()),
                val => Ok(val),
            })
        }
        "expect" => {
            if args.len() != 1 {
                return Some(Err(format!("expect() expects 1 argument, got {}", args.len())));
            }
            Some(match obj_value {
                Value::None => {
                    match eval_fn(args[0].clone()) {
                        Ok(msg) => Err(msg.to_string()),
                        Err(e) => Err(e),
                    }
                }
                val => Ok(val),
            })
        }
        _ => None, // Not an optional method, continue to type-specific methods
    }
}
