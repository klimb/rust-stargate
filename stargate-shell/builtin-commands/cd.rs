// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use std::path::PathBuf;

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME") {
            if path == "~" {
                return PathBuf::from(home);
            } else if path.starts_with("~/") {
                return PathBuf::from(home).join(&path[2..]);
            } else {
                return PathBuf::from(home).join(&path[1..]);
            }
        }
    }
    PathBuf::from(path)
}

pub fn execute(args: &[String]) -> Result<String, String> {
    let path = if args.is_empty() {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    } else if args[0] == "-" {
        std::env::var("OLDPWD").unwrap_or_else(|_| ".".to_string())
    } else {
        args[0].clone()
    };

    let expanded_path = expand_tilde(&path);

    if let Ok(current) = std::env::current_dir() {
        unsafe { std::env::set_var("OLDPWD", current); }
    }

    std::env::set_current_dir(&expanded_path)
        .map_err(|e| format!("cd: {}: {}", path, e))?;

    if let Ok(new_dir) = std::env::current_dir() {
        unsafe { std::env::set_var("PWD", new_dir); }
    }

    Ok(String::new())
}
