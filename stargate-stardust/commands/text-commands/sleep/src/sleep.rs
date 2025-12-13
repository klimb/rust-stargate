use clap::{Arg, ArgAction, Command};
use std::thread;
use std::time::Duration;
use serde_json::json;
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::translate;
use sgcore::{
    error::{SGResult, SGSimpleError, SGUsageError},
    format_usage,
    parser::parse_time,
    show_error,
};

mod options {
    pub const NUMBER: &str = "NUMBER";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let json_output_options = StardustOutputOptions::from_matches(&matches);

    let numbers = matches
        .get_many::<String>(options::NUMBER)
        .ok_or_else(|| {
            SGSimpleError::new(
                1,
                translate!("sleep-error-missing-operand", "program" => sgcore::execution_phrase())
            )
        })?
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    sleep(&numbers, json_output_options)
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
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
        );

    stardust_output::add_json_args(cmd)
}

fn sleep(args: &[&str], json_output_options: StardustOutputOptions) -> SGResult<()> {
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
        return Err(SGUsageError::new(1, ""));
    }

    if json_output_options.stardust_output {
        let output = json!({
            "duration_seconds": sleep_dur.as_secs(),
            "duration_nanos": sleep_dur.subsec_nanos(),
        });
        stardust_output::output(json_output_options, output, || Ok(()))?;
    }

    thread::sleep(sleep_dur);
    Ok(())
}

