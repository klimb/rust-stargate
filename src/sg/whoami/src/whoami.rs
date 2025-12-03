use clap::Command;
use std::ffi::OsString;
use sgcore::display::println_verbatim;
use sgcore::error::{FromIo, UResult};
use sgcore::translate;
use sgcore::object_output::{self, JsonOutputOptions};
use serde_json::json;

mod platform;

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;
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
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("whoami-about"))
        .override_usage(sgcore::util_name())
        .infer_long_args(true);

    object_output::add_json_args(cmd)
}