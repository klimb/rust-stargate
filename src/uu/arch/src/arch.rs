// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use platform_info::*;

use clap::Command;
use uucore::error::{UResult, USimpleError};
use uucore::translate;
use uucore::object_output::{self, JsonOutputOptions};
use serde_json::json;

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let opts = JsonOutputOptions::from_matches(&matches);
    let field_filter = matches.get_one::<String>(object_output::ARG_FIELD).map(|s| s.as_str());

    let uts =
        PlatformInfo::new().map_err(|_e| USimpleError::new(1, translate!("cannot-get-system")))?;
    let arch = uts.machine().to_string_lossy().trim().to_string();

    let output = object_output::filter_fields(json!({"architecture": arch}), field_filter);
    object_output::output(opts, output, || {
        println!("{}", arch);
        Ok(())
    })?;

    Ok(())
}

pub fn uu_app() -> Command {
    let cmd = Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template(uucore::util_name()))
        .about(translate!("arch-about"))
        .after_help(translate!("arch-after-help"))
        .infer_long_args(true);

    object_output::add_json_args(cmd)
}
