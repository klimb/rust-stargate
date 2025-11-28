// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::builder::ValueParser;
use clap::{Arg, Command};
use std::ffi::OsString;
use std::fs::hard_link;
use std::path::Path;
use serde_json::json;
use sgcore::display::Quotable;
use sgcore::error::{FromIo, UResult};
use sgcore::format_usage;
use sgcore::object_output::{self, JsonOutputOptions};
use sgcore::translate;

pub mod options {
    pub static FILES: &str = "FILES";
}

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;
    let json_output_options = JsonOutputOptions::from_matches(&matches);
    
    let files: Vec<_> = matches
        .get_many::<OsString>(options::FILES)
        .unwrap_or_default()
        .collect();

    let old = Path::new(files[0]);
    let new = Path::new(files[1]);

    let result = hard_link(old, new).map_err_context(
        || translate!("link-error-cannot-create-link", "new" => new.quote(), "old" => old.quote())
    );
    
    if json_output_options.object_output {
        let output = json!({
            "source": old.to_string_lossy(),
            "destination": new.to_string_lossy(),
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
    
    object_output::add_json_args(cmd)
}
