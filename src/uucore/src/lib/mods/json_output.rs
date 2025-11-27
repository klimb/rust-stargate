// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

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
    pub json_output: bool,
    /// Whether to include verbose output (-v/--verbose flag)
    pub verbose: bool,
}

impl JsonOutputOptions {
    /// Create a new default instance
    pub fn new() -> Self {
        Self {
            json_output: false,
            verbose: false,
        }
    }

    /// Create an instance from clap matches
    pub fn from_matches(matches: &clap::ArgMatches) -> Self {
        Self {
            json_output: matches.get_flag(ARG_JSON_OUTPUT),
            verbose: matches.get_flag(ARG_VERBOSE),
        }
    }
}

impl Default for JsonOutputOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Argument names for object (JSON) output and verbose flags
pub const ARG_JSON_OUTPUT: &str = "json_output";
pub const ARG_VERBOSE: &str = "verbose";

/// Add object (JSON) output and verbose arguments to a clap Command
pub fn add_json_args(cmd: clap::Command) -> clap::Command {
    cmd.arg(
        Arg::new(ARG_JSON_OUTPUT)
            .short('o')
            .long("obj")
            .help("Output as object (JSON)")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new(ARG_VERBOSE)
            .short('v')
            .long("verbose")
            .help("Include additional details in output")
            .action(ArgAction::SetTrue),
    )
}

/// Conditionally output object (JSON) or perform default output
///
/// If `options.json_output` is true, serializes the provided `value` as JSON and prints it.
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
    if options.json_output {
        println!("{}", value);
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
        assert!(!opts.json_output);
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
