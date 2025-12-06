// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use rustyline::Editor;
use std::sync::{Arc, Mutex};
use crate::stargate_shell::{describe_command, execute_pipeline, execute_stargate_script, Interpreter, builtin_commands};
use super::command_type::CommandType;

pub const DESCRIBE_COMMAND_PREFIX: &str = "describe-command ";
pub const SCRIPT_PREFIX: &str = "script ";
pub const SCRIPT_BLOCK_START: &str = "script{";
pub const SCRIPT_BLOCK_END: &str = "}";

/// Handle REPL command - returns false if should exit
pub fn handle_repl_command<H>(
    input: &str, 
    rl: &mut Editor<H, rustyline::history::DefaultHistory>, 
    interpreter: &Arc<Mutex<Interpreter>>,
    history_file: &str
) -> bool 
where
    H: rustyline::Helper,
{
    match input {
        "exit" | "quit" => return false,
        "help" => {
            crate::stargate_shell::print_help();
            return true;
        }
        _ if input == "list-history" || input.starts_with("list-history ") => {
            let args = if input == "list-history" { "" } else { &input[13..] };
            if let Err(e) = builtin_commands::execute_list_history(args, history_file) {
                eprintln!("Error: {}", e);
            }
            return true;
        }
        _ if input.starts_with(DESCRIBE_COMMAND_PREFIX) => {
            handle_describe_command(input);
            return true;
        }
        _ if input.starts_with(SCRIPT_PREFIX) => {
            handle_script_command(input, interpreter);
            return true;
        }
        _ if input.starts_with(SCRIPT_BLOCK_START) => {
            handle_script_block(rl, input, interpreter);
            return true;
        }
        _ if input.starts_with("class ") && !input.contains('}') => {
            handle_multiline_class(rl, input, interpreter);
            return true;
        }
        _ => {
            handle_general_input(input, interpreter);
            return true;
        }
    }
}

/// Handle describe-command
fn handle_describe_command(input: &str) {
    let cmd_name = input[DESCRIBE_COMMAND_PREFIX.len()..].trim();
    if cmd_name.is_empty() {
        eprintln!("Error: describe-command requires a command name");
        eprintln!("Usage: describe-command <command>");
    } else if let Err(e) = describe_command(cmd_name) {
        eprintln!("Error: {}", e);
    }
}

/// Handle script command
fn handle_script_command(input: &str, interpreter: &Arc<Mutex<Interpreter>>) {
    let script_code = input[SCRIPT_PREFIX.len()..].trim();
    execute_with_interpreter(script_code, interpreter);
}

/// Handle multi-line script block
fn handle_script_block<H>(rl: &mut Editor<H, rustyline::history::DefaultHistory>, input: &str, interpreter: &Arc<Mutex<Interpreter>>) 
where
    H: rustyline::Helper,
{
    let script = collect_multiline_input(rl, input[7..].to_string(), |line| line.trim() == SCRIPT_BLOCK_END);
    execute_with_interpreter(&script, interpreter);
}

/// Handle multi-line class definition
fn handle_multiline_class<H>(rl: &mut Editor<H, rustyline::history::DefaultHistory>, input: &str, interpreter: &Arc<Mutex<Interpreter>>) 
where
    H: rustyline::Helper,
{
    let class_def = collect_multiline_input(rl, input.to_string(), |line| {
        line.trim() == "}" || line.trim().ends_with('}')
    });
    execute_with_interpreter(&class_def, interpreter);
}

/// Collect multiline input until end condition is met
fn collect_multiline_input<H, F>(rl: &mut Editor<H, rustyline::history::DefaultHistory>, first_line: String, is_end: F) -> String
where
    H: rustyline::Helper,
    F: Fn(&str) -> bool,
{
    let mut lines = vec![first_line];
    
    loop {
        match rl.readline("... ") {
            Ok(line) => {
                if is_end(&line) {
                    break;
                }
                lines.push(line);
            }
            Err(_) => {
                eprintln!("Input interrupted");
                break;
            }
        }
    }
    
    lines.join("\n")
}

/// Execute script with locked interpreter
pub fn execute_with_interpreter(script: &str, interpreter: &Arc<Mutex<Interpreter>>) {
    if let Ok(mut interp) = interpreter.lock() {
        if let Err(e) = execute_stargate_script(script, &mut interp, true) {
            eprintln!("Script error: {}", e);
        }
    }
}

/// Handle general input (statements, expressions, pipelines)
fn handle_general_input(input: &str, interpreter: &Arc<Mutex<Interpreter>>) {
    let is_builtin_command = input.starts_with("cd ") || input.starts_with("change-directory ");
    let is_statement = CommandType::is_script_statement(input) || input.ends_with(';') || is_builtin_command;
    
    // Check for property access (excluding file paths)
    let is_path_like = input.starts_with("./") || input.starts_with("../") || input.starts_with('/');
    let has_prop_access = !is_path_like && CommandType::has_property_access(input);
    
    if is_statement || has_prop_access {
        let script_code = if is_statement && !input.ends_with(';') {
            format!("{};", input)
        } else if is_statement {
            input.to_string()
        } else {
            format!("print {};", input)
        };
        
        execute_with_interpreter(&script_code, interpreter);
    } else if let Err(e) = execute_pipeline(input) {
        eprintln!("Error: {}", e);
    }
}
