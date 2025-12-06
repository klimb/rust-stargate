// spell-checker:ignore hashset Addrs addrs

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde_json::json;

use sgcore::translate;
use sgcore::{
    error::{CommandResult, FromIo, UResult, USimpleError},
    format_usage,
};
use sgcore::error::CommandResult::{Success, Error};

static OBJ_FLAG: &str = "obj";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;

    if matches.get_flag(OBJ_FLAG) {
        produce_json(&matches)
    } else {
        produce(&matches)
    }
}

// `CommandResult` is now available in `sgcore::error` for shared use across all commands.
#[sgcore::to_obj]
pub fn to_obj(args: impl sgcore::Args) -> CommandResult<()> {
    let matches = match sgcore::clap_localization::handle_clap_result(sg_app(), args) {
        Ok(m) => m,
        Err(e) => return CommandResult::Error(e),
    };
    produce_object(&matches)
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
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

    // Split FQDN into hostname and domain
    let parts: Vec<&str> = fqdn.splitn(2, '.').collect();
    let hostname = parts.first().map(|s| s.to_string()).unwrap_or_default();
    let domain = if parts.len() > 1 {
        parts[1].to_string()
    } else {
        String::new()
    };

    let obj = json!({
        "fqdn": fqdn,
        "hostname": hostname,
        "domain": domain,
        "has_domain": !domain.is_empty(),
        "length": fqdn.len()
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
