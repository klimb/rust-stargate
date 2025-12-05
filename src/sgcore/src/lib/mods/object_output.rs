//! Common object (JSON) output utilities for uutils commands
//!
//! This module provides shared functionality for outputting command results
//! as structured JSON objects when the `-o`/`--obj` flag is specified, along with
//! optional verbose mode via `-v`/`--verbose` flag.

use clap::{Arg, ArgAction};
use serde_json::json;
use serde_json::Value as JsonValue;

/// Options for object (JSON) output and verbosity
#[derive(Debug, Clone, Copy)]
pub struct JsonOutputOptions {
    /// Whether to output as object (JSON) (-o/--obj flag)
    pub object_output: bool,
    /// Whether to include verbose output (-v/--verbose flag)
    pub verbose: bool,
    /// Whether to pretty-print JSON output (--pretty flag)
    pub pretty: bool,
}

impl JsonOutputOptions {
    /// Create a new default instance
    pub fn new() -> Self {
        Self {
            object_output: false,
            verbose: false,
            pretty: false,
        }
    }

    /// Create an instance from clap matches
    pub fn from_matches(matches: &clap::ArgMatches) -> Self {
        Self {
            object_output: matches.get_flag(ARG_OBJECT_OUTPUT),
            verbose: matches.get_flag(ARG_VERBOSE),
            pretty: matches.get_flag(ARG_PRETTY),
        }
    }
}

impl Default for JsonOutputOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Argument names for object (JSON) output and verbose flags
pub const ARG_OBJECT_OUTPUT: &str = "object_output";
pub const ARG_VERBOSE: &str = "verbose_json";
pub const ARG_FIELD: &str = "field";
pub const ARG_PRETTY: &str = "pretty";

/// Add object (JSON) output and verbose arguments to a clap Command
pub fn add_json_args(cmd: clap::Command) -> clap::Command {
    cmd.arg(
        Arg::new(ARG_OBJECT_OUTPUT)
            .long("obj")
            .help("Output as object (JSON)")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_VERBOSE)
            .long("verbose-json")
            .help("Include additional details in JSON output (use with --obj)")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_PRETTY)
            .long("pretty")
            .help("Pretty-print object (JSON) output (use with --obj)")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_FIELD)
            .long("field")
            .value_name("FIELD")
            .help("Filter object output to specific field(s) (comma-separated)")
            .action(ArgAction::Set),
    )
}

/// Filter a JSON object to include only specified fields
///
/// # Arguments
/// * `value` - The JSON value to filter (must be an Object)
/// * `field_spec` - Comma-separated field names (e.g., "architecture" or "path,absolute")
///
/// Returns filtered object, or original value if not an object or field_spec is empty
pub fn filter_fields(value: JsonValue, field_spec: Option<&str>) -> JsonValue {
    let Some(spec) = field_spec else { return value; };
    let JsonValue::Object(mut obj) = value else { return value; };
    
    let fields: Vec<&str> = spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if fields.is_empty() {
        return JsonValue::Object(obj);
    }
    
    let mut filtered = serde_json::map::Map::new();
    for field in fields {
        if let Some(val) = obj.remove(field) {
            filtered.insert(field.to_string(), val);
        }
    }
    JsonValue::Object(filtered)
}

/// Conditionally output object (JSON) or perform default output
///
/// If `options.object_output` is true, serializes the provided `value` as JSON and prints it.
/// Otherwise, calls the provided `default_output` closure to perform default (text) output.
///
/// # Arguments
/// * `options` - Object (JSON) output options
/// * `value` - The JSON value to output if object mode is enabled
/// * `default_output` - Closure that performs default (non-object) output
pub fn output<F>(options: JsonOutputOptions, value: JsonValue, default_output: F) -> std::io::Result<()>
where
    F: FnOnce() -> std::io::Result<()>,
{
    if options.object_output {
        if options.pretty {
            match serde_json::to_string_pretty(&value) {
                Ok(s) => println!("{}", s),
                Err(_) => println!("{}", value),
            }
        } else {
            println!("{}", value);
        }
    } else {
        default_output()?;
    }
    Ok(())
}

/// Create a basic JSON response object with a message
pub fn response(message: impl Into<String>) -> JsonValue {
    json!({
        "output": message.into()
    })
}

/// Create a JSON response object with multiple fields
pub fn response_with_fields(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    let mut obj = serde_json::map::Map::new();
    for (key, value) in fields {
        obj.insert(key.to_string(), value);
    }
    JsonValue::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_options_default() {
        let opts = JsonOutputOptions::default();
        assert!(!opts.object_output);
        assert!(!opts.verbose);
    }

    #[test]
    fn test_response() {
        let resp = response("test output");
        assert_eq!(resp["output"], "test output");
    }

    #[test]
    fn test_response_with_fields() {
        let resp = response_with_fields(vec![
            ("field1", JsonValue::String("value1".to_string())),
            ("field2", JsonValue::Number(42.into())),
        ]);
        assert_eq!(resp["field1"], "value1");
        assert_eq!(resp["field2"], 42);
    }
}
