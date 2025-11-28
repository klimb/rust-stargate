use super::scripting::*;
use super::execution::{execute_pipeline, execute_pipeline_capture};
use std::collections::HashMap;
use std::process::Command as ProcessCommand;

pub struct Interpreter {
    variables: HashMap<String, Value>,
    functions: HashMap<String, (Vec<String>, Vec<Statement>)>,
    return_value: Option<Value>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            variables: HashMap::new(),
            functions: HashMap::new(),
            return_value: None,
        }
    }

    pub fn execute(&mut self, statements: Vec<Statement>) -> Result<(), String> {
        for stmt in statements {
            self.execute_statement(stmt)?;
            if self.return_value.is_some() {
                break;
            }
        }
        Ok(())
    }

    fn execute_statement(&mut self, stmt: Statement) -> Result<(), String> {
        match stmt {
            Statement::VarDecl(name, expr) => {
                let value = self.eval_expression(expr)?;
                self.variables.insert(name, value);
            }
            Statement::Assignment(name, expr) => {
                let value = self.eval_expression(expr)?;
                if self.variables.contains_key(&name) {
                    self.variables.insert(name, value);
                } else {
                    return Err(format!("Variable '{}' not declared", name));
                }
            }
            Statement::If {
                condition,
                then_block,
                else_block,
            } => {
                let cond_value = self.eval_expression(condition)?;
                if cond_value.to_bool() {
                    for stmt in then_block {
                        self.execute_statement(stmt)?;
                        if self.return_value.is_some() {
                            break;
                        }
                    }
                } else if let Some(else_stmts) = else_block {
                    for stmt in else_stmts {
                        self.execute_statement(stmt)?;
                        if self.return_value.is_some() {
                            break;
                        }
                    }
                }
            }
            Statement::FunctionDef { name, params, body } => {
                self.functions.insert(name, (params, body));
            }
            Statement::FunctionCall { name, args } => {
                self.call_function(&name, args)?;
            }
            Statement::Command(cmd) => {
                // Check if it's a stargate pipeline (contains |)
                if cmd.contains('|') {
                    // Execute as stargate pipeline
                    if let Err(e) = execute_pipeline(&cmd) {
                        eprintln!("Pipeline error: {}", e);
                    }
                } else {
                    // Execute as regular shell command
                    let _ = ProcessCommand::new("sh")
                        .arg("-c")
                        .arg(&cmd)
                        .status();
                }
            }
            Statement::Return(expr) => {
                let value = self.eval_expression(expr)?;
                self.return_value = Some(value);
            }
            Statement::Print(expr) => {
                let value = self.eval_expression(expr)?;
                println!("{}", value.to_string());
            }
        }
        Ok(())
    }

    pub fn eval_expression(&mut self, expr: Expression) -> Result<Value, String> {
        match expr {
            Expression::Value(val) => Ok(val),
            Expression::Variable(name) => {
                self.variables
                    .get(&name)
                    .cloned()
                    .ok_or(format!("Variable '{}' not found", name))
            }
            Expression::BinaryOp { left, op, right } => {
                let left_val = self.eval_expression(*left)?;
                let right_val = self.eval_expression(*right)?;
                self.apply_operator(left_val, op, right_val)
            }
            Expression::FunctionCall { name, args } => {
                self.call_function(&name, args)
            }
            Expression::CommandOutput(cmd) => {
                // Execute command using stargate pipeline system
                let output = execute_pipeline_capture(&cmd)
                    .map_err(|e| format!("Pipeline error: {}", e))?;
                
                // Try to parse as JSON first
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&output) {
                    Ok(Value::Object(json_value))
                } else {
                    // Fallback to string
                    Ok(Value::String(output.trim().to_string()))
                }
            }
            Expression::InterpolatedString(template) => {
                // Replace {var} with variable values
                let mut result = template.clone();
                let mut start = 0;
                
                while let Some(open_pos) = result[start..].find('{') {
                    let open_pos = start + open_pos;
                    if let Some(close_pos) = result[open_pos..].find('}') {
                        let close_pos = open_pos + close_pos;
                        let var_name = &result[open_pos + 1..close_pos];
                        
                        let value = self.variables
                            .get(var_name)
                            .ok_or(format!("Variable '{}' not found in interpolation", var_name))?;
                        
                        let replacement = value.to_string();
                        result.replace_range(open_pos..=close_pos, &replacement);
                        start = open_pos + replacement.len();
                    } else {
                        break;
                    }
                }
                
                Ok(Value::String(result))
            }
            Expression::PropertyAccess { object, property } => {
                let obj_value = self.eval_expression(*object)?;
                match obj_value {
                    Value::Object(json_obj) => {
                        if let Some(value) = json_obj.get(&property) {
                            Ok(self.json_to_value(value.clone()))
                        } else {
                            Err(format!("Property '{}' not found in object", property))
                        }
                    }
                    _ => Err(format!("Cannot access property '{}' on non-object value", property))
                }
            }
            Expression::IndexAccess { object, index } => {
                let obj_value = self.eval_expression(*object)?;
                let index_value = self.eval_expression(*index)?;
                
                match obj_value {
                    Value::Object(json_obj) => {
                        match json_obj {
                            serde_json::Value::Array(arr) => {
                                let idx = index_value.to_number() as i64;
                                let actual_idx = if idx < 0 {
                                    // Python-style negative indexing
                                    (arr.len() as i64 + idx) as usize
                                } else {
                                    idx as usize
                                };
                                
                                if actual_idx < arr.len() {
                                    Ok(self.json_to_value(arr[actual_idx].clone()))
                                } else {
                                    Err(format!("Index {} out of bounds (array length: {})", idx, arr.len()))
                                }
                            }
                            serde_json::Value::Object(map) => {
                                let key = index_value.to_string();
                                if let Some(value) = map.get(&key) {
                                    Ok(self.json_to_value(value.clone()))
                                } else {
                                    Err(format!("Key '{}' not found in object", key))
                                }
                            }
                            _ => Err("Cannot index non-array/non-object JSON value".to_string())
                        }
                    }
                    _ => Err("Cannot index non-object value".to_string())
                }
            }
        }
    }

    fn apply_operator(&self, left: Value, op: Operator, right: Value) -> Result<Value, String> {
        match op {
            Operator::Add => match (left, right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{}{}", a, b))),
                (Value::String(a), b) => Ok(Value::String(format!("{}{}", a, b.to_string()))),
                (a, Value::String(b)) => Ok(Value::String(format!("{}{}", a.to_string(), b))),
                _ => Err("Invalid operands for +".to_string()),
            },
            Operator::Sub => Ok(Value::Number(left.to_number() - right.to_number())),
            Operator::Mul => Ok(Value::Number(left.to_number() * right.to_number())),
            Operator::Div => {
                let divisor = right.to_number();
                if divisor == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(Value::Number(left.to_number() / divisor))
                }
            }
            Operator::Eq => Ok(Value::Bool(match (left, right) {
                (Value::Number(a), Value::Number(b)) => a == b,
                (Value::String(a), Value::String(b)) => a == b,
                (Value::Bool(a), Value::Bool(b)) => a == b,
                _ => false,
            })),
            Operator::Ne => Ok(Value::Bool(match (left, right) {
                (Value::Number(a), Value::Number(b)) => a != b,
                (Value::String(a), Value::String(b)) => a != b,
                (Value::Bool(a), Value::Bool(b)) => a != b,
                _ => true,
            })),
            Operator::Lt => Ok(Value::Bool(left.to_number() < right.to_number())),
            Operator::Gt => Ok(Value::Bool(left.to_number() > right.to_number())),
            Operator::Le => Ok(Value::Bool(left.to_number() <= right.to_number())),
            Operator::Ge => Ok(Value::Bool(left.to_number() >= right.to_number())),
            Operator::And => Ok(Value::Bool(left.to_bool() && right.to_bool())),
            Operator::Or => Ok(Value::Bool(left.to_bool() || right.to_bool())),
        }
    }

    fn call_function(&mut self, name: &str, args: Vec<Expression>) -> Result<Value, String> {
        // Evaluate arguments first
        let arg_values: Result<Vec<Value>, String> = args
            .into_iter()
            .map(|arg| self.eval_expression(arg))
            .collect();
        let arg_values = arg_values?;

        // Get function definition
        let (params, body) = self
            .functions
            .get(name)
            .cloned()
            .ok_or(format!("Function '{}' not found", name))?;

        if params.len() != arg_values.len() {
            return Err(format!(
                "Function '{}' expects {} arguments, got {}",
                name,
                params.len(),
                arg_values.len()
            ));
        }

        // Save current variable state
        let saved_vars = self.variables.clone();

        // Bind parameters
        for (param, value) in params.iter().zip(arg_values.iter()) {
            self.variables.insert(param.clone(), value.clone());
        }

        // Execute function body
        self.return_value = None;
        for stmt in body {
            self.execute_statement(stmt)?;
            if self.return_value.is_some() {
                break;
            }
        }

        let result = self.return_value.take().unwrap_or(Value::Null);

        // Restore variable state
        self.variables = saved_vars;

        Ok(result)
    }

    fn json_to_value(&self, json: serde_json::Value) -> Value {
        match json {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Number(i as f64)
                } else if let Some(f) = n.as_f64() {
                    Value::Number(f)
                } else {
                    Value::Number(0.0)
                }
            }
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => Value::Object(json),
        }
    }
}

pub fn execute_script(script: &str) -> Result<(), String> {
    let mut parser = Parser::new(script);
    let statements = parser.parse()?;
    let mut interpreter = Interpreter::new();
    interpreter.execute(statements)?;
    Ok(())
}
