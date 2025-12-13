

mod error;

use clap::{Arg, ArgAction, Command};
use memchr::memmem;
use memmap2::Mmap;
use std::ffi::OsString;
use std::io::{BufWriter, Read, Write, stdin, stdout};
use std::{
    fs::{File, read},
    path::Path,
};
use sgcore::error::SGError;
use sgcore::error::SGResult;
use sgcore::{format_usage, show};

use crate::error::TacError;

use sgcore::translate;

mod options {
    pub static BEFORE: &str = "before";
    pub static REGEX: &str = "regex";
    pub static SEPARATOR: &str = "separator";
    pub static FILE: &str = "file";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath"])?;

    let before = matches.get_flag(options::BEFORE);
    let regex = matches.get_flag(options::REGEX);
    let raw_separator = matches
        .get_one::<String>(options::SEPARATOR)
        .map_or("\n", |s| s.as_str());
    let separator = if raw_separator.is_empty() {
        "\0"
    } else {
        raw_separator
    };

    let files: Vec<OsString> = match matches.get_many::<OsString>(options::FILE) {
        Some(v) => v.cloned().collect(),
        None => vec![OsString::from("-")],
    };

    tac(&files, before, regex, separator)
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .override_usage(format_usage(&translate!("tac-usage")))
        .about(translate!("tac-about"))
        .infer_long_args(true)
        .arg(
            Arg::new(options::BEFORE)
                .short('b')
                .long(options::BEFORE)
                .help(translate!("tac-help-before"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::REGEX)
                .short('r')
                .long(options::REGEX)
                .help(translate!("tac-help-regex"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::SEPARATOR)
                .short('s')
                .long(options::SEPARATOR)
                .help(translate!("tac-help-separator"))
                .value_name("STRING")
        )
        .arg(
            Arg::new(options::FILE)
                .hide(true)
                .action(ArgAction::Append)
                .value_parser(clap::value_parser!(OsString))
                .value_hint(clap::ValueHint::FilePath)
        )
}

/// Print lines of a buffer in reverse, with line separator given as a regex.
///
/// `data` contains the bytes of the file.
///
/// `pattern` is the regular expression given as a
/// [`regex::bytes::Regex`] (not a [`regex::Regex`], since the input is
/// given as a slice of bytes). If `before` is `true`, then each match
/// of this pattern in `data` is interpreted as the start of a line. If
/// `before` is `false`, then each match of this pattern is interpreted
/// as the end of a line.
///
/// This function writes each line in `data` to [`std::io::Stdout`] in
/// reverse.
///
/// # Errors
///
/// If there is a problem writing to `stdout`, then this function
/// returns [`std::io::Error`].
fn buffer_tac_regex(
    data: &[u8],
    pattern: &regex::bytes::Regex,
    before: bool
) -> std::io::Result<()> {
    let out = stdout();
    let mut out = BufWriter::new(out.lock());

    let mut this_line_end = data.len();

    let mut following_line_start = data.len();

    for i in (0..data.len()).rev() {
        if let Some(match_) = pattern.find_at(&data[..this_line_end], i) {
            this_line_end = i;

            let slen = match_.end() - match_.start();

            if before {
                out.write_all(&data[i..following_line_start])?;
                following_line_start = i;
            } else {
                out.write_all(&data[i + slen..following_line_start])?;
                following_line_start = i + slen;
            }
        }
    }

    out.write_all(&data[0..following_line_start])?;
    out.flush()?;
    Ok(())
}

/// Write lines from `data` to stdout in reverse.
///
/// This function writes to [`stdout`] each line appearing in `data`,
/// starting with the last line and ending with the first line. The
/// `separator` parameter defines what characters to use as a line
/// separator.
///
/// If `before` is `false`, then this function assumes that the
/// `separator` appears at the end of each line, as in `"abc\ndef\n"`.
/// If `before` is `true`, then this function assumes that the
/// `separator` appears at the beginning of each line, as in
/// `"/abc/def"`.
fn buffer_tac(data: &[u8], before: bool, separator: &str) -> std::io::Result<()> {
    let out = stdout();
    let mut out = BufWriter::new(out.lock());

    let slen = separator.len();

    let mut following_line_start = data.len();

    for i in memmem::rfind_iter(data, separator) {
        if before {
            out.write_all(&data[i..following_line_start])?;
            following_line_start = i;
        } else {
            out.write_all(&data[i + slen..following_line_start])?;
            following_line_start = i + slen;
        }
    }

    out.write_all(&data[0..following_line_start])?;
    out.flush()?;
    Ok(())
}

#[allow(clippy::cognitive_complexity)]
fn tac(filenames: &[OsString], before: bool, regex: bool, separator: &str) -> SGResult<()> {
    let maybe_pattern = if regex {
        match regex::bytes::Regex::new(separator) {
            Ok(p) => Some(p),
            Err(e) => return Err(TacError::InvalidRegex(e).into()),
        }
    } else {
        None
    };

    for filename in filenames {
        let mmap;
        let buf;

        let data: &[u8] = if filename == "-" {
            if let Some(mmap1) = try_mmap_stdin() {
                mmap = mmap1;
                &mmap
            } else {
                let mut buf1 = Vec::new();
                if let Err(e) = stdin().read_to_end(&mut buf1) {
                    let e: Box<dyn SGError> = TacError::ReadError(OsString::from("stdin"), e).into();
                    show!(e);
                    continue;
                }
                buf = buf1;
                &buf
            }
        } else {
            let path = Path::new(filename);
            if path.is_dir() {
                let e: Box<dyn SGError> = TacError::InvalidArgument(filename.clone()).into();
                show!(e);
                continue;
            }

            if path.metadata().is_err() {
                let e: Box<dyn SGError> = TacError::FileNotFound(filename.clone()).into();
                show!(e);
                continue;
            }

            if let Some(mmap1) = try_mmap_path(path) {
                mmap = mmap1;
                &mmap
            } else {
                match read(path) {
                    Ok(buf1) => {
                        buf = buf1;
                        &buf
                    }
                    Err(e) => {
                        let e: Box<dyn SGError> = TacError::ReadError(filename.clone(), e).into();
                        show!(e);
                        continue;
                    }
                }
            }
        };

        let result = match maybe_pattern {
            Some(ref pattern) => buffer_tac_regex(data, pattern, before),
            None => buffer_tac(data, before, separator),
        };

        if let Err(e) = result {
            return Err(TacError::WriteError(e).into());
        }
    }
    Ok(())
}

fn try_mmap_stdin() -> Option<Mmap> {
    unsafe { Mmap::map(&stdin()).ok() }
}

fn try_mmap_path(path: &Path) -> Option<Mmap> {
    let file = File::open(path).ok()?;

    let mmap = unsafe { Mmap::map(&file).ok()? };

    Some(mmap)
}

