use clap::ArgAction;
use clap::{Arg, Command};
use std::env;
use std::io;
use std::path::PathBuf;
use sgcore::format_usage;

use sgcore::display::println_verbatim;
use sgcore::error::{FromIo, SGResult};
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::json;

use sgcore::translate;
const OPT_LOGICAL: &str = "logical";
const OPT_PHYSICAL: &str = "physical";

fn physical_path() -> io::Result<PathBuf> {
    let path = env::current_dir()?;

    {
        Ok(path)
    }

    #[cfg(not(unix))]
    {
        path.canonicalize()
    }
}

fn logical_path() -> io::Result<PathBuf> {
        use std::path::Path;
        fn looks_reasonable(path: &Path) -> bool {
            if !path.has_root() {
                return false;
            }

            if path
                .to_string_lossy()
                .split(std::path::is_separator)
                .any(|piece| piece == "." || piece == "..")
            {
                return false;
            }

            {
                use std::fs::metadata;
                use std::os::unix::fs::MetadataExt;
                match (metadata(path), metadata(".")) {
                    (Ok(info1), Ok(info2)) => {
                        info1.dev() == info2.dev() && info1.ino() == info2.ino()
                    }
                    _ => false,
                }
            }

            #[cfg(not(unix))]
            {
                use std::fs::canonicalize;
                match (canonicalize(path), canonicalize(".")) {
                    (Ok(path1), Ok(path2)) => path1 == path2,
                    _ => false,
                }
            }
        }

        match env::var_os("PWD").map(PathBuf::from) {
            Some(value) if looks_reasonable(&value) => Ok(value),
            _ => env::current_dir(),
        }

}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;
    let opts = StardustOutputOptions::from_matches(&matches);
    let field_filter = matches.get_one::<String>(stardust_output::ARG_FIELD).map(|s| s.as_str());

    let cwd = if matches.get_flag(OPT_PHYSICAL) {
        physical_path()
    } else if matches.get_flag(OPT_LOGICAL) || env::var("POSIXLY_CORRECT").is_ok() {
        logical_path()
    } else {
        physical_path()
    }
    .map_err_context(|| translate!("pwd-error-failed-to-get-current-directory"))?;

    if opts.stardust_output {
        let path_str = cwd.to_string_lossy().to_string();
        let output = json!({
            "path": path_str,
            "absolute": cwd.is_absolute(),
            "mode": if matches.get_flag(OPT_PHYSICAL) { "physical" }
                    else if matches.get_flag(OPT_LOGICAL) { "logical" }
                    else { "physical" }
        });
        let filtered = stardust_output::filter_fields(output, field_filter);
        stardust_output::output(opts, filtered, || Ok(()))?;
    } else {
        println_verbatim(cwd)
            .map_err_context(|| translate!("pwd-error-failed-to-print-current-directory"))?;
    }
    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("pwd-about"))
        .override_usage(format_usage(&translate!("pwd-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(OPT_LOGICAL)
                .short('L')
                .long(OPT_LOGICAL)
                .help(translate!("pwd-help-logical"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_PHYSICAL)
                .short('P')
                .long(OPT_PHYSICAL)
                .overrides_with(OPT_LOGICAL)
                .help(translate!("pwd-help-physical"))
                .action(ArgAction::SetTrue)
        );

    stardust_output::add_json_args(cmd)
}

