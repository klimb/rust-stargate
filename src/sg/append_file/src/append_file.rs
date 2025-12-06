use clap::{Arg, ArgAction, Command};
use serde_json::json;
use sgcore::error::UResult;
use sgcore::format_usage;
use std::fs::OpenOptions;
use std::io::{self, Read, Write};

mod options {
    pub const PATH: &str = "PATH";
    pub const OBJECT_OUTPUT: &str = "object_output";
    pub const PRETTY: &str = "pretty";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    let path = matches.get_one::<String>(options::PATH).unwrap();
    let object_output = matches.get_flag(options::OBJECT_OUTPUT);
    let pretty = matches.get_flag(options::PRETTY);

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    let content_to_write = if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&input) {
        if let Some(output) = json_value.get("output").and_then(|v| v.as_str()) {
            output.to_string()
        } else {
            input
        }
    } else {
        input
    };

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(content_to_write.as_bytes())?;

    if object_output {
        let output = json!({
            "path": path,
            "bytes_written": content_to_write.len(),
            "success": true
        });

        if pretty {
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            println!("{}", output);
        }
    }

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(sgcore::util_name())
        .version(env!("CARGO_PKG_VERSION"))
        .about("Append text from stdin to a file")
        .override_usage(format_usage(
            "append-file [OPTION]... PATH"
        ))
        .infer_long_args(true)
        .disable_help_flag(true)
        .arg(
            Arg::new("help")
                .long("help")
                .help("Print help information")
                .action(ArgAction::Help),
        )
        .arg(
            Arg::new(options::PATH)
                .help("Path to the file to append to")
                .value_name("PATH")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new(options::OBJECT_OUTPUT)
                .short('o')
                .long("obj")
                .help("Output result as JSON object")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::PRETTY)
                .long("pretty")
                .help("Pretty-print JSON output")
                .action(ArgAction::SetTrue),
        )
}
