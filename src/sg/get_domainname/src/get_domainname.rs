// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore hashset Addrs addrs

#[cfg(not(any(target_os = "freebsd", target_os = "openbsd")))]
use std::net::ToSocketAddrs;

use clap::{Command};

use sgcore::translate;

use sgcore::{
    error::{FromIo, UResult},
    format_usage,
};

#[sgcore::main]
pub fn uumain(_args: impl sgcore::Args) -> UResult<()> {
    // hostname https://datatracker.ietf.org/doc/html/rfc952
    //    text string up to 24 characters drawn from the alphabet (A-Z), digits (0-9), minus
    //    sign (-), and period (.)
    // in FreeBSD the hostname is the unique name for a specific server, while the domain name
    // provides a broader organizational context. Together, they form a
    // Fully Qualified Domain Name (FQDN),
    print_domainname()
}

pub fn uu_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("get_domainname-about"))
        .override_usage(format_usage(&translate!("get_domainname-usage")))
}

fn print_domainname() -> UResult<()> {
    let fqdn = hostname::get()
        .map_err_context(|| "failed to get domain name".to_owned())?
        .to_string_lossy()
        .into_owned();

    let mut it = fqdn.char_indices().filter(|&ci| ci.1 == '.');
    if let Some(dot) = it.next() {
        let domain_name = &fqdn[dot.0 + 1..]; // from dot to end
        println!("{}", domain_name);
    }

    println!();

    Ok(())
}
