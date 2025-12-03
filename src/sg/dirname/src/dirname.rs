    //
use clap::{Arg, ArgAction, Command};
use std::ffi::OsString;
use std::path::Path;
use sgcore::display::print_verbatim;
use sgcore::error::{UResult, UUsageError};
use sgcore::format_usage;
use sgcore::line_ending::LineEnding;

use sgcore::translate;
use sgcore::object_output::{self, JsonOutputOptions};
use serde_json::json;

mod options {
    pub const ZERO: &str = "zero";
    pub const DIR: &str = "dir";
}

/// Handle the special case where a path ends with "/."
///
/// This matches GNU/POSIX behavior where `dirname("/home/dos/.")` returns "/home/dos"
/// rather than "/home" (which would be the result of `Path::parent()` due to normalization).
/// Per POSIX.1-2017 dirname specification and GNU coreutils manual:
/// - POSIX: <https://pubs.opengroup.org/onlinepubs/9699919799/utilities/dirname.html>
/// - GNU: <https://www.gnu.org/software/coreutils/manual/html_node/dirname-invocation.html>
///
/// dirname should do simple string manipulation without path normalization.
/// See issue #8910 and similar fix in basename (#8373, commit c5268a897).
///
/// Returns `Some(())` if the special case was handled (output already printed),
/// or `None` if normal `Path::parent()` logic should be used.
fn handle_trailing_dot(path_bytes: &[u8]) -> Option<()> {
    if !path_bytes.ends_with(b"/.") {
        return None;
    }

    // Strip the "/." suffix and print the result
    if path_bytes.len() == 2 {
        // Special case: "/." -> "/"
        print!("/");
        Some(())
    } else {
        // General case: "/home/dos/." -> "/home/dos"
        let stripped = &path_bytes[..path_bytes.len() - 2];
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let result = std::ffi::OsStr::from_bytes(stripped);
            print_verbatim(result).unwrap();
            Some(())
        }
        #[cfg(not(unix))]
        {
            // On non-Unix, fall back to lossy conversion
            if let Ok(s) = std::str::from_utf8(stripped) {
                print!("{s}");
                Some(())
            } else {
                // Can't handle non-UTF-8 on non-Unix, fall through to normal logic
                None
            }
        }
    }
}

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;

    let line_ending = LineEnding::from_zero_flag(matches.get_flag(options::ZERO));
    let opts = JsonOutputOptions::from_matches(&matches);
    let field_filter = matches.get_one::<String>(object_output::ARG_FIELD).map(|s| s.as_str());

    let dirnames: Vec<OsString> = matches
        .get_many::<OsString>(options::DIR)
        .unwrap_or_default()
        .cloned()
        .collect();

    if dirnames.is_empty() {
        return Err(UUsageError::new(1, translate!("dirname-missing-operand")));
    }

    if opts.object_output {
        // For object (JSON) output, collect results into a vector without printing
        let mut results = Vec::new();
        for path in &dirnames {
            let path_bytes = sgcore::os_str_as_bytes(path.as_os_str()).unwrap_or(&[]);

            let dirname_str = if path_bytes.ends_with(b"/.") {
                if path_bytes.len() == 2 {
                    "/".to_string()
                } else {
                    let stripped = &path_bytes[..path_bytes.len() - 2];
                    #[cfg(unix)]
                    {
                        use std::os::unix::ffi::OsStrExt;
                        std::ffi::OsStr::from_bytes(stripped).to_string_lossy().to_string()
                    }
                    #[cfg(not(unix))]
                    {
                        match std::str::from_utf8(stripped) {
                            Ok(s) => s.to_string(),
                            Err(_) => {
                                // Fallback to Path::parent logic
                                let p = Path::new(path);
                                match p.parent() {
                                    Some(d) => {
                                        if d.components().next().is_none() { ".".to_string() } else { d.to_string_lossy().to_string() }
                                    }
                                    None => {
                                        if p.is_absolute() || path.as_os_str() == "/" { "/".to_string() } else { ".".to_string() }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                let p = Path::new(path);
                match p.parent() {
                    Some(d) => {
                        if d.components().next().is_none() { ".".to_string() } else { d.to_string_lossy().to_string() }
                    }
                    None => {
                        if p.is_absolute() || path.as_os_str() == "/" { "/".to_string() } else { ".".to_string() }
                    }
                }
            };

            results.push(dirname_str);
        }
        let output = object_output::filter_fields(json!({"path": results}), field_filter);
        object_output::output(opts, output, || Ok(()))?;
    } else {
        for path in &dirnames {
            let path_bytes = sgcore::os_str_as_bytes(path.as_os_str()).unwrap_or(&[]);

            if handle_trailing_dot(path_bytes).is_none() {
                // Normal path handling using Path::parent()
                let p = Path::new(path);
                match p.parent() {
                    Some(d) => {
                        if d.components().next().is_none() {
                            print!(".");
                        } else {
                            print_verbatim(d).unwrap();
                        }
                    }
                    None => {
                        if p.is_absolute() || path.as_os_str() == "/" {
                            print!("/");
                        } else {
                            print!(".");
                        }
                    }
                }
            }
            print!("{line_ending}");
        }
    }

    Ok(())
}

pub fn uu_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .about(translate!("dirname-about"))
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .override_usage(format_usage(&translate!("dirname-usage")))
        .args_override_self(true)
        .infer_long_args(true)
        .after_help(translate!("dirname-after-help"))
        .arg(
            Arg::new(options::ZERO)
                .long(options::ZERO)
                .short('z')
                .help(translate!("dirname-zero-help"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::DIR)
                .hide(true)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::AnyPath)
                .value_parser(clap::value_parser!(OsString))
        );

    object_output::add_json_args(cmd)
}
