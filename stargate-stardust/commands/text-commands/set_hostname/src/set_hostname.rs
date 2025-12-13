

use std::ffi::OsString;

use clap::builder::ValueParser;
use clap::{Arg, Command};

use sgcore::translate;

use sgcore::{
    error::{FromIo, SGResult},
    format_usage,
};

static ARG_HOSTNAME: &str = "hostname";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "dns", "inet"])?;

    let host = matches.get_one::<OsString>(ARG_HOSTNAME)
        .ok_or_else(|| sgcore::error::SGUsageError::new(1, translate!("set-hostname-error-missing-operand")))?;

    hostname::set(host).map_err_context(|| translate!("set-hostname-error-set-hostname"))
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("set-hostname-about"))
        .override_usage(format_usage(&translate!("set-hostname-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(ARG_HOSTNAME)
                .required(true)
                .value_parser(ValueParser::os_string())
                .value_hint(clap::ValueHint::Hostname)
                .help(translate!("set-hostname-help-hostname"))
        )
}

