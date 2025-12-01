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
            let message = if args.len() == 3 {
                eval_fn(args[2].clone())?.to_string()
            } else {
                format!("Expected {:?} to equal {:?}", a.to_string(), b.to_string())
            };
            
            if a.to_string() != b.to_string() {
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
            let message = if args.len() == 3 {
                eval_fn(args[2].clone())?.to_string()
            } else {
                format!("Expected {:?} to not equal {:?}", a.to_string(), b.to_string())
            };
            
            if a.to_string() == b.to_string() {
                return Err(format!("Assertion failed: {}", message));
            }
            Ok(Value::Bool(true))
        }
        "assert_true" => {
            if args.len() < 1 || args.len() > 2 {
                return Err("ut.assert_true() expects 1 or 2 arguments (condition, [message])".to_string());
            }
            let condition = eval_fn(args[0].clone())?;
            let message = if args.len() == 2 {
                eval_fn(args[1].clone())?.to_string()
            } else {
                "Expected condition to be true".to_string()
            };
            
            if !condition.to_bool() {
                return Err(format!("Assertion failed: {}", message));
            }
            Ok(Value::Bool(true))
        }
        _ => Err(format!("Unknown ut method: {}", method))
    }
}
