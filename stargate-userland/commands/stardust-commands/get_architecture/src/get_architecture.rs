use platform_info::*;

use clap::Command;
use sgcore::error::{UResult, USimpleError};
use sgcore::translate;
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::json;

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;
    let opts = StardustOutputOptions::from_matches(&matches);
    let field_filter = matches.get_one::<String>(stardust_output::ARG_FIELD).map(|s| s.as_str());

    let uts =
        PlatformInfo::new().map_err(|_e| USimpleError::new(1, translate!("cannot-get-system")))?;
    let arch = uts.machine().to_string_lossy().trim().to_string();

    let output = stardust_output::filter_fields(json!({"architecture": arch}), field_filter);
    stardust_output::output(opts, output, || {
        println!("{}", arch);
        Ok(())
    })?;

    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("arch-about"))
        .after_help(translate!("arch-after-help"))
        .infer_long_args(true);

    stardust_output::add_json_args(cmd)
}
