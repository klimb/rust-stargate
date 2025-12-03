use std::ffi::OsString;
use std::fs::remove_file;
use std::path::Path;

use clap::builder::ValueParser;
use clap::{Arg, Command};
use serde_json::json;

use sgcore::display::Quotable;
use sgcore::error::{FromIo, UResult};
use sgcore::format_usage;
use sgcore::object_output::{self, JsonOutputOptions};
use sgcore::translate;

static OPT_PATH: &str = "FILE";

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;
    let json_output_options = JsonOutputOptions::from_matches(&matches);

    let path: &Path = matches.get_one::<OsString>(OPT_PATH).unwrap().as_ref();

    let result = remove_file(path)
        .map_err_context(|| translate!("unlink-error-cannot-unlink", "path" => path.quote()));
    
    if json_output_options.object_output {
        let output = json!({
            "path": path.to_string_lossy(),
            "success": result.is_ok(),
        });
        object_output::output(json_output_options, output, || Ok(()))?;
    }
    
    result
}

pub fn uu_app() -> Command {
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
    
    object_output::add_json_args(cmd)
}
