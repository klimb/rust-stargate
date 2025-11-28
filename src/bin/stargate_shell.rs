// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::io::{Write, BufRead, BufReader};
use std::process::{Command, Stdio};
use rustyline::error::ReadlineError;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper, Config, CompletionType};
use std::borrow::Cow;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// List of built-in shell commands
const SHELL_COMMANDS: &[&str] = &["help", "exit", "quit", "describe-command"];

// List of stargate commands (extracted from the binary)
fn get_stargate_commands() -> Vec<String> {
    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());

    let output = Command::new(&stargate_bin)
        .arg("--list")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            // Parse line-by-line, skipping '[' and ']'
            return text
                .lines()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty() && *line != "[" && *line != "]")
                .map(|s| s.to_string())
                .collect();
        }
    }

    // Fallback: empty list - user can still type commands manually
    Vec::new()
}

// Get available parameters/flags for a command
fn get_command_parameters(cmd_name: &str) -> Vec<String> {
    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());

    let output = Command::new(&stargate_bin)
        .arg(cmd_name)
        .arg("--help")
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            return extract_flags_from_help(&text);
        }
    }

    Vec::new()
}

// Extract flag names from help text
fn extract_flags_from_help(help_text: &str) -> Vec<String> {
    let mut flags = Vec::new();
    
    for line in help_text.lines() {
        let trimmed = line.trim();
        
        // Look for lines that start with - or contain flags
        if trimmed.starts_with('-') {
            // Parse flags like "-n" or "--name" or "-n, --name"
            for word in trimmed.split_whitespace() {
                if word.starts_with("--") {
                    // Long flag: extract up to '=' or end
                    if let Some(flag) = word.split(&['=', ',', '[', '<'][..]).next() {
                        if flag.len() > 2 {
                            flags.push(flag.to_string());
                        }
                    }
                } else if word.starts_with('-') && word.len() > 1 {
                    // Short flag: extract just the flag part
                    let flag = word.trim_end_matches(',');
                    if flag.len() == 2 && flag.chars().nth(1).map(|c| c.is_alphanumeric()).unwrap_or(false) {
                        flags.push(flag.to_string());
                    }
                }
            }
        }
    }
    
    flags.sort();
    flags.dedup();
    flags
}

struct StargateCompletion {
    commands: Vec<String>,
}

impl StargateCompletion {
    fn new() -> Self {
        let mut commands = get_stargate_commands();
        commands.extend(SHELL_COMMANDS.iter().map(|s| s.to_string()));
        commands.sort();
        commands.dedup();
        Self { commands }
    }
}

impl Helper for StargateCompletion {}

impl Completer for StargateCompletion {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line = &line[..pos];
        
        // Special handling for "describe-command "
        if let Some(rest) = line.strip_prefix("describe-command ") {
            let matches: Vec<Pair> = self.commands
                .iter()
                .filter(|cmd| !SHELL_COMMANDS.contains(&cmd.as_str())) // Exclude shell builtins
                .filter(|cmd| cmd.starts_with(rest))
                .map(|cmd| Pair {
                    display: cmd.clone(),
                    replacement: cmd.clone(),
                })
                .collect();
            
            return Ok((17, matches)); // "describe-command ".len() = 17
        }
        
        // Find the start of the current word
        let start = line.rfind(|c: char| c.is_whitespace() || c == '|')
            .map(|i| i + 1)
            .unwrap_or(0);
        
        let prefix = &line[start..];
        
        if prefix.is_empty() {
            return Ok((start, vec![]));
        }

        // Check if we're completing a parameter (starts with -)
        if prefix.starts_with('-') {
            // Extract the command name (first word after | or at start)
            let cmd_start = line[..start].rfind('|')
                .map(|i| i + 1)
                .unwrap_or(0);
            
            let cmd_part = line[cmd_start..start].trim();
            let cmd_name = cmd_part.split_whitespace().next().unwrap_or("");
            
            // Get parameter completions for this command
            if !cmd_name.is_empty() && !SHELL_COMMANDS.contains(&cmd_name) {
                let params = get_command_parameters(cmd_name);
                let matches: Vec<Pair> = params
                    .into_iter()
                    .filter(|param| param.starts_with(prefix))
                    .map(|param| Pair {
                        display: param.clone(),
                        replacement: param,
                    })
                    .collect();
                
                return Ok((start, matches));
            }
        }

        // Regular command completion
        let matches: Vec<Pair> = self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|cmd| Pair {
                display: cmd.clone(),
                replacement: cmd.clone(),
            })
            .collect();

        Ok((start, matches))
    }
}

impl Hinter for StargateCompletion {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        if pos < line.len() {
            return None;
        }
        
        // Find the start of the current word
        let start = line.rfind(|c: char| c.is_whitespace() || c == '|')
            .map(|i| i + 1)
            .unwrap_or(0);
        
        let prefix = &line[start..];
        
        if prefix.len() < 2 {
            return None;
        }
        
        // Find the first matching command
        self.commands
            .iter()
            .find(|cmd| cmd.starts_with(prefix) && cmd.len() > prefix.len())
            .map(|cmd| cmd[prefix.len()..].to_string())
    }
}

impl Highlighter for StargateCompletion {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Borrowed(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: bool) -> bool {
        false
    }
}

impl Validator for StargateCompletion {}

fn print_banner() {
    println!("Stargate Shell {VERSION}");
    println!("A Unix-like shell for chaining stargate commands with JSON pipes");
    println!("Type 'help' for usage, 'exit' to quit");
    println!("Use Tab for command completion\n");
}

fn print_help() {
    println!("Stargate Shell Commands:");
    println!("  help                      - Show this help message");
    println!("  exit, quit                - Exit the shell");
    println!("  describe-command <cmd>    - Show help for a stargate command");
    println!("  <cmd> [args...]           - Execute a stargate command");
    println!("  <cmd> | <cmd> | ...       - Chain commands with JSON pipes");
    println!();
    println!("Features:");
    println!("  Tab completion            - Press Tab to see/cycle through completions");
    println!("                              Works for commands, parameters (--flags), and options");
    println!("  Command hints             - Grayed suggestions appear as you type");
    println!("  Command history           - Use Up/Down arrows or Ctrl-P/Ctrl-N");
    println!("  Line editing              - Emacs-style keybindings (Ctrl-A, Ctrl-E, etc.)");
    println!();
    println!("Examples:");
    println!("  describe-command list-directory");
    println!("  list-directory --long");
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

fn describe_command(cmd_name: &str) -> Result<(), String> {
    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());

    let output = Command::new(&stargate_bin)
        .arg(cmd_name)
        .arg("--help")
        .output()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    if output.status.success() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
        Ok(())
    } else {
        let error = String::from_utf8_lossy(&output.stderr);
        if !error.is_empty() {
            eprint!("{}", error);
        }
        Err(format!("Command '{}' not found or invalid", cmd_name))
    }
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
                    _ if input.starts_with("describe-command ") => {
                        let cmd_name = input[17..].trim();
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
