// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::io::{Write, BufRead, BufReader};
use std::process::{Command, Stdio};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_banner() {
    println!("Stargate Shell {VERSION}");
    println!("A Unix-like shell for chaining stargate commands with JSON pipes");
    println!("Type 'help' for usage, 'exit' to quit\n");
}

fn print_help() {
    println!("Stargate Shell Commands:");
    println!("  help                 - Show this help message");
    println!("  exit, quit           - Exit the shell");
    println!("  <cmd> [args...]      - Execute a stargate command");
    println!("  <cmd> | <cmd> | ...  - Chain commands with JSON pipes");
    println!();
    println!("Examples:");
    println!("  list-directory | collect-count");
    println!();
    println!("When using pipes (|), commands automatically use -o for JSON output");
    println!("and feed the JSON to the next command via stdin. ");
    println!("(Unless it's the last command)");
}

fn execute_single_command(cmd_parts: &[String]) -> Result<String, String> {
    if cmd_parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());

    let mut child = Command::new(&stargate_bin)
        .args(cmd_parts)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let mut output = String::new();
    let mut error_output = String::new();

    BufReader::new(stdout)
        .lines()
        .for_each(|line| {
            if let Ok(line) = line {
                output.push_str(&line);
                output.push('\n');
            }
        });

    BufReader::new(stderr)
        .lines()
        .for_each(|line| {
            if let Ok(line) = line {
                error_output.push_str(&line);
                error_output.push('\n');
            }
        });

    let status = child.wait().map_err(|e| format!("Failed to wait for command: {}", e))?;

    if !error_output.is_empty() {
        eprint!("{}", error_output);
    }

    if status.success() {
        Ok(output)
    } else {
        Err(format!("Command failed with exit code: {}", status.code().unwrap_or(-1)))
    }
}

fn execute_with_json_pipe(cmd_parts: &[String], json_input: Option<&str>, should_output_json: bool) -> Result<String, String> {
    if cmd_parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());

    // Check if -o or --obj flag is already present
    let has_obj_flag = cmd_parts.iter().any(|s| s == "-o" || s == "--obj");
    
    let mut args = cmd_parts.to_vec();
    if should_output_json && !has_obj_flag {
        // Insert -o after the command name (first arg)
        if args.len() > 0 {
            args.insert(1, "-o".to_string());
        }
    }

    let mut child = Command::new(&stargate_bin)
        .args(&args)
        .stdin(if json_input.is_some() { Stdio::piped() } else { Stdio::inherit() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    // If we have JSON input, write it to stdin
    if let Some(input) = json_input {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())
                .map_err(|e| format!("Failed to write to stdin: {}", e))?;
        }
    }

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let mut output = String::new();
    let mut error_output = String::new();

    BufReader::new(stdout)
        .lines()
        .for_each(|line| {
            if let Ok(line) = line {
                output.push_str(&line);
                output.push('\n');
            }
        });

    BufReader::new(stderr)
        .lines()
        .for_each(|line| {
            if let Ok(line) = line {
                error_output.push_str(&line);
                error_output.push('\n');
            }
        });

    let status = child.wait().map_err(|e| format!("Failed to wait for command: {}", e))?;

    if !error_output.is_empty() {
        eprint!("{}", error_output);
    }

    if status.success() {
        Ok(output)
    } else {
        Err(format!("Command failed with exit code: {}", status.code().unwrap_or(-1)))
    }
}

fn parse_pipeline(input: &str) -> Vec<Vec<String>> {
    let mut pipelines = Vec::new();
    let mut current_cmd = Vec::new();
    let mut current_arg = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for ch in input.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            '"' | '\'' if in_quotes && ch == quote_char => {
                in_quotes = false;
            }
            '|' if !in_quotes => {
                if !current_arg.is_empty() {
                    current_cmd.push(current_arg.clone());
                    current_arg.clear();
                }
                if !current_cmd.is_empty() {
                    pipelines.push(current_cmd.clone());
                    current_cmd.clear();
                }
            }
            ' ' | '\t' if !in_quotes => {
                if !current_arg.is_empty() {
                    current_cmd.push(current_arg.clone());
                    current_arg.clear();
                }
            }
            _ => {
                current_arg.push(ch);
            }
        }
    }

    if !current_arg.is_empty() {
        current_cmd.push(current_arg);
    }
    if !current_cmd.is_empty() {
        pipelines.push(current_cmd);
    }

    pipelines
}

fn execute_pipeline(input: &str) -> Result<(), String> {
    let commands = parse_pipeline(input);
    
    if commands.is_empty() {
        return Ok(());
    }

    if commands.len() == 1 {
        // Single command, no pipe
        match execute_single_command(&commands[0]) {
            Ok(output) => {
                print!("{}", output);
                Ok(())
            }
            Err(e) => Err(e)
        }
    } else {
        // Pipeline
        let mut json_data: Option<String> = None;

        for (idx, cmd) in commands.iter().enumerate() {
            let is_last = idx == commands.len() - 1;
            let should_output_json = !is_last; // Only output JSON if not the last command
            
            match execute_with_json_pipe(cmd, json_data.as_deref(), should_output_json) {
                Ok(output) => {
                    if is_last {
                        // Last command, print output
                        print!("{}", output);
                    } else {
                        // Intermediate command, store JSON for next
                        json_data = Some(output);
                    }
                }
                Err(e) => return Err(e)
            }
        }

        Ok(())
    }
}

fn main() {
    print_banner();

    let mut rl = DefaultEditor::new().expect("Failed to create readline editor");

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
