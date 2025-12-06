// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::stargate_shell::{execute_pipeline, execute_stargate_script, Interpreter};
use super::command_type::CommandType;
use super::executor::{execute_command, execute_chained_commands};

/// Skip shebang line if present
pub fn skip_shebang(contents: &str) -> String {
    if contents.starts_with("#!") {
        contents.lines().skip(1).collect::<Vec<_>>().join("\n")
    } else {
        contents.to_string()
    }
}

/// Handle piped input (stdin)
pub fn handle_piped_input() {
    use std::io::Read;
    let mut script_code = String::new();
    if std::io::stdin().read_to_string(&mut script_code).is_err() {
        return;
    }

    let script_code = skip_shebang(&script_code);
    let trimmed = script_code.trim();
    
    // Single-line command without semicolons
    if !trimmed.contains('\n') && !trimmed.contains(';') {
        handle_single_line_piped(trimmed);
    } 
    // Multi-line input without semicolons - execute line by line
    else if !trimmed.contains(';') && trimmed.contains('\n') {
        handle_multiline_piped(trimmed);
    } 
    // Script with semicolons
    else {
        let mut interp = Interpreter::new();
        match execute_stargate_script(&script_code, &mut interp, false) {
            Ok(exit_code) => std::process::exit(exit_code),
            Err(e) => {
                eprintln!("Script error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Handle single-line piped input
fn handle_single_line_piped(input: &str) {
    let cmd_type = CommandType::detect(input);
    
    let exit_code = match cmd_type {
        CommandType::PropertyAccess => {
            let mut interp = Interpreter::new();
            execute_stargate_script(&format!("print {};", input), &mut interp, false)
                .unwrap_or(1)
        }
        CommandType::ChainedCommands => {
            if execute_chained_commands(input, None, false) { 0 } else { 1 }
        }
        _ => {
            execute_pipeline(input)
                .map(|_| 0)
                .unwrap_or_else(|e| {
                    eprintln!("Error: {}", e);
                    1
                })
        }
    };
    
    std::process::exit(exit_code);
}

/// Handle multi-line piped input
fn handle_multiline_piped(input: &str) {
    let mut exit_code = 0;
    
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "exit" || line == "quit" {
            break;
        }
        
        let cmd_type = CommandType::detect(line);
        let success = match cmd_type {
            CommandType::ChainedCommands => execute_chained_commands(line, None, false),
            _ => execute_command(line, None, false),
        };
        
        if !success {
            exit_code = 1;
        }
    }
    
    std::process::exit(exit_code);
}
