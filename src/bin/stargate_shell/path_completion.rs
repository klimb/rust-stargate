
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use rustyline::completion::Pair;
use std::fs;
use std::path::Path;

pub const DIRECTORY_COMMANDS: &[&str] = &["cd", "change-directory"];

pub const COMMANDS: &[&str] = &[
    "get-contents", "cat", "create-file", "touch", "remove-file", "rm",
    "copy-file", "cp", "move-file", "mv", "create-directory", "mkdir",
    "list-directory", "ls", "get-file-type", "file", "get-file-owner",
    "link", "ln", "readlink", "realpath", "stat", "truncate",
    "base64", "sum", "cksum", "hashsum", "md5sum", "sha1sum", "sha256sum",
    "split", "csplit", "paste", "cut", "sort", "uniq", "comm",
    "head", "tail", "collect-count", "wc", "find-text", "grep"
];

pub fn get_directory_completions(prefix: &str) -> Vec<Pair> {
    let expanded_prefix = expand_tilde(prefix);
    let (search_dir, partial_name) = parse_path(&expanded_prefix);
    
    let mut matches = Vec::new();
    if let Ok(entries) = fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with('.') && !partial_name.starts_with('.') {
                            continue;
                        }
                        
                        if name.starts_with(partial_name) {
                            let replacement = format_path_replacement(
                                prefix,
                                &expanded_prefix,
                                search_dir,
                                partial_name,
                                name,
                                true
                            );
                            
                            matches.push(Pair {
                                display: format!("{}/", name),
                                replacement,
                            });
                        }
                    }
                }
            }
        }
    }
    
    matches.sort_by(|a, b| a.display.cmp(&b.display));
    matches
}

pub fn get_path_completions(prefix: &str) -> Vec<Pair> {
    let expanded_prefix = expand_tilde(prefix);
    let (search_dir, partial_name) = parse_path(&expanded_prefix);
    
    let mut matches = Vec::new();
    if let Ok(entries) = fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with('.') && !partial_name.starts_with('.') {
                    continue;
                }
                
                if name.starts_with(partial_name) {
                    if let Ok(metadata) = entry.metadata() {
                        let is_dir = metadata.is_dir();
                        
                        let replacement = format_path_replacement(
                            prefix,
                            &expanded_prefix,
                            search_dir,
                            partial_name,
                            name,
                            is_dir
                        );
                        
                        let display = if is_dir {
                            format!("{}/", name)
                        } else {
                            name.to_string()
                        };
                        
                        matches.push(Pair {
                            display,
                            replacement,
                        });
                    }
                }
            }
        }
    }
    
    matches.sort_by(|a, b| a.display.cmp(&b.display));
    matches
}

fn expand_tilde(prefix: &str) -> String {
    if prefix.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            Path::new(&home).join(&prefix[2..]).to_string_lossy().to_string()
        } else {
            prefix.to_string()
        }
    } else {
        prefix.to_string()
    }
}

fn parse_path(expanded_prefix: &str) -> (&str, &str) {
    if expanded_prefix.is_empty() {
        (".", "")
    } else if expanded_prefix.ends_with('/') {
        (expanded_prefix, "")
    } else if let Some(last_slash) = expanded_prefix.rfind('/') {
        (&expanded_prefix[..last_slash + 1], &expanded_prefix[last_slash + 1..])
    } else {
        (".", expanded_prefix)
    }
}

fn format_path_replacement(
    prefix: &str,
    expanded_prefix: &str,
    search_dir: &str,
    partial_name: &str,
    name: &str,
    is_dir: bool
) -> String {
    let suffix = if is_dir { "/" } else { "" };
    
    if search_dir == "." && !prefix.contains('/') {
        format!("{}{}", name, suffix)
    } else if prefix.starts_with("~/") {
        let base_path = &prefix[2..prefix.len() - partial_name.len()];
        format!("~/{}{}{}", base_path, name, suffix)
    } else {
        let base_path = &expanded_prefix[..expanded_prefix.len() - partial_name.len()];
        format!("{}{}{}", base_path, name, suffix)
    }
}
