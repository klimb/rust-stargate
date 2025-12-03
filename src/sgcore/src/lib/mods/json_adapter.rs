// This file is part of the sgcore package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! JSON adapter for extracting file paths from various JSON formats.
//!
//! This module provides utilities to extract file paths from JSON input,
//! supporting multiple common patterns used by different commands.

use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;

pub fn extract_file_paths(value: &Value) -> Vec<PathBuf> {
    match value {
        Value::String(s) => vec![PathBuf::from(s)],
        
        Value::Array(arr) => arr.iter()
            .flat_map(extract_file_paths)
            .collect(),
        
        Value::Object(map) => {
            if let Some(Value::Array(entries)) = map.get("entries") {
                return entries.iter()
                    .filter_map(|entry| {
                        if let Value::Object(obj) = entry {
                            if obj.get("type").and_then(Value::as_str) == Some("directory") {
                                return None;
                            }
                        }
                        extract_single_file_path(entry)
                    })
                    .collect();
            }
            
            if let Some(Value::Array(files)) = map.get("files") {
                return files.iter()
                    .filter_map(extract_single_file_path)
                    .collect();
            }
            
            if let Some(Value::Array(results)) = map.get("results") {
                return results.iter()
                    .filter_map(extract_single_file_path)
                    .collect();
            }
            
            if let Some(path) = extract_single_file_path(value) {
                return vec![path];
            }
            
            map.values()
                .flat_map(extract_file_paths)
                .collect()
        }
        
        _ => vec![],
    }
}

fn extract_single_file_path(value: &Value) -> Option<PathBuf> {
    match value {
        Value::String(s) => Some(PathBuf::from(s)),
        Value::Object(obj) => {
            for field in ["path", "file", "filepath", "filename", "name"] {
                if let Some(Value::String(s)) = obj.get(field) {
                    return Some(PathBuf::from(s));
                }
            }
            None
        }
        _ => None,
    }
}

pub fn try_extract_paths_from_stdin() -> Option<Vec<PathBuf>> {
    if atty::is(atty::Stream::Stdin) {
        return None;
    }

    let mut stdin_data = String::new();
    std::io::stdin().read_to_string(&mut stdin_data).ok()?;
    
    let json: Value = serde_json::from_str(stdin_data.trim()).ok()?;
    let paths = extract_file_paths(&json);
    
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_from_string() {
        let json = json!("test.txt");
        let paths = extract_file_paths(&json);
        assert_eq!(paths, vec![PathBuf::from("test.txt")]);
    }

    #[test]
    fn test_extract_from_array() {
        let json = json!(["file1.txt", "file2.txt", "file3.txt"]);
        let paths = extract_file_paths(&json);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], PathBuf::from("file1.txt"));
    }

    #[test]
    fn test_extract_from_list_directory_format() {
        let json = json!({
            "entries": [
                {"path": "file1.txt", "type": "file"},
                {"path": "dir", "type": "directory"},
                {"path": "file2.txt", "type": "file"}
            ]
        });
        let paths = extract_file_paths(&json);
        assert_eq!(paths.len(), 2); // Should skip directory
        assert_eq!(paths[0], PathBuf::from("file1.txt"));
        assert_eq!(paths[1], PathBuf::from("file2.txt"));
    }

    #[test]
    fn test_extract_from_files_array() {
        let json = json!({
            "files": ["a.txt", "b.txt"]
        });
        let paths = extract_file_paths(&json);
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_extract_from_nested_objects() {
        let json = json!({
            "results": [
                {"filename": "test1.txt"},
                {"filepath": "test2.txt"},
                {"name": "test3.txt"}
            ]
        });
        let paths = extract_file_paths(&json);
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn test_extract_from_object_with_path() {
        let json = json!({"path": "single.txt"});
        let paths = extract_file_paths(&json);
        assert_eq!(paths, vec![PathBuf::from("single.txt")]);
    }

    #[test]
    fn test_empty_result() {
        let json = json!({"other": "data"});
        let paths = extract_file_paths(&json);
        assert_eq!(paths.len(), 0);
    }
}

pub fn extract_count_from_list_directory(value: &Value) -> Option<u64> {
    if let Some(arr) = value.as_array() {
        return Some(arr.len() as u64);
    }

    if let Some(entries) = value.get("entries").and_then(|v| v.as_array()) {
        return Some(entries.len() as u64);
    }

    if let Some(count) = value.get("count").and_then(|v| v.as_u64()) {
        return Some(count);
    }

    None
}

pub fn try_extract_from_stdin() -> Option<StdinResult> {
    use std::io::IsTerminal;
    
    if std::io::stdin().is_terminal() {
        return None;
    }

    let mut stdin_data = String::new();
    if std::io::stdin().read_to_string(&mut stdin_data).is_err() {
        return None;
    }

    let json: serde_json::Value = match serde_json::from_str(stdin_data.trim()) {
        Ok(v) => v,
        Err(_) => return None,
    };

    if let Some(count) = extract_count_from_list_directory(&json) {
        return Some(StdinResult::Count(count));
    }

    let paths = extract_file_paths(&json);
    if paths.is_empty() {
        None
    } else {
        Some(StdinResult::Paths(paths))
    }
}

pub enum StdinResult {
    Count(u64),
    Paths(Vec<PathBuf>),
}
