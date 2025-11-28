// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{Arg, ArgAction, Command};
use std::fs::File;
use std::io::{BufRead, BufReader, Result as IoResult};
use uucore::error::{UResult, USimpleError};
use uucore::object_output::{self, JsonOutputOptions};
use serde_json::json;

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

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let opts = JsonOutputOptions::from_matches(&matches);

    // Get pattern (required)
    let pattern = matches
        .get_one::<String>(options::PATTERN)
        .ok_or_else(|| USimpleError::new(1, "Pattern is required"))?;

    // Get files (at least one required)
    let files: Vec<&String> = matches
        .get_many::<String>(options::FILE)
        .ok_or_else(|| USimpleError::new(1, "At least one file is required"))?
        .collect();

    let case_insensitive = matches.get_flag(options::INSENSITIVE);

    if opts.object_output {
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

        object_output::output(
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

pub fn uu_app() -> Command {
    let cmd = Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template(uucore::util_name()))
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
                .required(true)
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

    object_output::add_json_args(cmd)
}
