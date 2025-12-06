use super::super::scripting::*;
use super::super::execution::{execute_pipeline_capture, execute_with_object_pipe};
use super::Interpreter;
use super::methods::*;
use std::collections::HashMap;

impl Interpreter {
    pub fn eval_expression(&mut self, expr: Expression) -> Result<Value, String> {
        match expr {
            Expression::Value(val) => Ok(val),
            Expression::Variable(name) => {
                self.variables
                    .get(&name)
                    .cloned()
                    .ok_or(format!("Variable '{}' not found", name))
            }
            Expression::This => {
                self.current_instance
                    .clone()
                    .ok_or("'this' can only be used inside a method".to_string())
            }
            Expression::UnaryOp { op, operand } => {
                match op {
                    Operator::Not => {
                        let operand_val = self.eval_expression(*operand)?;
                        Ok(Value::Bool(!operand_val.to_bool()))
                    }
                    _ => Err(format!("Unsupported unary operator: {:?}", op))
                }
            }
            Expression::BinaryOp { left, op, right } => {
                match op {
                    Operator::And => {
                        let left_val = self.eval_expression(*left)?;
                        if !left_val.to_bool() {
                            return Ok(Value::Bool(false));
                        }
                        let right_val = self.eval_expression(*right)?;
                        Ok(Value::Bool(right_val.to_bool()))
                    }
                    Operator::Or => {
                        let left_val = self.eval_expression(*left)?;
                        if left_val.to_bool() {
                            return Ok(Value::Bool(true));
                        }
                        let right_val = self.eval_expression(*right)?;
                        Ok(Value::Bool(right_val.to_bool()))
                    }
                    Operator::Lt | Operator::Gt | Operator::Le | Operator::Ge => {
                        let left_val = self.eval_expression(*left)?;
                        let right_val = self.eval_expression(*right)?;
                        
                        if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                            let result = match op {
                                Operator::Lt => l < r,
                                Operator::Gt => l > r,
                                Operator::Le => l <= r,
                                Operator::Ge => l >= r,
                                _ => unreachable!(),
                            };
                            return Ok(Value::Bool(result));
                        }
                        
                        self.apply_operator(left_val, op, right_val)
                    }
                    _ => {
                        let left_val = self.eval_expression(*left)?;
                        let right_val = self.eval_expression(*right)?;
                        self.apply_operator(left_val, op, right_val)
                    }
                }
            }
            Expression::FunctionCall { name, args } => {
                self.call_function(&name, args)
            }
            Expression::NewInstance { class_name } => {
                // Create a new instance of the class, inheriting from parent if exists
                let field_values = self.collect_inherited_fields(&class_name)?;
                
                Ok(Value::Instance {
                    class_name,
                    fields: field_values,
                })
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
                // Replace {var} or {expr} with evaluated values
                let mut result = template.clone();
                let mut start = 0;
                
                while let Some(open_pos) = result[start..].find('{') {
                    let open_pos = start + open_pos;
                    if let Some(close_pos) = result[open_pos..].find('}') {
                        let close_pos = open_pos + close_pos;
                        let expr_str = &result[open_pos + 1..close_pos];
                        
                        // Try to parse and evaluate as an expression
                        let value = if expr_str.contains('.') {
                            // Parse as property access expression
                            let mut parser = Parser::new(expr_str);
                            match parser.parse_expression() {
                                Ok(expr) => self.eval_expression(expr)?,
                                Err(_) => {
                                    // Fallback to simple variable lookup
                                    self.variables
                                        .get(expr_str)
                                        .ok_or(format!("Variable '{}' not found in interpolation", expr_str))?
                                        .clone()
                                }
                            }
                        } else {
                            // Simple variable lookup
                            self.variables
                                .get(expr_str)
                                .ok_or(format!("Variable '{}' not found in interpolation", expr_str))?
                                .clone()
                        };
                        
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
                    Value::Instance { class_name, mut fields } => {
                        // First check if it's a field
                        if let Some(value) = fields.get(&property) {
                            return Ok(value.clone());
                        }
                        
                        // Check if it's a method
                        if let Some((_, _, methods)) = self.classes.get(&class_name) {
                            for (method_name, _, _) in methods {
                                if method_name == &property {
                                    return Err(format!("Method calls not yet fully implemented: {}", property));
                                }
                            }
                        }
                        
                        // Try to interpret property name as a command (e.g., "env" -> "get-environment")
                        let potential_commands = if property.contains('_') {
                            vec![property.replace('_', "-")]
                        } else {
                            // Map common abbreviations to full commands
                            let full_name = match property.as_str() {
                                "env" => "environment",
                                "dir" => "directory",
                                "pwd" => "working-directory",
                                "host" => "hostname",
                                "user" => "username",
                                _ => property.as_str(),
                            };
                            
                            // Try common prefixes for plain names
                            vec![
                                format!("get-{}", full_name),
                                format!("list-{}", full_name),
                                property.clone(),
                            ]
                        };
                        
                        // Try to execute the first command that succeeds
                        for cmd in potential_commands {
                            let cmd_expr = Expression::CommandOutput(cmd.clone());
                            if let Ok(result) = self.eval_expression(cmd_expr) {
                                return Ok(result);
                            }
                        }
                        
                        Err(format!("Property or method '{}' not found in class {}", property, class_name))
                    }
                    _ => Err(format!("Cannot access property '{}' on non-object value", property))
                }
            }
            Expression::IndexAccess { object, index } => {
                let obj_value = self.eval_expression(*object)?;
                let index_value = self.eval_expression(*index)?;
                
                match obj_value {
                    Value::List(list) => {
                        let idx = index_value.to_number() as i64;
                        let actual_idx = if idx < 0 {
                            // Python-style negative indexing
                            let len = list.len() as i64;
                            if len + idx < 0 {
                                return Err(format!("Index {} out of bounds (list length: {})", idx, list.len()));
                            }
                            (len + idx) as usize
                        } else {
                            idx as usize
                        };
                        
                        if actual_idx < list.len() {
                            Ok(list[actual_idx].clone())
                        } else {
                            Err(format!("Index {} out of bounds (list length: {})", idx, list.len()))
                        }
                    }
                    Value::Dict(map) => {
                        if let Some(value) = map.get(&index_value) {
                            Ok(value.clone())
                        } else {
                            Err(format!("Key '{}' not found in dictionary", index_value.to_string()))
                        }
                    }
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
                    _ => Err("Cannot index non-list/non-object value".to_string())
                }
            }
            Expression::MethodCall { object, method, args } => {
                let obj_value = self.eval_expression(*object)?;
                
                // Handle universal optional value methods (Rust-style Option methods)
                if let Some(result) = handle_optional_method(&method, obj_value.clone(), &args, &mut |expr| self.eval_expression(expr)) {
                    return result;
                }
                
                // Handle universal apply method - apply a closure to any value
                if method == "apply" {
                    if args.len() != 1 {
                        return Err(format!("apply() expects 1 argument (closure), got {}", args.len()));
                    }
                    let closure = self.eval_expression(args[0].clone())?;
                    return self.apply_closure(closure, vec![obj_value]);
                }
                
                match obj_value {
                    Value::String(s) => {
                        handle_string_methods(&method, s, &args, &mut |expr| self.eval_expression(expr))
                    }
                    Value::List(list) => {
                        // Handle functional methods that need closures
                        match method.as_str() {
                            "map" => {
                                if args.len() != 1 {
                                    return Err(format!("map() expects 1 argument (closure), got {}", args.len()));
                                }
                                let closure = self.eval_expression(args[0].clone())?;
                                
                                let mut mapped = Vec::new();
                                for item in list {
                                    let result = self.apply_closure(closure.clone(), vec![item])?;
                                    mapped.push(result);
                                }
                                Ok(Value::List(mapped))
                            }
                            "filter" => {
                                if args.len() != 1 {
                                    return Err(format!("filter() expects 1 argument (closure), got {}", args.len()));
                                }
                                let closure = self.eval_expression(args[0].clone())?;
                                
                                let mut filtered = Vec::new();
                                for item in list {
                                    let result = self.apply_closure(closure.clone(), vec![item.clone()])?;
                                    if result.to_bool() {
                                        filtered.push(item);
                                    }
                                }
                                Ok(Value::List(filtered))
                            }
                            "reduce" => {
                                if args.len() != 2 {
                                    return Err(format!("reduce() expects 2 arguments (initial_value, closure), got {}", args.len()));
                                }
                                let mut accumulator = self.eval_expression(args[0].clone())?;
                                let closure = self.eval_expression(args[1].clone())?;
                                
                                for item in list {
                                    accumulator = self.apply_closure(closure.clone(), vec![accumulator, item])?;
                                }
                                Ok(accumulator)
                            }
                            _ => {
                                // Handle other list methods
                                handle_list_methods(&method, list, &args, &mut |expr| self.eval_expression(expr))
                            }
                        }
                    }
                    Value::Dict(map) => {
                        handle_dict_methods(&method, map, &args, &mut |expr| self.eval_expression(expr))
                    }
                    Value::Set(set) => {
                        handle_set_methods(&method, set, &args, &mut |expr| self.eval_expression(expr))
                    }
                    Value::Object(json_obj) => {
                        // Handle methods on JSON objects
                        // If the object is a JSON array, convert it to a List and handle functional methods
                        if let serde_json::Value::Array(arr) = json_obj {
                            // Convert JSON array to Value::List
                            let list: Vec<Value> = arr.iter()
                                .map(|v| self.json_to_value(v.clone()))
                                .collect();
                            
                            // Handle functional methods
                            match method.as_str() {
                                "map" => {
                                    if args.len() != 1 {
                                        return Err(format!("map() expects 1 argument (closure), got {}", args.len()));
                                    }
                                    let closure = self.eval_expression(args[0].clone())?;
                                    
                                    let mut mapped = Vec::new();
                                    for item in list {
                                        let result = self.apply_closure(closure.clone(), vec![item])?;
                                        mapped.push(result);
                                    }
                                    Ok(Value::List(mapped))
                                }
                                "filter" => {
                                    if args.len() != 1 {
                                        return Err(format!("filter() expects 1 argument (closure), got {}", args.len()));
                                    }
                                    let closure = self.eval_expression(args[0].clone())?;
                                    
                                    let mut filtered = Vec::new();
                                    for item in list {
                                        let result = self.apply_closure(closure.clone(), vec![item.clone()])?;
                                        if result.to_bool() {
                                            filtered.push(item);
                                        }
                                    }
                                    Ok(Value::List(filtered))
                                }
                                "reduce" => {
                                    if args.len() != 2 {
                                        return Err(format!("reduce() expects 2 arguments (initial_value, closure), got {}", args.len()));
                                    }
                                    let mut accumulator = self.eval_expression(args[0].clone())?;
                                    let closure = self.eval_expression(args[1].clone())?;
                                    
                                    for item in list {
                                        accumulator = self.apply_closure(closure.clone(), vec![accumulator, item])?;
                                    }
                                    Ok(accumulator)
                                }
                                _ => {
                                    // Handle other list methods on the converted list
                                    handle_list_methods(&method, list, &args, &mut |expr| self.eval_expression(expr))
                                }
                            }
                        } else {
                            Err(format!("Cannot call method '{}' on non-array JSON object", method))
                        }
                    }
                    Value::Instance { class_name, fields} => {
                        // Handle ut built-in object methods
                        if class_name == "UT" {
                            return call_ut_method(&method, &args, &mut |expr| self.eval_expression(expr));
                        }
                        
                        // Check cache first
                        let cache_key = (class_name.clone(), method.clone());
                        let method_data = if let Some(cached) = self.method_lookup_cache.get(&cache_key) {
                            cached.clone()
                        } else {
                            // Find the method in the class hierarchy
                            let mut current_class = Some(class_name.clone());
                            let mut found_method = None;
                            
                            while let Some(ref cls) = current_class {
                                if let Some((parent, _, methods)) = self.classes.get(cls) {
                                    // Look for the method in this class
                                    for (method_name, params, body) in methods {
                                        if method_name == &method {
                                            found_method = Some((params.clone(), body.clone()));
                                            break;
                                        }
                                    }
                                    if found_method.is_some() {
                                        break;
                                    }
                                    current_class = parent.clone();
                                } else {
                                    break;
                                }
                            }
                            
                            // Cache the result (even if None)
                            self.method_lookup_cache.insert(cache_key, found_method.clone());
                            found_method
                        };
                        
                        if let Some((params, body)) = method_data {
                            // Create a new scope for the method (only parameters, not fields)
                            let mut method_scope = HashMap::new();
                            
                            // Evaluate and bind arguments to parameters
                            if args.len() != params.len() {
                                return Err(format!(
                                    "Method {} expects {} arguments, got {}",
                                    method, params.len(), args.len()
                                ));
                            }
                            
                            for (i, arg_expr) in args.iter().enumerate() {
                                let arg_value = self.eval_expression(arg_expr.clone())?;
                                method_scope.insert(params[i].clone(), arg_value);
                            }
                            
                            // Save current variables and instance context
                            let saved_vars = self.variables.clone();
                            let saved_instance = self.current_instance.clone();
                            
                            // Merge fields into the method scope (so they can be accessed/modified)
                            for (field_name, field_value) in &fields {
                                // Don't overwrite parameters with field values
                                if !method_scope.contains_key(field_name) {
                                    method_scope.insert(field_name.clone(), field_value.clone());
                                }
                            }
                            
                            self.variables = method_scope;
                            
                            // Set current instance for 'this' keyword
                            self.current_instance = Some(Value::Instance {
                                class_name: class_name.clone(),
                                fields: fields.clone(),
                            });
                            
                            // Execute method body
                            let mut return_value = Value::None;
                            for stmt in &body {
                                match stmt {
                                    Statement::Return(expr) => {
                                        return_value = self.eval_expression(expr.clone())?;
                                        break;
                                    }
                                    _ => {
                                        self.execute_statement(stmt.clone())?;
                                        // Check if a return was triggered inside the statement (e.g., in an if block)
                                        if let Some(ret_val) = self.return_value.take() {
                                            return_value = ret_val;
                                            break;
                                        }
                                    }
                                }
                            }
                            
                            // Collect modified field values from the method scope
                            let mut updated_fields = fields.clone();
                            for (field_name, _) in &fields {
                                if let Some(modified_value) = self.variables.get(field_name) {
                                    updated_fields.insert(field_name.clone(), modified_value.clone());
                                }
                            }
                            
                            // If return value is 'this', update it with modified fields
                            if let Value::Instance { class_name: ret_class, fields: _ } = &return_value {
                                if ret_class == &class_name {
                                    return_value = Value::Instance {
                                        class_name: class_name.clone(),
                                        fields: updated_fields,
                                    };
                                }
                            }
                            
                            // Restore variables and instance context
                            self.variables = saved_vars;
                            self.current_instance = saved_instance;
                            
                            return Ok(return_value);
                        } else {
                            return Err(format!("Method '{}' not found in class {}", method, class_name));
                        }
                    }
                    _ => Err(format!("Cannot call method on non-instance/non-list value"))
                }
            }
            Expression::Pipeline { input, command } => {
                // Evaluate the input expression to get JSON
                let input_value = self.eval_expression(*input)?;
                
                // Convert the input value to JSON string
                let json_input = match &input_value {
                    Value::Object(json_obj) => serde_json::to_string(&json_obj)
                        .map_err(|e| format!("Failed to serialize input to JSON: {}", e))?,
                    Value::String(s) => s.clone(),
                    Value::SmallInt(i) => i.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::None => "none".to_string(),
                    Value::Instance { .. } => return Err("Cannot pipe instance objects".to_string()),
                    Value::List(_) => return Err("Cannot pipe list objects yet".to_string()),
                    Value::Dict(_) => return Err("Cannot pipe dict objects yet".to_string()),
                    Value::Set(_) => return Err("Cannot pipe set objects yet".to_string()),
                    Value::Closure { .. } => return Err("Cannot pipe closure objects".to_string()),
                };
                
                // Execute the pipeline with the JSON input
                let cmd_parts: Vec<String> = command.split_whitespace().map(|s| s.to_string()).collect();
                let output = execute_with_object_pipe(&cmd_parts, Some(&json_input), true)
                    .map_err(|e| format!("Pipeline error: {}", e))?;
                
                // Try to parse output as JSON
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&output) {
                    Ok(Value::Object(json_value))
                } else {
                    Ok(Value::String(output.trim().to_string()))
                }
            }
            Expression::ListLiteral(elements) => {
                // Evaluate each element expression and collect into a list
                let mut list = Vec::new();
                for elem_expr in elements {
                    let elem_value = self.eval_expression(elem_expr)?;
                    list.push(elem_value);
                }
                Ok(Value::List(list))
            }
            Expression::DictLiteral(pairs) => {
                // Evaluate each key-value pair and collect into a dict
                let mut map = std::collections::HashMap::new();
                for (key_expr, value_expr) in pairs {
                    let key = self.eval_expression(key_expr)?;
                    let value = self.eval_expression(value_expr)?;
                    map.insert(key, value);
                }
                Ok(Value::Dict(map))
            }
            Expression::SetLiteral(elements) => {
                // Evaluate each element and collect into a set
                let mut set = std::collections::HashSet::new();
                for elem_expr in elements {
                    let elem_value = self.eval_expression(elem_expr)?;
                    set.insert(elem_value);
                }
                Ok(Value::Set(set))
            }
            Expression::Closure { params, body } => {
                // Return the closure as a value without evaluating the body
                Ok(Value::Closure {
                    params,
                    body,
                })
            }
        }
    }

    pub(super) fn apply_operator(&self, left: Value, op: Operator, right: Value) -> Result<Value, String> {
        match op {
            Operator::Add => match (left, right) {
                (Value::SmallInt(a), Value::SmallInt(b)) => {
                    Ok(a.checked_add(b)
                        .map(Value::SmallInt)
                        .unwrap_or_else(|| Value::Number((a as f64) + (b as f64))))
                }
                (Value::SmallInt(a), Value::Number(b)) => Ok(Value::Number((a as f64) + b)),
                (Value::Number(a), Value::SmallInt(b)) => Ok(Value::Number(a + (b as f64))),
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{}{}", a, b))),
                // None concatenation: none + string = string
                (Value::None, Value::String(b)) => Ok(Value::String(b)),
                (Value::String(a), Value::None) => Ok(Value::String(a)),
                (Value::None, Value::None) => Ok(Value::String("".to_string())),
                // String concatenation with other types (none converts to empty string)
                (Value::String(a), b) => {
                    let b_str = match b {
                        Value::None => "".to_string(),
                        _ => b.to_string(),
                    };
                    Ok(Value::String(format!("{}{}", a, b_str)))
                }
                (a, Value::String(b)) => {
                    let a_str = match a {
                        Value::None => "".to_string(),
                        _ => a.to_string(),
                    };
                    Ok(Value::String(format!("{}{}", a_str, b)))
                }
                _ => Err("Invalid operands for +".to_string()),
            },
            Operator::Sub => match (left, right) {
                (Value::SmallInt(a), Value::SmallInt(b)) => {
                    Ok(a.checked_sub(b)
                        .map(Value::SmallInt)
                        .unwrap_or_else(|| Value::Number((a as f64) - (b as f64))))
                }
                (Value::SmallInt(a), Value::Number(b)) => Ok(Value::Number((a as f64) - b)),
                (Value::Number(a), Value::SmallInt(b)) => Ok(Value::Number(a - (b as f64))),
                (l, r) => Ok(Value::Number(l.to_number() - r.to_number())),
            },
            Operator::Mul => match (left, right) {
                (Value::SmallInt(a), Value::SmallInt(b)) => {
                    Ok(a.checked_mul(b)
                        .map(Value::SmallInt)
                        .unwrap_or_else(|| Value::Number((a as f64) * (b as f64))))
                }
                (Value::SmallInt(a), Value::Number(b)) => Ok(Value::Number((a as f64) * b)),
                (Value::Number(a), Value::SmallInt(b)) => Ok(Value::Number(a * (b as f64))),
                (l, r) => Ok(Value::Number(l.to_number() * r.to_number())),
            },
            Operator::Div => {
                let divisor = right.to_number();
                if divisor == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(Value::Number(left.to_number() / divisor))
                }
            }
            Operator::Mod => match (left, right) {
                (Value::SmallInt(a), Value::SmallInt(b)) => {
                    if b == 0 {
                        Err("Modulo by zero".to_string())
                    } else {
                        Ok(Value::SmallInt(a % b))
                    }
                }
                (l, r) => {
                    let divisor = r.to_number();
                    if divisor == 0.0 {
                        Err("Modulo by zero".to_string())
                    } else {
                        Ok(Value::Number(l.to_number() % divisor))
                    }
                }
            }
            Operator::Eq => Ok(Value::Bool(left == right)),
            Operator::Ne => Ok(Value::Bool(left != right)),
            Operator::Lt => Ok(Value::Bool(left.to_number() < right.to_number())),
            Operator::Gt => Ok(Value::Bool(left.to_number() > right.to_number())),
            Operator::Le => Ok(Value::Bool(left.to_number() <= right.to_number())),
            Operator::Ge => Ok(Value::Bool(left.to_number() >= right.to_number())),
            Operator::And => Ok(Value::Bool(left.to_bool() && right.to_bool())),
            Operator::Or => Ok(Value::Bool(left.to_bool() || right.to_bool())),
            Operator::Not => Err("NOT is a unary operator and should not be used in apply_operator".to_string()),
        }
    }

    /// Apply a closure to arguments
    pub(super) fn apply_closure(&mut self, closure: Value, args: Vec<Value>) -> Result<Value, String> {
        match closure {
            Value::Closure { params, body } => {
                if params.len() != args.len() {
                    return Err(format!(
                        "Closure expects {} arguments, got {}",
                        params.len(),
                        args.len()
                    ));
                }

                // Save current variable bindings
                let saved_vars: HashMap<String, Value> = params
                    .iter()
                    .filter_map(|p| self.variables.get(p).map(|v| (p.clone(), v.clone())))
                    .collect();

                // Bind closure parameters to arguments
                for (param, arg) in params.iter().zip(args.iter()) {
                    self.variables.insert(param.clone(), arg.clone());
                }

                // Evaluate the closure body
                let result = self.eval_expression(*body);

                // Restore previous variable bindings
                for param in &params {
                    if let Some(old_value) = saved_vars.get(param) {
                        self.variables.insert(param.clone(), old_value.clone());
                    } else {
                        self.variables.remove(param);
                    }
                }

                result
            }
            _ => Err("Expected a closure".to_string()),
        }
    }
    
    // Helper to evaluate method calls on an owned value (avoids cloning)
    pub(super) fn eval_method_call_on_value(
        &mut self,
        obj_value: &mut Value,
        method: &str,
        arg_values: &[Value],
    ) -> Result<Value, String> {
        // Convert arg_values to expressions for the helper functions
        // This is a workaround - ideally we'd refactor the helpers to take Values
        let args: Vec<Expression> = arg_values.iter().map(|v| Expression::Value(v.clone())).collect();
        let mut eval_fn = |expr: Expression| -> Result<Value, String> {
            if let Expression::Value(v) = expr {
                Ok(v)
            } else {
                self.eval_expression(expr)
            }
        };
        
        match obj_value {
            Value::List(list) => {
                let list_clone = std::mem::take(list); // Take ownership without cloning
                handle_list_methods(method, list_clone, &args, &mut eval_fn)
            }
            Value::String(s) => {
                handle_string_methods(method, s.clone(), &args, &mut eval_fn)
            }
            Value::Dict(map) => {
                let map_clone = std::mem::take(map);
                handle_dict_methods(method, map_clone, &args, &mut eval_fn)
            }
            Value::Set(set) => {
                let set_clone = std::mem::take(set);
                handle_set_methods(method, set_clone, &args, &mut eval_fn)
            }
            _ => Err(format!("Method '{}' not supported on this value type", method))
        }
    }
}
