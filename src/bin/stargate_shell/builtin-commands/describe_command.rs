use std::process::Command;

pub fn execute(cmd_name: &str) -> Result<(), String> {
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
