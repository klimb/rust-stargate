// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{Arg, ArgAction, Command};
use serde_json::Value;
use std::io::{self, Read};
use sgcore::error::{UResult, USimpleError};

pub mod options {
    pub static FIELD: &str = "field";
    pub static FIELDS: &str = "fields";
    pub static INDEX: &str = "index";
    pub static PRETTY: &str = "pretty";
}

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    // Read JSON from stdin
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| USimpleError::new(1, format!("Failed to read stdin: {}", e)))?;

    // Parse JSON
    let json: Value = serde_json::from_str(&input)
        .map_err(|e| USimpleError::new(1, format!("Failed to parse JSON: {}", e)))?;

    let pretty = matches.get_flag(options::PRETTY);

    // Handle field extraction
    if let Some(field) = matches.get_one::<String>(options::FIELD) {
        let result = extract_field(&json, field)?;
        output_json(&result, pretty)?;
    } else if let Some(fields) = matches.get_many::<String>(options::FIELDS) {
        let field_list: Vec<&str> = fields.map(|s| s.as_str()).collect();
        let result = extract_multiple_fields(&json, &field_list)?;
        output_json(&result, pretty)?;
    } else if let Some(index) = matches.get_one::<usize>(options::INDEX) {
        let result = extract_by_index(&json, *index)?;
        output_json(&result, pretty)?;
    } else {
        // No extraction specified, just output the JSON (possibly prettified)
        output_json(&json, pretty)?;
    }

    Ok(())
}

fn extract_field(json: &Value, field: &str) -> UResult<Value> {
    match json {
        Value::Object(map) => {
            if let Some(value) = map.get(field) {
                Ok(value.clone())
            } else {
                Err(USimpleError::new(
                    1,
                    format!("Field '{}' not found in JSON object", field),
                ))
            }
        }
        Value::Array(arr) => {
            // Extract field from all objects in array
            let results: Vec<Value> = arr
                .iter()
                .filter_map(|item| {
                    if let Value::Object(map) = item {
                        map.get(field).cloned()
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Value::Array(results))
        }
        _ => Err(USimpleError::new(
            1,
            "Input JSON must be an object or array".to_string(),
        )),
    }
}

fn extract_multiple_fields(json: &Value, fields: &[&str]) -> UResult<Value> {
    match json {
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for field in fields {
                if let Some(value) = map.get(*field) {
                    result.insert(field.to_string(), value.clone());
                }
            }
            Ok(Value::Object(result))
        }
        Value::Array(arr) => {
            // Extract fields from all objects in array
            let results: Vec<Value> = arr
                .iter()
                .filter_map(|item| {
                    if let Value::Object(map) = item {
                        let mut obj = serde_json::Map::new();
                        for field in fields {
                            if let Some(value) = map.get(*field) {
                                obj.insert(field.to_string(), value.clone());
                            }
                        }
                        Some(Value::Object(obj))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Value::Array(results))
        }
        _ => Err(USimpleError::new(
            1,
            "Input JSON must be an object or array".to_string(),
        )),
    }
}

fn extract_by_index(json: &Value, index: usize) -> UResult<Value> {
    match json {
        Value::Array(arr) => {
            if index < arr.len() {
                Ok(arr[index].clone())
            } else {
                Err(USimpleError::new(
                    1,
                    format!("Index {} out of bounds (array length: {})", index, arr.len()),
                ))
            }
        }
        _ => Err(USimpleError::new(
            1,
            "Input JSON must be an array for index extraction".to_string(),
        )),
    }
}

fn output_json(value: &Value, pretty: bool) -> UResult<()> {
    let output = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
    .map_err(|e| USimpleError::new(1, format!("Failed to serialize JSON: {}", e)))?;

    println!("{}", output);
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .about("Extract fields from JSON objects")
        .override_usage("slice-object [OPTIONS]")
        .arg(
            Arg::new(options::FIELD)
                .short('f')
                .long("field")
                .value_name("FIELD")
                .help("Extract a single field from JSON object(s)")
                .conflicts_with_all([options::FIELDS, options::INDEX])
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::FIELDS)
                .short('F')
                .long("fields")
                .value_name("FIELD")
                .help("Extract multiple fields from JSON object(s)")
                .conflicts_with_all([options::FIELD, options::INDEX])
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new(options::INDEX)
                .short('i')
                .long("index")
                .value_name("INDEX")
                .help("Extract element at index from JSON array")
                .conflicts_with_all([options::FIELD, options::FIELDS])
                .value_parser(clap::value_parser!(usize))
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::PRETTY)
                .short('p')
                .long("pretty")
                .help("Output pretty-printed JSON")
                .action(ArgAction::SetTrue),
        )
}
