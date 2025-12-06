// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

// PATH resolution and Unix command discovery
use std::path::PathBuf;
use std::sync::OnceLock;

/// Load PATH from common shell configuration files and system environment
pub fn get_extended_path() -> &'static String {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let mut path_entries = vec![];
        
        // Start with system PATH
        if let Ok(system_path) = std::env::var("PATH") {
            path_entries.push(system_path);
        }
        
        // Common shell config files to check
        let home = std::env::var("HOME").unwrap_or_default();
        let config_files = [
            format!("{}/.profile", home),
            format!("{}/.bash_profile", home),
            format!("{}/.bashrc", home),
            format!("{}/.zshrc", home),
            format!("{}/.zshenv", home),
        ];
        
        // Parse shell config files for PATH definitions
        for config_file in &config_files {
            if let Ok(content) = std::fs::read_to_string(config_file) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    // Look for PATH= or export PATH=...
                    if trimmed.starts_with("export PATH=") || trimmed.starts_with("PATH=") {
                        if let Some(path_part) = trimmed.split('=').nth(1) {
                            // Remove quotes and $PATH references
                            let cleaned = path_part
                                .trim_matches('"')
                                .trim_matches('\'')
                                .replace("$PATH:", "")
                                .replace(":$PATH", "");
                            if !cleaned.is_empty() {
                                path_entries.push(cleaned);
                            }
                        }
                    }
                }
            }
        }
        
        // Deduplicate and join
        let mut seen = std::collections::HashSet::new();
        path_entries.iter()
            .flat_map(|p| p.split(':'))
            .filter(|p| !p.is_empty() && seen.insert(p.to_string()))
            .collect::<Vec<_>>()
            .join(":")
    })
}

/// Find executable in PATH (system + shell configs)
pub fn find_in_path(cmd: &str) -> Option<PathBuf> {
    let path = get_extended_path();
    
    for dir in path.split(':') {
        let full_path = PathBuf::from(dir).join(cmd);
        if full_path.exists() && full_path.is_file() {
            // Check if executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = std::fs::metadata(&full_path) {
                    if metadata.permissions().mode() & 0o111 != 0 {
                        return Some(full_path);
                    }
                }
            }
            #[cfg(not(unix))]
            {
                return Some(full_path);
            }
        }
    }
    None
}
