

use clap::builder::{TypedValueParser, ValueParserFactory};
use clap::{Arg, ArgAction, ArgMatches, Command};
use sgcore::display::{Quotable, println_verbatim};
use sgcore::error::{FromIo, SGError, SGResult, SGUsageError};
use sgcore::format_usage;
use sgcore::translate;
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::json;

use std::env;
use std::ffi::{OsStr, OsString};
use std::io::ErrorKind;
use std::iter;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};
use std::fs;
use std::os::unix::prelude::PermissionsExt;

use rand::Rng;
use tempfile::Builder;
use thiserror::Error;

static DEFAULT_TEMPLATE: &str = "tmp.XXXXXXXXXX";

static OPT_DIRECTORY: &str = "directory";
static OPT_DRY_RUN: &str = "dry-run";
static OPT_QUIET: &str = "quiet";
static OPT_SUFFIX: &str = "suffix";
static OPT_TMPDIR: &str = "tmpdir";
static OPT_P: &str = "p";
static OPT_T: &str = "t";

static ARG_TEMPLATE: &str = "template";

const TMPDIR_ENV_VAR: &str = "TMPDIR";

#[derive(Error, Debug)]
enum MkTempError {
    #[error("{}", translate!("mktemp-error-persist-file", "path" => .0.quote()))]
    PersistError(PathBuf),

    #[error("{}", translate!("mktemp-error-must-end-in-x", "template" => .0.quote()))]
    MustEndInX(String),

    #[error("{}", translate!("mktemp-error-too-few-xs", "template" => .0.quote()))]
    TooFewXs(String),

    #[error("{}", translate!("mktemp-error-prefix-contains-separator", "template" => .0.quote()))]
    PrefixContainsDirSeparator(String),

    #[error("{}", translate!("mktemp-error-suffix-contains-separator", "suffix" => .0.quote()))]
    SuffixContainsDirSeparator(String),

    #[error("{}", translate!("mktemp-error-invalid-template", "template" => .0.quote()))]
    InvalidTemplate(String),

    #[error("{}", translate!("mktemp-error-too-many-templates"))]
    TooManyTemplates,

    #[error("{}", translate!("mktemp-error-not-found", "template_type" => .0.clone(), "template" => .1.quote()))]
    NotFound(String, String),
}

impl SGError for MkTempError {
    fn usage(&self) -> bool {
        matches!(self, Self::TooManyTemplates)
    }
}

/// Options parsed from the command-line.
///
/// This provides a layer of indirection between the application logic
/// and the argument parsing library `clap`, allowing each to vary
/// independently.
#[derive(Clone)]
pub struct Options {
    /// Whether to create a temporary directory instead of a file.
    pub directory: bool,

    /// Whether to just print the name of a file that would have been created.
    pub dry_run: bool,

    /// Whether to suppress file creation error messages.
    pub quiet: bool,

    /// The directory in which to create the temporary file.
    ///
    /// If `None`, the file will be created in the current directory.
    pub tmpdir: Option<PathBuf>,

    /// The suffix to append to the temporary file, if any.
    pub suffix: Option<String>,

    /// Whether to treat the template argument as a single file path component.
    pub treat_as_template: bool,

    /// The template to use for the name of the temporary file.
    pub template: OsString,
}

impl Options {
    fn from(matches: &ArgMatches) -> Self {
        let tmpdir = matches
            .get_one::<Option<PathBuf>>(OPT_TMPDIR)
            .or_else(|| matches.get_one::<Option<PathBuf>>(OPT_P))
            .map(|dir| match dir {
                Some(d) => d.clone(),
                None => env::var(TMPDIR_ENV_VAR)
                    .ok()
                    .map_or_else(env::temp_dir, PathBuf::from),
            });
        let (tmpdir, template) = match matches.get_one::<OsString>(ARG_TEMPLATE) {
            None => {
                let tmpdir = Some(tmpdir.unwrap_or_else(env::temp_dir));
                let template = DEFAULT_TEMPLATE;
                (tmpdir, OsString::from(template))
            }
            Some(template) => {
                let tmpdir = if env::var(TMPDIR_ENV_VAR).is_ok() && matches.get_flag(OPT_T) {
                    env::var_os(TMPDIR_ENV_VAR).map(|t| t.into())
                } else if tmpdir.is_some() {
                    tmpdir
                } else if matches.get_flag(OPT_T) || matches.contains_id(OPT_TMPDIR) {
                    Some(env::temp_dir())
                } else {
                    None
                };
                (tmpdir, template.clone())
            }
        };
        Self {
            directory: matches.get_flag(OPT_DIRECTORY),
            dry_run: matches.get_flag(OPT_DRY_RUN),
            quiet: matches.get_flag(OPT_QUIET),
            tmpdir,
            suffix: matches.get_one::<String>(OPT_SUFFIX).map(String::from),
            treat_as_template: matches.get_flag(OPT_T),
            template,
        }
    }
}

/// Parameters that control the path to and name of the temporary file.
///
/// The temporary file will be created at
///
/// ```text
/// {directory}/{prefix}{XXX}{suffix}
/// ```
///
/// where `{XXX}` is a sequence of random characters whose length is
/// `num_rand_chars`.
struct Params {
    /// The directory that will contain the temporary file.
    directory: PathBuf,

    /// The (non-random) prefix of the temporary file.
    prefix: String,

    /// The number of random characters in the name of the temporary file.
    num_rand_chars: usize,

    /// The (non-random) suffix of the temporary file.
    suffix: String,
}

/// Find the start and end indices of the last contiguous block of Xs.
///
/// If no contiguous block of at least three Xs could be found, this
/// function returns `None`.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(find_last_contiguous_block_of_xs("XXX_XXX"), Some((4, 7)));
/// assert_eq!(find_last_contiguous_block_of_xs("aXbXcX"), None);
/// ```
fn find_last_contiguous_block_of_xs(s: &str) -> Option<(usize, usize)> {
    let j = s.rfind("XXX")? + 3;
    let i = s[..j].rfind(|c| c != 'X').map_or(0, |i| i + 1);
    Some((i, j))
}

impl Params {
    fn from(options: Options) -> Result<Self, MkTempError> {
        let Some(template_str) = options.template.to_str() else {
            return Err(MkTempError::InvalidTemplate(
                options.template.to_string_lossy().into_owned()
            ));
        };

        if options.suffix.is_some() && !template_str.ends_with('X') {
            return Err(MkTempError::MustEndInX(template_str.to_string()));
        }

        let Some((i, j)) = find_last_contiguous_block_of_xs(template_str) else {
            let s = match options.suffix {
                Some(_) => template_str
                    .chars()
                    .take(template_str.len())
                    .collect::<String>(),
                None => template_str.to_string(),
            };
            return Err(MkTempError::TooFewXs(s));
        };

        let tmpdir = options.tmpdir;
        let prefix_from_option = tmpdir.clone().unwrap_or_default();
        let prefix_from_template = &template_str[..i];
        let prefix_path = Path::new(&prefix_from_option).join(prefix_from_template);
        if options.treat_as_template && prefix_from_template.contains(MAIN_SEPARATOR) {
            return Err(MkTempError::PrefixContainsDirSeparator(
                template_str.to_string()
            ));
        }
        if tmpdir.is_some() && Path::new(prefix_from_template).is_absolute() {
            return Err(MkTempError::InvalidTemplate(template_str.to_string()));
        }

        let (directory, prefix) = {
            let prefix_str = prefix_path.to_string_lossy();
            if prefix_str.ends_with(MAIN_SEPARATOR) {
                (prefix_path, String::new())
            } else {
                let directory = match prefix_path.parent() {
                    None => PathBuf::new(),
                    Some(d) => d.to_path_buf(),
                };
                let prefix = match prefix_path.file_name() {
                    None => String::new(),
                    Some(f) => f.to_str().unwrap().to_string(),
                };
                (directory, prefix)
            }
        };

        let suffix_from_option = options.suffix.unwrap_or_default();
        let suffix_from_template = &template_str[j..];
        let suffix = format!("{suffix_from_template}{suffix_from_option}");
        if suffix.contains(MAIN_SEPARATOR) {
            return Err(MkTempError::SuffixContainsDirSeparator(suffix));
        }

        let num_rand_chars = j - i;

        Ok(Self {
            directory,
            prefix,
            num_rand_chars,
            suffix,
        })
    }
}

/// Custom parser that converts empty string to `None`, and non-empty string to
/// `Some(PathBuf)`.
///
/// This parser is used for the `-p` and `--tmpdir` options where an empty string
/// argument should be treated as "not provided", causing mktemp to fall back to
/// using the `$TMPDIR` environment variable or the system's default temporary
/// directory.
///
/// # Examples
///
/// - Empty string `""` -> `None`
/// - Non-empty string `"/tmp"` -> `Some(PathBuf::from("/tmp"))`
///
/// This handles the special case where users can pass an empty directory name
/// to explicitly request fallback behavior.
#[derive(Clone, Debug)]
struct OptionalPathBufParser;

impl TypedValueParser for OptionalPathBufParser {
    type Value = Option<PathBuf>;

    fn parse_ref(
        &self,
        _cmd: &Command,
        _arg: Option<&Arg>,
        value: &OsStr
    ) -> Result<Self::Value, clap::Error> {
        if value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PathBuf::from(value)))
        }
    }
}

impl ValueParserFactory for OptionalPathBufParser {
    type Parser = Self;

    fn value_parser() -> Self::Parser {
        Self
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let args: Vec<_> = args.collect();
    let matches = match sg_app().try_get_matches_from(&args) {
        Ok(m) => m,
        Err(e) => {
            use sgcore::clap_localization::handle_clap_error_with_exit_code;
            if e.kind() == clap::error::ErrorKind::UnknownArgument {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath"])?;
                handle_clap_error_with_exit_code(e, 1);
            }
            if e.kind() == clap::error::ErrorKind::TooManyValues
                && e.context().any(|(kind, val)| {
                    kind == clap::error::ContextKind::InvalidArg
                        && val == &clap::error::ContextValue::String("[template]".into())
                })
            {
                return Err(SGUsageError::new(
                    1,
                    translate!("mktemp-error-too-many-templates")
                ));
            }
            return Err(e.into());
        }
    };

    let opts = StardustOutputOptions::from_matches(&matches);
    let mut opts = opts;
    if !matches.contains_id("stardust_output") {
        opts.stardust_output = true;
    }

    let options = Options::from(&matches);

    if env::var("POSIXLY_CORRECT").is_ok() {
        if matches.contains_id(ARG_TEMPLATE) {
            if args.last().unwrap() != &options.template {
                return Err(Box::new(MkTempError::TooManyTemplates));
            }
        }
    }

    let dry_run = options.dry_run;
    let suppress_file_err = options.quiet;
    let make_dir = options.directory;

    let Params {
        directory: tmpdir,
        prefix,
        num_rand_chars: rand,
        suffix,
    } = Params::from(options)?;

    let res = if dry_run {
        dry_exec(&tmpdir, &prefix, rand, &suffix)
    } else {
        exec(&tmpdir, &prefix, rand, &suffix, make_dir)
    };

    let res = if suppress_file_err {
        res.map_err(|e| e.code().into())
    } else {
        res
    };

    let path = res?;

    if opts.stardust_output {
        let output = json!({
            "path": path.to_string_lossy(),
            "type": if make_dir { "directory" } else { "file" },
            "dry_run": dry_run,
            "absolute": path.is_absolute(),
        });
        stardust_output::output(opts, output, || Ok(()))?;
    } else {
        println_verbatim(&path).map_err_context(|| translate!("mktemp-error-failed-print"))?;
    }

    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("mktemp-about"))
        .override_usage(format_usage(&translate!("mktemp-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(OPT_DIRECTORY)
                .short('d')
                .long(OPT_DIRECTORY)
                .help(translate!("mktemp-help-directory"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_DRY_RUN)
                .short('u')
                .long(OPT_DRY_RUN)
                .help(translate!("mktemp-help-dry-run"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_QUIET)
                .short('q')
                .long("quiet")
                .help(translate!("mktemp-help-quiet"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_SUFFIX)
                .long(OPT_SUFFIX)
                .help(translate!("mktemp-help-suffix"))
                .value_name("SUFFIX")
        )
        .arg(
            Arg::new(OPT_P)
                .short('p')
                .help(translate!("mktemp-help-p"))
                .value_name("DIR")
                .num_args(1)
                .value_parser(OptionalPathBufParser)
                .value_hint(clap::ValueHint::DirPath)
        )
        .arg(
            Arg::new(OPT_TMPDIR)
                .long(OPT_TMPDIR)
                .help(translate!("mktemp-help-tmpdir"))
                .value_name("DIR")
                .num_args(0..=1)
                .require_equals(true)
                .overrides_with(OPT_P)
                .value_parser(OptionalPathBufParser)
                .value_hint(clap::ValueHint::DirPath)
        )
        .arg(
            Arg::new(OPT_T)
                .short('t')
                .help(translate!("mktemp-help-t"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(ARG_TEMPLATE)
                .num_args(..=1)
                .value_parser(clap::value_parser!(OsString))
        );

    stardust_output::add_json_args(cmd)
}

fn dry_exec(tmpdir: &Path, prefix: &str, rand: usize, suffix: &str) -> SGResult<PathBuf> {
    let len = prefix.len() + suffix.len() + rand;
    let mut buf = Vec::with_capacity(len);
    buf.extend(prefix.as_bytes());
    buf.extend(iter::repeat_n(b'X', rand));
    buf.extend(suffix.as_bytes());

    let bytes = &mut buf[prefix.len()..prefix.len() + rand];
    rand::rng().fill(bytes);
    for byte in bytes {
        *byte = match *byte % 62 {
            v @ 0..=9 => v + b'0',
            v @ 10..=35 => v - 10 + b'a',
            v @ 36..=61 => v - 36 + b'A',
            _ => unreachable!(),
        }
    }
    let buf = String::from_utf8(buf).unwrap();
    let tmpdir = Path::new(tmpdir).join(buf);
    Ok(tmpdir)
}

/// Create a temporary directory with the given parameters.
///
/// This function creates a temporary directory as a subdirectory of
/// `dir`. The name of the directory is the concatenation of `prefix`,
/// a string of `rand` random characters, and `suffix`. The
/// permissions of the directory are set to `u+rwx`
///
/// # Errors
///
/// If the temporary directory could not be written to disk or if the
/// given directory `dir` does not exist.
fn make_temp_dir(dir: &Path, prefix: &str, rand: usize, suffix: &str) -> SGResult<PathBuf> {
    let mut builder = Builder::new();
    builder.prefix(prefix).rand_bytes(rand).suffix(suffix);

    builder.permissions(fs::Permissions::from_mode(0o700));

    match builder.tempdir_in(dir) {
        Ok(d) => {
            let path = d.keep();
            Ok(path)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let filename = format!("{prefix}{}{suffix}", "X".repeat(rand));
            let path = Path::new(dir).join(filename);
            let s = path.display().to_string();
            Err(MkTempError::NotFound(translate!("mktemp-template-type-directory"), s).into())
        }
        Err(e) => Err(e.into()),
    }
}

/// Create a temporary file with the given parameters.
///
/// This function creates a temporary file in the directory `dir`. The
/// name of the file is the concatenation of `prefix`, a string of
/// `rand` random characters, and `suffix`. The permissions of the
/// file are set to `u+rw`.
///
/// # Errors
///
/// If the file could not be written to disk or if the directory does
/// not exist.
fn make_temp_file(dir: &Path, prefix: &str, rand: usize, suffix: &str) -> SGResult<PathBuf> {
    let mut builder = Builder::new();
    builder.prefix(prefix).rand_bytes(rand).suffix(suffix);
    match builder.tempfile_in(dir) {
        Ok(named_tempfile) => match named_tempfile.keep() {
            Ok((_, pathbuf)) => Ok(pathbuf),
            Err(e) => Err(MkTempError::PersistError(e.file.path().to_path_buf()).into()),
        },
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let filename = format!("{prefix}{}{suffix}", "X".repeat(rand));
            let path = Path::new(dir).join(filename);
            let s = path.display().to_string();
            Err(MkTempError::NotFound(translate!("mktemp-template-type-file"), s).into())
        }
        Err(e) => Err(e.into()),
    }
}

fn exec(dir: &Path, prefix: &str, rand: usize, suffix: &str, make_dir: bool) -> SGResult<PathBuf> {
    let path = if make_dir {
        make_temp_dir(dir, prefix, rand, suffix)?
    } else {
        make_temp_file(dir, prefix, rand, suffix)?
    };

    let filename = path.file_name();
    let filename = filename.unwrap().to_str().unwrap();

    let path = Path::new(dir).join(filename);

    Ok(path)
}

/// Create a temporary file or directory
///
/// Behavior is determined by the `options` parameter, see [`Options`] for details.
pub fn mktemp(options: &Options) -> SGResult<PathBuf> {
    let Params {
        directory: tmpdir,
        prefix,
        num_rand_chars: rand,
        suffix,
    } = Params::from(options.clone())?;

    if options.dry_run {
        dry_exec(&tmpdir, &prefix, rand, &suffix)
    } else {
        exec(&tmpdir, &prefix, rand, &suffix, options.directory)
    }
}

#[cfg(test)]
mod tests {
    use crate::find_last_contiguous_block_of_xs as findxs;

    #[test]
    fn test_find_last_contiguous_block_of_xs() {
        assert_eq!(findxs("XXX"), Some((0, 3)));
        assert_eq!(findxs("XXX_XXX"), Some((4, 7)));
        assert_eq!(findxs("XXX_XXX_XXX"), Some((8, 11)));
        assert_eq!(findxs("aaXXXbb"), Some((2, 5)));
        assert_eq!(findxs(""), None);
        assert_eq!(findxs("X"), None);
        assert_eq!(findxs("XX"), None);
        assert_eq!(findxs("aXbXcX"), None);
        assert_eq!(findxs("aXXbXXcXX"), None);
    }
}

