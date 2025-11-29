// Command execution
use std::io::{Write, BufRead, BufReader};
use std::process::{Command, Stdio};

use super::parsing::parse_pipeline;
use super::commands::get_command_aliases;

// Commands that already consume/produce JSON and shouldn't get -o flag
const OBJECT_NATIVE_COMMANDS: &[&str] = &[
    "slice-object",
    "dice-object",
];

fn is_object_native_command(cmd: &str) -> bool {
    OBJECT_NATIVE_COMMANDS.contains(&cmd)
}

// Resolve alias to actual command name
fn resolve_alias(cmd: &str) -> String {
    // This will be resolved via the shared alias system in commands module
    // For now, just handle the most common ones inline
    match cmd {
        "ls" => "list-directory".to_string(),
        _ => {
            // Check auto-generated aliases by trying to find matching command
            let commands = get_all_commands();
            for full_cmd in commands {
                if let Some(alias) = generate_alias(&full_cmd) {
                    if alias == cmd {
                        return full_cmd;
                    }
                }
            }
            cmd.to_string()
        }
    }
}

// Generate an alias from a command name using first letters of each word
fn generate_alias(command: &str) -> Option<String> {
    let parts: Vec<&str> = command.split('-').collect();
    if parts.len() < 2 {
        return None;
    }
    let alias: String = parts.iter().filter_map(|part| part.chars().next()).collect();
    if alias.len() >= 2 {
        Some(alias)
    } else {
        None
    }
}

// Get all available stargate commands
fn get_all_commands() -> Vec<String> {
    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());

    if let Ok(output) = Command::new(&stargate_bin).arg("--list").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            return text
                .lines()
                .map(|line| line.trim())
                .filter(|line| !line.is_empty() && *line != "[" && *line != "]")
                .map(|s| s.to_string())
                .collect();
        }
    }
    Vec::new()
}

fn handle_cd(args: &[String]) -> Result<String, String> {
    let path = if args.is_empty() {
        // No argument - go to home directory
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    } else if args[0] == "-" {
        // cd - goes to previous directory
        std::env::var("OLDPWD").unwrap_or_else(|_| ".".to_string())
    } else {
        args[0].clone()
    };

    // Save current directory as OLDPWD before changing
    if let Ok(current) = std::env::current_dir() {
        unsafe { std::env::set_var("OLDPWD", current); }
    }

    // Change directory
    std::env::set_current_dir(&path)
        .map_err(|e| format!("cd: {}: {}", path, e))?;

    // Update PWD to reflect the new current directory
    if let Ok(new_dir) = std::env::current_dir() {
        unsafe { std::env::set_var("PWD", new_dir); }
    }

    Ok(String::new())
}

pub fn execute_single_command(cmd_parts: &[String]) -> Result<String, String> {
    execute_single_command_impl(cmd_parts, false)
}

pub fn execute_single_command_with_obj(cmd_parts: &[String]) -> Result<String, String> {
    execute_single_command_impl(cmd_parts, true)
}

fn execute_single_command_impl(cmd_parts: &[String], add_obj: bool) -> Result<String, String> {
    if cmd_parts.is_empty() {
        return Err("Empty command".to_string());
    }

    // Resolve alias to actual command name
    let cmd_name = resolve_alias(&cmd_parts[0]);
    
    // Handle built-in commands
    if cmd_name == "cd" || cmd_name == "change-directory" {
        return handle_cd(&cmd_parts[1..]);
    }
    
    // Handle pwd as built-in to reflect actual shell's current directory
    if cmd_name == "pwd" {
        if let Ok(current) = std::env::current_dir() {
            return Ok(format!("{}\n", current.to_string_lossy()));
        } else {
            return Err("Could not determine current directory".to_string());
        }
    }

    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());

    // Build command with resolved alias
    let mut args = vec![cmd_name.clone()];
    args.extend_from_slice(&cmd_parts[1..]);
    
    // Optionally add --obj flag for script mode
    if add_obj {
        let has_obj_flag = cmd_parts.iter().any(|s| s == "-o" || s == "--obj");
        let is_object_native = is_object_native_command(&cmd_name);
        
        if !has_obj_flag && !is_object_native {
            args.insert(1, "--obj".to_string());
        }
    }

    let mut child = Command::new(&stargate_bin)
        .args(&args)
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

pub fn execute_with_object_pipe(cmd_parts: &[String], json_input: Option<&str>, should_output_json: bool) -> Result<String, String> {
    if cmd_parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let stargate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("stargate")))
        .unwrap_or_else(|| "stargate".into());

    // Check if -o or --obj flag is already present
    let has_obj_flag = cmd_parts.iter().any(|s| s == "-o" || s == "--obj");
    
    // Check if this is a JSON-native command that doesn't need -o
    let cmd_name = cmd_parts.first().map(|s| s.as_str()).unwrap_or("");
    let is_object_native = is_object_native_command(cmd_name);
    
    // Automatically add --obj for JSON output in pipelines
    let mut args = cmd_parts.to_vec();
    if should_output_json && !has_obj_flag && !is_object_native {
        // Insert --obj after the command name (first arg)
        if args.len() > 0 {
            args.insert(1, "--obj".to_string());
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

pub fn execute_pipeline(input: &str) -> Result<(), String> {
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
            
            match execute_with_object_pipe(cmd, json_data.as_deref(), should_output_json) {
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

pub fn execute_pipeline_capture(input: &str) -> Result<String, String> {
    let commands = parse_pipeline(input);
    
    if commands.is_empty() {
        return Ok(String::new());
    }

    if commands.len() == 1 {
        // Single command, no pipe - add --obj for script mode
        execute_single_command_with_obj(&commands[0])
    } else {
        // Pipeline
        let mut json_data: Option<String> = None;

        for (idx, cmd) in commands.iter().enumerate() {
            let is_last = idx == commands.len() - 1;
            let should_output_json = !is_last;
            
            match execute_with_object_pipe(cmd, json_data.as_deref(), should_output_json) {
                Ok(output) => {
                    if is_last {
                        // Last command, return output
                        return Ok(output);
                    } else {
                        // Intermediate command, store JSON for next
                        json_data = Some(output);
                    }
                }
                Err(e) => return Err(e)
            }
        }

        Ok(String::new())
    }
}
