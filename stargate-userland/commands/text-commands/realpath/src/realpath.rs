// spell-checker:ignore (ToDO) retcode

use clap::{
    Arg, ArgAction, ArgMatches, Command,
    builder::{TypedValueParser, ValueParserFactory},
};
use serde::Serialize;
use sgcore::fs::make_path_relative_to;
use sgcore::object_output::{self, JsonOutputOptions};
use sgcore::translate;
use sgcore::{
    display::{Quotable, print_verbatim},
    error::{FromIo, UResult},
    format_usage,
    fs::{MissingHandling, ResolveMode, canonicalize},
    line_ending::LineEnding,
    show_if_err,
};
use std::{
    ffi::{OsStr, OsString},
    io::{Write, stdout},
    path::{Path, PathBuf},
};

#[derive(Serialize)]
struct PathResult {
    input: String,
    output: Option<String>,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct RealpathOutput {
    paths: Vec<PathResult>,
}

const OPT_QUIET: &str = "quiet";
const OPT_STRIP: &str = "strip";
const OPT_ZERO: &str = "zero";
const OPT_PHYSICAL: &str = "physical";
const OPT_LOGICAL: &str = "logical";
const OPT_CANONICALIZE_MISSING: &str = "canonicalize-missing";
const OPT_CANONICALIZE: &str = "canonicalize";
const OPT_CANONICALIZE_EXISTING: &str = "canonicalize-existing";
const OPT_RELATIVE_TO: &str = "relative-to";
const OPT_RELATIVE_BASE: &str = "relative-base";

const ARG_FILES: &str = "files";

/// Custom parser that validates `OsString` is not empty
#[derive(Clone, Debug)]
struct NonEmptyOsStringParser;

impl TypedValueParser for NonEmptyOsStringParser {
    type Value = OsString;

    fn parse_ref(
        &self,
        _cmd: &Command,
        _arg: Option<&Arg>,
        value: &OsStr
    ) -> Result<Self::Value, clap::Error> {
        if value.is_empty() {
            let mut err = clap::Error::new(clap::error::ErrorKind::ValueValidation);
            err.insert(
                clap::error::ContextKind::Custom,
                clap::error::ContextValue::String(translate!("realpath-invalid-empty-operand"))
            );
            return Err(err);
        }
        Ok(value.to_os_string())
    }
}

impl ValueParserFactory for NonEmptyOsStringParser {
    type Parser = Self;

    fn value_parser() -> Self::Parser {
        Self
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath"])?;

    let json_output_options = JsonOutputOptions::from_matches(&matches);

    /*  the list of files */

    let paths: Vec<PathBuf> = matches
        .get_many::<OsString>(ARG_FILES)
        .unwrap()
        .map(PathBuf::from)
        .collect();

    let strip = matches.get_flag(OPT_STRIP);
    let line_ending = LineEnding::from_zero_flag(matches.get_flag(OPT_ZERO));
    let quiet = matches.get_flag(OPT_QUIET);
    let logical = matches.get_flag(OPT_LOGICAL);
    let can_mode = if matches.get_flag(OPT_CANONICALIZE_MISSING) {
        MissingHandling::Missing
    } else if matches.get_flag(OPT_CANONICALIZE_EXISTING) {
        // -e: all components must exist
        // Despite the name, MissingHandling::Existing requires all components to exist
        MissingHandling::Existing
    } else {
        // Default behavior (same as -E): all but last component must exist
        // MissingHandling::Normal allows the final component to not exist
        MissingHandling::Normal
    };
    let resolve_mode = if strip {
        ResolveMode::None
    } else if logical {
        ResolveMode::Logical
    } else {
        ResolveMode::Physical
    };
    let (relative_to, relative_base) = prepare_relative_options(&matches, can_mode, resolve_mode)?;
    
    if json_output_options.object_output {
        return realpath_json(
            &paths,
            resolve_mode,
            can_mode,
            relative_to.as_deref(),
            relative_base.as_deref(),
            &json_output_options
        );
    }
    
    for path in &paths {
        let result = resolve_path(
            path,
            line_ending,
            resolve_mode,
            can_mode,
            relative_to.as_deref(),
            relative_base.as_deref()
        );
        if !quiet {
            show_if_err!(result.map_err_context(|| path.maybe_quote().to_string()));
        }
    }
    // Although we return `Ok`, it is possible that a call to
    // `show!()` above has set the exit code for the program to a
    // non-zero integer.
    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("realpath-about"))
        .override_usage(format_usage(&translate!("realpath-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(OPT_QUIET)
                .short('q')
                .long(OPT_QUIET)
                .help(translate!("realpath-help-quiet"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_STRIP)
                .short('s')
                .long(OPT_STRIP)
                .visible_alias("no-symlinks")
                .help(translate!("realpath-help-strip"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_ZERO)
                .short('z')
                .long(OPT_ZERO)
                .help(translate!("realpath-help-zero"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_LOGICAL)
                .short('L')
                .long(OPT_LOGICAL)
                .help(translate!("realpath-help-logical"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_PHYSICAL)
                .short('P')
                .long(OPT_PHYSICAL)
                .overrides_with_all([OPT_STRIP, OPT_LOGICAL])
                .help(translate!("realpath-help-physical"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_CANONICALIZE)
                .short('E')
                .long(OPT_CANONICALIZE)
                .overrides_with_all([OPT_CANONICALIZE_EXISTING, OPT_CANONICALIZE_MISSING])
                .help(translate!("realpath-help-canonicalize"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_CANONICALIZE_EXISTING)
                .short('e')
                .long(OPT_CANONICALIZE_EXISTING)
                .overrides_with_all([OPT_CANONICALIZE, OPT_CANONICALIZE_MISSING])
                .help(translate!("realpath-help-canonicalize-existing"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_CANONICALIZE_MISSING)
                .short('m')
                .long(OPT_CANONICALIZE_MISSING)
                .overrides_with_all([OPT_CANONICALIZE, OPT_CANONICALIZE_EXISTING])
                .help(translate!("realpath-help-canonicalize-missing"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_RELATIVE_TO)
                .long(OPT_RELATIVE_TO)
                .value_name("DIR")
                .value_parser(NonEmptyOsStringParser)
                .help(translate!("realpath-help-relative-to"))
        )
        .arg(
            Arg::new(OPT_RELATIVE_BASE)
                .long(OPT_RELATIVE_BASE)
                .value_name("DIR")
                .value_parser(NonEmptyOsStringParser)
                .help(translate!("realpath-help-relative-base"))
        )
        .arg(
            Arg::new(ARG_FILES)
                .action(ArgAction::Append)
                .required(true)
                .value_parser(NonEmptyOsStringParser)
                .value_hint(clap::ValueHint::AnyPath)
        );
    object_output::add_json_args(cmd)
}

/// Prepare `--relative-to` and `--relative-base` options.
/// Convert them to their absolute values.
/// Check if `--relative-to` is a descendant of `--relative-base`,
/// otherwise nullify their value.
fn prepare_relative_options(
    matches: &ArgMatches,
    can_mode: MissingHandling,
    resolve_mode: ResolveMode
) -> UResult<(Option<PathBuf>, Option<PathBuf>)> {
    let relative_to = matches
        .get_one::<OsString>(OPT_RELATIVE_TO)
        .map(PathBuf::from);
    let relative_base = matches
        .get_one::<OsString>(OPT_RELATIVE_BASE)
        .map(PathBuf::from);
    let relative_to = canonicalize_relative_option(relative_to, can_mode, resolve_mode)?;
    let relative_base = canonicalize_relative_option(relative_base, can_mode, resolve_mode)?;
    if let (Some(base), Some(to)) = (relative_base.as_deref(), relative_to.as_deref()) {
        if !to.starts_with(base) {
            return Ok((None, None));
        }
    }
    Ok((relative_to, relative_base))
}

fn realpath_json(
    paths: &[PathBuf],
    resolve_mode: ResolveMode,
    can_mode: MissingHandling,
    relative_to: Option<&Path>,
    relative_base: Option<&Path>,
    json_output_options: &JsonOutputOptions
) -> UResult<()> {
    let mut results = Vec::new();
    
    for path in paths {
        let result = match canonicalize(path, can_mode, resolve_mode) {
            Ok(abs) => {
                let final_path = process_relative(abs, relative_base, relative_to);
                PathResult {
                    input: path.display().to_string(),
                    output: Some(final_path.display().to_string()),
                    success: true,
                    error: None,
                }
            }
            Err(e) => PathResult {
                input: path.display().to_string(),
                output: None,
                success: false,
                error: Some(e.to_string()),
            },
        };
        results.push(result);
    }
    
    let output = RealpathOutput { paths: results };
    let json_value = serde_json::to_value(&output)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    object_output::output(*json_output_options, json_value, || Ok(()))?;
    Ok(())
}

/// Prepare single `relative-*` option.
fn canonicalize_relative_option(
    relative: Option<PathBuf>,
    can_mode: MissingHandling,
    resolve_mode: ResolveMode
) -> UResult<Option<PathBuf>> {
    Ok(match relative {
        None => None,
        Some(p) => Some(
            canonicalize_relative(&p, can_mode, resolve_mode)
                .map_err_context(|| p.maybe_quote().to_string())?
        ),
    })
}

/// Make `relative-to` or `relative-base` path values absolute.
///
/// # Errors
///
/// If the given path is not a directory the function returns an error.
/// If some parts of the file don't exist, or symlinks make loops, or
/// some other IO error happens, the function returns error, too.
fn canonicalize_relative(
    r: &Path,
    can_mode: MissingHandling,
    resolve: ResolveMode
) -> std::io::Result<PathBuf> {
    let abs = canonicalize(r, can_mode, resolve)?;
    if can_mode == MissingHandling::Existing && !abs.is_dir() {
        abs.read_dir()?; // raise not a directory error
    }
    Ok(abs)
}

/// Resolve a path to an absolute form and print it.
///
/// If `relative_to` and/or `relative_base` is given
/// the path is printed in a relative form to one of this options.
/// See the details in `process_relative` function.
/// If `zero` is `true`, then this function
/// prints the path followed by the null byte (`'\0'`) instead of a
/// newline character (`'\n'`).
///
/// # Errors
///
/// This function returns an error if there is a problem resolving
/// symbolic links.
fn resolve_path(
    p: &Path,
    line_ending: LineEnding,
    resolve: ResolveMode,
    can_mode: MissingHandling,
    relative_to: Option<&Path>,
    relative_base: Option<&Path>
) -> std::io::Result<()> {
    let abs = canonicalize(p, can_mode, resolve)?;

    let abs = process_relative(abs, relative_base, relative_to);

    print_verbatim(abs)?;
    stdout().write_all(&[line_ending.into()])?;
    Ok(())
}

/// Conditionally converts an absolute path to a relative form,
/// according to the rules:
/// 1. if only `relative_to` is given, the result is relative to `relative_to`
/// 2. if only `relative_base` is given, it checks whether given `path` is a descendant
///    of `relative_base`, on success the result is relative to `relative_base`, otherwise
///    the result is the given `path`
/// 3. if both `relative_to` and `relative_base` are given, the result is relative to `relative_to`
///    if `path` is a descendant of `relative_base`, otherwise the result is `path`
///
/// For more information see
/// <https://www.gnu.org/software/coreutils/manual/html_node/Realpath-usage-examples.html>
fn process_relative(
    path: PathBuf,
    relative_base: Option<&Path>,
    relative_to: Option<&Path>
) -> PathBuf {
    if let Some(base) = relative_base {
        if path.starts_with(base) {
            make_path_relative_to(path, relative_to.unwrap_or(base))
        } else {
            path
        }
    } else if let Some(to) = relative_to {
        make_path_relative_to(path, to)
    } else {
        path
    }
}
