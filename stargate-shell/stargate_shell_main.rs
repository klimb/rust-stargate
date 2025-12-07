// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

mod commands;
mod completion;
mod execution;
mod parsing;
mod path;
mod path_completion;
mod scripting;
mod interpreter;
mod testing;
mod theme;
mod ui;
mod jobs;
mod bytecode;
mod command_type;
mod executor;
mod piped_input;
mod repl_handlers;

#[path = "builtin-commands/mod.rs"]
mod builtin_commands;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType, KeyEvent, ExternalPrinter};
use rustyline::config::EditMode;
use std::sync::{Arc, Mutex};
use std::collections::HashSet;
use std::io::{IsTerminal, Write};
use std::fs::OpenOptions;
use std::time::SystemTime;

use completion::StargateCompletion;
use interpreter::{execute_script_with_path, Interpreter};
use jobs::start_job_monitor;
use command_type::CommandType;
use executor::execute_chained_commands;
use piped_input::{handle_piped_input, skip_shebang};
use repl_handlers::handle_repl_command;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_version_with_copyright() {
    println!(
        "This is stargate-shell {}, built on Rust.\n\nCopyright (c) 2025 Dmitry Kalashnikov\n\nDual Licensed: Open-Source (non-commercial) / Commercial (proprietary use)\nCommercial use requires a Commercial License.\nSee LICENSE file or contact author for details.",
        VERSION
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    // Handle version flag
    if args.len() > 1 && (args[1] == "--version" || args[1] == "-V") {
        print_version_with_copyright();
        std::process::exit(0);
    }
    
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

/// Handle && operator in interactive mode
fn handle_and_operator_interactive(input: &str, interpreter: &Arc<Mutex<Interpreter>>) {
    if let Ok(mut interp) = interpreter.lock() {
        execute_chained_commands(input, Some(&mut interp), true);
    }
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
                if CommandType::detect(input) == CommandType::ChainedCommands {
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

/// Save a command with timestamp to history file
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
