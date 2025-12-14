// Copyright (c) 2025 Dmitry Kalashnikov

use clap::{Arg, ArgAction};
use serde_json::json;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Copy)]
pub struct StardustOutputOptions {
    pub stardust_output: bool,
    pub verbose: bool,
    pub pretty: bool,
}

impl StardustOutputOptions {
    pub fn new() -> Self {
        Self {
            stardust_output: false,
            verbose: false,
            pretty: false,
        }
    }

    pub fn from_matches(matches: &clap::ArgMatches) -> Self {
        Self {
            stardust_output: matches.get_flag(ARG_STARDUST_OUTPUT),
            verbose: matches.get_flag(ARG_VERBOSE),
            pretty: matches.get_flag(ARG_PRETTY),
        }
    }
}

impl Default for StardustOutputOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub const ARG_STARDUST_OUTPUT: &str = "stardust_output";
pub const ARG_VERBOSE: &str = "verbose_json";
pub const ARG_FIELD: &str = "field";
pub const ARG_PRETTY: &str = "pretty";
pub const ARG_SCHEMA: &str = "schema";

pub fn add_json_args(cmd: clap::Command) -> clap::Command {
    cmd.arg(
        Arg::new(ARG_STARDUST_OUTPUT)
            .long("obj")
            .help("Output as stardust (JSON)")
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
            .help("Filter stardust output to specific field(s) (comma-separated)")
            .action(ArgAction::Set),
    )
    .arg(
        Arg::new(ARG_SCHEMA)
            .long("schema")
            .help("Print JSON schema of output structure")
            .action(ArgAction::SetTrue)
            .hide(true),  // Hidden from normal help
    )
}

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

pub fn output<F>(options: StardustOutputOptions, value: JsonValue, default_output: F) -> std::io::Result<()>
where
    F: FnOnce() -> std::io::Result<()>,
{
    if options.stardust_output {
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

pub fn response(message: impl Into<String>) -> JsonValue {
    json!({
        "output": message.into()
    })
}

pub fn response_with_fields(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    let mut obj = serde_json::map::Map::new();
    for (key, value) in fields {
        obj.insert(key.to_string(), value);
    }
    JsonValue::Object(obj)
}

/// Helper to create a JSON schema for command output
/// Returns a simple schema with property names and types
pub fn create_schema(properties: Vec<(&str, &str, Option<&str>)>) -> JsonValue {
    let mut props = serde_json::map::Map::new();
    for (name, type_name, description) in properties {
        let mut prop = serde_json::map::Map::new();
        prop.insert("type".to_string(), JsonValue::String(type_name.to_string()));
        if let Some(desc) = description {
            prop.insert("description".to_string(), JsonValue::String(desc.to_string()));
        }
        props.insert(name.to_string(), JsonValue::Object(prop));
    }
    
    json!({
        "type": "object",
        "properties": props
    })
}

/// Print schema to stdout
pub fn print_schema(schema: JsonValue) -> std::io::Result<()> {
    println!("{}", serde_json::to_string_pretty(&schema).unwrap_or_else(|_| schema.to_string()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_options_default() {
        let opts = StardustOutputOptions::default();
        assert!(!opts.stardust_output);
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
