use clap::{Arg, ArgAction, Command};
use std::{ffi::OsString, io::Write};
use sgcore::error::{UResult, set_exit_code};

use sgcore::translate;

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let mut command = uu_app();

    let args: Vec<OsString> = args.collect();
    if args.len() > 2 {
        return Ok(());
    }

    if let Err(e) = command.try_get_matches_from_mut(args) {
        let error = match e.kind() {
            clap::error::ErrorKind::DisplayHelp => command.print_help(),
            clap::error::ErrorKind::DisplayVersion => {
                write!(std::io::stdout(), "{}", command.render_version())
            }
            _ => Ok(()),
        };

        if let Err(print_fail) = error {
            // Try to display this error.
            let _ = writeln!(std::io::stderr(), "{}: {print_fail}", sgcore::util_name());
            // Mirror GNU options. When failing to print warnings or version flags, then we exit
            // with FAIL. This avoids allocation some error information which may result in yet
            // other types of failure.
            set_exit_code(1);
        }
    }

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("true-about"))
        // We provide our own help and version options, to ensure maximum compatibility with GNU.
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(
            Arg::new("help")
                .long("help")
                .help(translate!("true-help-text"))
                .action(ArgAction::Help)
        )
        .arg(
            Arg::new("version")
                .long("version")
                .help(translate!("true-version-text"))
                .action(ArgAction::Version)
        )
}
