use clap::{Arg, ArgAction, Command};
use serde_json::Value;
use std::io::{self, Read};
use sgcore::error::{SGResult, SGSimpleError};

pub mod options {
    pub static FIELD: &str = "field";
    pub static FIELDS: &str = "fields";
    pub static INDEX: &str = "index";
    pub static PRETTY: &str = "pretty";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sg_app().try_get_matches_from(args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| SGSimpleError::new(1, format!("Failed to read stdin: {}", e)))?;

    let json: Value = serde_json::from_str(&input)
        .map_err(|e| SGSimpleError::new(1, format!("Failed to parse JSON: {}", e)))?;

    let pretty = matches.get_flag(options::PRETTY);

    if let Some(field) = matches.get_one::<String>(options::FIELD) {
        let result = extract_field(&json, field)?;
        output_json(&result, pretty)?;
    } else {
        output_json(&json, pretty)?;
    }

    Ok(())
}

fn extract_field(json: &Value, field: &str) -> SGResult<Value> {
    match json {
        Value::Object(map) => {
            if let Some(value) = map.get(field) {
                Ok(value.clone())
            } else {
                Err(SGSimpleError::new(
                    1,
                    format!("Field '{}' not found in JSON object", field),
                ))
            }
        }
        Value::Array(arr) => {
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
        _ => Err(SGSimpleError::new(
            1,
            "Input JSON must be an object or array".to_string(),
        )),
    }
}

fn extract_multiple_fields(json: &Value, fields: &[&str]) -> SGResult<Value> {
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
        _ => Err(SGSimpleError::new(
            1,
            "Input JSON must be an object or array".to_string(),
        )),
    }
}

fn extract_by_index(json: &Value, index: usize) -> SGResult<Value> {
    match json {
        Value::Array(arr) => {
            if index < arr.len() {
                Ok(arr[index].clone())
            } else {
                Err(SGSimpleError::new(
                    1,
                    format!("Index {} out of bounds (array length: {})", index, arr.len()),
                ))
            }
        }
        _ => Err(SGSimpleError::new(
            1,
            "Input JSON must be an array for index extraction".to_string(),
        )),
    }
}

fn output_json(value: &Value, pretty: bool) -> SGResult<()> {
    let output = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
    .map_err(|e| SGSimpleError::new(1, format!("Failed to serialize JSON: {}", e)))?;

    println!("{}", output);
    Ok(())
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .about("Extract fields from JSON objects")
        .override_usage("slice-object [FIELD]")
        .arg(
            Arg::new(options::FIELD)
                .value_name("FIELD")
                .help("Extract a single field from JSON object(s)")
                .index(1)
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

