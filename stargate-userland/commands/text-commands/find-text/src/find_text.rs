use clap::{Arg, ArgAction, Command};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Result as IoResult};
use sgcore::error::{UResult, USimpleError};
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::{json, Value};

mod options {
    pub const PATTERN: &str = "pattern";
    pub const FILE: &str = "file";
    pub const INSENSITIVE: &str = "insensitive";
}

/// Represents a single match of the search pattern
#[derive(Debug, Clone)]
pub struct Match {
    pub line_number: usize,
    pub line_content: String,
}

/// Represents search results from a single file
#[derive(Debug, Clone)]
pub struct FileSearchResults {
    pub file_path: String,
    pub found: bool,
    pub match_count: usize,
    pub matches: Vec<Match>,
}


/// Search for pattern in a file
pub fn search_file(
    file_path: &str,
    pattern: &str,
    case_insensitive: bool,
) -> IoResult<FileSearchResults> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let mut matches = Vec::new();
    let mut line_number = 0;

    let search_pattern = if case_insensitive {
        pattern.to_lowercase()
    } else {
        pattern.to_string()
    };

    for line_result in reader.lines() {
        line_number += 1;
        let line = line_result?;

        let search_line = if case_insensitive {
            line.to_lowercase()
        } else {
            line.clone()
        };

        if search_line.contains(&search_pattern) {
            matches.push(Match {
                line_number,
                line_content: line,
            });
        }
    }

    Ok(FileSearchResults {
        file_path: file_path.to_string(),
        found: !matches.is_empty(),
        match_count: matches.len(),
        matches,
    })
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath"])?;
    let opts = StardustOutputOptions::from_matches(&matches);

    // Get pattern (required)
    let pattern = matches
        .get_one::<String>(options::PATTERN)
        .ok_or_else(|| USimpleError::new(1, "Pattern is required"))?;

    // Try to get files from stdin JSON first, then from command line args
    let files: Vec<String> = if atty::isnt(atty::Stream::Stdin) {
        // We have data on stdin, try to parse as JSON
        let mut stdin_data = String::new();
        std::io::stdin()
            .read_to_string(&mut stdin_data)
            .map_err(|e| USimpleError::new(1, format!("Failed to read stdin: {}", e)))?;

        if !stdin_data.trim().is_empty() {
            // Try to parse as JSON
            match serde_json::from_str::<Value>(&stdin_data) {
                Ok(json_value) => {
                    // Extract file paths from list-directory style JSON
                    if let Some(entries) = json_value.get("entries").and_then(|e| e.as_array()) {
                        entries
                            .iter()
                            .filter_map(|entry| {
                                // Only include files (not directories)
                                if entry.get("type").and_then(|t| t.as_str()) == Some("file") {
                                    entry.get("path").and_then(|p| p.as_str()).map(String::from)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    } else {
                        // Not the expected format, fall back to command line args
                        matches
                            .get_many::<String>(options::FILE)
                            .ok_or_else(|| {
                                USimpleError::new(1, "At least one file is required")
                            })?
                            .map(|s| s.to_string())
                            .collect()
                    }
                }
                Err(_) => {
                    // Not valid JSON, fall back to command line args
                    matches
                        .get_many::<String>(options::FILE)
                        .ok_or_else(|| USimpleError::new(1, "At least one file is required"))?
                        .map(|s| s.to_string())
                        .collect()
                }
            }
        } else {
            // Empty stdin, fall back to command line args
            matches
                .get_many::<String>(options::FILE)
                .ok_or_else(|| USimpleError::new(1, "At least one file is required"))?
                .map(|s| s.to_string())
                .collect()
        }
    } else {
        // No stdin, use command line args
        matches
            .get_many::<String>(options::FILE)
            .ok_or_else(|| USimpleError::new(1, "At least one file is required"))?
            .map(|s| s.to_string())
            .collect()
    };

    if files.is_empty() {
        return Err(USimpleError::new(
            1,
            "No files to search (no files found in JSON input or command line)",
        ));
    }

    let case_insensitive = matches.get_flag(options::INSENSITIVE);

    if opts.stardust_output {
        // Object (JSON) output mode
        let mut all_results = Vec::new();
        let mut total_matches = 0;

        for file_path in &files {
            match search_file(file_path, pattern, case_insensitive) {
                Ok(result) => {
                    total_matches += result.match_count;
                    all_results.push(json!({
                        "file": result.file_path,
                        "found": result.found,
                        "match_count": result.match_count,
                        "matches": result
                            .matches
                            .iter()
                            .map(|m| json!({
                                "line": m.line_number,
                                "content": m.line_content
                            }))
                            .collect::<Vec<_>>()
                    }));
                }
                Err(e) => {
                    return Err(USimpleError::new(
                        1,
                        format!("Error reading file {}: {}", file_path, e),
                    ))
                }
            }
        }

        stardust_output::output(
            opts,
            json!({
                "pattern": pattern,
                "case_insensitive": case_insensitive,
                "total_matches": total_matches,
                "files_searched": files.len(),
                "results": all_results
            }),
            || Ok(()),
        )?;
    } else {
        // Normal output mode - grep-like format
        for file_path in &files {
            match search_file(file_path, pattern, case_insensitive) {
                Ok(result) => {
                    for match_info in &result.matches {
                        println!(
                            "{}:{}:{}",
                            result.file_path, match_info.line_number, match_info.line_content
                        );
                    }
                }
                Err(e) => {
                    return Err(USimpleError::new(
                        1,
                        format!("Error reading file {}: {}", file_path, e),
                    ))
                }
            }
        }
    }

    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about("Search for text patterns in files")
        .arg(
            Arg::new(options::PATTERN)
                .help("Text pattern to search for")
                .value_name("PATTERN")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new(options::FILE)
                .help("Files to search in")
                .value_name("FILE")
                .required(false)  // Optional when stdin has JSON
                .action(ArgAction::Append)
                .index(2),
        )
        .arg(
            Arg::new(options::INSENSITIVE)
                .short('i')
                .long("ignore-case")
                .help("Ignore case distinctions")
                .action(ArgAction::SetTrue),
        );

    stardust_output::add_json_args(cmd)
}
