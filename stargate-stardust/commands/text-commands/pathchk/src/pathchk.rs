#![allow(unused_must_use)]

use clap::{Arg, ArgAction, Command};
use std::ffi::OsString;
use std::fs;
use std::io::{ErrorKind, Write};
use sgcore::display::Quotable;
use sgcore::error::{SGResult, SGUsageError, set_exit_code};
use sgcore::format_usage;
use sgcore::translate;
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::json;

enum Mode {
    Default,
    Basic,
    Extra,
    Both,
}

mod options {
    pub const POSIX: &str = "posix";
    pub const POSIX_SPECIAL: &str = "posix-special";
    pub const PORTABILITY: &str = "portability";
    pub const PATH: &str = "path";
}

const POSIX_PATH_MAX: usize = 256;
const POSIX_NAME_MAX: usize = 14;

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath"])?;
    let mut opts = StardustOutputOptions::from_matches(&matches);
    if !matches.contains_id("stardust_output") {
        opts.stardust_output = true;
    }

    let is_posix = matches.get_flag(options::POSIX);
    let is_posix_special = matches.get_flag(options::POSIX_SPECIAL);
    let is_portability = matches.get_flag(options::PORTABILITY);

    let mode = if (is_posix && is_posix_special) || is_portability {
        Mode::Both
    } else if is_posix {
        Mode::Basic
    } else if is_posix_special {
        Mode::Extra
    } else {
        Mode::Default
    };

    let paths = matches.get_many::<OsString>(options::PATH);
    if paths.is_none() {
        return Err(SGUsageError::new(
            1,
            translate!("pathchk-error-missing-operand")
        ));
    }

    let mut res = true;
    let mut path_results = Vec::new();

    for p in paths.unwrap() {
        let path_str = p.to_string_lossy();
        let mut path = Vec::new();
        for path_segment in path_str.split('/') {
            path.push(path_segment.to_string());
        }
        let is_valid = check_path(&mode, &path);
        res &= is_valid;

        if opts.stardust_output {
            path_results.push(json!({
                "path": path_str,
                "valid": is_valid,
            }));
        }
    }

    if opts.stardust_output {
        let output = json!({
            "mode": match mode {
                Mode::Default => "default",
                Mode::Basic => "basic",
                Mode::Extra => "extra",
                Mode::Both => "both",
            },
            "all_valid": res,
            "paths": path_results,
        });
        stardust_output::output(opts, output, || Ok(()))?;
    }

    if !res {
        set_exit_code(1);
    }
    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("pathchk-about"))
        .override_usage(format_usage(&translate!("pathchk-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::POSIX)
                .short('p')
                .help(translate!("pathchk-help-posix"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::POSIX_SPECIAL)
                .short('P')
                .help(translate!("pathchk-help-posix-special"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::PORTABILITY)
                .long(options::PORTABILITY)
                .help(translate!("pathchk-help-portability"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::PATH)
                .hide(true)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::AnyPath)
                .value_parser(clap::value_parser!(OsString))
        );

    stardust_output::add_json_args(cmd)
}

/// check a path, given as a slice of it's components and an operating mode
fn check_path(mode: &Mode, path: &[String]) -> bool {
    match *mode {
        Mode::Basic => check_basic(path),
        Mode::Extra => check_default(path) && check_extra(path),
        Mode::Both => check_basic(path) && check_extra(path),
        Mode::Default => check_default(path),
    }
}

/// check a path in basic compatibility mode
fn check_basic(path: &[String]) -> bool {
    let joined_path = path.join("/");
    let total_len = joined_path.len();
    if total_len > POSIX_PATH_MAX {
        writeln!(
            std::io::stderr(),
            "{}",
            translate!("pathchk-error-posix-path-length-exceeded", "limit" => POSIX_PATH_MAX, "length" => total_len, "path" => joined_path)
        );
        return false;
    } else if total_len == 0 {
        writeln!(
            std::io::stderr(),
            "{}",
            translate!("pathchk-error-empty-file-name")
        );
        return false;
    }
    for p in path {
        let component_len = p.len();
        if component_len > POSIX_NAME_MAX {
            writeln!(
                std::io::stderr(),
                "{}",
                translate!("pathchk-error-posix-name-length-exceeded", "limit" => POSIX_NAME_MAX, "length" => component_len, "component" => p.quote())
            );
            return false;
        }
        if !check_portable_chars(p) {
            return false;
        }
    }
    check_searchable(&joined_path)
}

/// check a path in extra compatibility mode
fn check_extra(path: &[String]) -> bool {
    for p in path {
        if p.starts_with('-') {
            writeln!(
                std::io::stderr(),
                "{}",
                translate!("pathchk-error-leading-hyphen", "component" => p.quote())
            );
            return false;
        }
    }
    if path.join("/").is_empty() {
        writeln!(
            std::io::stderr(),
            "{}",
            translate!("pathchk-error-empty-file-name")
        );
        return false;
    }
    true
}

/// check a path in default mode (using the file system)
fn check_default(path: &[String]) -> bool {
    let joined_path = path.join("/");
    let total_len = joined_path.len();
    if total_len > libc::PATH_MAX as usize {
        writeln!(
            std::io::stderr(),
            "{}",
            translate!("pathchk-error-path-length-exceeded", "limit" => libc::PATH_MAX, "length" => total_len, "path" => joined_path.quote())
        );
        return false;
    }
    if total_len == 0 {
        if fs::symlink_metadata(&joined_path).is_err() {
            writeln!(
                std::io::stderr(),
                "{}",
                translate!("pathchk-error-empty-path-not-found")
            );
            return false;
        }
    }

    for p in path {
        let component_len = p.len();
        if component_len > libc::FILENAME_MAX as usize {
            writeln!(
                std::io::stderr(),
                "{}",
                translate!("pathchk-error-name-length-exceeded", "limit" => libc::FILENAME_MAX, "length" => component_len, "component" => p.quote())
            );
            return false;
        }
    }
    check_searchable(&joined_path)
}

/// check whether a path is or if other problems arise
fn check_searchable(path: &str) -> bool {
    match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                true
            } else {
                writeln!(std::io::stderr(), "{e}");
                false
            }
        }
    }
}

/// check whether a path segment contains only valid (read: portable) characters
fn check_portable_chars(path_segment: &str) -> bool {
    const VALID_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-";
    for (i, ch) in path_segment.as_bytes().iter().enumerate() {
        if !VALID_CHARS.contains(ch) {
            let invalid = path_segment[i..].chars().next().unwrap();
            writeln!(
                std::io::stderr(),
                "{}",
                translate!("pathchk-error-nonportable-character", "character" => invalid, "component" => path_segment.quote())
            );
            return false;
        }
    }
    true
}

