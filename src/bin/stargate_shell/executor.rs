// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::stargate_shell::{execute_pipeline, execute_stargate_script, Interpreter};
use super::command_type::CommandType;

/// Execute command, determining if it's a statement or pipeline
pub fn execute_command(cmd: &str, interp: Option<&mut Interpreter>, is_interactive: bool) -> bool {
    if CommandType::is_script_statement(cmd) {
        let script = if cmd.ends_with(';') { cmd.to_string() } else { format!("{};", cmd) };
        execute_script(&script, interp, is_interactive)
    } else {
        execute_pipeline(cmd)
            .map(|_| true)
            .unwrap_or_else(|e| {
                eprintln!("Error: {}", e);
                false
            })
    }
}

/// Execute script with or without interpreter
pub fn execute_script(script: &str, interp: Option<&mut Interpreter>, is_interactive: bool) -> bool {
    let result = match interp {
        Some(interp) => execute_stargate_script(script, interp, is_interactive),
        None => {
            let mut new_interp = Interpreter::new();
            execute_stargate_script(script, &mut new_interp, is_interactive)
        }
    };
    
    match result {
        Ok(_) => true,
        Err(e) => {
            eprintln!("Script error: {}", e);
            false
        }
    }
}

/// Execute chained commands separated by &&
pub fn execute_chained_commands(input: &str, interp: Option<&mut Interpreter>, is_interactive: bool) -> bool {
    let commands: Vec<&str> = input.split("&&").map(|s| s.trim()).collect();
    
    // Handle with interpreter
    if let Some(interp) = interp {
        for cmd in commands {
            if cmd.is_empty() {
                continue;
            }
            if !execute_command(cmd, Some(interp), is_interactive) {
                return false;
            }
        }
    } else {
        // Handle without interpreter (piped mode)
        for cmd in commands {
            if cmd.is_empty() {
                continue;
            }
            if !execute_command(cmd, None, is_interactive) {
                return false;
            }
        }
    }
    true
}
