use super::scripting::*;
use super::execution::{execute_pipeline, execute_pipeline_capture};
use std::collections::{HashMap, HashSet};
use std::process::Command as ProcessCommand;
use std::sync::{Arc, Mutex};

pub struct Interpreter {
    variables: HashMap<String, Value>,
    functions: HashMap<String, (Vec<String>, Vec<Statement>)>,
    classes: HashMap<String, (Option<String>, Vec<(String, Expression)>, Vec<(String, Vec<String>, Vec<Statement>)>)>, // class name -> (parent, fields, methods)
    return_value: Option<Value>,
    exit_code: Option<i32>,
    variable_names: Option<Arc<Mutex<HashSet<String>>>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            variables: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            return_value: None,
            exit_code: None,
            variable_names: None,
        }
    }
    
    pub fn new_with_completion(variable_names: Arc<Mutex<HashSet<String>>>) -> Self {
        Interpreter {
            variables: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            return_value: None,
            exit_code: None,
            variable_names: Some(variable_names),
        }
    }

    pub fn execute(&mut self, statements: Vec<Statement>) -> Result<i32, String> {
        for stmt in statements {
            self.execute_statement(stmt)?;
            if self.return_value.is_some() || self.exit_code.is_some() {
                break;
            }
        }
        Ok(self.exit_code.unwrap_or(0))
    }

    fn execute_statement(&mut self, stmt: Statement) -> Result<(), String> {
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
                // Enforce strict type checking - only booleans allowed in conditions
                match &cond_value {
                    Value::Bool(_) => {
                        // OK - boolean value
                    }
                    Value::Number(_) => {
                        return Err("Type error: if condition must be a boolean. Use bool() to convert numbers.".to_string());
                    }
                    Value::String(_) => {
                        return Err("Type error: if condition must be a boolean. Use bool() to convert strings.".to_string());
                    }
                    Value::Null => {
                        return Err("Type error: if condition must be a boolean. Use bool() to convert null.".to_string());
                    }
                    Value::Object(_) => {
                        return Err("Type error: if condition must be a boolean. Use bool() to convert objects.".to_string());
                    }
                    Value::Instance { .. } => {
                        return Err("Type error: if condition must be a boolean. Use bool() to convert instances.".to_string());
                    }
                }
                
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
            Statement::ClassDef { name, parent, fields, methods } => {
                self.classes.insert(name, (parent, fields, methods));
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
            Statement::Exit(expr_opt) => {
                let code = if let Some(expr) = expr_opt {
                    let value = self.eval_expression(expr)?;
                    match value {
                        Value::Number(n) => n as i32,
                        _ => return Err("Exit code must be a number".to_string()),
                    }
                } else {
                    0
                };
                self.exit_code = Some(code);
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
                // Short-circuit evaluation for && and ||
                match op {
                    Operator::And => {
                        let left_val = self.eval_expression(*left)?;
                        if !left_val.to_bool() {
                            // Left is false, short-circuit
                            return Ok(Value::Bool(false));
                        }
                        // Left is true, evaluate right
                        let right_val = self.eval_expression(*right)?;
                        Ok(Value::Bool(right_val.to_bool()))
                    }
                    Operator::Or => {
                        let left_val = self.eval_expression(*left)?;
                        if left_val.to_bool() {
                            // Left is true, short-circuit
                            return Ok(Value::Bool(true));
                        }
                        // Left is false, evaluate right
                        let right_val = self.eval_expression(*right)?;
                        Ok(Value::Bool(right_val.to_bool()))
                    }
                    _ => {
                        // Normal evaluation for other operators
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
                    Value::Instance { class_name, mut fields } => {
                        // First check if it's a field
                        if let Some(value) = fields.get(&property) {
                            return Ok(value.clone());
                        }
                        
                        // Otherwise check if it's a method - return a callable representation
                        // For now, we'll store the method name in a special way
                        // This needs to be called later
                        if let Some((_, _, methods)) = self.classes.get(&class_name) {
                            for (method_name, _, _) in methods {
                                if method_name == &property {
                                    // Store instance in a temporary way for method calls
                                    // For now, return a special marker
                                    return Err(format!("Method calls not yet fully implemented: {}", property));
                                }
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
            Operator::Not => Err("NOT is a unary operator and should not be used in apply_operator".to_string()),
        }
    }

    fn call_function(&mut self, name: &str, args: Vec<Expression>) -> Result<Value, String> {
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
            _ => {}
        }

        // Get user-defined function definition
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

    /// Recursively collect all inherited fields from a class and its ancestors
    fn collect_inherited_fields(&mut self, class_name: &str) -> Result<HashMap<String, Value>, String> {
        let (parent, fields, _methods) = self
            .classes
            .get(class_name)
            .cloned()
            .ok_or(format!("Class '{}' not found", class_name))?;
        
        let mut field_values = HashMap::new();
        
        // First, recursively inherit fields from parent class if it exists
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
    
    // Methods for completion support
    pub fn get_variable_class(&self, var_name: &str) -> Option<String> {
        if let Some(Value::Instance { class_name, .. }) = self.variables.get(var_name) {
            Some(class_name.clone())
        } else {
            None
        }
    }
    
    pub fn get_class_fields(&self, class_name: &str) -> Option<Vec<String>> {
        // Collect all fields including inherited ones
        let mut field_names = HashSet::new();
        let mut current_class = Some(class_name.to_string());
        
        while let Some(ref cls) = current_class {
            if let Some((parent, fields, _methods)) = self.classes.get(cls) {
                for (field_name, _) in fields {
                    field_names.insert(field_name.clone());
                }
                current_class = parent.clone();
            } else {
                break;
            }
        }
        
        if field_names.is_empty() {
            None
        } else {
            let mut result: Vec<String> = field_names.into_iter().collect();
            result.sort();
            Some(result)
        }
    }
    
    pub fn get_all_class_names(&self) -> Vec<String> {
        let mut class_names: Vec<String> = self.classes.keys().cloned().collect();
        class_names.sort();
        class_names
    }
}

pub fn execute_script(script: &str) -> Result<i32, String> {
    let mut parser = Parser::new(script);
    let statements = parser.parse()?;
    let mut interpreter = Interpreter::new();
    let exit_code = interpreter.execute(statements)?;
    Ok(exit_code)
}

pub fn execute_script_with_interpreter(script: &str, interpreter: &mut Interpreter) -> Result<i32, String> {
    let mut parser = Parser::new(script);
    let statements = parser.parse()?;
    let exit_code = interpreter.execute(statements)?;
    Ok(exit_code)
}
