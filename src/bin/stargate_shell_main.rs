// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

mod stargate_shell;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType};
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use std::io::IsTerminal;

use stargate_shell::{StargateCompletion, execute_pipeline, execute_script, execute_script_with_interpreter, describe_command, print_banner, print_help, Interpreter};

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
                
                match execute_script(&script_code) {
                    Ok(exit_code) => std::process::exit(exit_code),
                    Err(e) => {
                        eprintln!("Script error: {}", e);
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
                    // Try executing as script statement first (for let, class, etc.)
                    let is_statement = line.starts_with("let ") 
                        || line.starts_with("class ")
                        || line.starts_with("print ")
                        || line.contains(" = ");
                    
                    if is_statement {
                        let script = format!("{};", line);
                        match execute_script(&script) {
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
                match execute_script(&script_code) {
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
    print_banner();

    // Shared variable names for completion
    let variable_names = Arc::new(Mutex::new(HashSet::new()));
    
    // Create persistent interpreter for REPL session with completion support
    let interpreter = Arc::new(Mutex::new(Interpreter::new_with_completion(variable_names.clone())));
    
    let helper = StargateCompletion::new(variable_names.clone(), interpreter.clone());
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .auto_add_history(true)
        .build();
    let mut rl = Editor::with_config(config).expect("Failed to create readline editor");
    rl.set_helper(Some(helper));

    loop {
        match rl.readline("stargate> ") {
            Ok(input) => {
                let input = input.trim();
                
                if input.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(input);

                match input {
                    "exit" | "quit" => break,
                    "help" => print_help(),
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
                            match execute_script_with_interpreter(script_code, &mut interp) {
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
                            match execute_script_with_interpreter(&script, &mut interp) {
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
                            match execute_script_with_interpreter(&class_def, &mut interp) {
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
                        
                        // Check for property access, but exclude file paths (./foo, ../bar, /path)
                        let is_path_like = input.starts_with("./") 
                            || input.starts_with("../")
                            || input.starts_with('/');
                        
                        let has_property_access = !is_path_like && (
                            (input.contains('.') && input.chars().filter(|c| *c == '.').count() > 0)
                            || (input.contains('[') && input.contains(']'))
                        );
                        
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
                                match execute_script_with_interpreter(&script_code, &mut interp) {
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
