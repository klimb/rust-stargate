// UI functions for the shell
use crate::VERSION;

pub fn print_banner() {
    println!("Stargate Shell {VERSION}");
    println!("A Unix-like shell for chaining stargate commands with JSON pipes");
    println!("Type 'help' for usage, 'exit' to quit");
    println!("Use Tab for command completion\n");
}

pub fn print_help() {
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

pub fn describe_command(cmd_name: &str) -> Result<(), String> {
    use std::process::Command;
    
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
