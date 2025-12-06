use super::super::super::scripting::{Expression, Value};

pub fn call_ut_method(
    method: &str,
    args: &[Expression],
    eval_fn: &mut dyn FnMut(Expression) -> Result<Value, String>,
) -> Result<Value, String> {
    match method {
        "assert_equals" => {
            if args.len() < 2 || args.len() > 3 {
                return Err("ut.assert_equals() expects 2 or 3 arguments (a, b, [message])".to_string());
            }
            let a = eval_fn(args[0].clone())?;
            let b = eval_fn(args[1].clone())?;
            let custom_message = if args.len() == 3 {
                Some(eval_fn(args[2].clone())?.to_string())
            } else {
                None
            };
            
            if a.to_string() != b.to_string() {
                let message = if let Some(msg) = custom_message {
                    format!("{}\n  Expected: {}\n  Actual:   {}", msg, b.to_string(), a.to_string())
                } else {
                    format!("Assertion failed\n  Expected: {}\n  Actual:   {}", b.to_string(), a.to_string())
                };
                return Err(format!("Assertion failed: {}", message));
            }
            Ok(Value::Bool(true))
        }
        "assert_not_equals" => {
            if args.len() < 2 || args.len() > 3 {
                return Err("ut.assert_not_equals() expects 2 or 3 arguments (a, b, [message])".to_string());
            }
            let a = eval_fn(args[0].clone())?;
            let b = eval_fn(args[1].clone())?;
            let custom_message = if args.len() == 3 {
                Some(eval_fn(args[2].clone())?.to_string())
            } else {
                None
            };
            
            if a.to_string() == b.to_string() {
                let message = if let Some(msg) = custom_message {
                    format!("{}\n  Both values: {}", msg, a.to_string())
                } else {
                    format!("Assertion failed: values should not be equal\n  Both values: {}", a.to_string())
                };
                return Err(format!("Assertion failed: {}", message));
            }
            Ok(Value::Bool(true))
        }
        "assert_true" => {
            if args.len() < 1 || args.len() > 2 {
                return Err("ut.assert_true() expects 1 or 2 arguments (condition, [message])".to_string());
            }
            let condition = eval_fn(args[0].clone())?;
            let custom_message = if args.len() == 2 {
                Some(eval_fn(args[1].clone())?.to_string())
            } else {
                None
            };
            
            if !condition.to_bool() {
                let message = if let Some(msg) = custom_message {
                    format!("{}\n  Expected: true\n  Actual:   {}", msg, condition.to_string())
                } else {
                    format!("Assertion failed\n  Expected: true\n  Actual:   {}", condition.to_string())
                };
                return Err(format!("Assertion failed: {}", message));
            }
            Ok(Value::Bool(true))
        }
        _ => Err(format!("Unknown ut method: {}", method))
    }
}
