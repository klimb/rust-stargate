use std::ffi::OsString;
use std::fs::remove_file;
use std::path::Path;

use clap::builder::ValueParser;
use clap::{Arg, Command};
use serde_json::json;

use sgcore::display::Quotable;
use sgcore::error::{FromIo, UResult};
use sgcore::format_usage;
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::translate;

static OPT_PATH: &str = "FILE";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "cpath"])?;
    let json_output_options = StardustOutputOptions::from_matches(&matches);

    let path: &Path = matches.get_one::<OsString>(OPT_PATH).unwrap().as_ref();

    let result = remove_file(path)
        .map_err_context(|| translate!("unlink-error-cannot-unlink", "path" => path.quote()));
    
    if json_output_options.stardust_output {
        let output = json!({
            "path": path.to_string_lossy(),
            "success": result.is_ok(),
        });
        stardust_output::output(json_output_options, output, || Ok(()))?;
    }
    
    result
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("unlink-about"))
        .override_usage(format_usage(&translate!("unlink-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(OPT_PATH)
                .required(true)
                .hide(true)
                .value_parser(ValueParser::os_string())
                .value_hint(clap::ValueHint::AnyPath)
        );
    
    stardust_output::add_json_args(cmd)
}
