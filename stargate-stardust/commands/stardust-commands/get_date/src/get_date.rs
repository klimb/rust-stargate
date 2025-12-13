

use clap::{Arg, ArgAction, Command};
use jiff::fmt::strtime;
use jiff::tz::{TimeZone, TimeZoneDatabase};
use jiff::{Timestamp, Zoned};
#[cfg(all(unix, not(target_os = "macos")))]
use libc::clock_settime;
use libc::{CLOCK_REALTIME, clock_getres, timespec};
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::OnceLock;
use sgcore::error::FromIo;
use sgcore::error::{SGResult, SGSimpleError};
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::translate;
use sgcore::{format_usage, show};
use sgcore::parser::shortcut_value_parser::ShortcutValueParser;

const DATE: &str = "date";
const HOURS: &str = "hours";
const MINUTES: &str = "minutes";
const SECONDS: &str = "seconds";
const NS: &str = "ns";

const OPT_DATE: &str = "date";
const OPT_FORMAT: &str = "format";
const OPT_FILE: &str = "file";
const OPT_DEBUG: &str = "debug";
const OPT_ISO_8601: &str = "iso-8601";
const OPT_RESOLUTION: &str = "resolution";
const OPT_RFC_EMAIL: &str = "rfc-email";
const OPT_RFC_822: &str = "rfc-822";
const OPT_RFC_2822: &str = "rfc-2822";
const OPT_RFC_3339: &str = "rfc-3339";
const OPT_SET: &str = "set";
const OPT_REFERENCE: &str = "reference";
const OPT_UNIVERSAL: &str = "universal";
const OPT_UNIVERSAL_2: &str = "utc";

/// Settings for this program, parsed from the command line
struct Settings {
    utc: bool,
    format: Format,
    date_source: DateSource,
    set_to: Option<Zoned>,
}

/// Various ways of displaying the date
enum Format {
    Iso8601(Iso8601Format),
    Rfc5322,
    Rfc3339(Rfc3339Format),
    Resolution,
    Custom(String),
    Default,
}

/// Various places that dates can come from
enum DateSource {
    Now,
    File(PathBuf),
    FileMtime(PathBuf),
    Stdin,
    Human(String),
    Resolution,
}

enum Iso8601Format {
    Date,
    Hours,
    Minutes,
    Seconds,
    Ns,
}

impl From<&str> for Iso8601Format {
    fn from(s: &str) -> Self {
        match s {
            HOURS => Self::Hours,
            MINUTES => Self::Minutes,
            SECONDS => Self::Seconds,
            NS => Self::Ns,
            DATE => Self::Date,
            _ => unreachable!(),
        }
    }
}

enum Rfc3339Format {
    Date,
    Seconds,
    Ns,
}

impl From<&str> for Rfc3339Format {
    fn from(s: &str) -> Self {
        match s {
            DATE => Self::Date,
            SECONDS => Self::Seconds,
            NS => Self::Ns,
            _ => panic!("Invalid format: {s}"),
        }
    }
}

/// Parse military timezone with optional hour offset.
/// Pattern: single letter (a-z except j) optionally followed by 1-2 digits.
/// Returns Some(total_hours_in_utc) or None if pattern doesn't match.
///
/// Military timezone mappings:
/// - A-I: UTC+1 to UTC+9 (J is skipped for local time)
/// - K-M: UTC+10 to UTC+12
/// - N-Y: UTC-1 to UTC-12
/// - Z: UTC+0
///
/// The hour offset from digits is added to the base military timezone offset.
/// Examples: "m" -> 12 (noon UTC), "m9" -> 21 (9pm UTC), "a5" -> 4 (4am UTC next day)
fn parse_military_timezone_with_offset(s: &str) -> Option<i32> {
    if s.is_empty() || s.len() > 3 {
        return None;
    }

    let mut chars = s.chars();
    let letter = chars.next()?.to_ascii_lowercase();

    if !letter.is_ascii_lowercase() || letter == 'j' {
        return None;
    }

    let additional_hours: i32 = if let Some(rest) = chars.as_str().chars().next() {
        if !rest.is_ascii_digit() {
            return None;
        }
        chars.as_str().parse().ok()?
    } else {
        0
    };

    let tz_offset = match letter {
        'a'..='i' => (letter as i32 - 'a' as i32) + 1,
        'k'..='m' => (letter as i32 - 'k' as i32) + 10,
        'n'..='y' => -((letter as i32 - 'n' as i32) + 1),
        'z' => 0,
        _ => return None,
    };

    let total_hours = (0 - tz_offset + additional_hours).rem_euclid(24);

    Some(total_hours)
}

#[sgcore::main]
#[allow(clippy::cognitive_complexity)]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let json_output_options = StardustOutputOptions::from_matches(&matches);
    let field_filter = matches.get_one::<String>(stardust_output::ARG_FIELD).map(|s| s.as_str());

    let format = if let Some(form) = matches.get_one::<String>(OPT_FORMAT) {
        if !form.starts_with('+') {
            return Err(SGSimpleError::new(
                1,
                translate!("date-error-invalid-date", "date" => form)
            ));
        }
        let form = form[1..].to_string();
        Format::Custom(form)
    } else if let Some(fmt) = matches
        .get_many::<String>(OPT_ISO_8601)
        .map(|mut iter| iter.next().unwrap_or(&DATE.to_string()).as_str().into())
    {
        Format::Iso8601(fmt)
    } else if matches.get_flag(OPT_RFC_EMAIL) {
        Format::Rfc5322
    } else if let Some(fmt) = matches
        .get_one::<String>(OPT_RFC_3339)
        .map(|s| s.as_str().into())
    {
        Format::Rfc3339(fmt)
    } else if matches.get_flag(OPT_RESOLUTION) {
        Format::Resolution
    } else {
        Format::Default
    };

    let date_source = if let Some(date) = matches.get_one::<String>(OPT_DATE) {
        DateSource::Human(date.into())
    } else if let Some(file) = matches.get_one::<String>(OPT_FILE) {
        match file.as_ref() {
            "-" => DateSource::Stdin,
            _ => DateSource::File(file.into()),
        }
    } else if let Some(file) = matches.get_one::<String>(OPT_REFERENCE) {
        DateSource::FileMtime(file.into())
    } else if matches.get_flag(OPT_RESOLUTION) {
        DateSource::Resolution
    } else {
        DateSource::Now
    };

    let set_to = match matches.get_one::<String>(OPT_SET).map(parse_date) {
        None => None,
        Some(Err((input, _err))) => {
            return Err(SGSimpleError::new(
                1,
                translate!("date-error-invalid-date", "date" => input)
            ));
        }
        Some(Ok(date)) => Some(date),
    };

    let settings = Settings {
        utc: matches.get_flag(OPT_UNIVERSAL),
        format,
        date_source,
        set_to,
    };

    if let Some(date) = settings.set_to {
        let date = if settings.utc {
            date.datetime().to_zoned(TimeZone::UTC).map_err(|e| {
                SGSimpleError::new(1, translate!("date-error-invalid-date", "error" => e))
            })?
        } else {
            date
        };

        return set_system_datetime(date);
    }

    let now = if settings.utc {
        Timestamp::now().to_zoned(TimeZone::UTC)
    } else {
        Zoned::now()
    };

    let dates: Box<dyn Iterator<Item = _>> = match settings.date_source {
        DateSource::Human(ref input) => {
            let input = input.trim();
            let is_empty_or_whitespace = input.is_empty();

            let is_military_j = input.eq_ignore_ascii_case("j");

            let military_tz_with_offset = parse_military_timezone_with_offset(input);

            let is_pure_digits =
                !input.is_empty() && input.len() <= 4 && input.chars().all(|c| c.is_ascii_digit());

            let date = if is_empty_or_whitespace || is_military_j {
                let date_part =
                    strtime::format("%F", &now).unwrap_or_else(|_| String::from("1970-01-01"));
                let offset = if settings.utc {
                    String::from("+00:00")
                } else {
                    strtime::format("%:z", &now).unwrap_or_default()
                };
                let composed = if offset.is_empty() {
                    format!("{date_part} 00:00")
                } else {
                    format!("{date_part} 00:00 {offset}")
                };
                parse_date(composed)
            } else if let Some(total_hours) = military_tz_with_offset {
                let date_part =
                    strtime::format("%F", &now).unwrap_or_else(|_| String::from("1970-01-01"));
                let composed = format!("{date_part} {total_hours:02}:00:00 +00:00");
                parse_date(composed)
            } else if is_pure_digits {
                let (hh_opt, mm_opt) = if input.len() <= 2 {
                    (input.parse::<u32>().ok(), Some(0u32))
                } else {
                    let (h, m) = input.split_at(input.len() - 2);
                    (h.parse::<u32>().ok(), m.parse::<u32>().ok())
                };

                if let (Some(hh), Some(mm)) = (hh_opt, mm_opt) {
                    let date_part =
                        strtime::format("%F", &now).unwrap_or_else(|_| String::from("1970-01-01"));
                    let offset = if settings.utc {
                        String::from("+00:00")
                    } else {
                        strtime::format("%:z", &now).unwrap_or_default()
                    };
                    let composed = if offset.is_empty() {
                        format!("{date_part} {hh:02}:{mm:02}")
                    } else {
                        format!("{date_part} {hh:02}:{mm:02} {offset}")
                    };
                    parse_date(composed)
                } else {
                    parse_date(input)
                }
            } else {
                parse_date(input)
            };

            let iter = std::iter::once(date);
            Box::new(iter)
        }
        DateSource::Stdin => {
            let lines = BufReader::new(std::io::stdin()).lines();
            let iter = lines.map_while(Result::ok).map(parse_date);
            Box::new(iter)
        }
        DateSource::File(ref path) => {
            if path.is_dir() {
                return Err(SGSimpleError::new(
                    2,
                    translate!("date-error-expected-file-got-directory", "path" => path.to_string_lossy())
                ));
            }
            let file = File::open(path)
                .map_err_context(|| path.as_os_str().to_string_lossy().to_string())?;
            let lines = BufReader::new(file).lines();
            let iter = lines.map_while(Result::ok).map(parse_date);
            Box::new(iter)
        }
        DateSource::FileMtime(ref path) => {
            let metadata = std::fs::metadata(path)
                .map_err_context(|| path.as_os_str().to_string_lossy().to_string())?;
            let mtime = metadata.modified()?;
            let ts = Timestamp::try_from(mtime).map_err(|e| {
                SGSimpleError::new(
                    1,
                    translate!("date-error-cannot-set-date", "path" => path.to_string_lossy(), "error" => e)
                )
            })?;
            let date = ts.to_zoned(TimeZone::try_system().unwrap_or(TimeZone::UTC));
            let iter = std::iter::once(Ok(date));
            Box::new(iter)
        }
        DateSource::Resolution => {
            let resolution = get_clock_resolution();
            let date = resolution.to_zoned(TimeZone::system());
            let iter = std::iter::once(Ok(date));
            Box::new(iter)
        }
        DateSource::Now => {
            let iter = std::iter::once(Ok(now));
            Box::new(iter)
        }
    };

    let format_string = make_format_string(&settings);

    if json_output_options.stardust_output {
        let mut date_outputs = Vec::new();

        for date in dates {
            match date {
                Ok(date) => {
                    let formatted = strtime::format(format_string, &date).map_err(|e| {
                        SGSimpleError::new(
                            1,
                            translate!("date-error-invalid-format", "format" => format_string, "error" => e)
                        )
                    })?;

                    date_outputs.push(json!({
                        "formatted": formatted,
                        "timestamp": date.timestamp().as_second(),
                        "nanosecond": date.timestamp().subsec_nanosecond(),
                        "timezone": date.time_zone().iana_name().unwrap_or("Unknown"),
                        "offset": format!("{}", date.offset()),
                    }));
                }
                Err((input, _err)) => {
                    date_outputs.push(json!({
                        "input": input,
                        "error": "Invalid date",
                        "success": false
                    }));
                }
            }
        }

        let output_json = if date_outputs.len() == 1 {
            date_outputs.into_iter().next().unwrap()
        } else {
            json!({ "dates": date_outputs })
        };

        let filtered_output = stardust_output::filter_fields(output_json, field_filter);

        stardust_output::output(json_output_options, filtered_output, || Ok(()))?;
    } else {
        for date in dates {
            match date {
                Ok(date) => match strtime::format(format_string, &date) {
                    Ok(s) => println!("{s}"),
                    Err(e) => {
                        return Err(SGSimpleError::new(
                            1,
                            translate!("date-error-invalid-format", "format" => format_string, "error" => e)
                        ));
                    }
                },
                Err((input, _err)) => show!(SGSimpleError::new(
                    1,
                    translate!("date-error-invalid-date", "date" => input)
                )),
            }
        }
    }

    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("date-about"))
        .override_usage(format_usage(&translate!("date-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(OPT_DATE)
                .short('d')
                .long(OPT_DATE)
                .value_name("STRING")
                .allow_hyphen_values(true)
                .overrides_with(OPT_DATE)
                .help(translate!("date-help-date"))
        )
        .arg(
            Arg::new(OPT_FILE)
                .short('f')
                .long(OPT_FILE)
                .value_name("DATEFILE")
                .value_hint(clap::ValueHint::FilePath)
                .conflicts_with(OPT_DATE)
                .help(translate!("date-help-file"))
        )
        .arg(
            Arg::new(OPT_ISO_8601)
                .short('I')
                .long(OPT_ISO_8601)
                .value_name("FMT")
                .value_parser(ShortcutValueParser::new([
                    DATE, HOURS, MINUTES, SECONDS, NS,
                ]))
                .num_args(0..=1)
                .default_missing_value(OPT_DATE)
                .help(translate!("date-help-iso-8601"))
        )
        .arg(
            Arg::new(OPT_RESOLUTION)
                .long(OPT_RESOLUTION)
                .conflicts_with_all([OPT_DATE, OPT_FILE])
                .overrides_with(OPT_RESOLUTION)
                .help(translate!("date-help-resolution"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_RFC_EMAIL)
                .short('R')
                .long(OPT_RFC_EMAIL)
                .alias(OPT_RFC_2822)
                .alias(OPT_RFC_822)
                .overrides_with(OPT_RFC_EMAIL)
                .help(translate!("date-help-rfc-email"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_RFC_3339)
                .long(OPT_RFC_3339)
                .value_name("FMT")
                .value_parser(ShortcutValueParser::new([DATE, SECONDS, NS]))
                .help(translate!("date-help-rfc-3339"))
        )
        .arg(
            Arg::new(OPT_DEBUG)
                .long(OPT_DEBUG)
                .help(translate!("date-help-debug"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_REFERENCE)
                .short('r')
                .long(OPT_REFERENCE)
                .value_name("FILE")
                .value_hint(clap::ValueHint::AnyPath)
                .conflicts_with_all([OPT_DATE, OPT_FILE, OPT_RESOLUTION])
                .help(translate!("date-help-reference"))
        )
        .arg(
            Arg::new(OPT_SET)
                .short('s')
                .long(OPT_SET)
                .value_name("STRING")
                .help({
                    #[cfg(not(target_os = "macos"))]
                    {
                        translate!("date-help-set")
                    }
                    #[cfg(target_os = "macos")]
                    {
                        translate!("date-help-set-macos")
                    }
                })
        )
        .arg(
            Arg::new(OPT_UNIVERSAL)
                .short('u')
                .long(OPT_UNIVERSAL)
                .visible_alias(OPT_UNIVERSAL_2)
                .alias("uct")
                .overrides_with(OPT_UNIVERSAL)
                .help(translate!("date-help-universal"))
                .action(ArgAction::SetTrue)
        )
        .arg(Arg::new(OPT_FORMAT));

    let cmd = cmd
        .arg(
            Arg::new(stardust_output::ARG_STARDUST_OUTPUT)
                .short('o')
                .long("obj")
                .help("Output as stardust (JSON)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(stardust_output::ARG_VERBOSE)
                .short('v')
                .long("verbose")
                .help("Include additional details in output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(stardust_output::ARG_PRETTY)
                .long("pretty")
                .help("Pretty-print object (JSON) output (use with -o)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(stardust_output::ARG_FIELD)
                .long("field")
                .value_name("FIELD")
                .help("Filter object output to specific field(s) (comma-separated)")
                .action(ArgAction::Set),
        );

    cmd
}

/// Return the appropriate format string for the given settings.
fn make_format_string(settings: &Settings) -> &str {
    match settings.format {
        Format::Iso8601(ref fmt) => match *fmt {
            Iso8601Format::Date => "%F",
            Iso8601Format::Hours => "%FT%H%:z",
            Iso8601Format::Minutes => "%FT%H:%M%:z",
            Iso8601Format::Seconds => "%FT%T%:z",
            Iso8601Format::Ns => "%FT%T,%N%:z",
        },
        Format::Rfc5322 => "%a, %d %h %Y %T %z",
        Format::Rfc3339(ref fmt) => match *fmt {
            Rfc3339Format::Date => "%F",
            Rfc3339Format::Seconds => "%F %T%:z",
            Rfc3339Format::Ns => "%F %T.%N%:z",
        },
        Format::Resolution => "%s.%N",
        Format::Custom(ref fmt) => fmt,
        Format::Default => "%a %b %e %X %Z %Y",
    }
}

/// Minimal disambiguation rules for highly ambiguous timezone abbreviations.
/// Only includes cases where multiple major timezones share the same abbreviation.
/// All other abbreviations are discovered dynamically from the IANA database.
///
/// Disambiguation rationale (GNU compatible):
/// - CST: Central Standard Time (US) preferred over China/Cuba Standard Time
/// - EST: Eastern Standard Time (US) preferred over Australian Eastern Standard Time
/// - IST: India Standard Time preferred over Israel/Irish Standard Time
/// - MST: Mountain Standard Time (US) preferred over Malaysia Standard Time
/// - PST: Pacific Standard Time (US) - widely used abbreviation
/// - GMT: Alias for UTC (universal)
/// - Australian timezones: AWST, ACST, AEST (cannot be dynamically discovered)
///
/// All other timezones (JST, CET, etc.) are dynamically resolved from IANA database. // spell-checker:disable-line
static PREFERRED_TZ_MAPPINGS: &[(&str, &str)] = &[
    ("UTC", "UTC"),
    ("GMT", "UTC"),
    ("PST", "America/Los_Angeles"),
    ("PDT", "America/Los_Angeles"),
    ("MST", "America/Denver"),
    ("MDT", "America/Denver"),
    ("CST", "America/Chicago"),
    ("CDT", "America/Chicago"),
    ("EST", "America/New_York"),
    ("EDT", "America/New_York"),
    ("IST", "Asia/Kolkata"),
    ("AWST", "Australia/Perth"),
    ("ACST", "Australia/Adelaide"),
    ("ACDT", "Australia/Adelaide"),
    ("AEST", "Australia/Sydney"),
    ("AEDT", "Australia/Sydney"),
];

/// Lazy-loaded timezone abbreviation lookup map built from IANA database.
static TZ_ABBREV_CACHE: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Build timezone abbreviation lookup map from IANA database.
/// Uses preferred mappings for disambiguation, then searches all timezones.
fn build_tz_abbrev_map() -> HashMap<String, String> {
    let mut map = HashMap::new();

    for (abbrev, iana) in PREFERRED_TZ_MAPPINGS {
        map.insert((*abbrev).to_string(), (*iana).to_string());
    }

    let tzdb = TimeZoneDatabase::from_env();
    for tz_name in tzdb.available() {
        let tz_str = tz_name.as_str();
        if !map.values().any(|v| v == tz_str) {
            if let Some(last_part) = tz_str.split('/').next_back() {
                let potential_abbrev = last_part.to_uppercase();
                if potential_abbrev.len() >= 2
                    && potential_abbrev.len() <= 5
                    && potential_abbrev.chars().all(|c| c.is_ascii_uppercase())
                {
                    map.entry(potential_abbrev)
                        .or_insert_with(|| tz_str.to_string());
                }
            }
        }
    }

    map
}

/// Get IANA timezone name for a given abbreviation.
/// Uses lazy-loaded cache with preferred mappings for disambiguation.
fn tz_abbrev_to_iana(abbrev: &str) -> Option<&str> {
    let cache = TZ_ABBREV_CACHE.get_or_init(build_tz_abbrev_map);
    cache.get(abbrev).map(|s| s.as_str())
}

/// Resolve timezone abbreviation in date string and replace with numeric offset.
/// Returns the modified string with offset, or original if no abbreviation found.
fn resolve_tz_abbreviation<S: AsRef<str>>(date_str: S) -> String {
    let s = date_str.as_ref();

    if let Some(last_word) = s.split_whitespace().last() {
        if last_word.len() >= 2
            && last_word.len() <= 5
            && last_word.chars().all(|c| c.is_ascii_uppercase())
        {
            if let Some(iana_name) = tz_abbrev_to_iana(last_word) {
                if let Ok(tz) = TimeZone::get(iana_name) {
                    let date_part = s.trim_end_matches(last_word).trim();

                    let date_with_utc = format!("{date_part} +00:00");
                    if let Ok(parsed) = parse_datetime::parse_datetime(&date_with_utc) {
                        let ts = parsed.timestamp();

                        let zoned = ts.to_zoned(tz);
                        let offset_str = format!("{}", zoned.offset());

                        return format!("{date_part} {offset_str}");
                    }
                }
            }
        }
    }

    s.to_string()
}

/// Parse a `String` into a `DateTime`.
/// If it fails, return a tuple of the `String` along with its `ParseError`.
///
/// **Update for parse_datetime 0.13:**
/// - parse_datetime 0.11: returned `chrono::DateTime` → required conversion to `jiff::Zoned`
/// - parse_datetime 0.13: returns `jiff::Zoned` directly → no conversion needed
///
/// This change was necessary to fix issue #8754 (parsing large second values like
/// "12345.123456789 seconds ago" which failed in 0.11 but works in 0.13).
fn parse_date<S: AsRef<str> + Clone>(
    s: S
) -> Result<Zoned, (String, parse_datetime::ParseDateTimeError)> {
    let resolved = resolve_tz_abbreviation(s.as_ref());

    match parse_datetime::parse_datetime(&resolved) {
        Ok(date) => {
            let timestamp = date.timestamp();
            Ok(timestamp.to_zoned(TimeZone::try_system().unwrap_or(TimeZone::UTC)))
        }
        Err(e) => Err((s.as_ref().into(), e)),
    }
}
fn get_clock_resolution() -> Timestamp {
    let mut timespec = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        clock_getres(CLOCK_REALTIME, &raw mut timespec);
    }
    #[allow(clippy::unnecessary_cast)]
    Timestamp::constant(timespec.tv_sec as i64, timespec.tv_nsec as i32)
}

#[cfg(target_os = "macos")]
fn set_system_datetime(_date: Zoned) -> SGResult<()> {
    Err(SGSimpleError::new(
        1,
        translate!("date-error-setting-date-not-supported-macos")
    ))
}

#[cfg(all(unix, not(target_os = "macos")))]
/// System call to set date (unix).
/// See here for more:
/// `<https://doc.rust-lang.org/libc/i686-unknown-linux-gnu/libc/fn.clock_settime.html>`
/// `<https://linux.die.net/man/3/clock_settime>`
/// `<https://www.gnu.org/software/libc/manual/html_node/Time-Types.html>`
fn set_system_datetime(date: Zoned) -> SGResult<()> {
    let ts = date.timestamp();
    let timespec = timespec {
        tv_sec: ts.as_second() as _,
        tv_nsec: ts.subsec_nanosecond() as _,
    };

    let result = unsafe { clock_settime(CLOCK_REALTIME, &raw const timespec) };

    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error()
            .map_err_context(|| translate!("date-error-cannot-set-date")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_military_timezone_with_offset() {
        assert_eq!(parse_military_timezone_with_offset("m"), Some(12));
        assert_eq!(parse_military_timezone_with_offset("m9"), Some(21));
        assert_eq!(parse_military_timezone_with_offset("a5"), Some(4));
        assert_eq!(parse_military_timezone_with_offset("z"), Some(0));
        assert_eq!(parse_military_timezone_with_offset("M9"), Some(21));

        assert_eq!(parse_military_timezone_with_offset("j"), None);
        assert_eq!(parse_military_timezone_with_offset(""), None);
        assert_eq!(parse_military_timezone_with_offset("m999"), None);
        assert_eq!(parse_military_timezone_with_offset("9m"), None);
    }
}

