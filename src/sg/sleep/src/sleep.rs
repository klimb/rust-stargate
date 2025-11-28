// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{Arg, ArgAction, Command};
use std::thread;
use std::time::Duration;
use sgcore::translate;
use sgcore::{
    error::{UResult, USimpleError, UUsageError},
    format_usage,
    parser::parse_time,
    show_error,
};

mod options {
    pub const NUMBER: &str = "NUMBER";
}

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;

    let numbers = matches
        .get_many::<String>(options::NUMBER)
        .ok_or_else(|| {
            USimpleError::new(
                1,
                translate!("sleep-error-missing-operand", "program" => sgcore::execution_phrase())
            )
        })?
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    sleep(&numbers)
}

pub fn uu_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("sleep-about"))
        .after_help(translate!("sleep-after-help"))
        .override_usage(format_usage(&translate!("sleep-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::NUMBER)
                .help(translate!("sleep-help-number"))
                .value_name(options::NUMBER)
                .action(ArgAction::Append)
        )
}

fn sleep(args: &[&str]) -> UResult<()> {
    let mut arg_error = false;

    let sleep_dur = args
        .iter()
        .filter_map(|input| match parse_time::from_str(input, true) {
            Ok(duration) => Some(duration),
            Err(error) => {
                arg_error = true;
                show_error!("{error}");
                None
            }
        })
        .fold(Duration::ZERO, |acc, n| acc.saturating_add(n));

    if arg_error {
        return Err(UUsageError::new(1, ""));
    }
    thread::sleep(sleep_dur);
    Ok(())
}
