

use clap::{ArgMatches, Command};

use sgcore::translate;

use sgcore::{
    error::{FromIo, SGResult},
    format_usage,
    stardust_output::{self, StardustOutputOptions},
};

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    let object_output = StardustOutputOptions::from_matches(&matches);

    if object_output.stardust_output {
        print_domainname_json(&matches, object_output)
    } else {
        print_domainname()
    }
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("get_domainname-about"))
        .override_usage(format_usage(&translate!("get_domainname-usage")));

    stardust_output::add_json_args(cmd)
}

fn print_domainname() -> SGResult<()> {
    let fqdn = hostname::get()
        .map_err_context(|| "failed to get domain name".to_owned())?
        .to_string_lossy()
        .into_owned();

    let mut it = fqdn.char_indices().filter(|&ci| ci.1 == '.');
    if let Some(dot) = it.next() {
        let domain_name = &fqdn[dot.0 + 1..];
        println!("{}", domain_name);
    }

    println!();

    Ok(())
}

fn print_domainname_json(_matches: &ArgMatches, object_output: StardustOutputOptions) -> SGResult<()> {
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

    stardust_output::output(object_output, output, || Ok(()))?;
    Ok(())
}

