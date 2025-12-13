

pub mod native_int_str;
pub mod split_iterator;
pub mod string_expander;
pub mod string_parser;
pub mod variable_parser;

use clap::builder::ValueParser;
use clap::{Arg, ArgAction, Command, crate_name};
use ini::Ini;
use native_int_str::{
    Convert, NCvt, NativeIntStr, NativeIntString, NativeStr, from_native_int_representation_owned,
};
use nix::libc;
use nix::sys::signal::{SigHandler::SigIgn, Signal, signal};
use nix::unistd::execvp;
use std::borrow::Cow;
use std::env;
use std::ffi::CString;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;

use sgcore::display::Quotable;
use sgcore::error::{ExitCode, SGError, SGResult, SGSimpleError, SGUsageError};
use sgcore::line_ending::LineEnding;
use sgcore::signals::signal_by_name_or_value;
use sgcore::translate;
use sgcore::{format_usage, show_warning};
use sgcore::stardust_output::{self, StardustOutputOptions};

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum EnvError {
    #[error("{}", translate!("env-error-missing-closing-quote", "position" => .0, "quote" => .1))]
    EnvMissingClosingQuote(usize, char),
    #[error("{}", translate!("env-error-invalid-backslash-at-end", "position" => .0, "context" => .1.clone()))]
    EnvInvalidBackslashAtEndOfStringInMinusS(usize, String),
    #[error("{}", translate!("env-error-backslash-c-not-allowed", "position" => .0))]
    EnvBackslashCNotAllowedInDoubleQuotes(usize),
    #[error("{}", translate!("env-error-invalid-sequence", "position" => .0, "char" => .1))]
    EnvInvalidSequenceBackslashXInMinusS(usize, char),
    #[error("{}", translate!("env-error-missing-closing-brace", "position" => .0))]
    EnvParsingOfVariableMissingClosingBrace(usize),
    #[error("{}", translate!("env-error-missing-variable", "position" => .0))]
    EnvParsingOfMissingVariable(usize),
    #[error("{}", translate!("env-error-missing-closing-brace-after-value", "position" => .0))]
    EnvParsingOfVariableMissingClosingBraceAfterValue(usize),
    #[error("{}", translate!("env-error-unexpected-number", "position" => .0, "char" => .1.clone()))]
    EnvParsingOfVariableUnexpectedNumber(usize, String),
    #[error("{}", translate!("env-error-expected-brace-or-colon", "position" => .0, "char" => .1.clone()))]
    EnvParsingOfVariableExceptedBraceOrColon(usize, String),
    #[error("")]
    EnvReachedEnd,
    #[error("")]
    EnvContinueWithDelimiter,
    #[error("{}{:?}",.0,.1)]
    EnvInternalError(usize, string_parser::Error),
}

impl From<string_parser::Error> for EnvError {
    fn from(value: string_parser::Error) -> Self {
        Self::EnvInternalError(value.peek_position, value)
    }
}

mod options {
    pub const IGNORE_ENVIRONMENT: &str = "ignore-environment";
    pub const CHDIR: &str = "chdir";
    pub const NULL: &str = "null";
    pub const FILE: &str = "file";
    pub const UNSET: &str = "unset";
    pub const DEBUG: &str = "debug";
    pub const SPLIT_STRING: &str = "split-string";
    pub const ARGV0: &str = "argv0";
    pub const IGNORE_SIGNAL: &str = "ignore-signal";
}

struct Options<'a> {
    ignore_env: bool,
    line_ending: LineEnding,
    running_directory: Option<&'a OsStr>,
    files: Vec<&'a OsStr>,
    unsets: Vec<&'a OsStr>,
    sets: Vec<(Cow<'a, OsStr>, Cow<'a, OsStr>)>,
    program: Vec<&'a OsStr>,
    argv0: Option<&'a OsStr>,
    ignore_signal: Vec<usize>,
}

/// print `name=value` env pairs on screen
/// if null is true, separate pairs with a \0, \n otherwise
fn print_env(line_ending: LineEnding) {
    let stdout_raw = io::stdout();
    let mut stdout = stdout_raw.lock();
    for (n, v) in env::vars() {
        write!(stdout, "{n}={v}{line_ending}").unwrap();
    }
}

/// produce JSON output of all environment variables
fn produce_json(object_output: StardustOutputOptions) -> SGResult<()> {
    let env_vars: serde_json::Map<String, serde_json::Value> = env::vars()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();

    let output = serde_json::json!({
        "environment": env_vars,
        "count": env_vars.len()
    });

    stardust_output::output(object_output, output, || Ok(()))?;
    Ok(())
}

fn parse_name_value_opt<'a>(opts: &mut Options<'a>, opt: &'a OsStr) -> SGResult<bool> {
    let wrap = NativeStr::<'a>::new(opt);
    let split_o = wrap.split_once(&'=');
    if let Some((name, value)) = split_o {
        opts.sets.push((name, value));
        Ok(false)
    } else {
        parse_program_opt(opts, opt).map(|_| true)
    }
}

fn parse_program_opt<'a>(opts: &mut Options<'a>, opt: &'a OsStr) -> SGResult<()> {
    if opts.line_ending == LineEnding::Nul {
        Err(SGUsageError::new(
            125,
            translate!("env-error-cannot-specify-null-with-command")
        ))
    } else {
        opts.program.push(opt);
        Ok(())
    }
}
fn parse_signal_value(signal_name: &str) -> SGResult<usize> {
    let signal_name_upcase = signal_name.to_uppercase();
    let optional_signal_value = signal_by_name_or_value(&signal_name_upcase);
    let error = SGSimpleError::new(
        125,
        translate!("env-error-invalid-signal", "signal" => signal_name.quote())
    );
    match optional_signal_value {
        Some(sig_val) => {
            if sig_val == 0 {
                Err(error)
            } else {
                Ok(sig_val)
            }
        }
        None => Err(error),
    }
}
fn parse_signal_opt<'a>(opts: &mut Options<'a>, opt: &'a OsStr) -> SGResult<()> {
    if opt.is_empty() {
        return Ok(());
    }
    let signals: Vec<&'a OsStr> = opt
        .as_bytes()
        .split(|&b| b == b',')
        .map(OsStr::from_bytes)
        .collect();

    let mut sig_vec = Vec::with_capacity(signals.len());
    for sig in signals {
        if !sig.is_empty() {
            sig_vec.push(sig);
        }
    }
    for sig in sig_vec {
        let Some(sig_str) = sig.to_str() else {
            return Err(SGSimpleError::new(
                1,
                translate!("env-error-invalid-signal", "signal" => sig.quote())
            ));
        };
        let sig_val = parse_signal_value(sig_str)?;
        if !opts.ignore_signal.contains(&sig_val) {
            opts.ignore_signal.push(sig_val);
        }
    }

    Ok(())
}

fn load_config_file(opts: &mut Options) -> SGResult<()> {
    for &file in &opts.files {
        let conf = if file == "-" {
            let stdin = io::stdin();
            let mut stdin_locked = stdin.lock();
            Ini::read_from(&mut stdin_locked)
        } else {
            Ini::load_from_file(file)
        };

        let conf = conf.map_err(|e| {
            SGSimpleError::new(
                1,
                translate!("env-error-config-file", "file" => file.maybe_quote(), "error" => e)
            )
        })?;

        for (_, prop) in &conf {
            for (key, value) in prop {
                unsafe {
                    env::set_var(key, value);
                }
            }
        }
    }

    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(crate_name!())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("env-about"))
        .override_usage(format_usage(&translate!("env-usage")))
        .after_help(translate!("env-after-help"))
        .infer_long_args(true)
        .arg(
            Arg::new(options::NULL)
                .short('0')
                .long(options::NULL)
                .help(translate!("env-help-null"))
                .action(ArgAction::SetTrue)
        );

    stardust_output::add_json_args(cmd)
}

pub fn parse_args_from_str(text: &NativeIntStr) -> SGResult<Vec<NativeIntString>> {
    split_iterator::split(text).map_err(|e| match e {
        EnvError::EnvBackslashCNotAllowedInDoubleQuotes(_) => SGSimpleError::new(125, e.to_string()),
        EnvError::EnvInvalidBackslashAtEndOfStringInMinusS(_, _) => {
            SGSimpleError::new(125, e.to_string())
        }
        EnvError::EnvInvalidSequenceBackslashXInMinusS(_, _) => {
            SGSimpleError::new(125, e.to_string())
        }
        EnvError::EnvMissingClosingQuote(_, _) => SGSimpleError::new(125, e.to_string()),
        EnvError::EnvParsingOfVariableMissingClosingBrace(pos) => SGSimpleError::new(
            125,
            translate!("env-error-variable-name-issue", "position" => pos, "error" => e)
        ),
        EnvError::EnvParsingOfMissingVariable(pos) => SGSimpleError::new(
            125,
            translate!("env-error-variable-name-issue", "position" => pos, "error" => e)
        ),
        EnvError::EnvParsingOfVariableMissingClosingBraceAfterValue(pos) => SGSimpleError::new(
            125,
            translate!("env-error-variable-name-issue", "position" => pos, "error" => e)
        ),
        EnvError::EnvParsingOfVariableUnexpectedNumber(pos, _) => SGSimpleError::new(
            125,
            translate!("env-error-variable-name-issue", "position" => pos, "error" => e)
        ),
        EnvError::EnvParsingOfVariableExceptedBraceOrColon(pos, _) => SGSimpleError::new(
            125,
            translate!("env-error-variable-name-issue", "position" => pos, "error" => e)
        ),
        _ => SGSimpleError::new(
            125,
            translate!("env-error-generic", "error" => format!("{e:?}"))
        ),
    })
}

fn debug_print_args(args: &[OsString]) {
    eprintln!("input args:");
    for (i, arg) in args.iter().enumerate() {
        eprintln!("arg[{i}]: {}", arg.quote());
    }
}

fn check_and_handle_string_args(
    arg: &OsString,
    prefix_to_test: &str,
    all_args: &mut Vec<OsString>,
    do_debug_print_args: Option<&Vec<OsString>>
) -> SGResult<bool> {
    let native_arg = NCvt::convert(arg);
    if let Some(remaining_arg) = native_arg.strip_prefix(&*NCvt::convert(prefix_to_test)) {
        if let Some(input_args) = do_debug_print_args {
            debug_print_args(input_args);
        }

        let arg_strings = parse_args_from_str(remaining_arg)?;
        all_args.extend(
            arg_strings
                .into_iter()
                .map(from_native_int_representation_owned)
        );

        Ok(true)
    } else {
        Ok(false)
    }
}

#[derive(Default)]
struct EnvAppData {
    do_debug_printing: bool,
    do_input_debug_printing: Option<bool>,
    had_string_argument: bool,
}

impl EnvAppData {
    fn make_error_no_such_file_or_dir(&self, prog: &OsStr) -> Box<dyn SGError> {
        sgcore::show_error!(
            "{}",
            translate!("env-error-no-such-file", "program" => prog.quote())
        );
        if !self.had_string_argument {
            sgcore::show_error!("{}", translate!("env-error-use-s-shebang"));
        }
        ExitCode::new(127)
    }

    fn process_all_string_arguments(
        &mut self,
        original_args: &Vec<OsString>
    ) -> SGResult<Vec<OsString>> {
        let mut all_args: Vec<OsString> = Vec::new();
        let mut process_flags = true;
        let mut expecting_arg = false;
        let flags_with_args = [
            options::ARGV0,
            options::CHDIR,
            options::FILE,
            options::IGNORE_SIGNAL,
            options::UNSET,
        ];
        let short_flags_with_args = ['a', 'C', 'f', 'u'];
        for (n, arg) in original_args.iter().enumerate() {
            let arg_str = arg.to_string_lossy();
            if 0 < n
                && !expecting_arg
                && (arg == "--" || !(arg_str.starts_with('-') || arg_str.contains('=')))
            {
                process_flags = false;
            }
            if !process_flags {
                all_args.push(arg.clone());
                continue;
            }
            expecting_arg = false;
            match arg {
                b if check_and_handle_string_args(b, "--split-string", &mut all_args, None)? => {
                    self.had_string_argument = true;
                }
                b if check_and_handle_string_args(b, "-S", &mut all_args, None)? => {
                    self.had_string_argument = true;
                }
                b if check_and_handle_string_args(b, "-vS", &mut all_args, None)? => {
                    self.do_debug_printing = true;
                    self.had_string_argument = true;
                }
                b if check_and_handle_string_args(
                    b,
                    "-vvS",
                    &mut all_args,
                    Some(original_args)
                )? =>
                {
                    self.do_debug_printing = true;
                    self.do_input_debug_printing = Some(false);
                    self.had_string_argument = true;
                }
                _ => {
                    if let Some(flag) = arg_str.strip_prefix("--") {
                        if flags_with_args.contains(&flag) {
                            expecting_arg = true;
                        }
                    } else if let Some(flag) = arg_str.strip_prefix("-") {
                        for c in flag.chars() {
                            expecting_arg = short_flags_with_args.contains(&c);
                        }
                    }
                    if arg_str.contains('=')
                        && arg_str.starts_with("-u")
                        && !arg_str.starts_with("--")
                    {
                        let name = &arg_str[arg_str.find('=').unwrap()..];
                        return Err(SGSimpleError::new(
                            125,
                            translate!("env-error-cannot-unset", "name" => name)
                        ));
                    }

                    all_args.push(arg.clone());
                }
            }
        }

        Ok(all_args)
    }

    fn parse_arguments(
        &mut self,
        original_args: impl sgcore::Args
    ) -> Result<(Vec<OsString>, clap::ArgMatches), Box<dyn SGError>> {
        let original_args: Vec<OsString> = original_args.collect();
        let args = self.process_all_string_arguments(&original_args)?;
        let app = sg_app();
        let matches = match app.try_get_matches_from(args) {
            Ok(matches) => matches,
            Err(e) => {
                match e.kind() {
                    clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion => return Err(e.into()),
                    _ => {
                        let formatter =
                            sgcore::clap_localization::ErrorFormatter::new(sgcore::util_name());
                        formatter.print_error_and_exit_with_callback(&e, 125, || {
                            eprintln!(
                                "{}: {}",
                                sgcore::util_name(),
                                translate!("env-error-use-s-shebang")
                            );
                        });
                    }
                }
            }
        };
        Ok((original_args, matches))
    }

    fn run_env(&mut self, original_args: impl sgcore::Args) -> SGResult<()> {
        let (_, matches) = self.parse_arguments(original_args)?;

        let object_output = StardustOutputOptions::from_matches(&matches);
        let line_ending = LineEnding::from_zero_flag(matches.get_flag(options::NULL));

        if object_output.stardust_output {
            produce_json(object_output)
        } else {
            print_env(line_ending);
            Ok(())
        }
    }

    /// Run the program specified by the options.
    ///
    /// Note that the env command must exec the program, not spawn it. See
    /// <https://github.com/uutils/coreutils/issues/8361> for more information.
    ///
    /// Exit status:
    /// - 125: if the env command itself fails
    /// - 126: if the program is found but cannot be invoked
    /// - 127: if the program cannot be found
    fn run_program(
        &mut self,
        opts: &Options<'_>,
        do_debug_printing: bool
    ) -> Result<(), Box<dyn SGError>> {
        let prog = Cow::from(opts.program[0]);
        let mut arg0 = prog.clone();
        #[cfg(not(unix))]
        let arg0 = prog.clone();
        let args = &opts.program[1..];

        if let Some(_argv0) = opts.argv0 {
            {
                arg0 = Cow::Borrowed(_argv0);
                if do_debug_printing {
                    eprintln!("argv0:     {}", arg0.quote());
                }
            }

            #[cfg(not(unix))]
            return Err(SGSimpleError::new(
                2,
                translate!("env-error-argv0-not-supported")
            ));
        }

        if do_debug_printing {
            eprintln!("executing: {}", prog.maybe_quote());
            let arg_prefix = "   arg";
            eprintln!("{arg_prefix}[{}]= {}", 0, arg0.quote());
            for (i, arg) in args.iter().enumerate() {
                eprintln!("{arg_prefix}[{}]= {}", i + 1, arg.quote());
            }
        }
        {
            let Ok(prog_cstring) = CString::new(prog.as_bytes()) else {
                return Err(self.make_error_no_such_file_or_dir(&prog));
            };

            let mut argv = Vec::new();

            let Ok(arg0_cstring) = CString::new(arg0.as_bytes()) else {
                return Err(self.make_error_no_such_file_or_dir(&prog));
            };
            argv.push(arg0_cstring);

            for arg in args {
                let Ok(arg_cstring) = CString::new(arg.as_bytes()) else {
                    return Err(self.make_error_no_such_file_or_dir(&prog));
                };
                argv.push(arg_cstring);
            }

            match execvp(&prog_cstring, &argv) {
                Err(nix::errno::Errno::ENOENT) => Err(self.make_error_no_such_file_or_dir(&prog)),
                Err(nix::errno::Errno::EACCES) => {
                    sgcore::show_error!(
                        "{}",
                        translate!(
                            "env-error-permission-denied",
                            "program" => prog.quote()
                        )
                    );
                    Err(126.into())
                }
                Err(_) => {
                    sgcore::show_error!(
                        "{}",
                        translate!(
                            "env-error-unknown",
                            "error" => "execvp failed"
                        )
                    );
                    Err(126.into())
                }
                Ok(_) => {
                    unreachable!("execvp should never return on success")
                }
            }
        }

        #[cfg(not(unix))]
        {
            let mut cmd = std::process::Command::new(&*prog);
            cmd.args(args);

            match cmd.status() {
                Ok(exit) if !exit.success() => Err(exit.code().unwrap_or(1).into()),
                Err(ref err) => match err.kind() {
                    io::ErrorKind::NotFound | io::ErrorKind::InvalidInput => {
                        Err(self.make_error_no_such_file_or_dir(&prog))
                    }
                    io::ErrorKind::PermissionDenied => {
                        sgcore::show_error!(
                            "{}",
                            translate!("env-error-permission-denied", "program" => prog.quote())
                        );
                        Err(126.into())
                    }
                    _ => {
                        sgcore::show_error!(
                            "{}",
                            translate!("env-error-unknown", "error" => format!("{err:?}"))
                        );
                        Err(126.into())
                    }
                },
                Ok(_) => Ok(()),
            }
        }
    }
}

fn apply_removal_of_all_env_vars(opts: &Options<'_>) {
    if opts.ignore_env {
        for (ref name, _) in env::vars_os() {
            unsafe {
                env::remove_var(name);
            }
        }
    }
}

fn make_options(matches: &clap::ArgMatches) -> SGResult<Options<'_>> {
    let ignore_env = matches.get_flag("ignore-environment");
    let line_ending = LineEnding::from_zero_flag(matches.get_flag("null"));
    let running_directory = matches.get_one::<OsString>("chdir").map(|s| s.as_os_str());
    let files = match matches.get_many::<OsString>("file") {
        Some(v) => v.map(|s| s.as_os_str()).collect(),
        None => Vec::with_capacity(0),
    };
    let unsets = match matches.get_many::<OsString>("unset") {
        Some(v) => v.map(|s| s.as_os_str()).collect(),
        None => Vec::with_capacity(0),
    };
    let argv0 = matches.get_one::<OsString>("argv0").map(|s| s.as_os_str());

    let mut opts = Options {
        ignore_env,
        line_ending,
        running_directory,
        files,
        unsets,
        sets: vec![],
        program: vec![],
        argv0,
        ignore_signal: vec![],
    };
    if let Some(iter) = matches.get_many::<OsString>("ignore-signal") {
        for opt in iter {
            parse_signal_opt(&mut opts, opt)?;
        }
    }

    let mut begin_prog_opts = false;
    if let Some(mut iter) = matches.get_many::<OsString>("vars") {
        while !begin_prog_opts {
            if let Some(opt) = iter.next() {
                if opt == "-" {
                    opts.ignore_env = true;
                } else {
                    begin_prog_opts = parse_name_value_opt(&mut opts, opt)?;
                }
            } else {
                break;
            }
        }

        for opt in iter {
            parse_program_opt(&mut opts, opt)?;
        }
    }

    Ok(opts)
}

fn apply_unset_env_vars(opts: &Options<'_>) -> Result<(), Box<dyn SGError>> {
    for name in &opts.unsets {
        let native_name = NativeStr::new(name);
        if name.is_empty()
            || native_name.contains(&'\0').unwrap()
            || native_name.contains(&'=').unwrap()
        {
            return Err(SGSimpleError::new(
                125,
                translate!("env-error-cannot-unset-invalid", "name" => name.quote())
            ));
        }
        unsafe {
            env::remove_var(name);
        }
    }
    Ok(())
}

fn apply_change_directory(opts: &Options<'_>) -> Result<(), Box<dyn SGError>> {
    if opts.program.is_empty() && opts.running_directory.is_some() {
        return Err(SGUsageError::new(
            125,
            translate!("env-error-must-specify-command-with-chdir")
        ));
    }

    if let Some(d) = opts.running_directory {
        match env::set_current_dir(d) {
            Ok(()) => d,
            Err(error) => {
                return Err(SGSimpleError::new(
                    125,
                    translate!("env-error-cannot-change-directory", "directory" => d.quote(), "error" => error)
                ));
            }
        };
    }
    Ok(())
}

fn apply_specified_env_vars(opts: &Options<'_>) {
    for (name, val) in &opts.sets {

        if name.is_empty() {
            show_warning!(
                "{}",
                translate!("env-warning-no-name-specified", "value" => val.quote())
            );
            continue;
        }
        unsafe {
            env::set_var(name, val);
        }
    }
}
fn apply_ignore_signal(opts: &Options<'_>) -> SGResult<()> {
    for &sig_value in &opts.ignore_signal {
        let sig: Signal = (sig_value as i32)
            .try_into()
            .map_err(|e| io::Error::from_raw_os_error(e as i32))?;

        ignore_signal(sig)?;
    }
    Ok(())
}
fn ignore_signal(sig: Signal) -> SGResult<()> {
    let result = unsafe { signal(sig, SigIgn) };
    if let Err(err) = result {
        return Err(SGSimpleError::new(
            125,
            translate!("env-error-failed-set-signal-action", "signal" => (sig as i32), "error" => err.desc())
        ));
    }
    Ok(())
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    sgcore::pledge::apply_pledge(&["stdio", "proc", "exec"])?;
    EnvAppData::default().run_env(args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sgcore::locale;

    #[test]
    fn test_split_string_environment_vars_test() {
        unsafe { env::set_var("FOO", "BAR") };
        assert_eq!(
            NCvt::convert(vec!["FOO=bar", "sh", "-c", "echo xBARx =$FOO="]),
            parse_args_from_str(&NCvt::convert(r#"FOO=bar sh -c "echo x${FOO}x =\$FOO=""#))
                .unwrap()
        );
    }

    #[test]
    fn test_split_string_misc() {
        assert_eq!(
            NCvt::convert(vec!["A=B", "FOO=AR", "sh", "-c", "echo $A$FOO"]),
            parse_args_from_str(&NCvt::convert(r#"A=B FOO=AR  sh -c "echo \$A\$FOO""#)).unwrap()
        );
        assert_eq!(
            NCvt::convert(vec!["A=B", "FOO=AR", "sh", "-c", "echo $A$FOO"]),
            parse_args_from_str(&NCvt::convert(r"A=B FOO=AR  sh -c 'echo $A$FOO'")).unwrap()
        );
        assert_eq!(
            NCvt::convert(vec!["A=B", "FOO=AR", "sh", "-c", "echo $A$FOO"]),
            parse_args_from_str(&NCvt::convert(r"A=B FOO=AR  sh -c 'echo $A$FOO'")).unwrap()
        );

        assert_eq!(
            NCvt::convert(vec!["-i", "A=B ' C"]),
            parse_args_from_str(&NCvt::convert(r"-i A='B \' C'")).unwrap()
        );
    }

    #[test]
    #[ignore]
    fn test_error_cases() {
        let _ = locale::setup_localization("env");

        let result = parse_args_from_str(&NCvt::convert(r#"sh -c "echo \c""#));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("env-error-backslash-c-not-allowed") ||
            error_msg.contains("must not appear in double-quoted")
        );

        let result = parse_args_from_str(&NCvt::convert(r#"sh -c "echo \"#));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("env-error-missing-closing-quote") ||
            error_msg.contains("no terminating quote")
        );

        let result = parse_args_from_str(&NCvt::convert(r#"sh -c "echo \x""#));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("env-error-invalid-sequence") ||
            error_msg.contains("invalid sequence '\\x' in -S")
        );

        let result = parse_args_from_str(&NCvt::convert(r#"sh -c "echo "#));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("env-error-missing-closing-quote") ||
            error_msg.contains("no terminating quote")
        );

        let result = parse_args_from_str(&NCvt::convert(r"echo ${FOO"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("env-error-variable-parse-error") ||
            error_msg.contains("variable name issue") ||
            error_msg.contains("Missing closing brace")
        );

        let result = parse_args_from_str(&NCvt::convert(r"echo ${FOO:-value"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("env-error-variable-parse-error") ||
            error_msg.contains("variable name issue") ||
            error_msg.contains("Missing closing brace")
        );

        let result = parse_args_from_str(&NCvt::convert(r"echo ${1FOO}"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("env-error-variable-parse-error") ||
            error_msg.contains("variable name issue") ||
            error_msg.contains("Unexpected character")
        );

        let result = parse_args_from_str(&NCvt::convert(r"echo ${FOO?}"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("env-error-variable-parse-error") ||
            error_msg.contains("variable name issue") ||
            error_msg.contains("Unexpected character")
        );
    }
}

