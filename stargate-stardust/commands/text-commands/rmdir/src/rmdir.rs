

use clap::builder::ValueParser;
use clap::{Arg, ArgAction, Command};
use std::ffi::OsString;
use std::fs::{read_dir, remove_dir};
use std::io;
use std::path::Path;
use sgcore::display::Quotable;
use sgcore::error::{SGResult, set_exit_code, strip_errno};
use sgcore::translate;

use sgcore::{format_usage, show_error, util_name};

static OPT_IGNORE_FAIL_NON_EMPTY: &str = "ignore-fail-on-non-empty";
static OPT_PARENTS: &str = "parents";
static OPT_VERBOSE: &str = "verbose";

static ARG_DIRS: &str = "dirs";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "cpath"])?;

    let opts = Opts {
        ignore: matches.get_flag(OPT_IGNORE_FAIL_NON_EMPTY),
        parents: matches.get_flag(OPT_PARENTS),
        verbose: matches.get_flag(OPT_VERBOSE),
    };

    for path in matches
        .get_many::<OsString>(ARG_DIRS)
        .unwrap_or_default()
        .map(Path::new)
    {
        if let Err(error) = remove(path, opts) {
            let Error { error, path } = error;

            if opts.ignore && dir_not_empty(&error, path) {
                continue;
            }

            set_exit_code(1);

            {
                use std::ffi::OsStr;
                use std::os::unix::ffi::OsStrExt;

                fn points_to_directory(path: &Path) -> io::Result<bool> {
                    Ok(path.metadata()?.file_type().is_dir())
                }

                let bytes = path.as_os_str().as_bytes();
                if error.raw_os_error() == Some(libc::ENOTDIR) && bytes.ends_with(b"/") {
                    let no_slash: &Path = OsStr::from_bytes(&bytes[..bytes.len() - 1]).as_ref();
                    if no_slash.is_symlink() && points_to_directory(no_slash).unwrap_or(true) {
                        show_error!(
                            "{}",
                            translate!("rmdir-error-symbolic-link-not-followed", "path" => path.quote())
                        );
                        continue;
                    }
                }
            }

            show_error!(
                "{}",
                translate!("rmdir-error-failed-to-remove", "path" => path.quote(), "err" => strip_errno(&error))
            );
        }
    }

    Ok(())
}

struct Error<'a> {
    error: io::Error,
    path: &'a Path,
}

fn remove(mut path: &Path, opts: Opts) -> Result<(), Error<'_>> {
    remove_single(path, opts)?;
    if opts.parents {
        while let Some(new) = path.parent() {
            path = new;
            if path.as_os_str().is_empty() {
                break;
            }
            remove_single(path, opts)?;
        }
    }
    Ok(())
}

fn remove_single(path: &Path, opts: Opts) -> Result<(), Error<'_>> {
    if opts.verbose {
        println!(
            "{}",
            translate!("rmdir-verbose-removing-directory", "util_name" => util_name(), "path" => path.quote())
        );
    }
    remove_dir(path).map_err(|error| Error { error, path })
}

const NOT_EMPTY_CODES: &[i32] = &[libc::ENOTEMPTY, libc::EEXIST];

const PERHAPS_EMPTY_CODES: &[i32] = &[libc::EACCES, libc::EBUSY, libc::EPERM, libc::EROFS];

fn dir_not_empty(error: &io::Error, path: &Path) -> bool {
    if let Some(code) = error.raw_os_error() {
        if NOT_EMPTY_CODES.contains(&code) {
            return true;
        }
        if PERHAPS_EMPTY_CODES.contains(&code) {
            if let Ok(mut iterator) = read_dir(path) {
                if iterator.next().is_some() {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Clone, Copy, Debug)]
struct Opts {
    ignore: bool,
    parents: bool,
    verbose: bool,
}

pub fn sg_app() -> Command {
    Command::new(util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(util_name()))
        .about(translate!("rmdir-about"))
        .override_usage(format_usage(&translate!("rmdir-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(OPT_IGNORE_FAIL_NON_EMPTY)
                .long(OPT_IGNORE_FAIL_NON_EMPTY)
                .help(translate!("rmdir-help-ignore-fail-non-empty"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_PARENTS)
                .short('p')
                .long(OPT_PARENTS)
                .help(translate!("rmdir-help-parents"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_VERBOSE)
                .short('v')
                .long(OPT_VERBOSE)
                .help(translate!("rmdir-help-verbose"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(ARG_DIRS)
                .action(ArgAction::Append)
                .num_args(1..)
                .required(true)
                .value_parser(ValueParser::os_string())
                .value_hint(clap::ValueHint::DirPath)
        )
}

