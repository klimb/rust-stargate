use clap::builder::ValueParser;
use clap::{Arg, ArgAction, Command};
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, StdoutLock, Write};
use sgcore::error::UResult;
use sgcore::format::{FormatChar, OctalParsing, parse_escape_only};
use sgcore::{format_usage, os_str_as_bytes};

use sgcore::translate;
use serde_json::json;

mod options {
    pub const STRING: &str = "STRING";
    pub const NO_NEWLINE: &str = "no_newline";
    pub const ENABLE_BACKSLASH_ESCAPE: &str = "enable_backslash_escape";
    pub const DISABLE_BACKSLASH_ESCAPE: &str = "disable_backslash_escape";
    pub const OBJECT_OUTPUT: &str = "object_output";
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
        // Argument doesn't start with '-' or is '-' => not a flag.
        return false;
    }

    // We don't modify the given options until after
    // the loop because there is a chance the flag isn't
    // valid after all & shouldn't affect the options.
    let mut options_: Options = *options;

    // Skip the '-' when processing characters.
    for c in &arg[1..] {
        match c {
            b'e' => options_.escape = true,
            b'E' => options_.escape = false,
            b'n' => options_.trailing_newline = false,

            // If there is any character in an supposed flag
            // that is not a valid flag character, it is not
            // a flag.
            //
            // "-eeEnEe" => is a flag.
            // "-eeBne" => not a flag, short circuit at the B.
            _ => return false,
        }
    }

    // We are now sure that the argument is a
    // flag, and can apply the modified options.
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

    // Process arguments until first non-flag is found.
    for arg in &mut args {
        // We parse flags and aggregate the options in `options`.
        // First call to `is_echo_flag` to return false will break the loop.
        if !is_flag(&arg, &mut options) {
            // Not a flag. Can break out of flag-processing loop.
            // Don't forget to push it to the arguments too.
            arguments.push(arg);
            break;
        }
    }

    // Collect remaining non-flag arguments.
    arguments.extend(args);

    (arguments, options)
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    // args[0] is the name of the binary.
    let args: Vec<OsString> = args.skip(1).collect();

    // Check POSIX compatibility mode
    //
    // From the GNU manual, on what it should do:
    //
    // > If the POSIXLY_CORRECT environment variable is set, then when
    // > echo’s first argument is not -n it outputs option-like arguments
    // > instead of treating them as options. For example, echo -ne hello
    // > outputs ‘-ne hello’ instead of plain ‘hello’. Also backslash
    // > escapes are always enabled. To echo the string ‘-n’, one of the
    // > characters can be escaped in either octal or hexadecimal
    // > representation. For example, echo -e '\x2dn'.
    let is_posixly_correct = env::var_os("POSIXLY_CORRECT").is_some();

    let (args, options) = if is_posixly_correct {
        if args.first().is_some_and(|arg| arg == "-n") {
            // if POSIXLY_CORRECT is set and the first argument is the "-n" flag
            // we filter flags normally but 'escaped' is activated nonetheless.
            let (args, _) = filter_flags(args.into_iter());
            (
                args,
                Options {
                    trailing_newline: false,
                    ..Options::posixly_correct_default()
                }
            )
        } else {
            // if POSIXLY_CORRECT is set and the first argument is not the "-n" flag
            // we just collect all arguments as no arguments are interpreted as flags.
            (args, Options::posixly_correct_default())
        }
    } else if args.len() == 1 && args[0] == "--help" {
        // If POSIXLY_CORRECT is not set and the first argument
        // is `--help`, GNU coreutils prints the help message.
        //
        // Verify this using:
        //
        //   POSIXLY_CORRECT=1 echo --help
        //                     echo --help
        uu_app().print_help()?;
        return Ok(());
    } else if args.len() == 1 && args[0] == "--version" {
        print!("{}", uu_app().render_version());
        return Ok(());
    } else {
        // if POSIXLY_CORRECT is not set we filter the flags normally
        filter_flags(args.into_iter())
    };

    // Check if object (JSON) output is requested (filter -o/--obj and --pretty from args)
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
            // If object output requested, we need to re-filter flags
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

pub fn uu_app() -> Command {
    // Note: echo is different from the other utils in that it should **not**
    // have `infer_long_args(true)`, because, for example, `--ver` should be
    // printed as `--ver` and not show the version text.
    Command::new(sgcore::util_name())
        // TrailingVarArg specifies the final positional argument is a VarArg
        // and it doesn't attempts the parse any further args.
        // Final argument must have multiple(true) or the usage string equivalent.
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
            Arg::new(options::OBJECT_OUTPUT)
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

fn execute(stdout: &mut StdoutLock, args: Vec<OsString>, options: Options) -> UResult<()> {
    if options.object_output {
        // Build the output string as usual
        let mut output = String::new();
        for (i, arg) in args.iter().enumerate() {
            let bytes = os_str_as_bytes(&arg)?;
            
            // Don't add a space before the first argument
            if i > 0 {
                output.push(' ');
            }
            
            if options.escape {
                // For escaped output, we need to process escape sequences
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
        // Original non-object output logic
        for (i, arg) in args.into_iter().enumerate() {
            let bytes = os_str_as_bytes(&arg)?;

            // Don't print a space before the first argument
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
