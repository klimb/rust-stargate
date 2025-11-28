// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

mod stargate_shell;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType};

use stargate_shell::{StargateCompletion, execute_pipeline, describe_command, print_banner, print_help};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DESCRIBE_COMMAND_PREFIX: &str = "describe-command ";

fn main() {
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
                    _ => {
                        if let Err(e) = execute_pipeline(input) {
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
