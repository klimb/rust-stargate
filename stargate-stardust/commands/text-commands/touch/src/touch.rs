


pub mod error;

use chrono::{
    DateTime, Datelike, Duration, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime,
    TimeZone, Timelike,
};
use clap::builder::{PossibleValue, ValueParser};
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command};
use filetime::{FileTime, set_file_times, set_symlink_file_times};
use jiff::{Timestamp, Zoned};
use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use sgcore::display::Quotable;
use sgcore::error::{FromIo, SGResult, SGSimpleError};
use sgcore::parser::shortcut_value_parser::ShortcutValueParser;
use sgcore::translate;
use sgcore::{format_usage, show};

use crate::error::TouchError;

/// Options contains all the possible behaviors and flags for touch.
///
/// All options are public so that the options can be programmatically
/// constructed by other crates, such as nushell. That means that this struct is
/// part of our public API. It should therefore not be changed without good reason.
///
/// The fields are documented with the arguments that determine their value.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Options {
    /// Do not create any files. Set by `-c`/`--no-create`.
    pub no_create: bool,

    /// Affect each symbolic link instead of any referenced file. Set by `-h`/`--no-dereference`.
    pub no_deref: bool,

    /// Where to get access and modification times from
    pub source: Source,

    /// If given, uses time from `source` but on given date
    pub date: Option<String>,

    /// Whether to change access time only, modification time only, or both
    pub change_times: ChangeTimes,

    /// When true, error when file doesn't exist and either `--no-dereference`
    /// was passed or the file couldn't be created
    pub strict: bool,
}

pub enum InputFile {
    /// A regular file
    Path(PathBuf),
    /// Touch stdout. `--no-dereference` will be ignored in this case.
    Stdout,
}

/// Whether to set access time only, modification time only, or both
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ChangeTimes {
    /// Change only access time
    AtimeOnly,
    /// Change only modification time
    MtimeOnly,
    /// Change both access and modification times
    Both,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Source {
    /// Use access/modification times of given file
    Reference(PathBuf),
    Timestamp(FileTime),
    /// Use current time
    Now,
}

pub mod options {
    pub static SOURCES: &str = "sources";
    pub mod sources {
        pub static DATE: &str = "date";
        pub static REFERENCE: &str = "reference";
        pub static TIMESTAMP: &str = "timestamp";
    }
    pub static HELP: &str = "help";
    pub static ACCESS: &str = "access";
    pub static MODIFICATION: &str = "modification";
    pub static NO_CREATE: &str = "no-create";
    pub static NO_DEREF: &str = "no-dereference";
    pub static TIME: &str = "time";
    pub static FORCE: &str = "force";
}

static ARG_FILES: &str = "files";

mod format {
    pub(crate) const POSIX_LOCALE: &str = "%a %b %e %H:%M:%S %Y";
    pub(crate) const ISO_8601: &str = "%Y-%m-%d";
    pub(crate) const YYYYMMDDHHMM_DOT_SS: &str = "%Y%m%d%H%M.%S";
    pub(crate) const YYYYMMDDHHMMSS: &str = "%Y-%m-%d %H:%M:%S.%f";
    pub(crate) const YYYYMMDDHHMMS: &str = "%Y-%m-%d %H:%M:%S";
    pub(crate) const YYYY_MM_DD_HH_MM: &str = "%Y-%m-%d %H:%M";
    pub(crate) const YYYYMMDDHHMM: &str = "%Y%m%d%H%M";
    pub(crate) const YYYYMMDDHHMM_OFFSET: &str = "%Y-%m-%d %H:%M %z";
}

/// Convert a [`DateTime`] with a TZ offset into a [`FileTime`]
///
/// The [`DateTime`] is converted into a unix timestamp from which the [`FileTime`] is
/// constructed.
fn datetime_to_filetime<T: TimeZone>(dt: &DateTime<T>) -> FileTime {
    FileTime::from_unix_time(dt.timestamp(), dt.timestamp_subsec_nanos())
}

fn filetime_to_datetime(ft: &FileTime) -> Option<DateTime<Local>> {
    Some(DateTime::from_timestamp(ft.unix_seconds(), ft.nanoseconds())?.into())
}

/// Whether all characters in the string are digits.
fn all_digits(s: &str) -> bool {
    s.as_bytes().iter().all(u8::is_ascii_digit)
}

/// Convert a two-digit year string to the corresponding number.
///
/// `s` must be of length two or more. The last two bytes of `s` are
/// assumed to be the two digits of the year.
fn get_year(s: &str) -> u8 {
    let bytes = s.as_bytes();
    let n = bytes.len();
    let y1 = bytes[n - 2] - b'0';
    let y2 = bytes[n - 1] - b'0';
    10 * y1 + y2
}

/// Whether the first filename should be interpreted as a timestamp.
fn is_first_filename_timestamp(
    reference: Option<&OsString>,
    date: Option<&str>,
    timestamp: Option<&str>,
    files: &[&OsString]
) -> bool {
    timestamp.is_none()
        && reference.is_none()
        && date.is_none()
        && files.len() >= 2
        && matches!(std::env::var("_POSIX2_VERSION").as_deref(), Ok("199209"))
        && files[0].to_str().is_some_and(is_timestamp)
}

fn is_timestamp(s: &str) -> bool {
    all_digits(s) && (s.len() == 8 || (s.len() == 10 && (69..=99).contains(&get_year(s))))
}

/// Cycle the last two characters to the beginning of the string.
///
/// `s` must have length at least two.
fn shr2(s: &str) -> String {
    let n = s.len();
    let (a, b) = s.split_at(n - 2);
    let mut result = String::with_capacity(n);
    result.push_str(b);
    result.push_str(a);
    result
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "fattr"])?;

    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;

    let mut filenames: Vec<&OsString> = matches
        .get_many::<OsString>(ARG_FILES)
        .ok_or_else(|| {
            SGSimpleError::new(
                1,
                translate!("touch-error-missing-file-operand", "help_command" => sgcore::execution_phrase().to_string(),)
            )
        })?
        .collect();

    let no_deref = matches.get_flag(options::NO_DEREF);

    let reference = matches.get_one::<OsString>(options::sources::REFERENCE);
    let date = matches
        .get_one::<String>(options::sources::DATE)
        .map(|date| date.to_owned());

    let mut timestamp = matches
        .get_one::<String>(options::sources::TIMESTAMP)
        .map(|t| t.to_owned());

    if is_first_filename_timestamp(reference, date.as_deref(), timestamp.as_deref(), &filenames) {
        let first_file = filenames[0].to_str().unwrap();
        timestamp = if first_file.len() == 10 {
            Some(shr2(first_file))
        } else {
            Some(first_file.to_string())
        };
        filenames = filenames[1..].to_vec();
    }

    let source = if let Some(reference) = reference {
        Source::Reference(PathBuf::from(reference))
    } else if let Some(ts) = timestamp {
        Source::Timestamp(parse_timestamp(&ts)?)
    } else {
        Source::Now
    };

    let files: Vec<InputFile> = filenames
        .into_iter()
        .map(|filename| {
            if filename == "-" {
                InputFile::Stdout
            } else {
                InputFile::Path(PathBuf::from(filename))
            }
        })
        .collect();

    let opts = Options {
        no_create: matches.get_flag(options::NO_CREATE),
        no_deref,
        source,
        date,
        change_times: determine_atime_mtime_change(&matches),
        strict: false,
    };

    touch(&files, &opts)?;

    Ok(())
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("touch-about"))
        .override_usage(format_usage(&translate!("touch-usage")))
        .infer_long_args(true)
        .disable_help_flag(true)
        .arg(
            Arg::new(options::HELP)
                .long(options::HELP)
                .help(translate!("touch-help-help"))
                .action(ArgAction::Help)
        )
        .arg(
            Arg::new(options::ACCESS)
                .short('a')
                .help(translate!("touch-help-access"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::sources::TIMESTAMP)
                .short('t')
                .help(translate!("touch-help-timestamp"))
                .value_name("STAMP")
        )
        .arg(
            Arg::new(options::sources::DATE)
                .short('d')
                .long(options::sources::DATE)
                .allow_hyphen_values(true)
                .help(translate!("touch-help-date"))
                .value_name("STRING")
                .conflicts_with(options::sources::TIMESTAMP)
        )
        .arg(
            Arg::new(options::FORCE)
                .short('f')
                .help("(ignored)")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::MODIFICATION)
                .short('m')
                .help(translate!("touch-help-modification"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::NO_CREATE)
                .short('c')
                .long(options::NO_CREATE)
                .help(translate!("touch-help-no-create"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::NO_DEREF)
                .short('h')
                .long(options::NO_DEREF)
                .help(translate!("touch-help-no-deref"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::sources::REFERENCE)
                .short('r')
                .long(options::sources::REFERENCE)
                .help(translate!("touch-help-reference"))
                .value_name("FILE")
                .value_parser(ValueParser::os_string())
                .value_hint(clap::ValueHint::AnyPath)
                .conflicts_with(options::sources::TIMESTAMP)
        )
        .arg(
            Arg::new(options::TIME)
                .long(options::TIME)
                .help(translate!("touch-help-time"))
                .value_name("WORD")
                .value_parser(ShortcutValueParser::new([
                    PossibleValue::new("atime").alias("access").alias("use"),
                    PossibleValue::new("mtime").alias("modify"),
                ]))
        )
        .arg(
            Arg::new(ARG_FILES)
                .action(ArgAction::Append)
                .num_args(1..)
                .value_parser(clap::value_parser!(OsString))
                .value_hint(clap::ValueHint::AnyPath)
        )
        .group(
            ArgGroup::new(options::SOURCES)
                .args([
                    options::sources::TIMESTAMP,
                    options::sources::DATE,
                    options::sources::REFERENCE,
                ])
                .multiple(true)
        )
}

/// Execute the touch command.
///
/// # Errors
///
/// Possible causes:
/// - The user doesn't have permission to access the file
/// - One of the directory components of the file path doesn't exist.
///
/// It will return an `Err` on the first error. However, for any of the files,
/// if all of the following are true, it will print the error and continue touching
/// the rest of the files.
/// - `opts.strict` is `false`
/// - The file doesn't already exist
/// - `-c`/`--no-create` was passed (`opts.no_create`)
/// - Either `-h`/`--no-dereference` was passed (`opts.no_deref`) or the file couldn't be created
pub fn touch(files: &[InputFile], opts: &Options) -> Result<(), TouchError> {
    let (atime, mtime) = match &opts.source {
        Source::Reference(reference) => {
            let (atime, mtime) = stat(reference, !opts.no_deref)
                .map_err(|e| TouchError::ReferenceFileInaccessible(reference.to_owned(), e))?;

            (atime, mtime)
        }
        Source::Now => {
            let now = datetime_to_filetime(&Local::now());
            (now, now)
        }
        &Source::Timestamp(ts) => (ts, ts),
    };

    let (atime, mtime) = if let Some(date) = &opts.date {
        (
            parse_date(
                filetime_to_datetime(&atime).ok_or_else(|| TouchError::InvalidFiletime(atime))?,
                date
            )?,
            parse_date(
                filetime_to_datetime(&mtime).ok_or_else(|| TouchError::InvalidFiletime(mtime))?,
                date
            )?
        )
    } else {
        (atime, mtime)
    };

    for (ind, file) in files.iter().enumerate() {
        let (path, is_stdout) = match file {
            InputFile::Stdout => (Cow::Owned(pathbuf_from_stdout()?), true),
            InputFile::Path(path) => (Cow::Borrowed(path), false),
        };
        touch_file(&path, is_stdout, opts, atime, mtime).map_err(|e| {
            TouchError::TouchFileError {
                path: path.into_owned(),
                index: ind,
                error: e,
            }
        })?;
    }

    Ok(())
}

/// Create or update the timestamp for a single file.
///
/// # Arguments
///
/// - `path` - The path to the file to create/update timestamp for
/// - `is_stdout` - Stdout is handled specially, see [`update_times`] for more info
/// - `atime` - Access time to set for the file
/// - `mtime` - Modification time to set for the file
fn touch_file(
    path: &Path,
    is_stdout: bool,
    opts: &Options,
    atime: FileTime,
    mtime: FileTime
) -> SGResult<()> {
    let filename = if is_stdout {
        String::from("-")
    } else {
        path.display().to_string()
    };

    let metadata_result = if opts.no_deref {
        path.symlink_metadata()
    } else {
        path.metadata()
    };

    if let Err(e) = metadata_result {
        if e.kind() != ErrorKind::NotFound {
            return Err(e.map_err_context(
                || translate!("touch-error-setting-times-of", "filename" => filename.quote())
            ));
        }

        if opts.no_create {
            return Ok(());
        }

        if opts.no_deref {
            let e = SGSimpleError::new(
                1,
                translate!("touch-error-setting-times-no-such-file", "filename" => filename.quote())
            );
            if opts.strict {
                return Err(e);
            }
            show!(e);
            return Ok(());
        }

        if let Err(e) = File::create(path) {
            let is_directory = if let Some(last_char) = path.to_string_lossy().chars().last() {
                last_char == std::path::MAIN_SEPARATOR
            } else {
                false
            };
            if is_directory {
                let custom_err = Error::other(translate!("touch-error-no-such-file-or-directory"));
                return Err(custom_err.map_err_context(
                    || translate!("touch-error-cannot-touch", "filename" => filename.quote())
                ));
            }
            let e = e.map_err_context(
                || translate!("touch-error-cannot-touch", "filename" => path.quote())
            );
            if opts.strict {
                return Err(e);
            }
            show!(e);
            return Ok(());
        }

        if opts.source == Source::Now && opts.date.is_none() {
            return Ok(());
        }
    }

    update_times(path, is_stdout, opts, atime, mtime)
}

/// Returns which of the times (access, modification) are to be changed.
///
/// Note that "-a" and "-m" may be passed together; this is not an xor.
/// - If `-a` is passed but not `-m`, only access time is changed
/// - If `-m` is passed but not `-a`, only modification time is changed
/// - If neither or both are passed, both times are changed
fn determine_atime_mtime_change(matches: &ArgMatches) -> ChangeTimes {
    let time_access_only = if matches.contains_id(options::TIME) {
        matches
            .get_one::<String>(options::TIME)
            .map(|time| time.contains("access") || time.contains("atime") || time.contains("use"))
    } else {
        None
    };

    let atime_only = matches.get_flag(options::ACCESS) || time_access_only.unwrap_or_default();
    let mtime_only = matches.get_flag(options::MODIFICATION) || !time_access_only.unwrap_or(true);

    if atime_only && !mtime_only {
        ChangeTimes::AtimeOnly
    } else if mtime_only && !atime_only {
        ChangeTimes::MtimeOnly
    } else {
        ChangeTimes::Both
    }
}

/// Updating file access and modification times based on user-specified options
///
/// If the file is not stdout (`!is_stdout`) and `-h`/`--no-dereference` was
/// passed, then, if the given file is a symlink, its own times will be updated,
/// rather than the file it points to.
fn update_times(
    path: &Path,
    is_stdout: bool,
    opts: &Options,
    atime: FileTime,
    mtime: FileTime
) -> SGResult<()> {
    let (atime, mtime) = match opts.change_times {
        ChangeTimes::AtimeOnly => (
            atime,
            stat(path, !opts.no_deref)
                .map_err_context(
                    || translate!("touch-error-failed-to-get-attributes", "path" => path.quote())
                )?
                .1
        ),
        ChangeTimes::MtimeOnly => (
            stat(path, !opts.no_deref)
                .map_err_context(
                    || translate!("touch-error-failed-to-get-attributes", "path" => path.quote())
                )?
                .0,
            mtime
        ),
        ChangeTimes::Both => (atime, mtime),
    };

    if opts.no_deref && !is_stdout {
        set_symlink_file_times(path, atime, mtime)
    } else {
        set_file_times(path, atime, mtime)
    }
    .map_err_context(|| translate!("touch-error-setting-times-of-path", "path" => path.quote()))
}

/// Get metadata of the provided path
/// If `follow` is `true`, the function will try to follow symlinks
/// If `follow` is `false` or the symlink is broken, the function will return metadata of the symlink itself
fn stat(path: &Path, follow: bool) -> std::io::Result<(FileTime, FileTime)> {
    let metadata = if follow {
        fs::metadata(path).or_else(|_| fs::symlink_metadata(path))
    } else {
        fs::symlink_metadata(path)
    }?;

    Ok((
        FileTime::from_last_access_time(&metadata),
        FileTime::from_last_modification_time(&metadata)
    ))
}

fn parse_date(ref_time: DateTime<Local>, s: &str) -> Result<FileTime, TouchError> {

    if let Ok(parsed) = NaiveDateTime::parse_from_str(s, format::POSIX_LOCALE) {
        return Ok(datetime_to_filetime(&parsed.and_utc()));
    }

    for fmt in [
        format::YYYYMMDDHHMMS,
        format::YYYYMMDDHHMMSS,
        format::YYYY_MM_DD_HH_MM,
        format::YYYYMMDDHHMM_OFFSET,
    ] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(datetime_to_filetime(&parsed.and_utc()));
        }
    }

    if let Ok(parsed_date) = NaiveDate::parse_from_str(s, format::ISO_8601) {
        let parsed = Local
            .from_local_datetime(&parsed_date.and_time(NaiveTime::MIN))
            .unwrap();
        return Ok(datetime_to_filetime(&parsed));
    }

    if s.bytes().next() == Some(b'@') {
        if let Ok(ts) = &s[1..].parse::<i64>() {
            return Ok(FileTime::from_unix_time(*ts, 0));
        }
    }

    let ref_zoned = {
        let ts = Timestamp::new(
            ref_time.timestamp(),
            ref_time.timestamp_subsec_nanos() as i32
        )
        .map_err(|_| TouchError::InvalidDateFormat(s.to_owned()))?;
        Zoned::new(ts, jiff::tz::TimeZone::system())
    };

    if let Ok(zoned) = parse_datetime::parse_datetime_at_date(ref_zoned, s) {
        let timestamp = zoned.timestamp();
        let dt =
            DateTime::from_timestamp(timestamp.as_second(), timestamp.subsec_nanosecond() as u32)
                .map(|dt| dt.with_timezone(&Local))
                .ok_or_else(|| TouchError::InvalidDateFormat(s.to_owned()))?;
        return Ok(datetime_to_filetime(&dt));
    }

    Err(TouchError::InvalidDateFormat(s.to_owned()))
}

/// Prepends 19 or 20 to the year if it is a 2 digit year
///
/// GNU `touch` behavior:
///
/// - 68 and before is interpreted as 20xx
/// - 69 and after is interpreted as 19xx
fn prepend_century(s: &str) -> SGResult<String> {
    let first_two_digits = s[..2].parse::<u32>().map_err(|_| {
        SGSimpleError::new(
            1,
            translate!("touch-error-invalid-date-ts-format", "date" => s.quote())
        )
    })?;
    Ok(format!(
        "{}{s}",
        if first_two_digits > 68 { 19 } else { 20 }
    ))
}

/// Parses a timestamp string into a [`FileTime`].
///
/// This function attempts to parse a string into a [`FileTime`]
/// As expected by gnu touch -t : `[[cc]yy]mmddhhmm[.ss]`
///
/// Note that  If the year is specified with only two digits,
/// then cc is 20 for years in the range 0 … 68, and 19 for years in 69 … 99.
/// in order to be compatible with GNU `touch`.
fn parse_timestamp(s: &str) -> SGResult<FileTime> {
    use format::*;

    let current_year = || Local::now().year();

    let (format, ts) = match s.chars().count() {
        15 => (YYYYMMDDHHMM_DOT_SS, s.to_owned()),
        12 => (YYYYMMDDHHMM, s.to_owned()),
        13 => (YYYYMMDDHHMM_DOT_SS, prepend_century(s)?),
        10 => (YYYYMMDDHHMM, prepend_century(s)?),
        11 => (YYYYMMDDHHMM_DOT_SS, format!("{}{s}", current_year())),
        8 => (YYYYMMDDHHMM, format!("{}{s}", current_year())),
        _ => {
            return Err(SGSimpleError::new(
                1,
                translate!("touch-error-invalid-date-format", "date" => s.quote())
            ));
        }
    };

    let local = NaiveDateTime::parse_from_str(&ts, format).map_err(|_| {
        SGSimpleError::new(
            1,
            translate!("touch-error-invalid-date-ts-format", "date" => ts.quote())
        )
    })?;
    let LocalResult::Single(mut local) = Local.from_local_datetime(&local) else {
        return Err(SGSimpleError::new(
            1,
            translate!("touch-error-invalid-date-ts-format", "date" => ts.quote())
        ));
    };

    if local.second() == 59 && ts.ends_with(".60") {
        local += Duration::try_seconds(1).unwrap();
    }

    let local2 = local + Duration::try_hours(1).unwrap() - Duration::try_hours(1).unwrap();
    if local.hour() != local2.hour() {
        return Err(SGSimpleError::new(
            1,
            translate!("touch-error-invalid-date-format", "date" => s.quote())
        ));
    }

    Ok(datetime_to_filetime(&local))
}

/// Returns a [`PathBuf`] to stdout.
///
/// On Windows, uses `GetFinalPathNameByHandleW` to attempt to get the path
/// from the stdout handle.
fn pathbuf_from_stdout() -> Result<PathBuf, TouchError> {
    Ok(PathBuf::from("/dev/stdout"))
}

#[cfg(test)]
mod tests {
    use filetime::FileTime;

    use crate::{
        ChangeTimes, Options, Source, determine_atime_mtime_change, error::TouchError, touch,
        sg_app,
    };

    #[test]
    fn test_determine_atime_mtime_change() {
        assert_eq!(
            ChangeTimes::Both,
            determine_atime_mtime_change(&sg_app().try_get_matches_from(vec!["touch"]).unwrap())
        );
        assert_eq!(
            ChangeTimes::Both,
            determine_atime_mtime_change(
                &sg_app()
                    .try_get_matches_from(vec!["touch", "-a", "-m", "--time", "modify"])
                    .unwrap()
            )
        );
        assert_eq!(
            ChangeTimes::AtimeOnly,
            determine_atime_mtime_change(
                &sg_app()
                    .try_get_matches_from(vec!["touch", "--time", "access"])
                    .unwrap()
            )
        );
        assert_eq!(
            ChangeTimes::MtimeOnly,
            determine_atime_mtime_change(
                &sg_app().try_get_matches_from(vec!["touch", "-m"]).unwrap()
            )
        );
    }

    #[test]
    fn test_invalid_filetime() {
        let invalid_filetime = FileTime::from_unix_time(0, 1_111_111_111);
        match touch(
            &[],
            &Options {
                no_create: false,
                no_deref: false,
                source: Source::Timestamp(invalid_filetime),
                date: Some("yesterday".to_owned()),
                change_times: ChangeTimes::Both,
                strict: false,
            }
        ) {
            Err(TouchError::InvalidFiletime(filetime)) => assert_eq!(filetime, invalid_filetime),
            Err(e) => panic!("Expected TouchError::InvalidFiletime, got {e}"),
            Ok(_) => panic!("Expected to error with TouchError::InvalidFiletime but succeeded"),
        }
    }
}

