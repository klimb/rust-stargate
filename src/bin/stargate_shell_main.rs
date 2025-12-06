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
        handle_script_file(&args[1]);
    }
    
    // Check if stdin is being piped (not a TTY)
    if !std::io::stdin().is_terminal() {
        handle_piped_input();
    }
    
    // Interactive REPL mode
    run_interactive_repl();
}

/// Execute a script file and exit
fn handle_script_file(script_file: &str) {
    match std::fs::read_to_string(script_file) {
        Ok(contents) => {
            let script_code = skip_shebang(&contents);
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

/// Skip shebang line if present
fn skip_shebang(contents: &str) -> String {
    if contents.starts_with("#!") {
        contents.lines().skip(1).collect::<Vec<_>>().join("\n")
    } else {
        contents.to_string()
    }
}

/// Check if input is a script statement
fn is_script_statement(input: &str) -> bool {
    input.starts_with("let ") 
        || input.starts_with("class ")
        || input.starts_with("print ")
        || input.starts_with("if ")
        || input.starts_with("while ")
        || input.starts_with("for ")
        || input.starts_with("fn ")
        || input.starts_with("return ")
        || input.contains(" = ")
}

/// Check for property access in input
fn has_property_access(input: &str) -> bool {
    let mut in_quotes = false;
    let mut has_dot_access = false;
    let chars: Vec<char> = input.chars().collect();
    
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
    has_dot_access || (!in_quotes && input.contains('[') && input.contains(']'))
}

/// Execute command, determining if it's a statement or pipeline
fn execute_command(cmd: &str, interp: Option<&mut Interpreter>, is_interactive: bool) -> bool {
    if is_script_statement(cmd) {
        let script = if cmd.ends_with(';') { cmd.to_string() } else { format!("{};", cmd) };
        match interp {
            Some(interp) => {
                match execute_stargate_script(&script, interp, is_interactive) {
                    Ok(_) => true,
                    Err(e) => {
                        eprintln!("Script error: {}", e);
                        false
                    }
                }
            }
            None => {
                let mut new_interp = Interpreter::new();
                match execute_stargate_script(&script, &mut new_interp, is_interactive) {
                    Ok(_) => true,
                    Err(e) => {
                        eprintln!("Script error: {}", e);
                        false
                    }
                }
            }
        }
    } else {
        match execute_pipeline(cmd) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Error: {}", e);
                false
            }
        }
    }
}

/// Handle piped input (stdin)
fn handle_piped_input() {
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
fn handle_single_line_piped(trimmed: &str) {
    // Property access expression
    if has_property_access(trimmed) {
        let mut interp = Interpreter::new();
        match execute_stargate_script(&format!("print {};", trimmed), &mut interp, false) {
            Ok(code) => std::process::exit(code),
            Err(e) => {
                eprintln!("Script error: {}", e);
                std::process::exit(1);
            }
        }
    }
    
    // && operator
    if trimmed.contains("&&") {
        let commands: Vec<&str> = trimmed.split("&&").map(|s| s.trim()).collect();
        for cmd in commands {
            if cmd.is_empty() {
                continue;
            }
            if !execute_command(cmd, None, false) {
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }
    
    // Single command
    match execute_pipeline(trimmed) {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Handle multi-line piped input
fn handle_multiline_piped(trimmed: &str) {
    let mut exit_code = 0;
    
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "exit" || line == "quit" {
            break;
        }
        
        // Handle && operator
        if line.contains("&&") {
            let commands: Vec<&str> = line.split("&&").map(|s| s.trim()).collect();
            let mut should_continue = true;
            
            for cmd in commands {
                if !should_continue || cmd.is_empty() {
                    break;
                }
                if !execute_command(cmd, None, false) {
                    exit_code = 1;
                    should_continue = false;
                }
            }
            continue;
        }
        
        // Execute single command
        if !execute_command(line, None, false) {
            exit_code = 1;
        }
    }
    
    std::process::exit(exit_code);
}

/// Run the interactive REPL
fn run_interactive_repl() {
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
                save_to_history(&history_file, input);

                // Handle && operator (conditional execution)
                if should_handle_and_operator(input) {
                    handle_and_operator_interactive(input, &interpreter);
                    continue;
                }

                // Handle special commands and regular input
                if !handle_repl_command(input, &mut rl, &interpreter, &history_file) {
                    break; // exit/quit was called
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }

    println!("\nGoodbye!");
}

/// Check if input should be handled as && operator
fn should_handle_and_operator(input: &str) -> bool {
    input.contains("&&") 
        && !input.starts_with("let ") 
        && !input.starts_with("class ") 
        && !is_control_flow_statement(input)
}

/// Check if input starts with a control flow keyword
fn is_control_flow_statement(input: &str) -> bool {
    input.starts_with("if ") 
        || input.starts_with("while ") 
        || input.starts_with("for ") 
        || input.starts_with("fn ")
}

/// Handle && operator in interactive mode
fn handle_and_operator_interactive(input: &str, interpreter: &Arc<Mutex<Interpreter>>) {
    let commands: Vec<&str> = input.split("&&").map(|s| s.trim()).collect();
    
    for cmd in commands {
        if cmd.is_empty() {
            continue;
        }
        
        let success = if is_script_statement(cmd) {
            let script_code = if cmd.ends_with(';') { cmd.to_string() } else { format!("{};", cmd) };
            if let Ok(mut interp) = interpreter.lock() {
                execute_command(&script_code, Some(&mut interp), true)
            } else {
                false
            }
        } else {
            execute_command(cmd, None, true)
        };
        
        if !success {
            break;
        }
    }
}

/// Handle REPL command - returns false if should exit
fn handle_repl_command<H>(
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
            print_help();
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
    if let Ok(mut interp) = interpreter.lock() {
        if let Err(e) = execute_stargate_script(script_code, &mut interp, true) {
            eprintln!("Script error: {}", e);
        }
    }
}

/// Handle multi-line script block
fn handle_script_block<H>(rl: &mut Editor<H, rustyline::history::DefaultHistory>, input: &str, interpreter: &Arc<Mutex<Interpreter>>) 
where
    H: rustyline::Helper,
{
    let mut script_lines = vec![input[7..].to_string()]; // Remove "script{"
    
    loop {
        match rl.readline("... ") {
            Ok(line) => {
                if line.trim() == SCRIPT_BLOCK_END {
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
        if let Err(e) = execute_stargate_script(&script, &mut interp, true) {
            eprintln!("Script error: {}", e);
        }
    }
}

/// Handle multi-line class definition
fn handle_multiline_class<H>(rl: &mut Editor<H, rustyline::history::DefaultHistory>, input: &str, interpreter: &Arc<Mutex<Interpreter>>) 
where
    H: rustyline::Helper,
{
    let mut class_lines = vec![input.to_string()];
    
    loop {
        match rl.readline("... ") {
            Ok(line) => {
                class_lines.push(line.clone());
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
        if let Err(e) = execute_stargate_script(&class_def, &mut interp, true) {
            eprintln!("Script error: {}", e);
        }
    }
}

/// Handle general input (statements, expressions, pipelines)
fn handle_general_input(input: &str, interpreter: &Arc<Mutex<Interpreter>>) {
    let is_builtin_command = input.starts_with("cd ") || input.starts_with("change-directory ");
    let is_statement = is_script_statement(input) || input.ends_with(';') || is_builtin_command;
    
    // Check for property access (excluding file paths)
    let is_path_like = input.starts_with("./") || input.starts_with("../") || input.starts_with('/');
    let has_prop_access = !is_path_like && has_property_access(input);
    
    if is_statement || has_prop_access {
        let script_code = if is_statement && !input.ends_with(';') {
            format!("{};", input)
        } else if is_statement {
            input.to_string()
        } else {
            format!("print {};", input)
        };
        
        if let Ok(mut interp) = interpreter.lock() {
            if let Err(e) = execute_stargate_script(&script_code, &mut interp, true) {
                eprintln!("Script error: {}", e);
            }
        }
    } else if let Err(e) = execute_pipeline(input) {
        eprintln!("Error: {}", e);
    }
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
