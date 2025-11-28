// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

mod stargate_shell;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType};

use stargate_shell::{StargateCompletion, execute_pipeline, execute_script, describe_command, print_banner, print_help};

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
                
                if let Err(e) = execute_script(&script_code) {
                    eprintln!("Script error: {}", e);
                    std::process::exit(1);
                }
                return;
            }
            Err(e) => {
                eprintln!("Error reading script file '{}': {}", script_file, e);
                std::process::exit(1);
            }
        }
    }
    
    // Interactive REPL mode
    print_banner();

    let helper = StargateCompletion::new();
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
                        if let Err(e) = execute_script(script_code) {
                            eprintln!("Script error: {}", e);
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
                        if let Err(e) = execute_script(&script) {
                            eprintln!("Script error: {}", e);
                        }
                    }
                    _ => {
                        // Check if this looks like a script expression (has property access or indexing)
                        if input.contains('.') || (input.contains('[') && input.contains(']')) {
                            // Treat as script expression - wrap in print statement
                            let script_code = format!("print {};", input);
                            if let Err(e) = execute_script(&script_code) {
                                eprintln!("Script error: {}", e);
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
