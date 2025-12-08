use clap::{Arg, ArgAction, Command};
use serde_json::json;
use sgcore::error::UResult;
use sgcore::format_usage;
use std::fs::File;
use std::io::{self, Read, Write};

mod options {
    pub const PATH: &str = "PATH";
    pub const OBJECT_OUTPUT: &str = "object_output";
    pub const PRETTY: &str = "pretty";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sg_app().try_get_matches_from(args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath"])?;

    let path = matches.get_one::<String>(options::PATH).unwrap();
    let object_output = matches.get_flag(options::OBJECT_OUTPUT);
    let pretty = matches.get_flag(options::PRETTY);

    // Read all input from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    // Check if input is JSON from another stargate command
    // If so, extract the text content from the "output" field
    let content_to_write = if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&input) {
        // If it's JSON with an "output" field, extract that
        if let Some(output) = json_value.get("output").and_then(|v| v.as_str()) {
            output.to_string()
        } else {
            // Otherwise write the JSON as-is
            input
        }
    } else {
        // Not JSON, write as-is
        input
    };

    // Write to file
    let mut file = File::create(path)?;
    file.write_all(content_to_write.as_bytes())?;

    // Output result
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
    // Silent operation for normal mode (like traditional file redirection)

    Ok(())
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(env!("CARGO_PKG_VERSION"))
        .about("Write text from stdin to a file")
        .override_usage(format_usage(
            "new-file [OPTION]... PATH"
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
                .help("Path to the file to create")
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
