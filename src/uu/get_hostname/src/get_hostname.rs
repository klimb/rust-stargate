// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore hashset Addrs addrs

#[cfg(not(any(target_os = "freebsd", target_os = "openbsd")))]
use std::net::ToSocketAddrs;
use std::str;
use std::{collections::hash_set::HashSet, ffi::OsString};

use clap::builder::ValueParser;
use clap::{Arg, ArgAction, ArgMatches, Command};

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
use dns_lookup::lookup_host;
use uucore::translate;

use uucore::{
    error::{FromIo, UResult},
    format_usage,
};

static DOMAIN_FLAG: &str = "domain";
static FQDN_FLAG: &str = "fqdn";
static SHORT_FLAG: &str = "short";

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    display_hostname(&matches)
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template(uucore::util_name()))
        .about(translate!("get_hostname-about"))
        .override_usage(format_usage(&translate!("get_hostname-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(DOMAIN_FLAG)
                .short('d')
                .long("domain")
                .overrides_with_all([DOMAIN_FLAG, FQDN_FLAG, SHORT_FLAG])
                .help(translate!("get_hostname-help-domain"))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(FQDN_FLAG)
                .short('f')
                .long("fqdn")
                .overrides_with_all([DOMAIN_FLAG, FQDN_FLAG, SHORT_FLAG])
                .help(translate!("get_hostname-help-fqdn"))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(SHORT_FLAG)
                .short('s')
                .long("short")
                .overrides_with_all([DOMAIN_FLAG, FQDN_FLAG, SHORT_FLAG])
                .help(translate!("get_hostname-help-short"))
                .action(ArgAction::SetTrue),
        )
}

fn display_hostname(matches: &ArgMatches) -> UResult<()> {
    let hostname = hostname::get()
        .map_err_context(|| "failed to get hostname".to_owned())?
        .to_string_lossy()
        .into_owned();

    if matches.get_flag(SHORT_FLAG) || matches.get_flag(DOMAIN_FLAG) {
        let mut it = hostname.char_indices().filter(|&ci| ci.1 == '.');
        if let Some(ci) = it.next() {
            if matches.get_flag(SHORT_FLAG) {
                println!("{}", &hostname[0..ci.0]);
            } else {
                println!("{}", &hostname[ci.0 + 1..]);
            }
        } else if matches.get_flag(SHORT_FLAG) {
            println!("{hostname}");
        }
        return Ok(());
    }

    println!("{hostname}");

    Ok(())
}
