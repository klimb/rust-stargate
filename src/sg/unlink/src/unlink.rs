// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
use std::ffi::OsString;
use std::fs::remove_file;
use std::path::Path;

use clap::builder::ValueParser;
use clap::{Arg, Command};

use sgcore::display::Quotable;
use sgcore::error::{FromIo, UResult};
use sgcore::format_usage;
use sgcore::translate;

static OPT_PATH: &str = "FILE";

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;

    let path: &Path = matches.get_one::<OsString>(OPT_PATH).unwrap().as_ref();

    remove_file(path)
        .map_err_context(|| translate!("unlink-error-cannot-unlink", "path" => path.quote()))
}

pub fn uu_app() -> Command {
    Command::new(sgcore::util_name())
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
        )
}
