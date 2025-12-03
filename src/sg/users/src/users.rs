// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (paths) wtmp

use std::ffi::OsString;
use std::path::Path;

use clap::builder::ValueParser;
use clap::{Arg, Command};
use sgcore::error::UResult;
use sgcore::format_usage;
use sgcore::translate;
use sgcore::object_output::{self, JsonOutputOptions};
use serde_json::json;

#[cfg(target_os = "openbsd")]
use utmp_classic::{UtmpEntry, parse_from_path};
#[cfg(not(target_os = "openbsd"))]
use sgcore::utmpx::{self, Utmpx};

#[cfg(target_os = "openbsd")]
const OPENBSD_UTMP_FILE: &str = "/var/run/utmp";

static ARG_FILE: &str = "file";

fn get_long_usage() -> String {
    #[cfg(not(target_os = "openbsd"))]
    let default_path: &str = utmpx::DEFAULT_FILE;
    #[cfg(target_os = "openbsd")]
    let default_path: &str = OPENBSD_UTMP_FILE;

    translate!("users-long-usage", "default_path" => default_path)
}

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;
    let mut opts = JsonOutputOptions::from_matches(&matches);
    // Object output is the default for this command
    if !matches.contains_id("object_output") {
        opts.object_output = true;
    }

    let maybe_file: Option<&Path> = matches.get_one::<OsString>(ARG_FILE).map(AsRef::as_ref);

    let mut users: Vec<String>;

    // OpenBSD uses the Unix version 1 UTMP, all other Unixes use the newer UTMPX
    #[cfg(target_os = "openbsd")]
    {
        let filename = maybe_file.unwrap_or(Path::new(OPENBSD_UTMP_FILE));
        let entries = parse_from_path(filename).unwrap_or_default();
        users = Vec::new();
        for entry in entries {
            if let UtmpEntry::UTMP {
                line: _,
                user,
                host: _,
                time: _,
            } = entry
            {
                if !user.is_empty() {
                    users.push(user);
                }
            }
        }
    };
    #[cfg(not(target_os = "openbsd"))]
    {
        let filename = maybe_file.unwrap_or(utmpx::DEFAULT_FILE.as_ref());

        users = Utmpx::iter_all_records_from(filename)
            .filter(|ut| ut.is_user_process())
            .map(|ut| ut.user())
            .collect::<Vec<_>>();
    };

    users.sort();

    if opts.object_output {
        let output = json!({
            "users": users,
            "count": users.len(),
        });
        object_output::output(opts, output, || Ok(()))?;
    } else {
        if !users.is_empty() {
            println!("{}", users.join(" "));
        }
    }

    Ok(())
}

pub fn uu_app() -> Command {
    #[cfg(not(target_env = "musl"))]
    let about = translate!("users-about");
    #[cfg(target_env = "musl")]
    let about = translate!("users-about") + &translate!("users-about-musl-warning");

    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(about)
        .override_usage(format_usage(&translate!("users-usage")))
        .infer_long_args(true)
        .after_help(get_long_usage())
        .arg(
            Arg::new(ARG_FILE)
                .num_args(1)
                .value_hint(clap::ValueHint::FilePath)
                .value_parser(ValueParser::os_string())
        );
    
    object_output::add_json_args(cmd)
}
