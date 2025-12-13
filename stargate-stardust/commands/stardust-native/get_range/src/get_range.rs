

use std::ffi::{OsStr, OsString};
use std::io::{BufWriter, ErrorKind, Write, stdout};

use clap::{Arg, ArgAction, Command};
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use num_traits::Zero;
use serde_json::json;

use sgcore::error::{FromIo, SGResult};
use sgcore::extendedbigdecimal::ExtendedBigDecimal;
use sgcore::format::num_format::FloatVariant;
use sgcore::format::{Format, num_format};
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::{fast_inc::fast_inc, format_usage};

mod error;

#[cfg(fuzzing)]
pub mod number;
#[cfg(not(fuzzing))]
mod number;
mod numberparse;
use crate::error::SeqError;
use crate::number::PreciseNumber;

use sgcore::translate;

const OPT_SEPARATOR: &str = "separator";
const OPT_TERMINATOR: &str = "terminator";
const OPT_EQUAL_WIDTH: &str = "equal-width";
const OPT_FORMAT: &str = "format";

const ARG_NUMBERS: &str = "numbers";

#[derive(Clone)]
struct SeqOptions<'a> {
    separator: OsString,
    terminator: OsString,
    equal_width: bool,
    format: Option<&'a str>,
}

/// A range of floats.
///
/// The elements are (first, increment, last).
type RangeFloat = (ExtendedBigDecimal, ExtendedBigDecimal, ExtendedBigDecimal);

/// Turn short args with attached value, for example "-s,", into two args "-s" and "," to make
/// them work with clap.
fn split_short_args_with_value(args: impl sgcore::Args) -> impl sgcore::Args {
    let mut v: Vec<OsString> = Vec::new();

    for arg in args {
        let bytes = arg.as_encoded_bytes();

        if bytes.len() > 2
            && (bytes.starts_with(b"-f") || bytes.starts_with(b"-s") || bytes.starts_with(b"-t"))
        {
            let (short_arg, value) = bytes.split_at(2);
            v.push(unsafe { OsString::from_encoded_bytes_unchecked(short_arg.to_vec()) });
            v.push(unsafe { OsString::from_encoded_bytes_unchecked(value.to_vec()) });
        } else {
            v.push(arg);
        }
    }

    v.into_iter()
}

fn select_precision(
    first: &PreciseNumber,
    increment: &PreciseNumber,
    last: &PreciseNumber
) -> Option<usize> {
    match (
        first.num_fractional_digits,
        increment.num_fractional_digits,
        last.num_fractional_digits
    ) {
        (Some(0), Some(0), Some(0)) => Some(0),
        (Some(f), Some(i), Some(_)) => Some(f.max(i)),
        _ => None,
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches =
        sgcore::clap_localization::handle_clap_result(sg_app(), split_short_args_with_value(args))?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let json_output_options = StardustOutputOptions::from_matches(&matches);

    let numbers_option = matches.get_many::<String>(ARG_NUMBERS);

    if numbers_option.is_none() {
        return Err(SeqError::NoArguments.into());
    }

    let numbers = numbers_option.unwrap().collect::<Vec<_>>();

    let options = SeqOptions {
        separator: matches
            .get_one::<OsString>(OPT_SEPARATOR)
            .cloned()
            .unwrap_or_else(|| OsString::from("\n")),
        terminator: matches
            .get_one::<OsString>(OPT_TERMINATOR)
            .cloned()
            .unwrap_or_else(|| OsString::from("\n")),
        equal_width: matches.get_flag(OPT_EQUAL_WIDTH),
        format: matches.get_one::<String>(OPT_FORMAT).map(|s| s.as_str()),
    };

    if options.equal_width && options.format.is_some() {
        return Err(SeqError::FormatAndEqualWidth.into());
    }

    let first = if numbers.len() > 1 {
        match numbers[0].parse() {
            Ok(num) => num,
            Err(e) => return Err(SeqError::ParseError(numbers[0].to_owned(), e).into()),
        }
    } else {
        PreciseNumber::one()
    };
    let increment = if numbers.len() > 2 {
        match numbers[1].parse() {
            Ok(num) => num,
            Err(e) => return Err(SeqError::ParseError(numbers[1].to_owned(), e).into()),
        }
    } else {
        PreciseNumber::one()
    };
    if increment.is_zero() {
        return Err(SeqError::ZeroIncrement(numbers[1].to_owned()).into());
    }
    let last: PreciseNumber = {
        let n: usize = numbers.len();
        match numbers[n - 1].parse() {
            Ok(num) => num,
            Err(e) => return Err(SeqError::ParseError(numbers[n - 1].to_owned(), e).into()),
        }
    };

    if json_output_options.stardust_output {
        let range = (first.number, increment.number, last.number);
        return generate_seq_json(range, json_output_options);
    }

    let (format, padding, fast_allowed) = match options.format {
        Some(str) => (
            Format::<num_format::Float, &ExtendedBigDecimal>::parse(str)?,
            0,
            false
        ),
        None => {
            let precision = select_precision(&first, &increment, &last);

            let padding = if options.equal_width {
                let precision_value = precision.unwrap_or(0);
                first
                    .num_integral_digits
                    .max(increment.num_integral_digits)
                    .max(last.num_integral_digits)
                    + if precision_value > 0 {
                        precision_value + 1
                    } else {
                        0
                    }
            } else {
                0
            };

            let formatter = match precision {
                Some(precision) => num_format::Float {
                    variant: FloatVariant::Decimal,
                    width: padding,
                    alignment: num_format::NumberAlignment::RightZero,
                    precision: Some(precision),
                    ..Default::default()
                },
                None => num_format::Float {
                    variant: FloatVariant::Shortest,
                    ..Default::default()
                },
            };
            (
                Format::from_formatter(formatter),
                padding,
                precision == Some(0)
            )
        }
    };

    let range = (first.number, increment.number, last.number);

    let result = print_seq(
        range,
        &options.separator,
        &options.terminator,
        &format,
        fast_allowed,
        padding
    );

    match result {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(err.map_err_context(|| "write error".into())),
    }
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .trailing_var_arg(true)
        .infer_long_args(true)
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("seq-about"))
        .override_usage(format_usage(&translate!("seq-usage")))
        .arg(
            Arg::new(OPT_SEPARATOR)
                .short('s')
                .long("separator")
                .help(translate!("seq-help-separator"))
                .value_parser(clap::value_parser!(OsString))
        )
        .arg(
            Arg::new(OPT_TERMINATOR)
                .short('t')
                .long("terminator")
                .help(translate!("seq-help-terminator"))
                .value_parser(clap::value_parser!(OsString))
        )
        .arg(
            Arg::new(OPT_EQUAL_WIDTH)
                .short('w')
                .long("equal-width")
                .help(translate!("seq-help-equal-width"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_FORMAT)
                .short('f')
                .long(OPT_FORMAT)
                .help(translate!("seq-help-format"))
        )
        .arg(
            Arg::new(ARG_NUMBERS)
                .allow_hyphen_values(true)
                .action(ArgAction::Append)
                .num_args(1..=3)
        )
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
                .long("verbose-json")
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

/// Integer print, default format, positive increment: fast code path
/// that avoids reformating digit at all iterations.
fn fast_print_seq(
    mut stdout: impl Write,
    first: &BigUint,
    increment: u64,
    last: &BigUint,
    separator: &OsStr,
    terminator: &OsStr,
    padding: usize
) -> std::io::Result<()> {
    if last < first {
        return Ok(());
    }

    let loop_cnt = ((last - first) / increment).to_u64().unwrap_or(u64::MAX);

    let first_str = first.to_string();

    let last_length = last.to_string().len();

    let size = last_length.max(padding) + separator.len();
    let mut buf = vec![b'0'; size];
    let buf = buf.as_mut_slice();

    let num_end = buf.len() - separator.len();
    let mut start = num_end - first_str.len();

    buf[start..num_end].copy_from_slice(first_str.as_bytes());
    buf[num_end..].copy_from_slice(separator.as_encoded_bytes());

    start = start.min(num_end - padding);

    let inc_str = increment.to_string();
    let inc_str = inc_str.as_bytes();

    for _ in 0..loop_cnt {
        stdout.write_all(&buf[start..])?;
        fast_inc(buf, &mut start, num_end, inc_str);
    }
    stdout.write_all(&buf[start..num_end])?;
    stdout.write_all(terminator.as_encoded_bytes())?;
    stdout.flush()?;
    Ok(())
}

fn done_printing<T: Zero + PartialOrd>(next: &T, increment: &T, last: &T) -> bool {
    if increment >= &T::zero() {
        next > last
    } else {
        next < last
    }
}

/// Generate sequence as JSON array
fn generate_seq_json(
    range: RangeFloat,
    json_output_options: StardustOutputOptions
) -> SGResult<()> {
    let (first, increment, last) = range;
    let mut values: Vec<String> = Vec::new();
    let mut value = first;

    while !done_printing(&value, &increment, &last) {
        let str_value = match &value {
            sgcore::extendedbigdecimal::ExtendedBigDecimal::BigDecimal(bd) => bd.to_string(),
            sgcore::extendedbigdecimal::ExtendedBigDecimal::Infinity => "inf".to_string(),
            sgcore::extendedbigdecimal::ExtendedBigDecimal::MinusInfinity => "-inf".to_string(),
            sgcore::extendedbigdecimal::ExtendedBigDecimal::MinusZero => "-0".to_string(),
            sgcore::extendedbigdecimal::ExtendedBigDecimal::Nan => "nan".to_string(),
            sgcore::extendedbigdecimal::ExtendedBigDecimal::MinusNan => "-nan".to_string(),
        };
        values.push(str_value);
        value = value + increment.clone();
    }

    let output = json!({ "sequence": values });
    stardust_output::output(json_output_options, output, || Ok(()))?;
    Ok(())
}

/// Arbitrary precision decimal number code path ("slow" path)
fn print_seq(
    range: RangeFloat,
    separator: &OsStr,
    terminator: &OsStr,
    format: &Format<num_format::Float, &ExtendedBigDecimal>,
    fast_allowed: bool,
    padding: usize,
) -> std::io::Result<()> {
    let stdout = stdout().lock();
    let mut stdout = BufWriter::new(stdout);
    let (first, increment, last) = range;

    if fast_allowed {
        let (first_bui, increment_u64, last_bui) = (
            first.to_biguint(),
            increment.to_biguint().and_then(|x| x.to_u64()),
            last.to_biguint()
        );
        if let (Some(first_bui), Some(increment_u64), Some(last_bui)) =
            (first_bui, increment_u64, last_bui)
        {
            return fast_print_seq(
                stdout,
                &first_bui,
                increment_u64,
                &last_bui,
                separator,
                terminator,
                padding
            );
        }
    }

    let mut value = first;

    let mut is_first_iteration = true;
    while !done_printing(&value, &increment, &last) {
        if !is_first_iteration {
            stdout.write_all(separator.as_encoded_bytes())?;
        }
        format.fmt(&mut stdout, &value)?;
        value = value + increment.clone();
        is_first_iteration = false;
    }
    if !is_first_iteration {
        stdout.write_all(terminator.as_encoded_bytes())?;
    }
    stdout.flush()?;
    Ok(())
}

