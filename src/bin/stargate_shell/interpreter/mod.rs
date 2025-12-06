use super::scripting::*;
use super::testing::TestRunner;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

mod methods;
mod statement_execution;
mod expression_eval;
mod function_class_utils;
mod object_methods;

pub struct Interpreter {
    variables: HashMap<String, Value>,
    functions: HashMap<String, (Vec<String>, Vec<Statement>, Vec<String>)>,
    classes: HashMap<String, (Option<String>, Vec<(String, Expression)>, Vec<(String, Vec<String>, Vec<Statement>)>)>,
    object_methods_cache: HashMap<String, bool>,
    method_lookup_cache: HashMap<(String, String), Option<(Vec<String>, Vec<Statement>)>>, // (class, method) -> (params, body)
    return_value: Option<Value>,
    exit_code: Option<i32>,
    variable_names: Option<Arc<Mutex<HashSet<String>>>>,
    test_runner: TestRunner,
    current_instance: Option<Value>,
    script_path: Option<String>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            variables: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            object_methods_cache: HashMap::new(),
            method_lookup_cache: HashMap::new(),
            return_value: None,
            exit_code: None,
            variable_names: None,
            test_runner: TestRunner::new(),
            current_instance: None,
            script_path: None,
        }
    }
    pub fn new_with_completion(variable_names: Arc<Mutex<HashSet<String>>>) -> Self {
        Interpreter {
            variables: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            object_methods_cache: HashMap::new(),
            method_lookup_cache: HashMap::new(),
            return_value: None,
            exit_code: None,
            variable_names: Some(variable_names),
            test_runner: TestRunner::new(),
            current_instance: None,
            script_path: None,
        }
    }

    pub fn execute(&mut self, statements: Vec<Statement>) -> Result<i32, String> {
        // Store print/exit statements that might reference ut for later
        let mut deferred_stmts = Vec::new();
        
        for stmt in statements {
            // Defer print and exit statements if ut is enabled
            if self.test_runner.is_enabled() && matches!(stmt, Statement::Print(_) | Statement::Exit(_)) {
                deferred_stmts.push(stmt);
                continue;
            }
            
            self.execute_statement(stmt)?;
            if self.return_value.is_some() || self.exit_code.is_some() {
                break;
            }
        }
        
        // If ut module was imported, automatically run all test functions
        if self.test_runner.is_enabled() && self.test_runner.has_tests() {
            self.run_all_tests()?;
        }
        
        // Now execute deferred statements (print/exit with ut.stats/ut.healthy)
        for stmt in deferred_stmts {
            self.execute_statement(stmt)?;
            if self.return_value.is_some() || self.exit_code.is_some() {
                break;
            }
        }
        
        Ok(self.exit_code.unwrap_or(0))
    }
    
    fn run_all_tests(&mut self) -> Result<(), String> {
        let test_fns = self.test_runner.test_functions.clone();
        
        println!("\nRunning {} test(s)\n", test_fns.len());
        
        self.test_runner.test_passed = 0;
        self.test_runner.test_failed = 0;
        
        for test_name in test_fns {
            print!("Running test: {}... ", test_name);
            match self.call_function(&test_name, Vec::new()) {
                Ok(_) => {
                    println!("✓ PASSED");
                    self.test_runner.test_passed += 1;
                }
                Err(e) => {
                    println!("✗ FAILED");
                    if let Some(ref path) = self.script_path {
                        eprintln!("  File: {}", path);
                    }
                    eprintln!("  Error: {}", e);
                    self.test_runner.test_failed += 1;
                }
            }
        }
        
        println!("\nTest Results");
        println!("Passed: {}", self.test_runner.test_passed);
        println!("Failed: {}", self.test_runner.test_failed);
        println!("Total:  {}\n", self.test_runner.test_passed + self.test_runner.test_failed);
        
        // Update ut object with stats and healthy properties
        self.update_ut_stats();
        
        // Don't return error - let the script handle it with exit((ut).healthy)
        Ok(())
    }
    
    fn update_ut_stats(&mut self) {
        let ut_instance = self.test_runner.create_ut_instance();
        self.variables.insert("ut".to_string(), ut_instance);
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



pub fn execute_script_with_path(script: &str, path: Option<&str>) -> Result<i32, String> {
    let mut parser = Parser::new(script);
    let statements = parser.parse()?;
    let mut interpreter = Interpreter::new();
    interpreter.script_path = path.map(|p| p.to_string());
    let exit_code = interpreter.execute(statements)?;
    Ok(exit_code)
}

pub fn execute_stargate_script(script: &str, interpreter: &mut Interpreter, is_interactive: bool) -> Result<i32, String> {
    let mut parser = if is_interactive {
        Parser::new_interactive(script)
    } else {
        Parser::new(script)
    };
    let statements = parser.parse()?;
    let exit_code = interpreter.execute(statements)?;
    Ok(exit_code)
}
