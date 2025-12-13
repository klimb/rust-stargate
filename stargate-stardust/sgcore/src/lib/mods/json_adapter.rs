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
