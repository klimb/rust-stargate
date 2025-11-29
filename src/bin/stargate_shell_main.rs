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
        // Reading from pipe/file - execute entire input as script
        use std::io::Read;
        let mut script_code = String::new();
        if let Ok(_) = std::io::stdin().read_to_string(&mut script_code) {
            // Skip shebang line if present
            let script_code = if script_code.starts_with("#!") {
                script_code.lines().skip(1).collect::<Vec<_>>().join("\n")
            } else {
                script_code
            };
            
            match execute_script(&script_code) {
                Ok(exit_code) => std::process::exit(exit_code),
                Err(e) => {
                    eprintln!("Script error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
    
    // Interactive REPL mode
    print_banner();

    // Shared variable names for completion
    let variable_names = Arc::new(Mutex::new(HashSet::new()));
    
    let helper = StargateCompletion::new(variable_names.clone());
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .auto_add_history(true)
        .build();
    let mut rl = Editor::with_config(config).expect("Failed to create readline editor");
    rl.set_helper(Some(helper));
    
    // Create persistent interpreter for REPL session with completion support
    let mut interpreter = Interpreter::new_with_completion(variable_names);

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
                        match execute_script_with_interpreter(script_code, &mut interpreter) {
                            Ok(_exit_code) => {}, // In REPL mode, don't exit the process
                            Err(e) => eprintln!("Script error: {}", e),
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
                        match execute_script_with_interpreter(&script, &mut interpreter) {
                            Ok(_exit_code) => {}, // In REPL mode, don't exit the process
                            Err(e) => eprintln!("Script error: {}", e),
                        }
                    }
                    _ => {
                        // Check if this looks like a script statement or expression
                        let is_statement = input.starts_with("let ") 
                            || input.starts_with("print ")
                            || input.contains(" = ")
                            || input.ends_with(';');
                        
                        let has_property_access = input.contains('.') 
                            || (input.contains('[') && input.contains(']'));
                        
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
                            
                            match execute_script_with_interpreter(&script_code, &mut interpreter) {
                                Ok(_exit_code) => {}, // In REPL mode, don't exit the process
                                Err(e) => eprintln!("Script error: {}", e),
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
