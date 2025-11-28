// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::Command;
use std::ffi::OsString;
use uucore::display::println_verbatim;
use uucore::error::{FromIo, UResult};
use uucore::translate;
use uucore::object_output::{self, JsonOutputOptions};
use serde_json::json;

mod platform;

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let opts = JsonOutputOptions::from_matches(&matches);
    let field_filter = matches.get_one::<String>(object_output::ARG_FIELD).map(|s| s.as_str());
    let username = whoami()?;

    if opts.object_output {
        let username_str = username.to_string_lossy().to_string();
        let output = object_output::filter_fields(json!({"username": username_str}), field_filter);
        object_output::output(opts, output, || Ok(()))?;
    } else {
        println_verbatim(username).map_err_context(|| translate!("whoami-error-failed-to-print"))?;
    }
    Ok(())
}

/// Get the current username
pub fn whoami() -> UResult<OsString> {
    platform::get_username().map_err_context(|| translate!("whoami-error-failed-to-get"))
}

pub fn uu_app() -> Command {
    let cmd = Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template(uucore::util_name()))
        .about(translate!("whoami-about"))
        .override_usage(uucore::util_name())
        .infer_long_args(true);

    object_output::add_json_args(cmd)
}