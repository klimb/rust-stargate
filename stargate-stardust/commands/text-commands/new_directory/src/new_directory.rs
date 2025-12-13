

use clap::builder::ValueParser;
use clap::parser::ValuesRef;
use clap::{Arg, ArgAction, ArgMatches, Command};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use sgcore::error::FromIo;
use sgcore::error::{SGResult, SGSimpleError};
use sgcore::translate;

use sgcore::mode;
use sgcore::{display::Quotable, fs::dir_strip_dot_for_creation};
use sgcore::{format_usage, show_if_err};

static DEFAULT_PERM: u32 = 0o777;

mod options {
    pub const MODE: &str = "mode";
    pub const PARENTS: &str = "parents";
    pub const VERBOSE: &str = "verbose";
    pub const DIRS: &str = "dirs";
}

/// Configuration for directory creation.
pub struct Config<> {
    /// Create parent directories as needed.
    pub recursive: bool,

    /// File permissions (octal).
    pub mode: u32,

    /// Print message for each created directory.
    pub verbose: bool,
}

fn get_mode(matches: &ArgMatches) -> Result<u32, String> {
    let mut new_mode = DEFAULT_PERM;

    if let Some(m) = matches.get_one::<String>(options::MODE) {
        for mode in m.split(',') {
            if mode.chars().any(|c| c.is_ascii_digit()) {
                new_mode = mode::parse_numeric(new_mode, m, true)?;
            } else {
                new_mode = mode::parse_symbolic(new_mode, mode, mode::get_umask(), true)?;
            }
        }
        Ok(new_mode)
    } else {
        Ok(!mode::get_umask() & 0o0777)
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath"])?;

    let dirs = matches
        .get_many::<OsString>(options::DIRS)
        .unwrap_or_default();
    let verbose = matches.get_flag(options::VERBOSE);
    let recursive = matches.get_flag(options::PARENTS);

    match get_mode(&matches) {
        Ok(mode) => {
            let config = Config {
                recursive,
                mode,
                verbose,
            };
            exec(dirs, &config)
        }
        Err(f) => Err(SGSimpleError::new(1, f)),
    }
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("mkdir-about"))
        .override_usage(format_usage(&translate!("mkdir-usage")))
        .infer_long_args(true)
        .after_help(translate!("mkdir-after-help"))
        .arg(
            Arg::new(options::MODE)
                .short('m')
                .long(options::MODE)
                .help(translate!("mkdir-help-mode"))
                .allow_hyphen_values(true)
                .num_args(1)
        )
        .arg(
            Arg::new(options::PARENTS)
                .short('p')
                .long(options::PARENTS)
                .help(translate!("mkdir-help-parents"))
                .overrides_with(options::PARENTS)
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::VERBOSE)
                .short('v')
                .long(options::VERBOSE)
                .help(translate!("mkdir-help-verbose"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::DIRS)
                .action(ArgAction::Append)
                .num_args(1..)
                .required(true)
                .value_parser(ValueParser::os_string())
                .value_hint(clap::ValueHint::DirPath)
        )
}

fn exec(dirs: ValuesRef<OsString>, config: &Config) -> SGResult<()> {
    for dir in dirs {
        let path_buf = PathBuf::from(dir);
        let path = path_buf.as_path();

        show_if_err!(mkdir(path, config));
    }
    Ok(())
}

/// Create directory at a given `path`.
///
/// ## Options
///
/// * `recursive` --- create parent directories for the `path`, if they do not
///   exist.
/// * `mode` --- file mode for the directories (not implemented on windows).
/// * `verbose` --- print a message for each printed directory.
///
/// ## Trailing dot
///
/// To match the GNU behavior, a path with the last directory being a single dot
/// (like `some/path/to/.`) is created (with the dot stripped).
pub fn mkdir(path: &Path, config: &Config) -> SGResult<()> {
    if path.as_os_str().is_empty() {
        return Err(SGSimpleError::new(
            1,
            translate!("mkdir-error-empty-directory-name")
        ));
    }
    let path_buf = dir_strip_dot_for_creation(path);
    let path = path_buf.as_path();
    create_dir(path, false, config)
}
fn chmod(path: &Path, mode: u32) -> SGResult<()> {
    use std::fs::{Permissions, set_permissions};
    use std::os::unix::fs::PermissionsExt;
    let mode = Permissions::from_mode(mode);
    set_permissions(path, mode).map_err_context(
        || translate!("mkdir-error-cannot-set-permissions", "path" => path.quote())
    )
}

fn create_dir(path: &Path, is_parent: bool, config: &Config) -> SGResult<()> {
    let path_exists = path.exists();
    if path_exists && !config.recursive {
        return Err(SGSimpleError::new(
            1,
            translate!("mkdir-error-file-exists", "path" => path.to_string_lossy())
        ));
    }
    if path == Path::new("") {
        return Ok(());
    }

    if config.recursive {
        let mut dirs_to_create = Vec::with_capacity(16);
        let mut current = path;

        while let Some(parent) = current.parent() {
            if parent == Path::new("") {
                break;
            }
            dirs_to_create.push(parent);
            current = parent;
        }

        for dir in dirs_to_create.iter().rev() {
            if !dir.exists() {
                create_single_dir(dir, true, config)?;
            }
        }
    }

    create_single_dir(path, is_parent, config)
}

#[allow(unused_variables)]
fn create_single_dir(path: &Path, is_parent: bool, config: &Config) -> SGResult<()> {
    let path_exists = path.exists();

    match std::fs::create_dir(path) {
        Ok(()) => {
            if config.verbose {
                println!(
                    "{}",
                    translate!("mkdir-verbose-created-directory", "util_name" => sgcore::util_name(), "path" => path.quote())
                );
            }

            #[cfg(all(unix, target_os = "linux"))]
            let new_mode = if path_exists {
                config.mode
            } else {
                let acl_perm_bits = sgcore::fsxattr::get_acl_perm_bits_from_xattr(path);

                if is_parent {
                    (!mode::get_umask() & 0o777) | 0o300 | acl_perm_bits
                } else {
                    config.mode | acl_perm_bits
                }
            };
            #[cfg(all(unix, not(target_os = "linux")))]
            let new_mode = if is_parent {
                (!mode::get_umask() & 0o777) | 0o300
            } else {
                config.mode
            };

            chmod(path, new_mode)?;

            Ok(())
        }

        Err(_) if path.is_dir() => {
            let ends_with_parent_dir = matches!(
                path.components().next_back(),
                Some(std::path::Component::ParentDir)
            );

            if config.verbose && is_parent && config.recursive && !ends_with_parent_dir {
                println!(
                    "{}",
                    translate!("mkdir-verbose-created-directory", "util_name" => sgcore::util_name(), "path" => path.quote())
                );
            }
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

