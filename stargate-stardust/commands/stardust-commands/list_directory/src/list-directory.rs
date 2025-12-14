

use std::collections::HashSet;
use std::collections::HashMap;
use std::borrow::Cow;
use std::cell::RefCell;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::{
    cell::{LazyCell, OnceCell},
    cmp::Reverse,
    ffi::{OsStr, OsString},
    fmt::Write as FmtWrite,
    fs::{self, DirEntry, FileType, Metadata, ReadDir},
    io::{BufWriter, ErrorKind, IsTerminal, Stdout, Write, stdout},
    iter,
    num::IntErrorKind,
    ops::RangeInclusive,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ansi_width::ansi_width;
use clap::{
    Arg, ArgAction, Command,
    builder::{NonEmptyStringValueParser, PossibleValue, ValueParser},
};
use glob::{MatchOptions, Pattern};
use lscolors::{Colorable, LsColors};
use term_grid::{DEFAULT_SEPARATOR_SIZE, Direction, Filling, Grid, GridOptions, SPACES_IN_TAB};
use thiserror::Error;

use sgcore::entries;
use sgcore::libc::{S_IXGRP, S_IXOTH, S_IXUSR};
#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd"
))]
use sgcore::libc::{dev_t, major, minor};
use sgcore::{
    display::Quotable,
    error::{SGError, SGResult, SGSimpleError, set_exit_code},
    format::human::{SizeFormat, human_readable},
    format_usage,
    fs::FileInformation,
    fs::display_permissions,
    fsext::{MetadataTimeField, metadata_get_time},
    stardust_output::{self, StardustOutputOptions},
    line_ending::LineEnding,
    os_str_as_bytes_lossy,
    parser::parse_glob,
    parser::parse_size::parse_size_non_zero_u64,
    parser::shortcut_value_parser::ShortcutValueParser,
    quoting_style::{QuotingStyle, locale_aware_escape_dir_name, locale_aware_escape_name},
    show, show_error, show_warning,
    time::{FormatSystemTimeFallback, format, format_system_time},
    translate,
    version_cmp::version_cmp,
};
use serde_json::json;


mod dired;
use dired::{DiredOutput, is_dired_arg_present};
mod colors;
use crate::options::QUOTING_STYLE;
use colors::{StyleManager, color_name};

pub mod options {
    pub mod format {
        pub static ONE_LINE: &str = "1";
        pub static LONG: &str = "long";
        pub static COLUMNS: &str = "C";
        pub static ACROSS: &str = "x";
        pub static TAB_SIZE: &str = "tabsize";
        pub static COMMAS: &str = "m";
        pub static LONG_NO_OWNER: &str = "g";
        pub static LONG_NO_GROUP: &str = "o";
        pub static LONG_NUMERIC_UID_GID: &str = "numeric-uid-gid";
    }

    pub mod files {
        pub static ALL: &str = "all";
        pub static ALMOST_ALL: &str = "almost-all";
        pub static UNSORTED_ALL: &str = "f";
    }

    pub mod sort {
        pub static SIZE: &str = "S";
        pub static TIME: &str = "t";
        pub static NONE: &str = "U";
        pub static VERSION: &str = "v";
        pub static EXTENSION: &str = "X";
    }

    pub mod time {
        pub static ACCESS: &str = "u";
        pub static CHANGE: &str = "c";
    }

    pub mod size {
        pub static ALLOCATION_SIZE: &str = "size";
        pub static BLOCK_SIZE: &str = "block-size";
        pub static HUMAN_READABLE: &str = "human-readable";
        pub static SI: &str = "si";
        pub static KIBIBYTES: &str = "kibibytes";
    }

    pub mod quoting {
        pub static ESCAPE: &str = "escape";
        pub static LITERAL: &str = "literal";
        pub static C: &str = "quote-name";
    }

    pub mod indicator_style {
        pub static SLASH: &str = "p";
        pub static FILE_TYPE: &str = "file-type";
        pub static CLASSIFY: &str = "classify";
    }

    pub mod dereference {
        pub static ALL: &str = "dereference";
        pub static ARGS: &str = "dereference-command-line";
        pub static DIR_ARGS: &str = "dereference-command-line-symlink-to-dir";
    }

    pub static HELP: &str = "help";
    pub static QUOTING_STYLE: &str = "quoting-style";
    pub static HIDE_CONTROL_CHARS: &str = "hide-control-chars";
    pub static SHOW_CONTROL_CHARS: &str = "show-control-chars";
    pub static WIDTH: &str = "width";
    pub static AUTHOR: &str = "author";
    pub static NO_GROUP: &str = "no-group";
    pub static FORMAT: &str = "format";
    pub static SORT: &str = "sort";
    pub static TIME: &str = "time";
    pub static IGNORE_BACKUPS: &str = "ignore-backups";
    pub static DIRECTORY: &str = "directory";
    pub static INODE: &str = "inode";
    pub static REVERSE: &str = "reverse";
    pub static RECURSIVE: &str = "recursive";
    pub static COLOR: &str = "color";
    pub static PATHS: &str = "paths";
    pub static INDICATOR_STYLE: &str = "indicator-style";
    pub static TIME_STYLE: &str = "time-style";
    pub static FULL_TIME: &str = "full-time";
    pub static HIDE: &str = "hide";
    pub static IGNORE: &str = "ignore";
    pub static GROUP_DIRECTORIES_FIRST: &str = "group-directories-first";
    pub static ZERO: &str = "zero";
    pub static DIRED: &str = "dired";
    pub static HYPERLINK: &str = "hyperlink";
}

const DEFAULT_TERM_WIDTH: u16 = 80;
const POSIXLY_CORRECT_BLOCK_SIZE: u64 = 512;
const DEFAULT_BLOCK_SIZE: u64 = 1024;
const DEFAULT_FILE_SIZE_BLOCK_SIZE: u64 = 1;

#[derive(Error, Debug)]
enum LsError {
    #[error("{}", translate!("ls-error-invalid-line-width", "width" => format!("'{_0}'")))]
    InvalidLineWidth(String),

    #[error("{}", translate!("ls-error-general-io", "error" => _0))]
    IOError(#[from] std::io::Error),

    #[error("{}", match .1.kind() {
        ErrorKind::NotFound => translate!("ls-error-cannot-access-no-such-file", "path" => .0.to_string_lossy()),
        ErrorKind::PermissionDenied => match .1.raw_os_error().unwrap_or(1) {
            1 => translate!("ls-error-cannot-access-operation-not-permitted", "path" => .0.to_string_lossy()),
            _ => if .0.is_dir() {
                translate!("ls-error-cannot-open-directory-permission-denied", "path" => .0.to_string_lossy())
            } else {
                translate!("ls-error-cannot-open-file-permission-denied", "path" => .0.to_string_lossy())
            },
        },
        _ => match .1.raw_os_error().unwrap_or(1) {
            9 => translate!("ls-error-cannot-open-directory-bad-descriptor", "path" => .0.to_string_lossy()),
            _ => translate!("ls-error-unknown-io-error", "path" => .0.to_string_lossy(), "error" => format!("{:?}", .1)),
        },
    })]
    IOErrorContext(PathBuf, std::io::Error, bool),

    #[error("{}", translate!("ls-error-invalid-block-size", "size" => format!("'{_0}'")))]
    BlockSizeParseError(String),

    #[error("{}", translate!("ls-error-dired-and-zero-incompatible"))]
    DiredAndZeroAreIncompatible,

    #[error("{}", translate!("ls-error-not-listing-already-listed", "path" => .0.to_string_lossy()))]
    AlreadyListedError(PathBuf),

    #[error("{}", translate!("ls-error-invalid-time-style", "style" => .0.quote()))]
    TimeStyleParseError(String),
}

impl SGError for LsError {
    fn code(&self) -> i32 {
        match self {
            Self::InvalidLineWidth(_) => 2,
            Self::IOError(_) => 1,
            Self::IOErrorContext(_, _, false) => 1,
            Self::IOErrorContext(_, _, true) => 2,
            Self::BlockSizeParseError(_) => 2,
            Self::DiredAndZeroAreIncompatible => 2,
            Self::AlreadyListedError(_) => 2,
            Self::TimeStyleParseError(_) => 2,
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub enum Format {
    Columns,
    Long,
    OneLine,
    Across,
    Commas,
}

#[derive(PartialEq, Eq)]
enum Sort {
    None,
    Name,
    Size,
    Time,
    Version,
    Extension,
    Width,
}

#[derive(PartialEq, Eq)]
enum Files {
    All,
    AlmostAll,
    Normal,
}

fn parse_time_style(options: &clap::ArgMatches) -> Result<(String, Option<String>), LsError> {

    const LOCALE_FORMAT: (&str, Option<&str>) = ("%b %e %H:%M", Some("%b %e  %Y"));


    fn ok((recent, older): (&str, Option<&str>)) -> Result<(String, Option<String>), LsError> {
        Ok((recent.to_string(), older.map(String::from)))
    }

    if let Some(field) = options
        .get_one::<String>(options::TIME_STYLE)
        .map(|s| s.to_owned())
        .or_else(|| std::env::var("TIME_STYLE").ok())
    {


        if options.get_flag(options::FULL_TIME)
            && options.indices_of(options::FULL_TIME).unwrap().next_back()
                > options.indices_of(options::TIME_STYLE).unwrap().next_back()
        {
            ok((format::FULL_ISO, None))
        } else {
            let field = if let Some(field) = field.strip_prefix("posix-") {




                if std::env::var("LC_TIME").unwrap_or_default() == "POSIX"
                    || std::env::var("LC_ALL").unwrap_or_default() == "POSIX"
                {
                    return ok(LOCALE_FORMAT);
                }
                field
            } else {
                &field
            };

            match field {
                "full-iso" => ok((format::FULL_ISO, None)),
                "long-iso" => ok((format::LONG_ISO, None)),

                "iso" => Ok((
                    "%m-%d %H:%M".to_string(),
                    Some(format::ISO.to_string() + " ")
                )),
                "locale" => ok(LOCALE_FORMAT),
                _ => match field.chars().next().unwrap() {
                    '+' => {

                        let mut it = field[1..].split('\n');
                        let recent = it.next().unwrap_or_default();
                        let older = it.next();
                        match it.next() {
                            None => ok((recent, older)),
                            Some(_) => Err(LsError::TimeStyleParseError(String::from(field))),
                        }
                    }
                    _ => Err(LsError::TimeStyleParseError(String::from(field))),
                },
            }
        }
    } else if options.get_flag(options::FULL_TIME) {
        ok((format::FULL_ISO, None))
    } else {
        ok(LOCALE_FORMAT)
    }
}

enum Dereference {
    None,
    DirArgs,
    Args,
    All,
}

#[derive(PartialEq, Eq)]
enum IndicatorStyle {
    None,
    Slash,
    FileType,
    Classify,
}

pub struct Config {

    pub format: Format,
    files: Files,
    sort: Sort,
    recursive: bool,
    reverse: bool,
    dereference: Dereference,
    ignore_patterns: Vec<Pattern>,
    size_format: SizeFormat,
    directory: bool,
    time: MetadataTimeField,
    inode: bool,
    color: Option<LsColors>,
    long: LongFormat,
    alloc_size: bool,
    file_size_block_size: u64,
    #[allow(dead_code)]
    block_size: u64,
    width: u16,

    pub quoting_style: QuotingStyle,
    indicator_style: IndicatorStyle,
    time_format_recent: String,
    time_format_older: Option<String>,
    group_directories_first: bool,
    line_ending: LineEnding,
    dired: bool,
    hyperlink: bool,
    tab_size: usize,
    object_output: StardustOutputOptions,
    object_fields: Vec<String>,
}


struct LongFormat {
    author: bool,
    group: bool,
    owner: bool,
    numeric_uid_gid: bool,
}

struct PaddingCollection {
    inode: usize,
    link_count: usize,
    uname: usize,
    group: usize,
    size: usize,
    major: usize,
    minor: usize,
    block_size: usize,
}

/// Extracts the format to display the information based on the options provided.
///
/// # Returns
///
/// A tuple containing the Format variant and an Option containing a &'static str
/// which corresponds to the option used to define the format.
fn extract_format(options: &clap::ArgMatches) -> (Format, Option<&'static str>) {
    if let Some(format_) = options.get_one::<String>(options::FORMAT) {
        (
            match format_.as_str() {
                "long" | "verbose" => Format::Long,
                "single-column" => Format::OneLine,
                "columns" | "vertical" => Format::Columns,
                "across" | "horizontal" => Format::Across,
                "commas" => Format::Commas,

                _ => unreachable!("Invalid field for --format"),
            },
            Some(options::FORMAT)
        )
    } else if options.get_flag(options::format::LONG) {
        (Format::Long, Some(options::format::LONG))
    } else if options.get_flag(options::format::ACROSS) {
        (Format::Across, Some(options::format::ACROSS))
    } else if options.get_flag(options::format::COMMAS) {
        (Format::Commas, Some(options::format::COMMAS))
    } else if options.get_flag(options::format::COLUMNS) {
        (Format::Columns, Some(options::format::COLUMNS))
    } else if stdout().is_terminal() {
        (Format::Columns, None)
    } else {
        (Format::OneLine, None)
    }
}

/// Extracts the type of files to display
///
/// # Returns
///
/// A Files variant representing the type of files to display.
fn extract_files(options: &clap::ArgMatches) -> Files {
    let get_last_index = |flag: &str| -> usize {
        if options.value_source(flag) == Some(clap::parser::ValueSource::CommandLine) {
            options.index_of(flag).unwrap_or(0)
        } else {
            0
        }
    };

    let all_index = get_last_index(options::files::ALL);
    let almost_all_index = get_last_index(options::files::ALMOST_ALL);
    let unsorted_all_index = get_last_index(options::files::UNSORTED_ALL);

    let max_index = all_index.max(almost_all_index).max(unsorted_all_index);

    if max_index == 0 {
        Files::Normal
    } else if max_index == almost_all_index {
        Files::AlmostAll
    } else {

        Files::All
    }
}

/// Extracts the sorting method to use based on the options provided.
///
/// # Returns
///
/// A Sort variant representing the sorting method to use.
fn extract_sort(options: &clap::ArgMatches) -> Sort {
    let get_last_index = |flag: &str| -> usize {
        if options.value_source(flag) == Some(clap::parser::ValueSource::CommandLine) {
            options.index_of(flag).unwrap_or(0)
        } else {
            0
        }
    };

    let sort_index = options
        .get_one::<String>(options::SORT)
        .and_then(|_| options.indices_of(options::SORT))
        .map(|mut indices| indices.next_back().unwrap_or(0))
        .unwrap_or(0);
    let time_index = get_last_index(options::sort::TIME);
    let size_index = get_last_index(options::sort::SIZE);
    let none_index = get_last_index(options::sort::NONE);
    let version_index = get_last_index(options::sort::VERSION);
    let extension_index = get_last_index(options::sort::EXTENSION);
    let unsorted_all_index = get_last_index(options::files::UNSORTED_ALL);

    let max_sort_index = sort_index
        .max(time_index)
        .max(size_index)
        .max(none_index)
        .max(version_index)
        .max(extension_index)
        .max(unsorted_all_index);

    match max_sort_index {
        0 => {

            if !options.get_flag(options::format::LONG)
                && (options.get_flag(options::time::ACCESS)
                    || options.get_flag(options::time::CHANGE)
                    || options.get_one::<String>(options::TIME).is_some())
            {
                Sort::Time
            } else {
                Sort::Name
            }
        }
        idx if idx == unsorted_all_index || idx == none_index => Sort::None,
        idx if idx == sort_index => {
            if let Some(field) = options.get_one::<String>(options::SORT) {
                match field.as_str() {
                    "none" => Sort::None,
                    "name" => Sort::Name,
                    "time" => Sort::Time,
                    "size" => Sort::Size,
                    "version" => Sort::Version,
                    "extension" => Sort::Extension,
                    "width" => Sort::Width,
                    _ => unreachable!("Invalid field for --sort"),
                }
            } else {
                Sort::Name
            }
        }
        idx if idx == time_index => Sort::Time,
        idx if idx == size_index => Sort::Size,
        idx if idx == version_index => Sort::Version,
        idx if idx == extension_index => Sort::Extension,
        _ => Sort::Name,
    }
}

/// Extracts the time to use based on the options provided.
///
/// # Returns
///
/// A `MetadataTimeField` variant representing the time to use.
fn extract_time(options: &clap::ArgMatches) -> MetadataTimeField {
    if let Some(field) = options.get_one::<String>(options::TIME) {
        field.as_str().into()
    } else if options.get_flag(options::time::ACCESS) {
        MetadataTimeField::Access
    } else if options.get_flag(options::time::CHANGE) {
        MetadataTimeField::Change
    } else {
        MetadataTimeField::Modification
    }
}

/// Some env variables can be passed
/// For now, we are only verifying if empty or not and known for `TERM`
fn is_color_compatible_term() -> bool {
    let is_term_set = std::env::var("TERM").is_ok();
    let is_colorterm_set = std::env::var("COLORTERM").is_ok();

    let term = std::env::var("TERM").unwrap_or_default();
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();


    let term_matches = |term: &str| -> bool {
        sgcore::colors::TERMS.iter().any(|&pattern| {
            term == pattern
                || (pattern.ends_with('*') && term.starts_with(&pattern[..pattern.len() - 1]))
        })
    };

    if is_term_set && term.is_empty() && is_colorterm_set && colorterm.is_empty() {
        return false;
    }

    if !term.is_empty() && !term_matches(&term) {
        return false;
    }
    true
}

/// Extracts the color option to use based on the options provided.
///
/// # Returns
///
/// A boolean representing whether or not to use color.
fn extract_color(options: &clap::ArgMatches) -> bool {
    let get_last_index = |flag: &str| -> usize {
        if options.value_source(flag) == Some(clap::parser::ValueSource::CommandLine) {
            options.index_of(flag).unwrap_or(0)
        } else {
            0
        }
    };

    let color_index = options
        .get_one::<String>(options::COLOR)
        .and_then(|_| options.indices_of(options::COLOR))
        .map(|mut indices| indices.next_back().unwrap_or(0))
        .unwrap_or(0);
    let unsorted_all_index = get_last_index(options::files::UNSORTED_ALL);

    let color_enabled = match options.get_one::<String>(options::COLOR) {
        None => {


            if options.contains_id(options::COLOR) {
                true
            } else {

                is_color_compatible_term()
            }
        }
        Some(val) => match val.as_str() {
            "" | "always" | "yes" | "force" => true,
            "auto" | "tty" | "if-tty" => {
                is_color_compatible_term() && stdout().is_terminal()
            }
            /* "never" | "no" | "none" | */ _ => false,
        },
    };



    if color_index > 0 {

        color_enabled
    } else if unsorted_all_index > 0 {

        false
    } else {
        color_enabled
    }
}

/// Extracts the hyperlink option to use based on the options provided.
///
/// # Returns
///
/// A boolean representing whether to hyperlink files.
fn extract_hyperlink(options: &clap::ArgMatches) -> bool {
    let hyperlink = options
        .get_one::<String>(options::HYPERLINK)
        .unwrap()
        .as_str();

    match hyperlink {
        "always" | "yes" | "force" => true,
        "auto" | "tty" | "if-tty" => stdout().is_terminal(),
        "never" | "no" | "none" => false,
        _ => unreachable!("should be handled by clap"),
    }
}

/// Match the argument given to --quoting-style or the [`QUOTING_STYLE`] env variable.
///
/// # Arguments
///
/// * `style`: the actual argument string
/// * `show_control` - A boolean value representing whether to show control characters.
///
/// # Returns
///
/// * An option with None if the style string is invalid, or a `QuotingStyle` wrapped in `Some`.
fn match_quoting_style_name(style: &str, show_control: bool) -> Option<QuotingStyle> {
    match style {
        "literal" => Some(QuotingStyle::Literal { show_control }),
        "shell" => Some(QuotingStyle::SHELL),
        "shell-always" => Some(QuotingStyle::SHELL_QUOTE),
        "shell-escape" => Some(QuotingStyle::SHELL_ESCAPE),
        "shell-escape-always" => Some(QuotingStyle::SHELL_ESCAPE_QUOTE),
        "c" => Some(QuotingStyle::C_DOUBLE),
        "escape" => Some(QuotingStyle::C_NO_QUOTES),
        _ => None,
    }
    .map(|qs| qs.show_control(show_control))
}

/// Extracts the quoting style to use based on the options provided.
/// If no options are given, it looks if a default quoting style is provided
/// through the [`QUOTING_STYLE`] environment variable.
///
/// # Arguments
///
/// * `options` - A reference to a [`clap::ArgMatches`] object containing command line arguments.
/// * `show_control` - A boolean value representing whether or not to show control characters.
///
/// # Returns
///
/// A [`QuotingStyle`] variant representing the quoting style to use.
fn extract_quoting_style(options: &clap::ArgMatches, show_control: bool) -> QuotingStyle {
    let opt_quoting_style = options.get_one::<String>(QUOTING_STYLE);

    if let Some(style) = opt_quoting_style {
        match match_quoting_style_name(style, show_control) {
            Some(qs) => qs,
            None => unreachable!("Should have been caught by Clap"),
        }
    } else if options.get_flag(options::quoting::LITERAL) {
        QuotingStyle::Literal { show_control }
    } else if options.get_flag(options::quoting::ESCAPE) {
        QuotingStyle::C_NO_QUOTES
    } else if options.get_flag(options::quoting::C) {
        QuotingStyle::C_DOUBLE
    } else if options.get_flag(options::DIRED) {
        QuotingStyle::Literal { show_control }
    } else {

        if let Ok(style) = std::env::var("QUOTING_STYLE") {
            match match_quoting_style_name(style.as_str(), show_control) {
                Some(qs) => return qs,
                None => eprintln!(
                    "{}",
                    translate!("ls-invalid-quoting-style", "program" => std::env::args().next().unwrap_or_else(|| "ls".to_string()), "style" => style.clone())
                ),
            }
        }



        if stdout().is_terminal() {
            QuotingStyle::SHELL_ESCAPE.show_control(show_control)
        } else {
            QuotingStyle::Literal { show_control }
        }
    }
}

/// Extracts the indicator style to use based on the options provided.
///
/// # Returns
///
/// An [`IndicatorStyle`] variant representing the indicator style to use.
fn extract_indicator_style(options: &clap::ArgMatches) -> IndicatorStyle {
    if let Some(field) = options.get_one::<String>(options::INDICATOR_STYLE) {
        match field.as_str() {
            "none" => IndicatorStyle::None,
            "file-type" => IndicatorStyle::FileType,
            "classify" => IndicatorStyle::Classify,
            "slash" => IndicatorStyle::Slash,
            &_ => IndicatorStyle::None,
        }
    } else if let Some(field) = options.get_one::<String>(options::indicator_style::CLASSIFY) {
        match field.as_str() {
            "never" | "no" | "none" => IndicatorStyle::None,
            "always" | "yes" | "force" => IndicatorStyle::Classify,
            "auto" | "tty" | "if-tty" => {
                if stdout().is_terminal() {
                    IndicatorStyle::Classify
                } else {
                    IndicatorStyle::None
                }
            }
            &_ => IndicatorStyle::None,
        }
    } else if options.get_flag(options::indicator_style::SLASH) {
        IndicatorStyle::Slash
    } else if options.get_flag(options::indicator_style::FILE_TYPE) {
        IndicatorStyle::FileType
    } else {

        IndicatorStyle::Classify
    }
}

/// Parses the width value from either the command line arguments or the environment variables.
fn parse_width(width_match: Option<&String>) -> Result<u16, LsError> {
    let parse_width_from_args = |s: &str| -> Result<u16, LsError> {
        let radix = if s.starts_with('0') && s.len() > 1 {
            8
        } else {
            10
        };
        match u16::from_str_radix(s, radix) {
            Ok(x) => Ok(x),
            Err(e) => match e.kind() {
                IntErrorKind::PosOverflow => Ok(u16::MAX),
                _ => Err(LsError::InvalidLineWidth(s.into())),
            },
        }
    };

    let parse_width_from_env =
        |columns: OsString| match columns.to_str().and_then(|s| s.parse().ok()) {
            Some(columns) => columns,
            None => {
                show_error!(
                    "{}",
                    translate!("ls-invalid-columns-width", "width" => columns.quote())
                );
                DEFAULT_TERM_WIDTH
            }
        };

    let calculate_term_size = || match terminal_size::terminal_size() {
        Some((width, _)) => width.0,
        None => DEFAULT_TERM_WIDTH,
    };

    let ret = match width_match {
        Some(x) => parse_width_from_args(x)?,
        None => match std::env::var_os("COLUMNS") {
            Some(columns) => parse_width_from_env(columns),
            None => calculate_term_size(),
        },
    };

    Ok(ret)
}

impl Config {
    #[allow(clippy::cognitive_complexity)]
    pub fn from(options: &clap::ArgMatches) -> SGResult<Self> {
        let (mut format, opt) = extract_format(options);
        let files = extract_files(options);
















        if format != Format::Long {
            let idx = opt
                .and_then(|opt| options.indices_of(opt).map(|x| x.max().unwrap()))
                .unwrap_or(0);
            if [
                options::format::LONG_NO_OWNER,
                options::format::LONG_NO_GROUP,
                options::format::LONG_NUMERIC_UID_GID,
                options::FULL_TIME,
            ]
            .iter()
            .filter_map(|opt| {
                if options.value_source(opt) == Some(clap::parser::ValueSource::CommandLine) {
                    options.indices_of(opt)
                } else {
                    None
                }
            })
            .flatten()
            .any(|i| i >= idx)
            {
                format = Format::Long;
            } else if let Some(mut indices) = options.indices_of(options::format::ONE_LINE) {
                if options.value_source(options::format::ONE_LINE)
                    == Some(clap::parser::ValueSource::CommandLine)
                    && indices.any(|i| i > idx)
                {
                    format = Format::OneLine;
                }
            }
        }

        let sort = extract_sort(options);
        let time = extract_time(options);
        let mut needs_color = extract_color(options);
        let hyperlink = extract_hyperlink(options);

        let opt_block_size = options.get_one::<String>(options::size::BLOCK_SIZE);
        let opt_si = opt_block_size.is_some()
            && options
                .get_one::<String>(options::size::BLOCK_SIZE)
                .unwrap()
                .eq("si")
            || options.get_flag(options::size::SI);
        let opt_hr = (opt_block_size.is_some()
            && options
                .get_one::<String>(options::size::BLOCK_SIZE)
                .unwrap()
                .eq("human-readable"))
            || options.get_flag(options::size::HUMAN_READABLE);
        let opt_kb = options.get_flag(options::size::KIBIBYTES);

        let size_format = if opt_si {
            SizeFormat::Decimal
        } else if opt_hr {
            SizeFormat::Binary
        } else {
            SizeFormat::Bytes
        };

        let env_var_blocksize = std::env::var_os("BLOCKSIZE");
        let env_var_block_size = std::env::var_os("BLOCK_SIZE");
        let env_var_ls_block_size = std::env::var_os("LS_BLOCK_SIZE");
        let env_var_posixly_correct = std::env::var_os("POSIXLY_CORRECT");
        let mut is_env_var_blocksize = false;

        let raw_block_size = if let Some(opt_block_size) = opt_block_size {
            OsString::from(opt_block_size)
        } else if let Some(env_var_ls_block_size) = env_var_ls_block_size {
            env_var_ls_block_size
        } else if let Some(env_var_block_size) = env_var_block_size {
            env_var_block_size
        } else if let Some(env_var_blocksize) = env_var_blocksize {
            is_env_var_blocksize = true;
            env_var_blocksize
        } else {
            OsString::from("")
        };

        let (file_size_block_size, block_size) = if !opt_si && !opt_hr && !raw_block_size.is_empty()
        {
            match parse_size_non_zero_u64(&raw_block_size.to_string_lossy()) {
                Ok(size) => match (is_env_var_blocksize, opt_kb) {
                    (true, true) => (DEFAULT_FILE_SIZE_BLOCK_SIZE, DEFAULT_BLOCK_SIZE),
                    (true, false) => (DEFAULT_FILE_SIZE_BLOCK_SIZE, size),
                    (false, true) => {

                        if opt_block_size.is_some() {
                            (size, size)
                        } else {
                            (size, DEFAULT_BLOCK_SIZE)
                        }
                    }
                    (false, false) => (size, size),
                },
                Err(_) => {


                    if let Some(invalid_block_size) = opt_block_size {
                        return Err(Box::new(LsError::BlockSizeParseError(
                            invalid_block_size.clone()
                        )));
                    }
                    if is_env_var_blocksize {
                        (DEFAULT_FILE_SIZE_BLOCK_SIZE, DEFAULT_BLOCK_SIZE)
                    } else {
                        (DEFAULT_BLOCK_SIZE, DEFAULT_BLOCK_SIZE)
                    }
                }
            }
        } else if env_var_posixly_correct.is_some() {
            if opt_kb {
                (DEFAULT_FILE_SIZE_BLOCK_SIZE, DEFAULT_BLOCK_SIZE)
            } else {
                (DEFAULT_FILE_SIZE_BLOCK_SIZE, POSIXLY_CORRECT_BLOCK_SIZE)
            }
        } else if opt_si {
            (DEFAULT_FILE_SIZE_BLOCK_SIZE, 1000)
        } else {
            (DEFAULT_FILE_SIZE_BLOCK_SIZE, DEFAULT_BLOCK_SIZE)
        };

        let long = {
            let author = options.get_flag(options::AUTHOR);
            let group = !options.get_flag(options::NO_GROUP)
                && !options.get_flag(options::format::LONG_NO_GROUP);
            let owner = !options.get_flag(options::format::LONG_NO_OWNER);
            let numeric_uid_gid = options.get_flag(options::format::LONG_NUMERIC_UID_GID);
            LongFormat {
                author,
                group,
                owner,
                numeric_uid_gid,
            }
        };
        let width = parse_width(options.get_one::<String>(options::WIDTH))?;

        #[allow(clippy::needless_bool)]
        let mut show_control = if options.get_flag(options::HIDE_CONTROL_CHARS) {
            false
        } else if options.get_flag(options::SHOW_CONTROL_CHARS) {
            true
        } else {
            !stdout().is_terminal()
        };

        let mut quoting_style = extract_quoting_style(options, show_control);
        let indicator_style = extract_indicator_style(options);

        let dired = options.get_flag(options::DIRED);
        let (time_format_recent, time_format_older) = if format == Format::Long || dired {
            parse_time_style(options)?
        } else {
            Default::default()
        };

        let mut ignore_patterns: Vec<Pattern> = Vec::new();

        if options.get_flag(options::IGNORE_BACKUPS) {
            ignore_patterns.push(Pattern::new("*~").unwrap());
            ignore_patterns.push(Pattern::new(".*~").unwrap());
        }

        for pattern in options
            .get_many::<String>(options::IGNORE)
            .into_iter()
            .flatten()
        {
            match parse_glob::from_str(pattern) {
                Ok(p) => {
                    ignore_patterns.push(p);
                }
                Err(_) => show_warning!(
                    "{}",
                    translate!("ls-invalid-ignore-pattern", "pattern" => pattern.quote())
                ),
            }
        }

        if files == Files::Normal {
            for pattern in options
                .get_many::<String>(options::HIDE)
                .into_iter()
                .flatten()
            {
                match parse_glob::from_str(pattern) {
                    Ok(p) => {
                        ignore_patterns.push(p);
                    }
                    Err(_) => show_warning!(
                        "{}",
                        translate!("ls-invalid-hide-pattern", "pattern" => pattern.quote())
                    ),
                }
            }
        }








        let zero_formats_opts = [
            options::format::ACROSS,
            options::format::COLUMNS,
            options::format::COMMAS,
            options::format::LONG,
            options::format::LONG_NO_GROUP,
            options::format::LONG_NO_OWNER,
            options::format::LONG_NUMERIC_UID_GID,
            options::format::ONE_LINE,
            options::FORMAT,
        ];
        let zero_colors_opts = [options::COLOR];
        let zero_show_control_opts = [options::HIDE_CONTROL_CHARS, options::SHOW_CONTROL_CHARS];
        let zero_quoting_style_opts = [
            QUOTING_STYLE,
            options::quoting::C,
            options::quoting::ESCAPE,
            options::quoting::LITERAL,
        ];
        let get_last = |flag: &str| -> usize {
            if options.value_source(flag) == Some(clap::parser::ValueSource::CommandLine) {
                options.index_of(flag).unwrap_or(0)
            } else {
                0
            }
        };
        if get_last(options::ZERO)
            > zero_formats_opts
                .into_iter()
                .map(get_last)
                .max()
                .unwrap_or(0)
        {
            format = if format == Format::Long {
                format
            } else {
                Format::OneLine
            };
        }
        if get_last(options::ZERO)
            > zero_colors_opts
                .into_iter()
                .map(get_last)
                .max()
                .unwrap_or(0)
        {
            needs_color = false;
        }
        if get_last(options::ZERO)
            > zero_show_control_opts
                .into_iter()
                .map(get_last)
                .max()
                .unwrap_or(0)
        {
            show_control = true;
        }
        if get_last(options::ZERO)
            > zero_quoting_style_opts
                .into_iter()
                .map(get_last)
                .max()
                .unwrap_or(0)
        {
            quoting_style = QuotingStyle::Literal { show_control };
        }

        let color = if needs_color {
            Some(LsColors::from_env().unwrap_or_default())
        } else {
            None
        };

        if dired || is_dired_arg_present() {



            format = Format::Long;
        }
        if dired && options.get_flag(options::ZERO) {
            return Err(Box::new(LsError::DiredAndZeroAreIncompatible));
        }

        let dereference = if options.get_flag(options::dereference::ALL) {
            Dereference::All
        } else if options.get_flag(options::dereference::ARGS) {
            Dereference::Args
        } else if options.get_flag(options::dereference::DIR_ARGS) {
            Dereference::DirArgs
        } else if options.get_flag(options::DIRECTORY)
            || indicator_style == IndicatorStyle::Classify
            || format == Format::Long
        {
            Dereference::None
        } else {
            Dereference::DirArgs
        };

        let tab_size = if needs_color {
            Some(0)
        } else {
            options
                .get_one::<String>(options::format::TAB_SIZE)
                .and_then(|size| size.parse::<usize>().ok())
                .or_else(|| std::env::var("TABSIZE").ok().and_then(|s| s.parse().ok()))
        }
        .unwrap_or(SPACES_IN_TAB);

        Ok(Self {
            format,
            files,
            sort,
            recursive: options.get_flag(options::RECURSIVE),
            reverse: options.get_flag(options::REVERSE),
            dereference,
            ignore_patterns,
            size_format,
            directory: options.get_flag(options::DIRECTORY),
            time,
            color,
            inode: options.get_flag(options::INODE),
            long,
            alloc_size: options.get_flag(options::size::ALLOCATION_SIZE),
            file_size_block_size,
            block_size,
            width,
            quoting_style,
            indicator_style,
            time_format_recent,
            time_format_older,
            group_directories_first: options.get_flag(options::GROUP_DIRECTORIES_FIRST),
            line_ending: LineEnding::from_zero_flag(options.get_flag(options::ZERO)),
            dired,
            hyperlink,
            tab_size,
            object_output: StardustOutputOptions::from_matches(options),
            object_fields: if let Some(field) = options.get_one::<String>("object_field") {
                vec![field.clone()]
            } else if let Some(fields) = options.get_many::<String>("object_fields") {
                fields.map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            },
        })
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result_with_exit_code(sg_app(), args, 2)?;

    // Handle --schema flag
    if matches.get_flag(stardust_output::ARG_SCHEMA) {
        let schema = stardust_output::create_schema(vec![
            ("entries", "array", Some("List of directory entries with file information")),
            ("count", "integer", Some("Total number of entries")),
            ("recursive", "boolean", Some("Whether recursive listing was enabled")),
        ]);
        return stardust_output::print_schema(schema)
            .map_err(|e| SGSimpleError::new(1, e.to_string()).into());
    }

    let config = Config::from(&matches)?;

    sgcore::pledge::apply_pledge(&["stdio", "rpath", "getpw"])?;

    let locs = matches
        .get_many::<OsString>(options::PATHS)
        .map_or_else(|| vec![Path::new(".")], |v| v.map(Path::new).collect());

    list(locs, &config)
}

pub fn sg_app() -> Command {
    let cmd = sgcore::clap_localization::configure_localized_command(
        Command::new(sgcore::util_name())
            .version(sgcore::crate_version!())
            .override_usage(format_usage(&translate!("ls-usage")))
            .about(translate!("ls-about"))
    )
    .infer_long_args(true)
    .disable_help_flag(true)
    .args_override_self(true)
    .arg(
        Arg::new(options::HELP)
            .long(options::HELP)
            .help(translate!("ls-help-print-help"))
            .action(ArgAction::Help)
    )

    .arg(
        Arg::new(options::FORMAT)
            .long(options::FORMAT)
            .help(translate!("ls-help-set-display-format"))
            .value_parser(ShortcutValueParser::new([
                "long",
                "verbose",
                "single-column",
                "columns",
                "vertical",
                "across",
                "horizontal",
                "commas",
            ]))
            .hide_possible_values(true)
            .require_equals(true)
            .overrides_with_all([
                options::FORMAT,
                options::format::COLUMNS,
                options::format::LONG,
                options::format::ACROSS,
                options::format::COLUMNS,
                options::DIRED,
            ])
    )
    .arg(
        Arg::new(options::format::COLUMNS)
            .short('C')
            .help(translate!("ls-help-display-files-columns"))
            .overrides_with_all([
                options::FORMAT,
                options::format::COLUMNS,
                options::format::LONG,
                options::format::ACROSS,
                options::format::COLUMNS,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::format::LONG)
            .short('l')
            .long(options::format::LONG)
            .help(translate!("ls-help-display-detailed-info"))
            .overrides_with_all([
                options::FORMAT,
                options::format::COLUMNS,
                options::format::LONG,
                options::format::ACROSS,
                options::format::COLUMNS,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::format::ACROSS)
            .short('x')
            .help(translate!("ls-help-list-entries-rows"))
            .overrides_with_all([
                options::FORMAT,
                options::format::COLUMNS,
                options::format::LONG,
                options::format::ACROSS,
                options::format::COLUMNS,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::format::TAB_SIZE)
            .short('T')
            .long(options::format::TAB_SIZE)
            .env("TABSIZE")
            .value_name("COLS")
            .help(translate!("ls-help-assume-tab-stops"))
    )
    .arg(
        Arg::new(options::format::COMMAS)
            .short('m')
            .help(translate!("ls-help-list-entries-commas"))
            .overrides_with_all([
                options::FORMAT,
                options::format::COLUMNS,
                options::format::LONG,
                options::format::ACROSS,
                options::format::COLUMNS,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::ZERO)
            .long(options::ZERO)
            .overrides_with(options::ZERO)
            .help(translate!("ls-help-list-entries-nul"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::DIRED)
            .long(options::DIRED)
            .short('D')
            .help(translate!("ls-help-generate-dired-output"))
            .action(ArgAction::SetTrue)
            .overrides_with(options::HYPERLINK)
    )
    .arg(
        Arg::new(options::HYPERLINK)
            .long(options::HYPERLINK)
            .help(translate!("ls-help-hyperlink-filenames"))
            .value_parser(ShortcutValueParser::new([
                PossibleValue::new("always").alias("yes").alias("force"),
                PossibleValue::new("auto").alias("tty").alias("if-tty"),
                PossibleValue::new("never").alias("no").alias("none"),
            ]))
            .require_equals(true)
            .num_args(0..=1)
            .default_missing_value("always")
            .default_value("never")
            .value_name("WHEN")
            .overrides_with(options::DIRED)
    )






    .arg(
        Arg::new(options::format::ONE_LINE)
            .short('1')
            .help(translate!("ls-help-list-one-file-per-line"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::format::LONG_NO_GROUP)
            .long("long-no-group")
            .help(translate!("ls-help-long-format-no-group"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::format::LONG_NO_OWNER)
            .short('g')
            .help(translate!("ls-help-long-no-owner"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::format::LONG_NUMERIC_UID_GID)
            .short('n')
            .long(options::format::LONG_NUMERIC_UID_GID)
            .help(translate!("ls-help-long-numeric-uid-gid"))
            .action(ArgAction::SetTrue)
    )

    .arg(
        Arg::new(QUOTING_STYLE)
            .long(QUOTING_STYLE)
            .help(translate!("ls-help-set-quoting-style"))
            .value_parser(ShortcutValueParser::new([
                PossibleValue::new("literal"),
                PossibleValue::new("shell"),
                PossibleValue::new("shell-escape"),
                PossibleValue::new("shell-always"),
                PossibleValue::new("shell-escape-always"),
                PossibleValue::new("c").alias("c-maybe"),
                PossibleValue::new("escape"),
            ]))
            .overrides_with_all([
                QUOTING_STYLE,
                options::quoting::LITERAL,
                options::quoting::ESCAPE,
                options::quoting::C,
            ])
    )
    .arg(
        Arg::new(options::quoting::LITERAL)
            .short('N')
            .long(options::quoting::LITERAL)
            .alias("l")
            .help(translate!("ls-help-literal-quoting-style"))
            .overrides_with_all([
                QUOTING_STYLE,
                options::quoting::LITERAL,
                options::quoting::ESCAPE,
                options::quoting::C,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::quoting::ESCAPE)
            .short('b')
            .long(options::quoting::ESCAPE)
            .help(translate!("ls-help-escape-quoting-style"))
            .overrides_with_all([
                QUOTING_STYLE,
                options::quoting::LITERAL,
                options::quoting::ESCAPE,
                options::quoting::C,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::quoting::C)
            .short('Q')
            .long(options::quoting::C)
            .help(translate!("ls-help-c-quoting-style"))
            .overrides_with_all([
                QUOTING_STYLE,
                options::quoting::LITERAL,
                options::quoting::ESCAPE,
                options::quoting::C,
            ])
            .action(ArgAction::SetTrue)
    )

    .arg(
        Arg::new(options::HIDE_CONTROL_CHARS)
            .short('q')
            .long(options::HIDE_CONTROL_CHARS)
            .help(translate!("ls-help-replace-control-chars"))
            .overrides_with_all([options::HIDE_CONTROL_CHARS, options::SHOW_CONTROL_CHARS])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::SHOW_CONTROL_CHARS)
            .long(options::SHOW_CONTROL_CHARS)
            .help(translate!("ls-help-show-control-chars"))
            .overrides_with_all([options::HIDE_CONTROL_CHARS, options::SHOW_CONTROL_CHARS])
            .action(ArgAction::SetTrue)
    )

    .arg(
        Arg::new(options::TIME)
            .long(options::TIME)
            .help(translate!("ls-help-show-time-field"))
            .value_name("field")
            .value_parser(ShortcutValueParser::new([
                PossibleValue::new("atime").alias("access").alias("use"),
                PossibleValue::new("ctime").alias("status"),
                PossibleValue::new("mtime").alias("modification"),
                PossibleValue::new("birth").alias("creation"),
            ]))
            .hide_possible_values(true)
            .require_equals(true)
            .overrides_with_all([options::TIME, options::time::ACCESS, options::time::CHANGE])
    )
    .arg(
        Arg::new(options::time::CHANGE)
            .short('c')
            .help(translate!("ls-help-time-change"))
            .overrides_with_all([options::TIME, options::time::ACCESS, options::time::CHANGE])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::time::ACCESS)
            .short('u')
            .help(translate!("ls-help-time-access"))
            .overrides_with_all([options::TIME, options::time::ACCESS, options::time::CHANGE])
            .action(ArgAction::SetTrue)
    )

    .arg(
        Arg::new(options::HIDE)
            .long(options::HIDE)
            .action(ArgAction::Append)
            .value_name("PATTERN")
            .help(translate!("ls-help-hide-pattern"))
    )
    .arg(
        Arg::new(options::IGNORE)
            .short('I')
            .long(options::IGNORE)
            .action(ArgAction::Append)
            .value_name("PATTERN")
            .help(translate!("ls-help-ignore-pattern"))
    )
    .arg(
        Arg::new(options::IGNORE_BACKUPS)
            .short('B')
            .long(options::IGNORE_BACKUPS)
            .help(translate!("ls-help-ignore-backups"))
            .action(ArgAction::SetTrue)
    )

    .arg(
        Arg::new(options::SORT)
            .long(options::SORT)
            .help(translate!("ls-help-sort-by-field"))
            .value_name("field")
            .value_parser(ShortcutValueParser::new([
                "name",
                "none",
                "time",
                "size",
                "version",
                "extension",
                "width",
            ]))
            .require_equals(true)
            .overrides_with_all([
                options::SORT,
                options::sort::SIZE,
                options::sort::TIME,
                options::sort::NONE,
                options::sort::VERSION,
                options::sort::EXTENSION,
            ])
    )
    .arg(
        Arg::new(options::sort::SIZE)
            .short('S')
            .help(translate!("ls-help-sort-by-size"))
            .overrides_with_all([
                options::SORT,
                options::sort::SIZE,
                options::sort::TIME,
                options::sort::NONE,
                options::sort::VERSION,
                options::sort::EXTENSION,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::sort::TIME)
            .short('t')
            .help(translate!("ls-help-sort-by-time"))
            .overrides_with_all([
                options::SORT,
                options::sort::SIZE,
                options::sort::TIME,
                options::sort::NONE,
                options::sort::VERSION,
                options::sort::EXTENSION,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::sort::VERSION)
            .long("sort-version")
            .help(translate!("ls-help-sort-by-version"))
            .overrides_with_all([
                options::SORT,
                options::sort::SIZE,
                options::sort::TIME,
                options::sort::NONE,
                options::sort::VERSION,
                options::sort::EXTENSION,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::sort::EXTENSION)
            .short('X')
            .help(translate!("ls-help-sort-by-extension"))
            .overrides_with_all([
                options::SORT,
                options::sort::SIZE,
                options::sort::TIME,
                options::sort::NONE,
                options::sort::VERSION,
                options::sort::EXTENSION,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::sort::NONE)
            .short('U')
            .help(translate!("ls-help-sort-none"))
            .overrides_with_all([
                options::SORT,
                options::sort::SIZE,
                options::sort::TIME,
                options::sort::NONE,
                options::sort::VERSION,
                options::sort::EXTENSION,
            ])
            .action(ArgAction::SetTrue)
    )

    .arg(
        Arg::new(options::dereference::ALL)
            .short('L')
            .long(options::dereference::ALL)
            .help(translate!("ls-help-dereference-all"))
            .overrides_with_all([
                options::dereference::ALL,
                options::dereference::DIR_ARGS,
                options::dereference::ARGS,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::dereference::DIR_ARGS)
            .long(options::dereference::DIR_ARGS)
            .help(translate!("ls-help-dereference-dir-args"))
            .overrides_with_all([
                options::dereference::ALL,
                options::dereference::DIR_ARGS,
                options::dereference::ARGS,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::dereference::ARGS)
            .short('H')
            .long(options::dereference::ARGS)
            .help(translate!("ls-help-dereference-args"))
            .overrides_with_all([
                options::dereference::ALL,
                options::dereference::DIR_ARGS,
                options::dereference::ARGS,
            ])
            .action(ArgAction::SetTrue)
    )

    .arg(
        Arg::new(options::NO_GROUP)
            .long(options::NO_GROUP)
            .short('G')
            .help(translate!("ls-help-no-group"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::AUTHOR)
            .long(options::AUTHOR)
            .help(translate!("ls-help-author"))
            .action(ArgAction::SetTrue)
    )

    .arg(
        Arg::new(options::files::ALL)
            .short('a')
            .long(options::files::ALL)

            .overrides_with_all([options::files::ALL, options::files::ALMOST_ALL])
            .help(translate!("ls-help-all-files"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::files::ALMOST_ALL)
            .short('A')
            .long(options::files::ALMOST_ALL)

            .overrides_with_all([options::files::ALL, options::files::ALMOST_ALL])
            .help(translate!("ls-help-almost-all"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::files::UNSORTED_ALL)
            .short('f')
            .help(translate!("ls-help-unsorted-all"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::DIRECTORY)
            .short('d')
            .long(options::DIRECTORY)
            .help(translate!("ls-help-directory"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::size::HUMAN_READABLE)
            .short('h')
            .long(options::size::HUMAN_READABLE)
            .help(translate!("ls-help-human-readable"))
            .overrides_with_all([options::size::BLOCK_SIZE, options::size::SI])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::size::KIBIBYTES)
            .short('k')
            .long(options::size::KIBIBYTES)
            .help(translate!("ls-help-kibibytes"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::size::SI)
            .long(options::size::SI)
            .help(translate!("ls-help-si"))
            .overrides_with_all([options::size::BLOCK_SIZE, options::size::HUMAN_READABLE])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::size::BLOCK_SIZE)
            .long(options::size::BLOCK_SIZE)
            .require_equals(true)
            .value_name("BLOCK_SIZE")
            .help(translate!("ls-help-block-size"))
            .overrides_with_all([options::size::SI, options::size::HUMAN_READABLE])
    )
    .arg(
        Arg::new(options::INODE)
            .short('i')
            .long(options::INODE)
            .help(translate!("ls-help-print-inode"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::REVERSE)
            .short('r')
            .long(options::REVERSE)
            .help(translate!("ls-help-reverse-sort"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::RECURSIVE)
            .short('R')
            .long(options::RECURSIVE)
            .help(translate!("ls-help-recursive"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::WIDTH)
            .long(options::WIDTH)
            .short('w')
            .help(translate!("ls-help-terminal-width"))
            .value_name("COLS")
    )
    .arg(
        Arg::new(options::size::ALLOCATION_SIZE)
            .short('s')
            .long(options::size::ALLOCATION_SIZE)
            .help(translate!("ls-help-allocation-size"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::COLOR)
            .long(options::COLOR)
            .help(translate!("ls-help-color-output"))
            .value_parser(ShortcutValueParser::new([
                PossibleValue::new("always").alias("yes").alias("force"),
                PossibleValue::new("auto").alias("tty").alias("if-tty"),
                PossibleValue::new("never").alias("no").alias("none"),
            ]))
            .require_equals(true)
            .num_args(0..=1)
    )
    .arg(
        Arg::new(options::INDICATOR_STYLE)
            .long(options::INDICATOR_STYLE)
            .help(translate!("ls-help-indicator-style"))
            .value_parser(ShortcutValueParser::new([
                "none",
                "slash",
                "file-type",
                "classify",
            ]))
            .overrides_with_all([
                options::indicator_style::FILE_TYPE,
                options::indicator_style::SLASH,
                options::indicator_style::CLASSIFY,
                options::INDICATOR_STYLE,
            ])
    )
    .arg(





        Arg::new(options::indicator_style::CLASSIFY)
            .short('F')
            .long(options::indicator_style::CLASSIFY)
            .help(translate!("ls-help-classify"))
            .value_name("when")
            .value_parser(ShortcutValueParser::new([
                PossibleValue::new("always").alias("yes").alias("force"),
                PossibleValue::new("auto").alias("tty").alias("if-tty"),
                PossibleValue::new("never").alias("no").alias("none"),
            ]))
            .default_missing_value("always")
            .require_equals(true)
            .num_args(0..=1)
            .overrides_with_all([
                options::indicator_style::FILE_TYPE,
                options::indicator_style::SLASH,
                options::indicator_style::CLASSIFY,
                options::INDICATOR_STYLE,
            ])
    )
    .arg(
        Arg::new(options::indicator_style::FILE_TYPE)
            .long(options::indicator_style::FILE_TYPE)
            .help(translate!("ls-help-file-type"))
            .overrides_with_all([
                options::indicator_style::FILE_TYPE,
                options::indicator_style::SLASH,
                options::indicator_style::CLASSIFY,
                options::INDICATOR_STYLE,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::indicator_style::SLASH)
            .short('p')
            .help(translate!("ls-help-slash-directories"))
            .overrides_with_all([
                options::indicator_style::FILE_TYPE,
                options::indicator_style::SLASH,
                options::indicator_style::CLASSIFY,
                options::INDICATOR_STYLE,
            ])
            .action(ArgAction::SetTrue)
    )
    .arg(

        Arg::new(options::TIME_STYLE)
            .long(options::TIME_STYLE)
            .help(translate!("ls-help-time-style"))
            .value_name("TIME_STYLE")
            .env("TIME_STYLE")
            .value_parser(NonEmptyStringValueParser::new())
            .overrides_with_all([options::TIME_STYLE])
    )
    .arg(
        Arg::new(options::FULL_TIME)
            .long(options::FULL_TIME)
            .overrides_with(options::FULL_TIME)
            .help(translate!("ls-help-full-time"))
            .action(ArgAction::SetTrue)
    )
    .arg(
        Arg::new(options::GROUP_DIRECTORIES_FIRST)
            .long(options::GROUP_DIRECTORIES_FIRST)
            .help(translate!("ls-help-group-directories-first"))
            .action(ArgAction::SetTrue)
    );


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
            Arg::new(stardust_output::ARG_SCHEMA)
                .long("schema")
                .help("Print JSON schema of output structure")
                .action(ArgAction::SetTrue)
                .hide(true),
        );

    let cmd = cmd
        .arg(
            Arg::new("object_field")
                .long("field")
                .value_name("FIELD")
                .help("Filter object output to a single field (use with -o)")
                .conflicts_with("object_fields")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("object_fields")
                .long("fields")
                .value_name("FIELD")
                .help("Filter object output to multiple fields (use with -o)")
                .conflicts_with("object_field")
                .action(ArgAction::Append),
        );

    cmd

    .arg(
        Arg::new(options::PATHS)
            .action(ArgAction::Append)
            .value_hint(clap::ValueHint::AnyPath)
            .value_parser(ValueParser::os_string())
    )
    .after_help(translate!("ls-after-help"))
}

/// Represents a Path along with it's associated data.
/// Any data that will be reused several times makes sense to be added to this structure.
/// Caching data here helps eliminate redundant syscalls to fetch same information.
#[derive(Debug)]
struct PathData {

    md: OnceCell<Option<Metadata>>,
    ft: OnceCell<Option<FileType>>,


    de: RefCell<Option<Box<DirEntry>>>,

    display_name: OsString,

    p_buf: PathBuf,
    must_dereference: bool,
    command_line: bool,
}

impl PathData {
    fn new(
        p_buf: PathBuf,
        dir_entry: Option<DirEntry>,
        file_name: Option<OsString>,
        config: &Config,
        command_line: bool
    ) -> Self {


        let display_name = if let Some(name) = file_name {
            name
        } else if command_line {
            p_buf.as_os_str().to_os_string()
        } else {
            dir_entry
                .as_ref()
                .map(|inner| inner.file_name())
                .unwrap_or_default()
        };

        let must_dereference = match &config.dereference {
            Dereference::All => true,
            Dereference::Args => command_line,
            Dereference::DirArgs => {
                if command_line {
                    if let Ok(md) = p_buf.metadata() {
                        md.is_dir()
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Dereference::None => false,
        };



        let ft: OnceCell<Option<FileType>> = OnceCell::new();
        let md: OnceCell<Option<Metadata>> = OnceCell::new();

        let de: RefCell<Option<Box<DirEntry>>> = if let Some(de) = dir_entry {
            if must_dereference {
                if let Ok(md_pb) = p_buf.metadata() {
                    md.get_or_init(|| Some(md_pb.clone()));
                    ft.get_or_init(|| Some(md_pb.file_type()));
                }
            }

            if let Ok(ft_de) = de.file_type() {
                ft.get_or_init(|| Some(ft_de));
            }

            RefCell::new(Some(de.into()))
        } else {
            RefCell::new(None)
        };

        Self {
            md,
            ft,
            de,
            display_name,
            p_buf,
            must_dereference,
            command_line,
        }
    }

    fn metadata(&self) -> Option<&Metadata> {
        self.md
            .get_or_init(|| {
                if !self.must_dereference {
                    if let Some(dir_entry) = RefCell::take(&self.de).as_deref() {
                        return dir_entry.metadata().ok();
                    }
                }

                match get_metadata_with_deref_opt(self.path(), self.must_dereference) {
                    Err(err) => {

                        let mut out: std::io::StdoutLock<'static> = stdout().lock();
                        let _ = out.flush();
                        let errno = err.raw_os_error().unwrap_or(1i32);




                        if self.must_dereference && errno == 9i32 {
                            if let Ok(file) = self.path().read_link() {
                                return file.symlink_metadata().ok();
                            }
                        }
                        show!(LsError::IOErrorContext(
                            self.path().to_path_buf(),
                            err,
                            self.command_line
                        ));
                        None
                    }
                    Ok(md) => Some(md),
                }
            })
            .as_ref()
    }

    fn file_type(&self) -> Option<&FileType> {
        self.ft
            .get_or_init(|| self.metadata().map(|md| md.file_type()))
            .as_ref()
    }

    fn is_dangling_link(&self) -> bool {

        self.must_dereference && self.file_type().is_none() && self.metadata().is_none()
    }

    fn is_executable_file(&self) -> bool {
        self.file_type().is_some_and(|f| f.is_file())
            && self.metadata().is_some_and(file_is_executable)
    }

    fn path(&self) -> &Path {
        &self.p_buf
    }

    fn display_name(&self) -> &OsStr {
        &self.display_name
    }
}

impl Colorable for PathData {
    fn file_name(&self) -> OsString {
        self.display_name().to_os_string()
    }
    fn file_type(&self) -> Option<FileType> {
        self.file_type().copied()
    }
    fn metadata(&self) -> Option<Metadata> {
        self.metadata().cloned()
    }
    fn path(&self) -> PathBuf {
        self.path().to_path_buf()
    }
}

/// Show the directory name in the case where several arguments are given to ls
/// or the recursive flag is passed.
///
/// ```no-exec
/// $ ls -R
/// .:                  <- This is printed by this function
/// dir1 file1 file2
///
/// dir1:               <- This as well
/// file11
/// ```
fn show_dir_name(
    path_data: &PathData,
    out: &mut BufWriter<Stdout>,
    config: &Config
) -> std::io::Result<()> {
    let escaped_name =
        locale_aware_escape_dir_name(path_data.path().as_os_str(), config.quoting_style);

    let name = if config.hyperlink && !config.dired {
        create_hyperlink(&escaped_name, path_data)
    } else {
        escaped_name
    };

    write_os_str(out, &name)?;
    write!(out, ":")
}


struct ListState<'a> {
    out: BufWriter<Stdout>,
    style_manager: Option<StyleManager<'a>>,





    uid_cache: HashMap<u32, String>,
    gid_cache: HashMap<u32, String>,
    recent_time_range: RangeInclusive<SystemTime>,
}

fn list_json(locs: Vec<&Path>, config: &Config) -> SGResult<()> {
    use serde_json::json;

    let mut all_entries = Vec::new();

    fn collect_entries(path: &Path, config: &Config, entries: &mut Vec<serde_json::Value>) -> SGResult<()> {
        let metadata = match path.metadata() {
            Ok(m) => m,
            Err(e) => {
                show_error!("cannot access '{}': {}", path.display(), e);
                return Ok(());
            }
        };

        if metadata.is_dir() && !config.directory {

            let read_dir = match fs::read_dir(path) {
                Ok(rd) => rd,
                Err(e) => {
                    show_error!("cannot open directory '{}': {}", path.display(), e);
                    return Ok(());
                }
            };

            for entry in read_dir {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        show_error!("error reading directory entry: {}", e);
                        continue;
                    }
                };

                let entry_path = entry.path();
                let entry_metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(e) => {
                        show_error!("cannot access '{}': {}", entry_path.display(), e);
                        continue;
                    }
                };

                let mut file_info = json!({
                    "path": entry_path.to_string_lossy(),
                    "name": entry.file_name().to_string_lossy(),
                    "type": if entry_metadata.is_dir() { "directory" }
                           else if entry_metadata.is_symlink() { "symlink" }
                           else if entry_metadata.is_file() { "file" }
                           else { "other" },
                    "size": entry_metadata.len(),
                });
                {
                    file_info["permissions"] = json!(format!("{:o}", entry_metadata.mode() & 0o777));
                    file_info["inode"] = json!(entry_metadata.ino());
                    file_info["nlink"] = json!(entry_metadata.nlink());
                    file_info["uid"] = json!(entry_metadata.uid());
                    file_info["gid"] = json!(entry_metadata.gid());
                }

                if let Ok(modified) = entry_metadata.modified() {
                    if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                        file_info["modified"] = json!(duration.as_secs());
                    }
                }



                if !config.object_fields.is_empty() {
                    let is_top_level = config.object_fields.iter().all(|f|
                        f == "entries" || f == "count" || f == "recursive"
                    );
                    if !is_top_level {
                        file_info = filter_object_fields(&file_info, &config.object_fields);
                    }
                }

                entries.push(file_info);


                if config.recursive && entry_metadata.is_dir() {
                    collect_entries(&entry_path, config, entries)?;
                }
            }
        } else {

            let mut file_info = json!({
                "path": path.to_string_lossy(),
                "name": path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                "type": if metadata.is_dir() { "directory" }
                       else if metadata.is_symlink() { "symlink" }
                       else if metadata.is_file() { "file" }
                       else { "other" },
                "size": metadata.len(),
            });
            {
                file_info["permissions"] = json!(format!("{:o}", metadata.mode() & 0o777));
                file_info["inode"] = json!(metadata.ino());
                file_info["nlink"] = json!(metadata.nlink());
                file_info["uid"] = json!(metadata.uid());
                file_info["gid"] = json!(metadata.gid());
            }

            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    file_info["modified"] = json!(duration.as_secs());
                }
            }



            if !config.object_fields.is_empty() {
                let is_top_level = config.object_fields.iter().all(|f|
                    f == "entries" || f == "count" || f == "recursive"
                );
                if !is_top_level {
                    file_info = filter_object_fields(&file_info, &config.object_fields);
                }
            }

            entries.push(file_info);
        }

        Ok(())
    }

    for loc in locs {
        collect_entries(loc, config, &mut all_entries)?;
    }




    let has_top_level_fields = !config.object_fields.is_empty() &&
        config.object_fields.iter().any(|f|
            f == "entries" || f == "count" || f == "recursive"
        );

    let mut output = json!({
        "entries": all_entries,
        "count": all_entries.len(),
        "recursive": config.recursive,
    });


    if has_top_level_fields {
        output = filter_object_fields(&output, &config.object_fields);
    }

    Ok(stardust_output::output(config.object_output, output, || Ok(()))?)
}

fn filter_object_fields(value: &serde_json::Value, fields: &[String]) -> serde_json::Value {
    if let serde_json::Value::Object(map) = value {
        let mut result = serde_json::Map::new();
        for field in fields {
            if let Some(v) = map.get(field) {
                result.insert(field.clone(), v.clone());
            }
        }
        serde_json::Value::Object(result)
    } else {
        value.clone()
    }
}

#[allow(clippy::cognitive_complexity)]
pub fn list(locs: Vec<&Path>, config: &Config) -> SGResult<()> {

    if config.object_output.stardust_output {
        return list_json(locs, config);
    }

    let mut files = Vec::<PathData>::new();
    let mut dirs = Vec::<PathData>::new();
    let mut dired = DiredOutput::default();
    let initial_locs_len = locs.len();

    let mut state = ListState {
        out: BufWriter::new(stdout()),
        style_manager: config.color.as_ref().map(StyleManager::new),
        uid_cache: HashMap::default(),
        gid_cache: HashMap::default(),



        recent_time_range: (SystemTime::now() - Duration::new(31_556_952 / 2, 0))
            ..=SystemTime::now(),
    };

    for loc in locs {
        let path_data = PathData::new(PathBuf::from(loc), None, None, config, true);







        if path_data.metadata().is_none() {
            continue;
        }

        let show_dir_contents = match path_data.file_type() {
            Some(ft) => !config.directory && ft.is_dir(),
            None => {
                set_exit_code(1);
                false
            }
        };

        if show_dir_contents {
            dirs.push(path_data);
        } else {
            files.push(path_data);
        }
    }

    sort_entries(&mut files, config);
    sort_entries(&mut dirs, config);

    if let Some(style_manager) = state.style_manager.as_mut() {


        if style_manager.get_normal_style().is_some() {
            let to_write = style_manager.reset(true);
            write!(state.out, "{to_write}")?;
        }
    }

    display_items(&files, config, &mut state, &mut dired)?;

    for (pos, path_data) in dirs.iter().enumerate() {


        let read_dir = match fs::read_dir(path_data.path()) {
            Err(err) => {

                state.out.flush()?;
                show!(LsError::IOErrorContext(
                    path_data.path().to_path_buf(),
                    err,
                    path_data.command_line
                ));
                continue;
            }
            Ok(rd) => rd,
        };


        if initial_locs_len > 1 || config.recursive {
            if pos.eq(&0usize) && files.is_empty() {
                if config.dired {
                    dired::indent(&mut state.out)?;
                }
                show_dir_name(path_data, &mut state.out, config)?;
                writeln!(state.out)?;
                if config.dired {

                    let dir_len = path_data.display_name().len();

                    dired::calculate_subdired(&mut dired, dir_len);

                    dired::add_dir_name(&mut dired, dir_len);
                }
            } else {
                writeln!(state.out)?;
                show_dir_name(path_data, &mut state.out, config)?;
                writeln!(state.out)?;
            }
        }
        let mut listed_ancestors = HashSet::default();
        listed_ancestors.insert(FileInformation::from_path(
            path_data.path(),
            path_data.must_dereference
        )?);
        enter_directory(
            path_data,
            read_dir,
            config,
            &mut state,
            &mut listed_ancestors,
            &mut dired
        )?;
    }
    if config.dired && !config.hyperlink {
        dired::print_dired_output(config, &dired, &mut state.out)?;
    }
    Ok(())
}

fn sort_entries(entries: &mut [PathData], config: &Config) {
    match config.sort {
        Sort::Time => entries.sort_by_key(|k| {
            Reverse(
                k.metadata()
                    .and_then(|md| metadata_get_time(md, config.time))
                    .unwrap_or(UNIX_EPOCH)
            )
        }),
        Sort::Size => {
            entries.sort_by_key(|k| Reverse(k.metadata().map_or(0, |md| md.len())));
        }

        Sort::Name => entries.sort_by(|a, b| a.display_name().cmp(b.display_name())),
        Sort::Version => entries.sort_by(|a, b| {
            version_cmp(
                os_str_as_bytes_lossy(a.path().as_os_str()).as_ref(),
                os_str_as_bytes_lossy(b.path().as_os_str()).as_ref()
            )
            .then(a.path().to_string_lossy().cmp(&b.path().to_string_lossy()))
        }),
        Sort::Extension => entries.sort_by(|a, b| {
            a.path()
                .extension()
                .cmp(&b.path().extension())
                .then(a.path().file_stem().cmp(&b.path().file_stem()))
        }),
        Sort::Width => entries.sort_by(|a, b| {
            a.display_name()
                .len()
                .cmp(&b.display_name().len())
                .then(a.display_name().cmp(b.display_name()))
        }),
        Sort::None => {}
    }

    if config.reverse {
        entries.reverse();
    }

    if config.group_directories_first && config.sort != Sort::None {
        entries.sort_by_key(|p| {
            let ft = {


                if p.must_dereference {
                    p.file_type()
                } else {
                    None
                }
            };

            !match ft {
                None => {

                    get_metadata_with_deref_opt(p.p_buf.as_path(), true)
                        .map_or_else(|_| false, |m| m.is_dir())
                }
                Some(ft) => ft.is_dir(),
            }
        });
    }
}

fn is_hidden(file_path: &DirEntry) -> bool {
        file_path
            .file_name()
            .to_str()
            .is_some_and(|res| res.starts_with('.'))
}

fn should_display(entry: &DirEntry, config: &Config) -> bool {

    if config.files == Files::Normal && is_hidden(entry) {
        return false;
    }


    let options = MatchOptions {

        require_literal_leading_dot: true,
        require_literal_separator: false,
        case_sensitive: true,
    };

    let file_name = entry.file_name();






    let file_name = match file_name.to_str() {
        Some(s) => Cow::Borrowed(s),
        None => file_name.to_string_lossy(),
    };

    !config
        .ignore_patterns
        .iter()
        .any(|p| p.matches_with(&file_name, options))
}

#[allow(clippy::cognitive_complexity)]
fn enter_directory(
    path_data: &PathData,
    read_dir: ReadDir,
    config: &Config,
    state: &mut ListState,
    listed_ancestors: &mut HashSet<FileInformation>,
    dired: &mut DiredOutput
) -> SGResult<()> {

    let mut entries: Vec<PathData> = if config.files == Files::All {
        vec![
            PathData::new(
                path_data.path().to_path_buf(),
                None,
                Some(".".into()),
                config,
                false
            ),
            PathData::new(
                path_data.path().join(".."),
                None,
                Some("..".into()),
                config,
                false
            ),
        ]
    } else {
        vec![]
    };


    for raw_entry in read_dir {
        let dir_entry = match raw_entry {
            Ok(path) => path,
            Err(err) => {
                state.out.flush()?;
                show!(LsError::IOError(err));
                continue;
            }
        };

        if should_display(&dir_entry, config) {
            let entry_path_data =
                PathData::new(dir_entry.path(), Some(dir_entry), None, config, false);
            entries.push(entry_path_data);
        }
    }

    sort_entries(&mut entries, config);


    if config.format == Format::Long || config.alloc_size {
        let total = return_total(&entries, config, &mut state.out)?;
        write!(state.out, "{}", total.as_str())?;
        if config.dired {
            dired::add_total(dired, total.len());
        }
    }

    display_items(&entries, config, state, dired)?;

    if config.recursive {
        for e in entries
            .iter()
            .skip(if config.files == Files::All { 2 } else { 0 })
            .filter(|p| p.file_type().is_some_and(|ft| ft.is_dir()))
        {
            match fs::read_dir(e.path()) {
                Err(err) => {
                    state.out.flush()?;
                    show!(LsError::IOErrorContext(
                        e.path().to_path_buf(),
                        err,
                        e.command_line
                    ));
                }
                Ok(rd) => {
                    if listed_ancestors
                        .insert(FileInformation::from_path(e.path(), e.must_dereference)?)
                    {


                        writeln!(state.out)?;
                        if config.dired {



                            dired.padding = 2;
                            dired::indent(&mut state.out)?;
                            let dir_name_size = e.path().to_string_lossy().len();
                            dired::calculate_subdired(dired, dir_name_size);

                            dired::add_dir_name(dired, dir_name_size);
                        }

                        show_dir_name(e, &mut state.out, config)?;
                        writeln!(state.out)?;
                        enter_directory(e, rd, config, state, listed_ancestors, dired)?;
                        listed_ancestors
                            .remove(&FileInformation::from_path(e.path(), e.must_dereference)?);
                    } else {
                        state.out.flush()?;
                        show!(LsError::AlreadyListedError(e.path().to_path_buf()));
                    }
                }
            }
        }
    }

    Ok(())
}

fn get_metadata_with_deref_opt(p_buf: &Path, dereference: bool) -> std::io::Result<Metadata> {
    if dereference {
        p_buf.metadata()
    } else {
        p_buf.symlink_metadata()
    }
}

fn display_dir_entry_size(
    entry: &PathData,
    config: &Config,
    state: &mut ListState
) -> (usize, usize, usize, usize, usize, usize) {

    if let Some(md) = entry.metadata() {
        let (size_len, major_len, minor_len) = match display_len_or_rdev(md, config) {
            SizeOrDeviceId::Device(major, minor) => {
                (major.len() + minor.len() + 2usize, major.len(), minor.len())
            }
            SizeOrDeviceId::Size(size) => (size.len(), 0usize, 0usize),
        };
        (
            display_symlink_count(md).len(),
            display_uname(md, config, state).len(),
            display_group(md, config, state).len(),
            size_len,
            major_len,
            minor_len
        )
    } else {
        (0, 0, 0, 0, 0, 0)
    }
}



trait ExtendPad {
    fn extend_pad_left(&mut self, string: &str, count: usize);
    fn extend_pad_right(&mut self, string: &str, count: usize);
}

impl ExtendPad for Vec<u8> {
    fn extend_pad_left(&mut self, string: &str, count: usize) {
        if string.len() < count {
            self.extend(iter::repeat_n(b' ', count - string.len()));
        }
        self.extend(string.as_bytes());
    }

    fn extend_pad_right(&mut self, string: &str, count: usize) {
        self.extend(string.as_bytes());
        if string.len() < count {
            self.extend(iter::repeat_n(b' ', count - string.len()));
        }
    }
}



fn pad_left(string: &str, count: usize) -> String {
    format!("{string:>count$}")
}

fn return_total(
    items: &[PathData],
    config: &Config,
    out: &mut BufWriter<Stdout>
) -> SGResult<String> {
    let mut total_size = 0;
    for item in items {
        total_size += item
            .metadata()
            .as_ref()
            .map_or(0, |md| get_block_size(md, config));
    }
    if config.dired {
        dired::indent(out)?;
    }
    Ok(format!(
        "{}{}",
        translate!("ls-total", "size" => display_size(total_size, config)),
        config.line_ending
    ))
}

fn display_additional_leading_info(
    item: &PathData,
    padding: &PaddingCollection,
    config: &Config
) -> SGResult<String> {
    let mut result = String::new();
    {
        if config.inode {
            let i = if let Some(md) = item.metadata() {
                get_inode(md)
            } else {
                "?".to_owned()
            };
            write!(result, "{} ", pad_left(&i, padding.inode)).unwrap();
        }
    }

    if config.alloc_size {
        let s = if let Some(md) = item.metadata() {
            display_size(get_block_size(md, config), config)
        } else {
            "?".to_owned()
        };

        if config.format == Format::Commas {
            write!(result, "{s} ").unwrap();
        } else {
            write!(result, "{} ", pad_left(&s, padding.block_size)).unwrap();
        }
    }
    Ok(result)
}

#[allow(clippy::cognitive_complexity)]
fn display_items(
    items: &[PathData],
    config: &Config,
    state: &mut ListState,
    dired: &mut DiredOutput
) -> SGResult<()> {

    let quoted = items.iter().any(|item| {
        let name = locale_aware_escape_name(item.display_name(), config.quoting_style);
        os_str_starts_with(&name, b"'")
    });

    if config.format == Format::Long {
        let padding_collection = calculate_padding_collection(items, config, state);

        for item in items {
            let should_display_leading_info = config.inode || config.alloc_size;
            #[cfg(not(unix))]
            let should_display_leading_info = config.alloc_size;

            if should_display_leading_info {
                let more_info = display_additional_leading_info(item, &padding_collection, config)?;

                write!(state.out, "{more_info}")?;
            }

            display_item_long(item, &padding_collection, config, state, dired, quoted)?;
        }
    } else {
        let padding = calculate_padding_collection(items, config, state);


        if let Some(style_manager) = &mut state.style_manager {
            write!(state.out, "{}", style_manager.apply_normal())?;
        }

        let mut names_vec = Vec::new();
        let should_display_leading_info = config.inode || config.alloc_size;
        #[cfg(not(unix))]
        let should_display_leading_info = config.alloc_size;

        for i in items {
            let more_info = if should_display_leading_info {
                Some(display_additional_leading_info(i, &padding, config)?)
            } else {
                None
            };




            let cell = display_item_name(
                i,
                config,
                more_info,
                state,
                LazyCell::new(Box::new(|| 0))
            );

            names_vec.push(cell);
        }

        let mut names = names_vec.into_iter();

        match config.format {
            Format::Columns => {
                display_grid(
                    names,
                    config.width,
                    Direction::TopToBottom,
                    &mut state.out,
                    quoted,
                    config.tab_size
                )?;
            }
            Format::Across => {
                display_grid(
                    names,
                    config.width,
                    Direction::LeftToRight,
                    &mut state.out,
                    quoted,
                    config.tab_size
                )?;
            }
            Format::Commas => {
                let mut current_col = 0;
                if let Some(name) = names.next() {
                    write_os_str(&mut state.out, &name)?;
                    current_col = ansi_width(&name.to_string_lossy()) as u16 + 2;
                }
                for name in names {
                    let name_width = ansi_width(&name.to_string_lossy()) as u16;

                    if config.width != 0 && current_col + name_width + 1 > config.width {
                        current_col = name_width + 2;
                        writeln!(state.out, ",")?;
                    } else {
                        current_col += name_width + 2;
                        write!(state.out, ", ")?;
                    }
                    write_os_str(&mut state.out, &name)?;
                }


                if current_col > 0 {
                    write!(state.out, "{}", config.line_ending)?;
                }
            }
            _ => {
                for name in names {
                    write_os_str(&mut state.out, &name)?;
                    write!(state.out, "{}", config.line_ending)?;
                }
            }
        }
    }

    Ok(())
}

#[allow(unused_variables)]
fn get_block_size(md: &Metadata, config: &Config) -> u64 {
    /* GNU ls will display sizes in terms of block size
       md.len() will differ from this value when the file has some holes
    */
    {
        let raw_blocks = if md.file_type().is_char_device() || md.file_type().is_block_device() {
            0u64
        } else {
            md.blocks() * 512
        };
        match config.size_format {
            SizeFormat::Binary | SizeFormat::Decimal => raw_blocks,
            SizeFormat::Bytes => raw_blocks / config.block_size,
        }
    }
    #[cfg(not(unix))]
    {

        md.len()
    }
}

fn display_grid(
    names: impl Iterator<Item = OsString>,
    width: u16,
    direction: Direction,
    out: &mut BufWriter<Stdout>,
    quoted: bool,
    tab_size: usize
) -> SGResult<()> {
    if width == 0 {

        let mut printed_something = false;
        for name in names {
            if printed_something {
                write!(out, "  ")?;
            }
            printed_something = true;
            write_os_str(out, &name)?;
        }
        if printed_something {
            writeln!(out)?;
        }
    } else {
        let names: Vec<_> = if quoted {












            names
                .map(|n| {
                    if os_str_starts_with(&n, b"'") || os_str_starts_with(&n, b"\"") {
                        n
                    } else {
                        let mut ret: OsString = " ".into();
                        ret.push(n);
                        ret
                    }
                })
                .collect()
        } else {
            names.collect()
        };


        let names: Vec<_> = names
            .into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();


        let filling = match tab_size {
            0 => Filling::Spaces(DEFAULT_SEPARATOR_SIZE),
            _ => Filling::Tabs {
                spaces: DEFAULT_SEPARATOR_SIZE,
                tab_size,
            },
        };

        let grid = Grid::new(
            names,
            GridOptions {
                filling,
                direction,
                width: width as usize,
            }
        );
        write!(out, "{grid}")?;
    }
    Ok(())
}

/// This writes to the [`BufWriter`] `state.out` a single string of the output of `ls -l`.
///
/// It writes the following keys, in order:
/// * `inode` ([`get_inode`], config-optional)
/// * `permissions` ([`display_permissions`])
/// * `symlink_count` ([`display_symlink_count`])
/// * `owner` ([`display_uname`], config-optional)
/// * `group` ([`display_group`], config-optional)
/// * `author` ([`display_uname`], config-optional)
/// * `size / rdev` ([`display_len_or_rdev`])
/// * `system_time` ([`display_date`])
/// * `item_name` ([`display_item_name`])
///
/// This function needs to display information in columns:
/// * permissions and `system_time` are already guaranteed to be pre-formatted in fixed length.
/// * `item_name` is the last column and is left-aligned.
/// * Everything else needs to be padded using [`pad_left`].
///
/// That's why we have the parameters:
/// ```txt
///    longest_link_count_len: usize,
///    longest_uname_len: usize,
///    longest_group_len: usize,
///    longest_context_len: usize,
///    longest_size_len: usize,
/// ```
/// that decide the maximum possible character count of each field.
#[allow(clippy::write_literal)]
#[allow(clippy::cognitive_complexity)]
fn display_item_long(
    item: &PathData,
    padding: &PaddingCollection,
    config: &Config,
    state: &mut ListState,
    dired: &mut DiredOutput,
    quoted: bool
) -> SGResult<()> {
    let mut output_display: Vec<u8> = Vec::with_capacity(128);


    if let Some(style_manager) = &mut state.style_manager {
        output_display.extend(style_manager.apply_normal().as_bytes());
    }
    if config.dired {
        output_display.extend(b"  ");
    }
    if let Some(md) = item.metadata() {

        output_display.extend(display_permissions(md, true).as_bytes());
        output_display.extend(b" ");
        output_display.extend_pad_left(&display_symlink_count(md), padding.link_count);

        if config.long.owner {
            output_display.extend(b" ");
            output_display.extend_pad_right(display_uname(md, config, state), padding.uname);
        }

        if config.long.group {
            output_display.extend(b" ");
            output_display.extend_pad_right(display_group(md, config, state), padding.group);
        }




        if config.long.author {
            output_display.extend(b" ");
            output_display.extend_pad_right(display_uname(md, config, state), padding.uname);
        }

        match display_len_or_rdev(md, config) {
            SizeOrDeviceId::Size(size) => {
                output_display.extend(b" ");
                output_display.extend_pad_left(&size, padding.size);
            }
            SizeOrDeviceId::Device(major, minor) => {
                output_display.extend(b" ");
                output_display.extend_pad_left(
                    &major,
                    #[cfg(not(unix))]
                    0usize,
                    padding.major.max(
                        padding
                            .size
                            .saturating_sub(padding.minor.saturating_add(2usize))
                    )
                );
                output_display.extend(b", ");
                output_display.extend_pad_left(
                    &minor,
                    #[cfg(not(unix))]
                    0usize,
                    padding.minor
                );
            }
        }

        output_display.extend(b" ");
        display_date(md, config, state, &mut output_display)?;
        output_display.extend(b" ");

        let item_name = display_item_name(
            item,
            config,
            None,
            state,
            LazyCell::new(Box::new(|| {
                ansi_width(&String::from_utf8_lossy(&output_display))
            }))
        );

        let displayed_item = if quoted && !os_str_starts_with(&item_name, b"'") {
            let mut ret: OsString = " ".into();
            ret.push(item_name);
            ret
        } else {
            item_name
        };

        if config.dired {
            let (start, end) = dired::calculate_dired(
                &dired.dired_positions,
                output_display.len(),
                displayed_item.len()
            );
            dired::update_positions(dired, start, end);
        }
        write_os_str(&mut output_display, &displayed_item)?;
        output_display.extend(config.line_ending.to_string().as_bytes());
    } else {
        let leading_char = {
            if let Some(ft) = item.file_type() {
                if ft.is_symlink() {
                    "l"
                } else if ft.is_dir() {
                    "d"
                } else {
                    "-"
                }
            } else if item.is_dangling_link() {
                "l"
            } else {
                "-"
            }
        };

        output_display.extend(leading_char.as_bytes());
        output_display.extend(b"?????????");

        output_display.extend(b" ");
        output_display.extend_pad_left("?", padding.link_count);

        if config.long.owner {
            output_display.extend(b" ");
            output_display.extend_pad_right("?", padding.uname);
        }

        if config.long.group {
            output_display.extend(b" ");
            output_display.extend_pad_right("?", padding.group);
        }



        if config.long.author {
            output_display.extend(b" ");
            output_display.extend_pad_right("?", padding.uname);
        }

        let displayed_item = display_item_name(
            item,
            config,
            None,
            state,
            LazyCell::new(Box::new(|| {
                ansi_width(&String::from_utf8_lossy(&output_display))
            }))
        );
        let date_len = 12;

        output_display.extend(b" ");
        output_display.extend_pad_left("?", padding.size);
        output_display.extend(b" ");
        output_display.extend_pad_left("?", date_len);
        output_display.extend(b" ");

        if config.dired {
            dired::calculate_and_update_positions(
                dired,
                output_display.len(),
                displayed_item.to_string_lossy().trim().len()
            );
        }
        write_os_str(&mut output_display, &displayed_item)?;
        output_display.extend(config.line_ending.to_string().as_bytes());
    }
    state.out.write_all(&output_display)?;

    Ok(())
}
fn get_inode(metadata: &Metadata) -> String {
    format!("{}", metadata.ino())
}



fn display_uname<'a>(metadata: &Metadata, config: &Config, state: &'a mut ListState) -> &'a String {
    let uid = metadata.uid();

    state.uid_cache.entry(uid).or_insert_with(|| {
        if config.long.numeric_uid_gid {
            uid.to_string()
        } else {
            entries::uid2usr(uid).unwrap_or_else(|_| uid.to_string())
        }
    })
}
fn display_group<'a>(metadata: &Metadata, config: &Config, state: &'a mut ListState) -> &'a String {
    let gid = metadata.gid();
    state.gid_cache.entry(gid).or_insert_with(|| {
        if config.long.numeric_uid_gid {
            gid.to_string()
        } else {
            entries::gid2grp(gid).unwrap_or_else(|_| gid.to_string())
        }
    })
}

fn display_date(
    metadata: &Metadata,
    config: &Config,
    state: &mut ListState,
    out: &mut Vec<u8>
) -> SGResult<()> {
    let Some(time) = metadata_get_time(metadata, config.time) else {
        out.extend(b"???");
        return Ok(());
    };



    let fmt = match &config.time_format_older {
        Some(time_format_older) if !state.recent_time_range.contains(&time) => time_format_older,
        _ => &config.time_format_recent,
    };

    format_system_time(out, time, fmt, FormatSystemTimeFallback::Integer)
}

#[allow(dead_code)]
enum SizeOrDeviceId {
    Size(String),
    Device(String, String),
}

fn display_len_or_rdev(metadata: &Metadata, config: &Config) -> SizeOrDeviceId {
    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd"
    ))]
    {
        let ft = metadata.file_type();
        if ft.is_char_device() || ft.is_block_device() {

            let dev = metadata.rdev() as dev_t;
            let major = major(dev);
            let minor = minor(dev);
            return SizeOrDeviceId::Device(major.to_string(), minor.to_string());
        }
    }
    let len_adjusted = {
        let d = metadata.len() / config.file_size_block_size;
        let r = metadata.len() % config.file_size_block_size;
        if r == 0 { d } else { d + 1 }
    };
    SizeOrDeviceId::Size(display_size(len_adjusted, config))
}

fn display_size(size: u64, config: &Config) -> String {
    human_readable(size, config.size_format)
}
fn file_is_executable(md: &Metadata) -> bool {





    #[allow(clippy::unnecessary_cast)]
    return md.mode() & ((S_IXUSR | S_IXGRP | S_IXOTH) as u32) != 0;
}

fn classify_file(path: &PathData) -> Option<char> {
    let file_type = path.file_type()?;

    if file_type.is_dir() {
        Some('/')
    } else if file_type.is_symlink() {
        Some('@')
    } else {
        {
            if file_type.is_socket() {
                Some('=')
            } else if file_type.is_fifo() {
                Some('|')


            } else if path.is_executable_file() {
                Some('*')
            } else {
                None
            }
        }
        #[cfg(not(unix))]
        None
    }
}

/// Takes a [`PathData`] struct and returns a cell with a name ready for displaying.
///
/// This function relies on the following parameters in the provided `&Config`:
/// * `config.quoting_style` to decide how we will escape `name` using [`locale_aware_escape_name`].
/// * `config.inode` decides whether to display inode numbers beside names using [`get_inode`].
/// * `config.color` decides whether it's going to color `name` using [`color_name`].
/// * `config.indicator_style` to append specific characters to `name` using [`classify_file`].
/// * `config.format` to display symlink targets if `Format::Long`. This function is also
///   responsible for coloring symlink target names if `config.color` is specified.
/// * `config.hyperlink` decides whether to hyperlink the item
///
/// Note that non-unicode sequences in symlink targets are dealt with using
/// [`std::path::Path::to_string_lossy`].
#[allow(clippy::cognitive_complexity)]
fn display_item_name(
    path: &PathData,
    config: &Config,
    more_info: Option<String>,
    state: &mut ListState,
    current_column: LazyCell<usize, Box<dyn FnOnce() -> usize + '_>>
) -> OsString {

    let mut name = locale_aware_escape_name(path.display_name(), config.quoting_style);

    let is_wrap =
        |namelen: usize| config.width != 0 && *current_column + namelen > config.width.into();

    if config.hyperlink {
        name = create_hyperlink(&name, path);
    }

    if let Some(style_manager) = &mut state.style_manager {
        let len = name.len();
        name = color_name(name, path, style_manager, None, is_wrap(len));
    }

    if config.format != Format::Long {
        if let Some(info) = more_info {
            let old_name = name;
            name = info.into();
            name.push(&old_name);
        }
    }

    if config.indicator_style != IndicatorStyle::None {
        let sym = classify_file(path);

        let char_opt = match config.indicator_style {
            IndicatorStyle::Classify => sym,
            IndicatorStyle::FileType => {

                match sym {
                    Some('*') => None,
                    _ => sym,
                }
            }
            IndicatorStyle::Slash => {

                match sym {
                    Some('/') => Some('/'),
                    _ => None,
                }
            }
            IndicatorStyle::None => None,
        };

        if let Some(c) = char_opt {
            name.push(OsStr::new(&c.to_string()));
        }
    }

    if config.format == Format::Long
        && path.file_type().is_some_and(|ft| ft.is_symlink())
        && !path.must_dereference
    {
        match path.path().read_link() {
            Ok(target_path) => {
                name.push(" -> ");




                if let Some(style_manager) = &mut state.style_manager {


                    let mut absolute_target = target_path.clone();
                    if target_path.is_relative() {
                        if let Some(parent) = path.path().parent() {
                            absolute_target = parent.join(absolute_target);
                        }
                    }

                    let target_data = PathData::new(absolute_target, None, None, config, false);





                    if path.metadata().is_none() && target_data.metadata().is_none() {
                        name.push(target_path);
                    } else {
                        name.push(color_name(
                            locale_aware_escape_name(target_path.as_os_str(), config.quoting_style),
                            path,
                            style_manager,
                            Some(&target_data),
                            is_wrap(name.len())
                        ));
                    }
                } else {


                    name.push(locale_aware_escape_name(
                        target_path.as_os_str(),
                        config.quoting_style
                    ));
                }
            }
            Err(err) => {
                show!(LsError::IOErrorContext(
                    path.path().to_path_buf(),
                    err,
                    false
                ));
            }
        }
    }

    name
}

fn create_hyperlink(name: &OsStr, path: &PathData) -> OsString {
    let hostname = hostname::get().unwrap_or_else(|_| OsString::from(""));
    let hostname = hostname.to_string_lossy();

    let absolute_path = fs::canonicalize(path.path()).unwrap_or_default();
    let absolute_path = absolute_path.to_string_lossy();

    let unencoded_chars = "_-.:~/";


    let absolute_path: String = absolute_path
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || unencoded_chars.contains(c) {
                c.to_string()
            } else {
                format!("%{:02x}", c as u8)
            }
        })
        .collect();


    let mut ret: OsString = format!("\x1b]8;;file://{hostname}{absolute_path}\x07").into();
    ret.push(name);
    ret.push("\x1b]8;;\x07");
    ret
}

#[cfg(not(unix))]
fn display_symlink_count(_metadata: &Metadata) -> String {


    String::from("1")
}
fn display_symlink_count(metadata: &Metadata) -> String {
    metadata.nlink().to_string()
}
fn display_inode(metadata: &Metadata) -> String {
    get_inode(metadata)
}
fn calculate_padding_collection(
    items: &[PathData],
    config: &Config,
    state: &mut ListState
) -> PaddingCollection {
    let mut padding_collections = PaddingCollection {
        inode: 1,
        link_count: 1,
        uname: 1,
        group: 1,
        size: 1,
        major: 1,
        minor: 1,
        block_size: 1,
    };

    for item in items {
        if config.inode {
            let inode_len = if let Some(md) = item.metadata() {
                display_inode(md).len()
            } else {
                continue;
            };
            padding_collections.inode = inode_len.max(padding_collections.inode);
        }

        if config.alloc_size {
            if let Some(md) = item.metadata() {
                let block_size_len = display_size(get_block_size(md, config), config).len();
                padding_collections.block_size = block_size_len.max(padding_collections.block_size);
            }
        }

        if config.format == Format::Long {
            let (link_count_len, uname_len, group_len, size_len, major_len, minor_len) =
                display_dir_entry_size(item, config, state);
            padding_collections.link_count = link_count_len.max(padding_collections.link_count);
            padding_collections.uname = uname_len.max(padding_collections.uname);
            padding_collections.group = group_len.max(padding_collections.group);

            if items.len() == 1usize {
                padding_collections.size = 0usize;
                padding_collections.major = 0usize;
                padding_collections.minor = 0usize;
            } else {
                padding_collections.major = major_len.max(padding_collections.major);
                padding_collections.minor = minor_len.max(padding_collections.minor);
                padding_collections.size = size_len
                    .max(padding_collections.size)
                    .max(padding_collections.major);
            }
        }
    }

    padding_collections
}

fn os_str_starts_with(haystack: &OsStr, needle: &[u8]) -> bool {
    os_str_as_bytes_lossy(haystack).starts_with(needle)
}

fn write_os_str<W: Write>(writer: &mut W, string: &OsStr) -> std::io::Result<()> {
    writer.write_all(&os_str_as_bytes_lossy(string))
}

