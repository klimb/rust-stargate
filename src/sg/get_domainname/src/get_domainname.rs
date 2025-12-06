// spell-checker:ignore hashset Addrs addrs

#[cfg(not(any(target_os = "freebsd", target_os = "openbsd")))]
use std::net::ToSocketAddrs;

use clap::{ArgMatches, Command};

use sgcore::translate;

use sgcore::{
    error::{FromIo, UResult},
    format_usage,
    object_output::{self, JsonOutputOptions},
};

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    // hostname https://datatracker.ietf.org/doc/html/rfc952
    //    text string up to 24 characters drawn from the alphabet (A-Z), digits (0-9), minus
    //    sign (-), and period (.)
    // in FreeBSD the hostname is the unique name for a specific server, while the domain name
    // provides a broader organizational context. Together, they form a
    // Fully Qualified Domain Name (FQDN),
    
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;
    let object_output = JsonOutputOptions::from_matches(&matches);
    
    if object_output.object_output {
        print_domainname_json(&matches, object_output)
    } else {
        print_domainname()
    }
}

pub fn uu_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("get_domainname-about"))
        .override_usage(format_usage(&translate!("get_domainname-usage")));
    
    object_output::add_json_args(cmd)
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

fn print_domainname_json(_matches: &ArgMatches, object_output: JsonOutputOptions) -> UResult<()> {
    let fqdn = hostname::get()
        .map_err_context(|| "failed to get domain name".to_owned())?
        .to_string_lossy()
        .into_owned();

    let domain_name = {
        let mut it = fqdn.char_indices().filter(|&ci| ci.1 == '.');
        if let Some(dot) = it.next() {
            fqdn[dot.0 + 1..].to_string()
        } else {
            String::new()
        }
    };

    let output = serde_json::json!({
        "domainname": domain_name
    });

    object_output::output(object_output, output, || Ok(()))?;
    Ok(())
}
