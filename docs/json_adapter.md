# JSON Adapter Module

The `sgcore::json_adapter` module provides a reusable adapter for extracting file paths from various JSON input formats. This module enables commands to work seamlessly in JSON pipelines without knowing the specific structure of upstream command outputs.

## Purpose

When building pipeline-based utilities, commands often need to accept JSON output from other commands. However, different commands may structure their JSON differently:

- `list-directory` outputs: `{"entries": [{"path": "...", "type": "file"}]}`
- Custom tools might output: `{"files": ["file1", "file2"]}`
- Simple arrays: `["file1", "file2"]`
- Nested structures: `{"results": [{"filename": "..."}]}`

The JSON adapter handles all these patterns automatically, making your command work with any JSON source that contains file paths.

## API

### `extract_file_paths(value: &Value) -> Vec<PathBuf>`

Recursively extracts file paths from a JSON value.

**Supported patterns:**
- String values are treated as file paths
- Arrays are processed recursively
- Objects with `entries`, `files`, or `results` arrays
- Objects with common file path fields: `path`, `file`, `filepath`, `filename`, `name`
- Automatically filters out directories when `type: "directory"` field is present
- Recursively searches nested structures

**Example:**
```rust
use sgcore::json_adapter::extract_file_paths;
use serde_json::json;

// list-directory format
let json = json!({"entries": [{"path": "test.txt", "type": "file"}]});
let paths = extract_file_paths(&json);
assert_eq!(paths.len(), 1);

// Simple array
let json = json!(["file1.txt", "file2.txt"]);
let paths = extract_file_paths(&json);
assert_eq!(paths.len(), 2);
```

### `try_extract_paths_from_stdin() -> Option<Vec<PathBuf>>`

Convenience function that reads from stdin, parses JSON, and extracts file paths.

**Returns:**
- `None` if stdin is a TTY (interactive terminal)
- `None` if stdin is empty or doesn't contain valid JSON
- `Some(paths)` if paths were successfully extracted

**Example:**
```rust
use sgcore::json_adapter::try_extract_paths_from_stdin;

if let Some(paths) = try_extract_paths_from_stdin() {
    for path in paths {
        println!("Processing: {}", path.display());
    }
}
```

## Usage in Commands

### Basic Pattern

The typical usage pattern in a command is:

```rust
use sgcore::json_adapter;
use std::path::PathBuf;

fn get_input_files(args: &ArgMatches) -> Vec<PathBuf> {
    // First, check if files were specified on command line
    if let Some(files) = args.get_many::<PathBuf>("FILE") {
        return files.cloned().collect();
    }
    
    // Otherwise, try to extract from JSON stdin
    if let Some(paths) = json_adapter::try_extract_paths_from_stdin() {
        return paths;
    }
    
    // Fall back to reading from stdin directly
    vec![]
}
```

### Integration with Command Input Enum

For more complex input handling (like in `wc`):

```rust
use sgcore::json_adapter;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

enum Input<'a> {
    Path(Cow<'a, Path>),
    Stdin,
}

impl Input<'_> {
    fn from_args_or_json(matches: &ArgMatches) -> Vec<Input<'static>> {
        // Check command-line arguments first
        if let Some(files) = matches.get_many::<OsString>("FILE") {
            return files.map(|f| Input::Path(Cow::Owned(PathBuf::from(f)))).collect();
        }
        
        // Try JSON input from stdin
        if let Some(paths) = json_adapter::try_extract_paths_from_stdin() {
            return paths.into_iter()
                .map(|p| Input::Path(Cow::Owned(p)))
                .collect();
        }
        
        // Default to stdin
        vec![Input::Stdin]
    }
}
```

## Field Name Priority

When extracting from JSON objects, the adapter tries field names in this order:

1. `path` - Most common in file listings
2. `file` - Common alternative
3. `filepath` - Explicit full path
4. `filename` - Name-focused variant
5. `name` - Generic fallback

This ordering ensures maximum compatibility with various JSON schemas.

## Directory Filtering

When processing `list-directory` output or similar structured data, the adapter automatically excludes entries marked as directories:

```rust
let json = json!({
    "entries": [
        {"path": "file.txt", "type": "file"},      // ✓ Included
        {"path": "mydir", "type": "directory"},    // ✗ Excluded
        {"path": "script.sh", "type": "file"}      // ✓ Included
    ]
});

let paths = extract_file_paths(&json);
assert_eq!(paths.len(), 2); // Only files, not directory
```

## Design Philosophy

The JSON adapter follows these principles:

1. **Be Permissive**: Accept any JSON that reasonably contains file paths
2. **Be Predictable**: Use consistent field name conventions
3. **Be Safe**: Filter inappropriate entries (directories when files are expected)
4. **Be Efficient**: Use recursive pattern matching, not exhaustive searching
5. **Be Silent**: Return empty results rather than error for unknown formats

## Testing

The module includes comprehensive tests for all supported patterns. When adding new JSON formats, add corresponding tests:

```rust
#[test]
fn test_your_custom_format() {
    let json = json!({
        "your_custom_structure": [
            {"filepath": "test.txt"}
        ]
    });
    let paths = extract_file_paths(&json);
    assert_eq!(paths.len(), 1);
}
```

## Future Extensions

Potential enhancements to consider:

- Support for URL extraction in addition to file paths
- Configuration for field name preferences
- Type hints for specialized extraction (files only, directories only, etc.)
- Streaming JSON parsing for very large inputs
- Error reporting mode for debugging JSON schema mismatches
