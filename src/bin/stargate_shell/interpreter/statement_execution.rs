use super::super::scripting::*;
use super::super::execution::execute_pipeline;
use super::Interpreter;

impl Interpreter {
    pub(super) fn execute_statement(&mut self, stmt: Statement) -> Result<(), String> {
        match stmt {
            Statement::Assignment(name, expr) => {
                let value = self.eval_expression(expr)?;
                self.variables.insert(name.clone(), value);
                // Update completion list
                if let Some(ref var_names) = self.variable_names {
                    if let Ok(mut names) = var_names.lock() {
                        names.insert(name);
                    }
                }
            }
            Statement::IndexAssignment { object, index, value } => {
                // Get the list/dict to modify
                let obj_value = self.variables.get(&object).cloned()
                    .ok_or(format!("Variable '{}' not found", object))?;
                
                let index_value = self.eval_expression(index)?;
                let new_value = self.eval_expression(value)?;
                
                match obj_value {
                    Value::List(mut list) => {
                        let idx = index_value.to_number() as i64;
                        let actual_idx = if idx < 0 {
                            let len = list.len() as i64;
                            if len + idx < 0 {
                                return Err(format!("Index {} out of bounds (list length: {})", idx, list.len()));
                            }
                            (len + idx) as usize
                        } else {
                            idx as usize
                        };
                        
                        if actual_idx >= list.len() {
                            return Err(format!("Index {} out of bounds (list length: {})", idx, list.len()));
                        }
                        
                        list[actual_idx] = new_value;
                        self.variables.insert(object.clone(), Value::List(list));
                    }
                    Value::Dict(mut dict) => {
                        dict.insert(index_value, new_value);
                        self.variables.insert(object.clone(), Value::Dict(dict));
                    }
                    _ => return Err(format!("Cannot use index assignment on non-list/non-dict value"))
                }
            }
            Statement::VarDecl(name, expr) => {
                let value = self.eval_expression(expr)?;
                self.variables.insert(name.clone(), value);
                // Update completion list
                if let Some(ref var_names) = self.variable_names {
                    if let Ok(mut names) = var_names.lock() {
                        names.insert(name);
                    }
                }
            }
            Statement::If {
                condition,
                then_block,
                else_block,
            } => {
                let cond_value = self.eval_expression(condition)?;
                // Auto-convert to boolean (Rust-style: none is falsy, values are truthy)
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
            Statement::For {
                var_name,
                value_name,
                iterable,
                body,
            } => {
                let source_var = if let Expression::Variable(var) = &iterable {
                    Some(var.clone())
                } else {
                    None
                };
                
                let iter_value = self.eval_expression(iterable)?;
                
                match iter_value {
                    // Dictionary iteration
                    Value::Dict(map) => {
                        if let Some(val_name) = value_name {
                            // for k, v in dict - iterate over key-value pairs
                            for (key, value) in map {
                                self.variables.insert(var_name.clone(), key);
                                self.variables.insert(val_name.clone(), value);
                                
                                // Update completion list
                                if let Some(ref var_names) = self.variable_names {
                                    if let Ok(mut names) = var_names.lock() {
                                        names.insert(var_name.clone());
                                        names.insert(val_name.clone());
                                    }
                                }
                                
                                // Execute loop body
                                for stmt in &body {
                                    self.execute_statement(stmt.clone())?;
                                    if self.return_value.is_some() {
                                        break;
                                    }
                                }
                                
                                if self.return_value.is_some() {
                                    break;
                                }
                            }
                        } else {
                            // for k in dict - iterate over keys only
                            for key in map.keys() {
                                self.variables.insert(var_name.clone(), key.clone());
                                
                                // Update completion list
                                if let Some(ref var_names) = self.variable_names {
                                    if let Ok(mut names) = var_names.lock() {
                                        names.insert(var_name.clone());
                                    }
                                }
                                
                                // Execute loop body
                                for stmt in &body {
                                    self.execute_statement(stmt.clone())?;
                                    if self.return_value.is_some() {
                                        break;
                                    }
                                }
                                
                                if self.return_value.is_some() {
                                    break;
                                }
                            }
                        }
                    }
                    // Array iteration (existing behavior)
                    Value::Object(serde_json::Value::Array(arr)) => {
                        if value_name.is_some() {
                            return Err("Cannot use key-value syntax with arrays. Use 'for item in array' instead.".to_string());
                        }
                        
                        // Convert JSON array to Vec<Value>
                        let items: Vec<Value> = arr.into_iter().map(|v| self.json_to_value(v)).collect();
                        
                        // Iterate through items
                        for item in items {
                            self.variables.insert(var_name.clone(), item);
                            
                            // Update completion list
                            if let Some(ref var_names) = self.variable_names {
                                if let Ok(mut names) = var_names.lock() {
                                    names.insert(var_name.clone());
                                }
                            }
                            
                            // Execute loop body
                            for stmt in &body {
                                self.execute_statement(stmt.clone())?;
                                if self.return_value.is_some() {
                                    break;
                                }
                            }
                            
                            if self.return_value.is_some() {
                                break;
                            }
                        }
                    }
                    // List iteration
                    Value::List(items) => {
                        if value_name.is_some() {
                            return Err("Cannot use key-value syntax with lists. Use 'for item in list' instead.".to_string());
                        }
                        
                        let items_clone = items.clone();
                        let mut updated_items = Vec::new();
                        let mut early_break = false;
                        
                        for (index, item) in items.into_iter().enumerate() {
                            self.variables.insert(var_name.clone(), item.clone());
                            
                            // Update completion list
                            if let Some(ref var_names) = self.variable_names {
                                if let Ok(mut names) = var_names.lock() {
                                    names.insert(var_name.clone());
                                }
                            }
                            
                            // Execute loop body
                            for stmt in &body {
                                self.execute_statement(stmt.clone())?;
                                if self.return_value.is_some() {
                                    early_break = true;
                                    break;
                                }
                            }
                            
                            // Get the potentially modified value
                            let final_value = self.variables.get(&var_name).cloned().unwrap_or(item);
                            updated_items.push(final_value);
                            
                            if early_break {
                                // If we're breaking early, keep the rest of the items unchanged
                                for remaining_item in items_clone.iter().skip(index + 1) {
                                    updated_items.push(remaining_item.clone());
                                }
                                break;
                            }
                        }
                        
                        // If iterating over a variable, update it with potentially modified items
                        if let Some(var) = source_var {
                            self.variables.insert(var, Value::List(updated_items));
                        }
                    }
                    // Set iteration
                    Value::Set(set) => {
                        if value_name.is_some() {
                            return Err("Cannot use key-value syntax with sets. Use 'for item in set' instead.".to_string());
                        }
                        
                        // Convert to sorted vec for deterministic iteration
                        let mut items: Vec<_> = set.into_iter().collect();
                        items.sort_by_key(|v| v.to_string());
                        
                        for item in items {
                            self.variables.insert(var_name.clone(), item);
                            
                            // Update completion list
                            if let Some(ref var_names) = self.variable_names {
                                if let Ok(mut names) = var_names.lock() {
                                    names.insert(var_name.clone());
                                }
                            }
                            
                            // Execute loop body
                            for stmt in &body {
                                self.execute_statement(stmt.clone())?;
                                if self.return_value.is_some() {
                                    break;
                                }
                            }
                            
                            if self.return_value.is_some() {
                                break;
                            }
                        }
                    }
                    _ => {
                        return Err(format!("For loop requires an iterable (array, list, or dict), got: {:?}", iter_value));
                    }
                }
            }
            Statement::Use(module) => {
                // Handle use statements
                if module == "ut" {
                    let ut_instance = self.test_runner.enable_ut_module();
                    self.variables.insert("ut".to_string(), ut_instance);
                }
            }
            Statement::FunctionDef { name, params, body, annotations } => {
                // Track test functions
                if annotations.contains(&"test".to_string()) {
                    self.test_runner.register_test(name.clone());
                }
                self.functions.insert(name, (params, body, annotations));
            }
            Statement::ClassDef { name, parent, fields, methods } => {
                self.classes.insert(name, (parent, fields, methods));
            }
            Statement::FunctionCall { name, args } => {
                self.call_function(&name, args)?;
            }
            Statement::Command(cmd) => {
                // Execute all commands through execute_pipeline (handles built-ins like cd)
                if let Err(e) = execute_pipeline(&cmd) {
                    eprintln!("Pipeline error: {}", e);
                }
            }
            Statement::Return(expr) => {
                let value = self.eval_expression(expr)?;
                self.return_value = Some(value);
            }
            Statement::Print(expr) => {
                let value = self.eval_expression(expr)?;
                let output = self.value_to_display_string(value)?;
                println!("{}", output);
            }
            Statement::Assert { condition, message } => {
                let result = self.eval_expression(condition.clone())?;
                if !result.to_bool() {
                    let msg = if let Some(msg_expr) = message {
                        let msg_val = self.eval_expression(msg_expr)?;
                        msg_val.to_string()
                    } else {
                        format!("Assertion failed: {:?}", condition)
                    };
                    return Err(format!("Assertion failed: {}", msg));
                }
            }
            Statement::Exit(expr_opt) => {
                let code = if let Some(expr) = expr_opt {
                    let value = self.eval_expression(expr)?;
                    match value {
                        Value::Number(n) => n as i32,
                        Value::Bool(b) => if b { 0 } else { 1 },  // true = 0 (success), false = 1 (failure)
                        _ => return Err("Exit code must be a number or boolean".to_string()),
                    }
                } else {
                    0
                };
                self.exit_code = Some(code);
            }
            Statement::ExprStmt(expr) => {
                if let Expression::MethodCall { ref object, .. } = expr {
                    if let Expression::Variable(var_name) = object.as_ref() {
                        let var_name = var_name.clone();
                        let result = self.eval_expression(expr)?;
                        if matches!(result, Value::Instance { .. }) {
                            self.variables.insert(var_name, result);
                        }
                        return Ok(());
                    }
                }
                self.eval_expression(expr)?;
            }
        }
        Ok(())
    }
}
