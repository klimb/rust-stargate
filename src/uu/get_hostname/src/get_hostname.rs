// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore hashset Addrs addrs

#[cfg(not(any(target_os = "freebsd", target_os = "openbsd")))]
use std::net::ToSocketAddrs;
use std::str;

use clap::{Arg, ArgAction, ArgMatches, Command};

use uucore::translate;

use uucore::{
    error::{FromIo, UResult},
    format_usage,
};

static SHORT_FLAG: &str = "short";
static DOMAIN_FLAG: &str = "domain";
static FQDN_FLAG: &str = "fqdn";

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    // hostname https://datatracker.ietf.org/doc/html/rfc952
    //    text string up to 24 characters drawn from the alphabet (A-Z), digits (0-9), minus
    //    sign (-), and period (.)
    // in FreeBSD the hostname is the unique name for a specific server, while the domain name
    // provides a broader organizational context. Together, they form a
    // Fully Qualified Domain Name (FQDN),
    print_name(&matches)
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

fn print_name(matches: &ArgMatches) -> UResult<()> {
    let fqdn = hostname::get()
        .map_err_context(|| "failed to get hostname".to_owned())?
        .to_string_lossy()
        .into_owned();

    let has_short_flag = matches.get_flag(SHORT_FLAG);
    let has_domain_flag = matches.get_flag(DOMAIN_FLAG);
    if has_short_flag || has_domain_flag {
        let mut it = fqdn.char_indices().filter(|&ci| ci.1 == '.');
        if let Some(dot) = it.next() {
            if has_short_flag {
                let short_name = &fqdn[0..dot.0];
                println!("{}", short_name); // up to dot
            } else {
                let domain_name = &fqdn[dot.0 + 1..]; // from dot to end
                println!("{}", domain_name);
            }
        } else if has_short_flag { // happens when domain is not set (it can be empty)
            println!("{fqdn}");    // in that case fqdn is the short name
        }
        return Ok(());
    }

    println!("{fqdn}");

    Ok(())
}
