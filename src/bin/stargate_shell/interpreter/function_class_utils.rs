// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use super::super::scripting::*;
use super::Interpreter;
use std::collections::HashMap;
use std::process::Command as ProcessCommand;

impl Interpreter {
    pub(super) fn call_function(&mut self, name: &str, args: Vec<Expression>) -> Result<Value, String> {
        // Evaluate arguments first
        let arg_values: Result<Vec<Value>, String> = args
            .into_iter()
            .map(|arg| self.eval_expression(arg))
            .collect();
        let arg_values = arg_values?;

        // Handle built-in functions
        match name {
            "bool" => {
                if arg_values.len() != 1 {
                    return Err(format!("bool() expects 1 argument, got {}", arg_values.len()));
                }
                return Ok(Value::Bool(arg_values[0].to_bool()));
            }
            "range" => {
                if arg_values.len() != 2 {
                    return Err(format!("range() expects 2 arguments (from, to), got {}", arg_values.len()));
                }
                let from = arg_values[0].to_number() as i64;
                let to = arg_values[1].to_number() as i64;
                
                let mut numbers = Vec::new();
                for i in from..to {
                    numbers.push(serde_json::Value::Number(serde_json::Number::from(i)));
                }
                
                return Ok(Value::Object(serde_json::Value::Array(numbers)));
            }
            "execute-process" => {
                if arg_values.is_empty() {
                    return Err("execute-process() expects at least 1 argument (command path)".to_string());
                }
                
                let cmd_path = arg_values[0].to_string();
                let cmd_args: Vec<String> = arg_values[1..].iter().map(|v| v.to_string()).collect();
                
                // Execute the native command
                let output = ProcessCommand::new(&cmd_path)
                    .args(&cmd_args)
                    .output()
                    .map_err(|e| format!("Failed to execute '{}': {}", cmd_path, e))?;
                
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    // Automatically trim trailing newline from command output
                    return Ok(Value::String(stdout.trim_end().to_string()));
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    return Err(format!("Command '{}' failed: {}", cmd_path, stderr));
                }
            }
            _ => {}
        }

        // Get user-defined function definition
        let (params, body, _annotations) = self
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

        let result = self.return_value.take().unwrap_or(Value::None);

        // Restore variable state
        self.variables = saved_vars;

        Ok(result)
    }

    pub(super) fn json_to_value(&self, json: serde_json::Value) -> Value {
        match json {
            serde_json::Value::Null => Value::None,
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

    /// Recursively collect all inherited fields from a class and its ancestors
    pub(super) fn collect_inherited_fields(&mut self, class_name: &str) -> Result<HashMap<String, Value>, String> {
        let (parent, fields, _methods) = self
            .classes
            .get(class_name)
            .cloned()
            .ok_or(format!("Class '{}' not found", class_name))?;
        
        let mut field_values = HashMap::new();
        
        // First, recursively inherit fields from parent class if exists
        if let Some(parent_name) = parent {
            let parent_fields = self.collect_inherited_fields(&parent_name)?;
            field_values.extend(parent_fields);
        }
        
        // Then, add/override with current class fields
        for (field_name, default_expr) in fields {
            let value = self.eval_expression(default_expr)?;
            field_values.insert(field_name, value);
        }
        
        Ok(field_values)
    }
}
