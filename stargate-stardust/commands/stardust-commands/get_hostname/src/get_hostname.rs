

use std::str;

use clap::{Arg, ArgAction, ArgMatches, Command};
use sgcore::translate;

use sgcore::{
    error::{FromIo, SGResult},
    format_usage,
    stardust_output::{self, StardustOutputOptions},
};

static SHORT_FLAG: &str = "short";
static DOMAIN_FLAG: &str = "domain";
static FQDN_FLAG: &str = "fqdn";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let object_output = StardustOutputOptions::from_matches(&matches);

    if object_output.stardust_output {
        produce_json(&matches, object_output)
    } else {
        produce(&matches)
    }
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("get_hostname-about"))
        .override_usage(format_usage(&translate!("get_hostname-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(DOMAIN_FLAG)
                .short('d')
                .long("domain")
                .overrides_with_all([DOMAIN_FLAG, FQDN_FLAG, SHORT_FLAG])
                .help(translate!("get_hostname-help-domain"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(FQDN_FLAG)
                .long("fqdn")
                .overrides_with_all([DOMAIN_FLAG, FQDN_FLAG, SHORT_FLAG])
                .help(translate!("get_hostname-help-fqdn"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(SHORT_FLAG)
                .short('s')
                .long("short")
                .overrides_with_all([DOMAIN_FLAG, FQDN_FLAG, SHORT_FLAG])
                .help(translate!("get_hostname-help-short"))
                .action(ArgAction::SetTrue)
        );

    stardust_output::add_json_args(cmd)
}

fn produce(matches: &ArgMatches) -> SGResult<()> {
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
                println!("{}", short_name);
            } else {
                let domain_name = &fqdn[dot.0 + 1..];
                println!("{}", domain_name);
            }
        } else if has_short_flag {
            println!("{fqdn}");
        }
        return Ok(());
    }

    println!("{fqdn}");

    Ok(())
}

fn produce_json(matches: &ArgMatches, object_output: StardustOutputOptions) -> SGResult<()> {
    let fqdn = hostname::get()
        .map_err_context(|| "failed to get hostname".to_owned())?
        .to_string_lossy()
        .into_owned();

    let has_short_flag = matches.get_flag(SHORT_FLAG);
    let has_domain_flag = matches.get_flag(DOMAIN_FLAG);
    let has_fqdn_flag = matches.get_flag(FQDN_FLAG);

    let value = if has_short_flag || has_domain_flag {
        let mut it = fqdn.char_indices().filter(|&ci| ci.1 == '.');
        if let Some(dot) = it.next() {
            if has_short_flag {
                fqdn[0..dot.0].to_string()
            } else {
                fqdn[dot.0 + 1..].to_string()
            }
        } else if has_short_flag {
            fqdn.clone()
        } else {
            String::new()
        }
    } else {
        fqdn.clone()
    };

    let output = serde_json::json!({
        "hostname": value,
        "flags": {
            "short": has_short_flag,
            "domain": has_domain_flag,
            "fully_qualified": has_fqdn_flag
        }
    });

    stardust_output::output(object_output, output, || Ok(()))?;
    Ok(())
}

