use clap::{Arg, ArgAction, Command};
use serde_json::Value;
use std::io::{self, Read};
use sgcore::error::{SGResult, SGSimpleError};

pub mod options {
    pub static FIELD: &str = "field";
    pub static FIELDS: &str = "fields";
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

    let fields: Vec<String> = if let Some(fields_iter) = matches.get_many::<String>(options::FIELDS) {
        fields_iter.map(|s| s.to_string()).collect()
    } else {
        output_json(&json, pretty)?;
        return Ok(());
    };

    let result = filter_columns(&json, &fields)?;
    output_json(&result, pretty)?;

    Ok(())
}

fn filter_columns(json: &Value, fields: &[String]) -> SGResult<Value> {
    match json {
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for field in fields {
                if let Some(value) = map.get(field) {
                    result.insert(field.clone(), value.clone());
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
        _ => Err(SGSimpleError::new(
            1,
            "Input JSON must be an object or array of objects".to_string(),
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
        .about("Filter JSON object columns/fields")
        .override_usage("dice-object [FIELD]...")
        .arg(
            Arg::new(options::FIELDS)
                .value_name("FIELD")
                .help("Extract multiple fields (columns)")
                .num_args(1..)
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

