// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

mod stargate_shell;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType, ExternalPrinter, KeyEvent};
use rustyline::config::EditMode;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use std::io::{IsTerminal, Write};
use std::fs::OpenOptions;
use std::time::SystemTime;

use stargate_shell::{StargateCompletion, execute_pipeline, execute_script_with_path, execute_stargate_script, describe_command, print_help, Interpreter, start_job_monitor, builtin_commands};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIBE_COMMAND_PREFIX: &str = "describe-command ";
const SCRIPT_PREFIX: &str = "script ";
const SCRIPT_BLOCK_START: &str = "script{";
const SCRIPT_BLOCK_END: &str = "}";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    // If a script file is provided, execute it and exit
    if args.len() > 1 {
        let script_file = &args[1];
        match std::fs::read_to_string(script_file) {
            Ok(contents) => {
                // Skip shebang line if present
                let script_code = if contents.starts_with("#!") {
                    contents.lines().skip(1).collect::<Vec<_>>().join("\n")
                } else {
                    contents
                };
                
                match execute_script_with_path(&script_code, Some(script_file)) {
                    Ok(exit_code) => std::process::exit(exit_code),
                    Err(e) => {
                        eprintln!("Script error in {}: {}", script_file, e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading script file '{}': {}", script_file, e);
                std::process::exit(1);
            }
        }
    }
    
    // Check if stdin is being piped (not a TTY)
    if !std::io::stdin().is_terminal() {
        // Reading from pipe/file
        use std::io::Read;
        let mut script_code = String::new();
        if let Ok(_) = std::io::stdin().read_to_string(&mut script_code) {
            // Skip shebang line if present
            let script_code = if script_code.starts_with("#!") {
                script_code.lines().skip(1).collect::<Vec<_>>().join("\n")
            } else {
                script_code.clone()
            };
            
            // Check if it's a single-line command or multi-line script
            // Single-line commands without semicolons are executed as pipelines (interactive style)
            // Multi-line input can either be scripts (with semicolons) or line-by-line commands
            let trimmed = script_code.trim();
            if !trimmed.contains('\n') && !trimmed.contains(';') {
                // Check if this looks like a property access expression
                let has_property_access = {
                    let mut in_quotes = false;
                    let mut has_dot_access = false;
                    let chars: Vec<char> = trimmed.chars().collect();
                    
                    for i in 0..chars.len() {
                        if chars[i] == '"' {
                            in_quotes = !in_quotes;
                        } else if !in_quotes && chars[i] == '.' && i > 0 && i < chars.len() - 1 {
                            let before = chars[i-1];
                            let after = chars[i+1];
                            let valid_before = before.is_alphanumeric() || before == '_' || before == ')' || before == ']';
                            let valid_after = after.is_alphanumeric() || after == '_';
                            if valid_before && valid_after {
                                has_dot_access = true;
                                break;
                            }
                        }
                    }
                    has_dot_access
                };
                
                // If it has property access, execute as script expression
                if has_property_access {
                    let mut interp = Interpreter::new();
                    match execute_stargate_script(&format!("print {};", trimmed), &mut interp, false) {
                        Ok(code) => std::process::exit(code),
                        Err(e) => {
                            eprintln!("Script error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                
                // Handle && operator in single-line mode
                if trimmed.contains("&&") {
                    let commands: Vec<&str> = trimmed.split("&&").map(|s| s.trim()).collect();
                    let mut exit_code = 0;
                    
                    for cmd in commands {
                        if cmd.is_empty() {
                            continue;
                        }
                        
                        match execute_pipeline(cmd) {
                            Ok(_) => {},
                            Err(e) => {
                                eprintln!("Error: {}", e);
                                exit_code = 1;
                                break; // Stop on first failure
                            }
                        }
                    }
                    std::process::exit(exit_code);
                }
                
                // Single-line command - execute as pipeline for human-readable output
                match execute_pipeline(trimmed) {
                    Ok(_) => std::process::exit(0),
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else if !trimmed.contains(';') && trimmed.contains('\n') {
                // Multi-line input without semicolons - treat each line as a separate command
                // This allows piping multiple commands like: echo -e "cmd1\ncmd2\nexit"
                let mut exit_code = 0;
                for line in trimmed.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue; // Skip empty lines and comments
                    }
                    if line == "exit" || line == "quit" {
                        break;
                    }
                    
                    // Handle && operator in line-by-line mode
                    if line.contains("&&") {
                        let commands: Vec<&str> = line.split("&&").map(|s| s.trim()).collect();
                        let mut should_continue = true;
                        
                        for cmd in commands {
                            if !should_continue {
                                break;
                            }
                            if cmd.is_empty() {
                                continue;
                            }
                            
                            let is_statement = cmd.starts_with("let ") 
                                || cmd.starts_with("class ")
                                || cmd.starts_with("print ")
                                || cmd.contains(" = ");
                            
                            let success = if is_statement {
                                let script = format!("{};", cmd);
                                let mut interp = Interpreter::new();
                                match execute_stargate_script(&script, &mut interp, false) {
                                    Ok(_) => true,
                                    Err(e) => {
                                        eprintln!("Script error: {}", e);
                                        exit_code = 1;
                                        false
                                    }
                                }
                            } else {
                                match execute_pipeline(cmd) {
                                    Ok(_) => true,
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                        exit_code = 1;
                                        false
                                    }
                                }
                            };
                            
                            should_continue = success;
                        }
                        continue;
                    }
                    
                    // Try executing as script statement first (for let, class, etc.)
                    let is_statement = line.starts_with("let ") 
                        || line.starts_with("class ")
                        || line.starts_with("print ")
                        || line.contains(" = ");
                    
                    if is_statement {
                        let script = format!("{};", line);
                        let mut interp = Interpreter::new();
                        match execute_stargate_script(&script, &mut interp, false) {
                            Ok(code) => exit_code = code,
                            Err(e) => {
                                eprintln!("Script error: {}", e);
                                exit_code = 1;
                            }
                        }
                    } else {
                        // Execute as pipeline
                        match execute_pipeline(line) {
                            Ok(_) => {},
                            Err(e) => {
                                eprintln!("Error: {}", e);
                                exit_code = 1;
                            }
                        }
                    }
                }
                std::process::exit(exit_code);
            } else {
                // Script with semicolons - execute as script
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
    }
    
    // Interactive REPL mode

    // Shared variable names for completion
    let variable_names = Arc::new(Mutex::new(HashSet::new()));
    
    // Create persistent interpreter for REPL session with completion support
    let interpreter = Arc::new(Mutex::new(Interpreter::new_with_completion(variable_names.clone())));
    
    let helper = StargateCompletion::new(variable_names.clone(), interpreter.clone());
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .auto_add_history(true)
        .edit_mode(EditMode::Emacs) // Enable Emacs key bindings for Ctrl+P/N
        .build();
    let mut rl = Editor::with_config(config).expect("Failed to create readline editor");
    rl.set_helper(Some(helper));
    
    // Configure history
    let history_file = std::env::var("HOME")
        .map(|home| format!("{}/.stargate_history", home))
        .unwrap_or_else(|_| ".stargate_history".to_string());
    
    // Load timestamped history
    let history_with_timestamps = builtin_commands::load_timestamped_history(&history_file);
    
    // Load commands into rustyline (without timestamps for navigation)
    for (_, cmd) in &history_with_timestamps {
        let _ = rl.add_history_entry(cmd.as_str());
    }
    
    // Bind Ctrl+S for forward search (Ctrl+R for reverse is already default)
    use rustyline::{Cmd, KeyCode, Modifiers};
    rl.bind_sequence(
        KeyEvent(KeyCode::Char('s'), Modifiers::CTRL),
        rustyline::EventHandler::Simple(Cmd::ForwardSearchHistory)
    );

    let mut printer = rl.create_external_printer().expect("Failed to create external printer");
    let job_monitor_rx = start_job_monitor();
    
    std::thread::spawn(move || {
        loop {
            if let Ok(msg) = job_monitor_rx.recv() {
                let _ = printer.print(msg);
            } else {
                break;
            }
        }
    });

    loop {
        match rl.readline("stargate> ") {
            Ok(input) => {
                let input = input.trim();
                
                if input.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(input);
                
                // Save to timestamped history file
                save_to_history(&history_file, input);

                // Handle && operator (conditional execution like bash/zsh)
                if input.contains("&&") && !input.starts_with("let ") && !input.starts_with("class ") {
                    let commands: Vec<&str> = input.split("&&").map(|s| s.trim()).collect();
                    let mut should_continue = true;
                    
                    for cmd in commands {
                        if !should_continue {
                            break;
                        }
                        
                        if cmd.is_empty() {
                            continue;
                        }
                        
                        // Execute each command and check success
                        let success = if cmd.starts_with("let ") || cmd.starts_with("print ") || cmd.contains(" = ") {
                            // Script statement
                            let script_code = if cmd.ends_with(';') { cmd.to_string() } else { format!("{};", cmd) };
                            if let Ok(mut interp) = interpreter.lock() {
                                match execute_stargate_script(&script_code, &mut interp, true) {
                                    Ok(_) => true,
                                    Err(e) => {
                                        eprintln!("Script error: {}", e);
                                        false
                                    }
                                }
                            } else {
                                false
                            }
                        } else {
                            // Pipeline command
                            match execute_pipeline(cmd) {
                                Ok(_) => true,
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                    false
                                }
                            }
                        };
                        
                        should_continue = success;
                    }
                    continue;
                }

                match input {
                    "exit" | "quit" => break,
                    "help" => print_help(),
                    _ if input == "list-history" || input.starts_with("list-history ") => {
                        // Extract arguments after "list-history"
                        let args = if input == "list-history" {
                            ""
                        } else {
                            &input[13..]
                        };
                        
                        if let Err(e) = builtin_commands::execute_list_history(args, &history_file) {
                            eprintln!("Error: {}", e);
                        }
                    }
                    _ if input.starts_with(DESCRIBE_COMMAND_PREFIX) => {
                        let cmd_name = input[DESCRIBE_COMMAND_PREFIX.len()..].trim();
                        if cmd_name.is_empty() {
                            eprintln!("Error: describe-command requires a command name");
                            eprintln!("Usage: describe-command <command>");
                        } else if let Err(e) = describe_command(cmd_name) {
                            eprintln!("Error: {}", e);
                        }
                    }
                    _ if input.starts_with(SCRIPT_PREFIX) => {
                        let script_code = input[SCRIPT_PREFIX.len()..].trim();
                        if let Ok(mut interp) = interpreter.lock() {
                            match execute_stargate_script(script_code, &mut interp, true) {
                                Ok(_exit_code) => {}, // In REPL mode, don't exit the process
                                Err(e) => eprintln!("Script error: {}", e),
                            }
                        }
                    }
                    _ if input.starts_with(SCRIPT_BLOCK_START) => {
                        // Multi-line script mode
                        let mut script_lines = vec![input[7..].to_string()]; // Remove "script{"
                        
                        loop {
                            match rl.readline("... ") {
                                Ok(line) => {
                                    let trimmed = line.trim();
                                    if trimmed == SCRIPT_BLOCK_END {
                                        break;
                                    }
                                    script_lines.push(line);
                                }
                                Err(_) => {
                                    eprintln!("Script input interrupted");
                                    break;
                                }
                            }
                        }
                        
                        let script = script_lines.join("\n");
                        if let Ok(mut interp) = interpreter.lock() {
                            match execute_stargate_script(&script, &mut interp, true) {
                                Ok(_exit_code) => {}, // In REPL mode, don't exit the process
                                Err(e) => eprintln!("Script error: {}", e),
                            }
                        }
                    }
                    _ if input.starts_with("class ") && !input.contains('}') => {
                        // Multi-line class definition mode
                        let mut class_lines = vec![input.to_string()];
                        
                        loop {
                            match rl.readline("... ") {
                                Ok(line) => {
                                    class_lines.push(line.clone());
                                    // Check if we've found the closing brace
                                    if line.trim() == "}" || line.trim().ends_with('}') {
                                        break;
                                    }
                                }
                                Err(_) => {
                                    eprintln!("Class definition interrupted");
                                    break;
                                }
                            }
                        }
                        
                        let class_def = class_lines.join("\n");
                        if let Ok(mut interp) = interpreter.lock() {
                            match execute_stargate_script(&class_def, &mut interp, true) {
                                Ok(_exit_code) => {}, // In REPL mode, don't exit the process
                                Err(e) => eprintln!("Script error: {}", e),
                            }
                        }
                    }
                    _ => {
                        // Check if this looks like a script statement or expression
                        let is_builtin_command = input.starts_with("cd ") 
                            || input.starts_with("change-directory ");
                        
                        let is_statement = input.starts_with("let ") 
                            || input.starts_with("class ")
                            || input.starts_with("print ")
                            || input.contains(" = ")
                            || input.ends_with(';')
                            || is_builtin_command;
                        
                        // Check for property access, but exclude file paths and command arguments
                        // Property access looks like: obj.property, variable.method(), (cmd).property, list[0]
                        // Not property access: get-contents "/tmp/file.txt", ./script.sh
                        let is_path_like = input.starts_with("./") 
                            || input.starts_with("../")
                            || input.starts_with('/');
                        
                        // Check for property access pattern: word.word or ).word (not ".word" inside quotes)
                        let has_property_access = !is_path_like && {
                            let mut in_quotes = false;
                            let mut has_dot_access = false;
                            let chars: Vec<char> = input.chars().collect();
                            
                            for i in 0..chars.len() {
                                if chars[i] == '"' {
                                    in_quotes = !in_quotes;
                                } else if !in_quotes && chars[i] == '.' && i > 0 && i < chars.len() - 1 {
                                    // Check if it's a property access
                                    let before = chars[i-1];
                                    let after = chars[i+1];
                                    // Valid before dot: alphanumeric, underscore, closing paren/bracket
                                    // Valid after dot: alphanumeric, underscore
                                    let valid_before = before.is_alphanumeric() || before == '_' || before == ')' || before == ']';
                                    let valid_after = after.is_alphanumeric() || after == '_';
                                    if valid_before && valid_after {
                                        has_dot_access = true;
                                        break;
                                    }
                                }
                            }
                            
                            has_dot_access || (!in_quotes && input.contains('[') && input.contains(']'))
                        };
                        
                        if is_statement || has_property_access {
                            // Execute as script
                            let script_code = if is_statement && !input.ends_with(';') {
                                // Statement missing semicolon - add it
                                format!("{};", input)
                            } else if is_statement {
                                // Already a complete statement
                                input.to_string()
                            } else {
                                // Expression - wrap in print
                                format!("print {};", input)
                            };
                            
                            if let Ok(mut interp) = interpreter.lock() {
                                match execute_stargate_script(&script_code, &mut interp, true) {
                                    Ok(_exit_code) => {}, // In REPL mode, don't exit the process
                                    Err(e) => eprintln!("Script error: {}", e),
                                }
                            }
                        } else if let Err(e) = execute_pipeline(input) {
                            eprintln!("Error: {}", e);
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D or EOF
                break;
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }

    println!("\nGoodbye!");
}

// Save a command with timestamp to history file
fn save_to_history(history_file: &str, command: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_file)
    {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        
        let _ = writeln!(file, "{}|{}", timestamp, command);
    }
}
