// Command discovery and parameter extraction
use std::process::Command;

// List of built-in shell commands
pub const SHELL_COMMANDS: &[&str] = &["help", "exit", "quit", "describe-command"];

// List of stargate commands (extracted from the binary)
pub fn get_stargate_commands() -> Vec<String> {
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
pub fn get_command_parameters(cmd_name: &str) -> Vec<String> {
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
