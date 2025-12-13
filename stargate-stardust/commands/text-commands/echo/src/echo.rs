use clap::builder::ValueParser;
use clap::{Arg, ArgAction, Command};
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, StdoutLock, Write};
use sgcore::error::SGResult;
use sgcore::format::{FormatChar, OctalParsing, parse_escape_only};
use sgcore::{format_usage, os_str_as_bytes};

use sgcore::translate;
use serde_json::json;

mod options {
    pub const STRING: &str = "STRING";
    pub const NO_NEWLINE: &str = "no_newline";
    pub const ENABLE_BACKSLASH_ESCAPE: &str = "enable_backslash_escape";
    pub const DISABLE_BACKSLASH_ESCAPE: &str = "disable_backslash_escape";
    pub const STARDUST_OUTPUT: &str = "stardust_output";
    pub const PRETTY: &str = "pretty";
}

/// Options for the echo command.
#[derive(Debug, Clone, Copy)]
struct Options {
    /// Whether the output should have a trailing newline.
    ///
    /// True by default. `-n` disables it.
    pub trailing_newline: bool,

    /// Whether given string literals should be parsed for
    /// escape characters.
    ///
    /// False by default, can be enabled with `-e`. Always true if
    /// `POSIXLY_CORRECT` (cannot be disabled with `-E`).
    pub escape: bool,

    /// Whether to output object (JSON) format.
    pub object_output: bool,
    /// Whether to pretty-print JSON output.
    pub json_pretty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            trailing_newline: true,
            escape: false,
            object_output: false,
            json_pretty: false,
        }
    }
}

impl Options {
    fn posixly_correct_default() -> Self {
        Self {
            trailing_newline: true,
            escape: true,
            object_output: false,
            json_pretty: false,
        }
    }
}

/// Checks if an argument is a valid echo flag, and if
/// it is records the changes in [`Options`].
fn is_flag(arg: &OsStr, options: &mut Options) -> bool {
    let arg = arg.as_encoded_bytes();

    if arg.first() != Some(&b'-') || arg == b"-" {
        return false;
    }

    let mut options_: Options = *options;

    for c in &arg[1..] {
        match c {
            b'e' => options_.escape = true,
            b'E' => options_.escape = false,
            b'n' => options_.trailing_newline = false,

            _ => return false,
        }
    }

    *options = options_;
    true
}

/// Processes command line arguments, separating flags from normal arguments.
///
/// # Returns
///
/// - Vector of non-flag arguments.
/// - [`Options`], describing how teh arguments should be interpreted.
fn filter_flags(mut args: impl Iterator<Item = OsString>) -> (Vec<OsString>, Options) {
    let mut arguments = Vec::with_capacity(args.size_hint().0);
    let mut options = Options::default();

    for arg in &mut args {
        if !is_flag(&arg, &mut options) {
            arguments.push(arg);
            break;
        }
    }

    arguments.extend(args);

    (arguments, options)
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let args: Vec<OsString> = args.skip(1).collect();

    let is_posixly_correct = env::var_os("POSIXLY_CORRECT").is_some();

    let (args, options) = if is_posixly_correct {
        if args.first().is_some_and(|arg| arg == "-n") {
            let (args, _) = filter_flags(args.into_iter());
            (
                args,
                Options {
                    trailing_newline: false,
                    ..Options::posixly_correct_default()
                }
            )
        } else {
            (args, Options::posixly_correct_default())
        }
    } else if args.len() == 1 && args[0] == "--help" {
        sg_app().print_help()?;
        return Ok(());
    } else if args.len() == 1 && args[0] == "--version" {
        print!("{}", sg_app().render_version());
        return Ok(());
    } else {
        filter_flags(args.into_iter())
    };

    let (args, options) = {
        let mut json_flag = false;
        let mut json_pretty = false;
        let filtered: Vec<OsString> = args
            .into_iter()
            .filter(|arg| {
                if arg == "-o" || arg == "--obj" {
                    json_flag = true;
                    false
                } else if arg == "--pretty" {
                    json_pretty = true;
                    false
                } else {
                    true
                }
            })
            .collect();

        let (final_args, mut opts) = if json_flag {
            filter_flags(filtered.into_iter())
        } else {
            (filtered, options)
        };

        opts.object_output = json_flag;
        opts.json_pretty = json_pretty;
        (final_args, opts)
    };

    execute(&mut io::stdout().lock(), args, options)?;

    Ok(())
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .trailing_var_arg(true)
        .allow_hyphen_values(true)
        .version(sgcore::crate_version!())
        .about(translate!("echo-about"))
        .after_help(translate!("echo-after-help"))
        .override_usage(format_usage(&translate!("echo-usage")))
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .arg(
            Arg::new(options::NO_NEWLINE)
                .short('n')
                .help(translate!("echo-help-no-newline"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::ENABLE_BACKSLASH_ESCAPE)
                .short('e')
                .help(translate!("echo-help-enable-escapes"))
                .action(ArgAction::SetTrue)
                .overrides_with(options::DISABLE_BACKSLASH_ESCAPE)
        )
        .arg(
            Arg::new(options::DISABLE_BACKSLASH_ESCAPE)
                .short('E')
                .help(translate!("echo-help-disable-escapes"))
                .action(ArgAction::SetTrue)
                .overrides_with(options::ENABLE_BACKSLASH_ESCAPE)
        )
        .arg(
            Arg::new(options::STARDUST_OUTPUT)
                .short('o')
                .long("obj")
                .help("Output as JSON object")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::PRETTY)
                .long("pretty")
                .help("Pretty-print object (JSON) output (use with -o)")
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::STRING)
                .action(ArgAction::Append)
                .value_parser(ValueParser::os_string())
        )
}

fn execute(stdout: &mut StdoutLock, args: Vec<OsString>, options: Options) -> SGResult<()> {
    if options.object_output {
        let mut output = String::new();
        for (i, arg) in args.iter().enumerate() {
            let bytes = os_str_as_bytes(&arg)?;

            if i > 0 {
                output.push(' ');
            }

            if options.escape {
                let mut temp: Vec<u8> = Vec::new();
                for item in parse_escape_only(bytes, OctalParsing::ThreeDigits) {
                    item.write(&mut temp)?;
                }
                output.push_str(&String::from_utf8_lossy(&temp));
            } else {
                output.push_str(&String::from_utf8_lossy(bytes));
            }
        }

        let json_output = json!({
            "output": output,
            "trailing_newline": options.trailing_newline
        });

        if options.json_pretty {
            match serde_json::to_string_pretty(&json_output) {
                Ok(s) => writeln!(stdout, "{}", s)?,
                Err(_) => writeln!(stdout, "{}", json_output)?,
            }
        } else {
            writeln!(stdout, "{}", json_output)?;
        }
    } else {
        for (i, arg) in args.into_iter().enumerate() {
            let bytes = os_str_as_bytes(&arg)?;

            if i > 0 {
                stdout.write_all(b" ")?;
            }

            if options.escape {
                for item in parse_escape_only(bytes, OctalParsing::ThreeDigits) {
                    if item.write(&mut *stdout)?.is_break() {
                        return Ok(());
                    }
                }
            } else {
                stdout.write_all(bytes)?;
            }
        }

        if options.trailing_newline {
            stdout.write_all(b"\n")?;
        }
    }

    Ok(())
}

