use clap::builder::ValueParser;
use clap::{Arg, Command};
use std::ffi::OsString;
use std::fs::hard_link;
use std::path::Path;
use serde_json::json;
use sgcore::display::Quotable;
use sgcore::error::{FromIo, SGResult};
use sgcore::format_usage;
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::translate;

pub mod options {
    pub static FILES: &str = "FILES";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath"])?;
    let json_output_options = StardustOutputOptions::from_matches(&matches);

    let files: Vec<_> = matches
        .get_many::<OsString>(options::FILES)
        .unwrap_or_default()
        .collect();

    let old = Path::new(files[0]);
    let new = Path::new(files[1]);

    let result = hard_link(old, new).map_err_context(
        || translate!("link-error-cannot-create-link", "new" => new.quote(), "old" => old.quote())
    );

    if json_output_options.stardust_output {
        let output = json!({
            "source": old.to_string_lossy(),
            "destination": new.to_string_lossy(),
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
        .about(translate!("link-about"))
        .override_usage(format_usage(&translate!("link-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILES)
                .hide(true)
                .required(true)
                .num_args(2)
                .value_hint(clap::ValueHint::AnyPath)
                .value_parser(ValueParser::os_string())
        );

    stardust_output::add_json_args(cmd)
}

