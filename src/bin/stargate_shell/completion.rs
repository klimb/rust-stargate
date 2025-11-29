// Tab completion, hints, and validation for the shell
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;

use super::commands::{get_stargate_commands, get_command_parameters, SHELL_COMMANDS};
use super::interpreter::Interpreter;

const DESCRIBE_COMMAND_PREFIX: &str = "describe-command ";

pub struct StargateCompletion {
    commands: Vec<String>,
    variables: Arc<Mutex<HashSet<String>>>,
    interpreter: Arc<Mutex<Interpreter>>,
}

impl StargateCompletion {
    pub fn new(variables: Arc<Mutex<HashSet<String>>>, interpreter: Arc<Mutex<Interpreter>>) -> Self {
        let mut commands = get_stargate_commands();
        commands.extend(SHELL_COMMANDS.iter().map(|s| s.to_string()));
        commands.sort();
        commands.dedup();
        Self { commands, variables, interpreter }
    }
}

impl Helper for StargateCompletion {}

impl Completer for StargateCompletion {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line = &line[..pos];
        
        // Check for property access pattern: command.property or (command).property or complex expressions
        if let Some(dot_pos) = line.rfind('.') {
            let before_dot = &line[..dot_pos];
            let after_dot = &line[dot_pos + 1..];
            
            // Extract just the expression part if we're in an assignment or declaration
            // e.g., "let foo = (list-directory)" -> "(list-directory)"
            let expr_before_dot = if let Some(eq_pos) = before_dot.rfind('=') {
                before_dot[eq_pos + 1..].trim()
            } else if before_dot.starts_with("print ") {
                before_dot[6..].trim()
            } else {
                before_dot
            };
            
            // Check if this looks like a complex expression (contains [], ., or parentheses)
            let is_complex = expr_before_dot.contains('[') || expr_before_dot.matches('.').count() > 0 
                || (expr_before_dot.contains('(') && expr_before_dot.contains(')'));
            
            if is_complex {
                // Use expression evaluation for complex property access
                if let Some(properties) = get_expression_properties(expr_before_dot) {
                    // When nothing is typed after the dot, print all properties ourselves
                    if after_dot.is_empty() && !properties.is_empty() {
                        // Print properties in columns like bash completion
                        println!();
                        let term_width = 80; // Default terminal width
                        let max_len = properties.iter().map(|s| s.len()).max().unwrap_or(0);
                        let col_width = max_len + 2;
                        let num_cols = (term_width / col_width).max(1);
                        
                        for (i, prop) in properties.iter().enumerate() {
                            print!("{:<width$}", prop, width = col_width);
                            if (i + 1) % num_cols == 0 {
                                println!();
                            }
                        }
                        if properties.len() % num_cols != 0 {
                            println!();
                        }
                    }
                    
                    let matches: Vec<Pair> = properties
                        .into_iter()
                        .filter(|prop| prop.starts_with(after_dot))
                        .map(|prop| Pair {
                            display: prop.clone(),
                            replacement: prop,
                        })
                        .collect();
                    
                    // If we have matches and nothing typed after dot, force immediate display
                    // by ensuring we return from the position after the dot
                    if !matches.is_empty() {
                        return Ok((dot_pos + 1, matches));
                    }
                }
            } else {
                // Simple pattern: command.property or (command).property
                let (cmd_str, needs_parens) = if let Some(close_paren) = before_dot.rfind(')') {
                    // Pattern: (command).property
                    if let Some(open_paren) = before_dot[..close_paren].rfind('(') {
                        (before_dot[open_paren + 1..close_paren].trim(), false)
                    } else {
                        (before_dot, false)
                    }
                } else {
                    // Pattern: command.property (needs auto-parens)
                    let cmd_start = before_dot.rfind(|c: char| c.is_whitespace() || c == '|' || c == '=')
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    (before_dot[cmd_start..].trim(), true)
                };
                
                // Check if it's a variable with an instance value
                if let Ok(interp) = self.interpreter.lock() {
                    if let Some(class_name) = interp.get_variable_class(cmd_str) {
                        if let Some(fields) = interp.get_class_fields(&class_name) {
                            let matches: Vec<Pair> = fields
                                .into_iter()
                                .filter(|field| field.starts_with(after_dot))
                                .map(|field| Pair {
                                    display: field.clone(),
                                    replacement: field,
                                })
                                .collect();
                            
                            if !matches.is_empty() {
                                return Ok((dot_pos + 1, matches));
                            }
                        }
                    }
                }
                
                // Check if it's a stargate command
                if self.commands.contains(&cmd_str.to_string()) && !SHELL_COMMANDS.contains(&cmd_str) {
                    // Execute command to get JSON schema
                    if let Some(properties) = get_command_properties(cmd_str) {
                        let matches: Vec<Pair> = properties
                            .into_iter()
                            .filter(|prop| prop.starts_with(after_dot))
                            .map(|prop| {
                                let replacement = if needs_parens {
                                    format!("({}).{}", cmd_str, prop)
                                } else {
                                    prop.clone()
                                };
                                Pair {
                                    display: prop,
                                    replacement,
                                }
                            })
                            .collect();
                        
                        if needs_parens && !matches.is_empty() {
                            let cmd_start = line.rfind(cmd_str).unwrap();
                            return Ok((cmd_start, matches));
                        } else {
                            return Ok((dot_pos + 1, matches));
                        }
                    }
                }
            }
        }
        
        // Special handling for "describe-command "
        if let Some(rest) = line.strip_prefix(DESCRIBE_COMMAND_PREFIX) {
            let matches: Vec<Pair> = self.commands
                .iter()
                .filter(|cmd| !SHELL_COMMANDS.contains(&cmd.as_str())) // Exclude shell builtins
                .filter(|cmd| cmd.starts_with(rest))
                .map(|cmd| Pair {
                    display: cmd.clone(),
                    replacement: cmd.clone(),
                })
                .collect();
            
            return Ok((DESCRIBE_COMMAND_PREFIX.len(), matches));
        }
        
        // Find the start of the current word
        let start = line.rfind(|c: char| c.is_whitespace() || c == '|')
            .map(|i| i + 1)
            .unwrap_or(0);
        
        let prefix = &line[start..];
        
        if prefix.is_empty() {
            return Ok((start, vec![]));
        }

        // Check if we're after 'print ' or '= ' - suggest variables
        let before_word = &line[..start].trim_end();
        if before_word.ends_with("print") || before_word.ends_with("=") {
            // Get variable names from shared state
            if let Ok(vars) = self.variables.lock() {
                let matches: Vec<Pair> = vars
                    .iter()
                    .filter(|var| var.starts_with(prefix))
                    .map(|var| Pair {
                        display: var.clone(),
                        replacement: var.clone(),
                    })
                    .collect();
                
                if !matches.is_empty() {
                    return Ok((start, matches));
                }
            }
        }

        // Check if we're after 'new ' - suggest class names
        if before_word.ends_with("new") {
            // Get class names from interpreter
            if let Ok(interp) = self.interpreter.lock() {
                let class_names = interp.get_all_class_names();
                let matches: Vec<Pair> = class_names
                    .into_iter()
                    .filter(|class| class.starts_with(prefix))
                    .map(|class| Pair {
                        display: class.clone(),
                        replacement: class,
                    })
                    .collect();
                
                if !matches.is_empty() {
                    return Ok((start, matches));
                }
            }
        }

        // Check if we're completing a parameter (starts with -)
        if prefix.starts_with('-') {
            // Extract the command name (first word after | or at start)
            let cmd_start = line[..start].rfind('|')
                .map(|i| i + 1)
                .unwrap_or(0);
            
            let cmd_part = line[cmd_start..start].trim();
            let cmd_name = cmd_part.split_whitespace().next().unwrap_or("");
            
            // Get parameter completions for this command
            if !cmd_name.is_empty() && !SHELL_COMMANDS.contains(&cmd_name) {
                let params = get_command_parameters(cmd_name);
                let matches: Vec<Pair> = params
                    .into_iter()
                    .filter(|param| param.starts_with(prefix))
                    .map(|param| Pair {
                        display: param.clone(),
                        replacement: param,
                    })
                    .collect();
                
                return Ok((start, matches));
            }
        }

        // Regular command completion
        let matches: Vec<Pair> = self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|cmd| Pair {
                display: cmd.clone(),
                replacement: cmd.clone(),
            })
            .collect();

        Ok((start, matches))
    }
}

// Helper function to get property names from a command's JSON output
fn get_command_properties(cmd: &str) -> Option<Vec<String>> {
    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());
    
    // Execute command with --obj flag
    let output = Command::new(&stargate_bin)
        .arg(cmd)
        .arg("--obj")
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let json_str = String::from_utf8_lossy(&output.stdout);
    let json_value: serde_json::Value = serde_json::from_str(&json_str).ok()?;
    
    // Extract top-level keys from JSON object
    if let serde_json::Value::Object(map) = &json_value {
        let mut properties: Vec<String> = map.keys().cloned().collect();
        properties.sort();
        Some(properties)
    } else {
        None
    }
}

// Helper function to evaluate an expression and get properties from the result
fn get_expression_properties(expr: &str) -> Option<Vec<String>> {
    use super::interpreter::Interpreter;
    use super::scripting::{Parser, Value, Statement};
    
    // Wrap expression in a script that prints it as JSON
    let script_code = format!("print {};", expr);
    
    // Parse and execute the script
    let mut parser = Parser::new(&script_code);
    let statements = parser.parse().ok()?;
    let mut interpreter = Interpreter::new();
    
    // Execute statements and capture print output
    for stmt in statements {
        if let Statement::Print(expr) = stmt {
            if let Ok(value) = interpreter.eval_expression(expr) {
                // Convert value to JSON and extract properties
                let json_value = match value {
                    Value::Object(obj) => obj,
                    Value::String(s) => {
                        // Try to parse as JSON
                        serde_json::from_str(&s).ok()?
                    }
                    _ => return None,
                };
                
                // Extract keys from JSON object
                if let serde_json::Value::Object(map) = &json_value {
                    let mut properties: Vec<String> = map.keys().cloned().collect();
                    properties.sort();
                    return Some(properties);
                } else {
                    return None;
                }
            }
        }
    }
    
    None
}

impl Hinter for StargateCompletion {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        if pos < line.len() {
            return None;
        }
        
        // Find the start of the current word
        let start = line.rfind(|c: char| c.is_whitespace() || c == '|')
            .map(|i| i + 1)
            .unwrap_or(0);
        
        let prefix = &line[start..];
        
        if prefix.len() < 2 {
            return None;
        }
        
        // Find the first matching command
        self.commands
            .iter()
            .find(|cmd| cmd.starts_with(prefix) && cmd.len() > prefix.len())
            .map(|cmd| cmd[prefix.len()..].to_string())
    }
}

impl Highlighter for StargateCompletion {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Borrowed(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: bool) -> bool {
        false
    }
}

impl Validator for StargateCompletion {}
