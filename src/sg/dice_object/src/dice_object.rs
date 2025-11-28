// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{Arg, ArgAction, Command};
use serde_json::Value;
use std::io::{self, Read};
use uucore::error::{UResult, USimpleError};

pub mod options {
    pub static FIELD: &str = "field";
    pub static FIELDS: &str = "fields";
    pub static PRETTY: &str = "pretty";
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
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

    // Collect fields to extract
    let fields: Vec<String> = if let Some(field) = matches.get_one::<String>(options::FIELD) {
        vec![field.clone()]
    } else if let Some(fields_iter) = matches.get_many::<String>(options::FIELDS) {
        fields_iter.map(|s| s.to_string()).collect()
    } else {
        // No fields specified, output as-is
        output_json(&json, pretty)?;
        return Ok(());
    };

    // Filter columns
    let result = filter_columns(&json, &fields)?;
    output_json(&result, pretty)?;

    Ok(())
}

fn filter_columns(json: &Value, fields: &[String]) -> UResult<Value> {
    match json {
        Value::Object(map) => {
            // Single object: extract specified fields
            let mut result = serde_json::Map::new();
            for field in fields {
                if let Some(value) = map.get(field) {
                    result.insert(field.clone(), value.clone());
                }
            }
            Ok(Value::Object(result))
        }
        Value::Array(arr) => {
            // Array of objects: extract specified fields from each
            let results: Vec<Value> = arr
                .iter()
                .filter_map(|item| {
                    if let Value::Object(map) = item {
                        let mut obj = serde_json::Map::new();
                        for field in fields {
                            if let Some(value) = map.get(field) {
                                obj.insert(field.clone(), value.clone());
                            }
                        }
                        if !obj.is_empty() {
                            Some(Value::Object(obj))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Value::Array(results))
        }
        _ => Err(USimpleError::new(
            1,
            "Input JSON must be an object or array of objects".to_string(),
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
    Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .about("Filter JSON object columns/fields")
        .override_usage("dice-object [OPTIONS]")
        .arg(
            Arg::new(options::FIELD)
                .short('f')
                .long("field")
                .value_name("FIELD")
                .help("Extract a single field (column)")
                .conflicts_with(options::FIELDS)
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::FIELDS)
                .short('F')
                .long("fields")
                .value_name("FIELD")
                .help("Extract multiple fields (columns)")
                .conflicts_with(options::FIELD)
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new(options::PRETTY)
                .short('p')
                .long("pretty")
                .help("Output pretty-printed JSON")
                .action(ArgAction::SetTrue),
        )
}
