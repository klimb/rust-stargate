// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore hashset Addrs addrs

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::json;

use uucore::translate;
use uucore::{
    error::{CommandResult, FromIo, UResult, USimpleError},
    format_usage,
};
use uucore::error::CommandResult::{Success, Error};

static OBJ_FLAG: &str = "obj";

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;

    if matches.get_flag(OBJ_FLAG) {
        produce_json(&matches)
    } else {
        produce(&matches)
    }
}

// `CommandResult` is now available in `uucore::error` for shared use across all commands.
#[uucore::to_obj]
pub fn to_obj(args: impl uucore::Args) -> CommandResult<()> {
    let matches = match uucore::clap_localization::handle_clap_result(uu_app(), args) {
        Ok(m) => m,
        Err(e) => return CommandResult::Error(e),
    };
    produce_object(&matches)
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template(uucore::util_name()))
        .about(translate!("get_fqdn-about"))
        .override_usage(format_usage(&translate!("get_fqdn-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(OBJ_FLAG)
                .short('o')
                .long("obj")
                .help("Output result as JSON object")
                .action(ArgAction::SetTrue)
        )
}

fn produce(_matches: &ArgMatches) -> UResult<()> {
    let fqdn = hostname::get()
        .map_err_context(|| "failed to get FQDN".to_owned())?
        .to_string_lossy()
        .into_owned();

    println!("{}", fqdn);

    Ok(())
}

fn produce_json(_matches: &ArgMatches) -> UResult<()> {
    let fqdn = hostname::get()
        .map_err_context(|| "failed to get FQDN".to_owned())?
        .to_string_lossy()
        .into_owned();

    let obj = json!({
        "status": "success",
        "value": fqdn,
        "flags": {
            "obj": true
        }
    });

    println!("{}", obj.to_string());

    Ok(())
}

fn produce_object(_matches: &ArgMatches) -> CommandResult<()> {
    let fqdn = match hostname::get() {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(e) => return CommandResult::Error(USimpleError::new(1, format!("failed to get FQDN: {}", e))),
    };

    println!("{}", fqdn);

    CommandResult::Success(())
}
