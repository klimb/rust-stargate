use clap::{Arg, ArgAction, Command};
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::translate;
use sgcore::{error::SGResult, format_usage};
use std::collections::HashMap;
use std::env;

static OPT_NULL: &str = "null";

static ARG_VARIABLES: &str = "variables";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result_with_exit_code(sg_app(), args, 2)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let json_output_options = StardustOutputOptions::from_matches(&matches);

    let variables: Vec<String> = matches
        .get_many::<String>(ARG_VARIABLES)
        .map(|v| v.map(ToString::to_string).collect())
        .unwrap_or_default();

    let separator = if matches.get_flag(OPT_NULL) {
        "\x00"
    } else {
        "\n"
    };

    if json_output_options.stardust_output {
        if variables.is_empty() {
            let env_map: HashMap<String, String> = env::vars().collect();
            let json_value = serde_json::to_value(&env_map)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            stardust_output::output(json_output_options, json_value, || Ok(()))?;
        } else {
            let mut env_map: HashMap<String, Option<String>> = HashMap::new();
            let mut error_found = false;
            for env_var in &variables {
                if env_var.contains('=') {
                    error_found = true;
                    env_map.insert(env_var.clone(), None);
                } else if let Ok(var) = env::var(env_var) {
                    env_map.insert(env_var.clone(), Some(var));
                } else {
                    error_found = true;
                    env_map.insert(env_var.clone(), None);
                }
            }
            let json_value = serde_json::to_value(&env_map)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            stardust_output::output(json_output_options, json_value, || Ok(()))?;
            if error_found { return Err(1.into()); }
        }
        return Ok(());
    }

    if variables.is_empty() {
        for (env_var, value) in env::vars() {
            print!("{env_var}={value}{separator}");
        }
        return Ok(());
    }

    let mut error_found = false;
    for env_var in variables {
        if env_var.contains('=') {
            error_found = true;
            continue;
        }
        if let Ok(var) = env::var(env_var) {
            print!("{var}{separator}");
        } else {
            error_found = true;
        }
    }

    if error_found { Err(1.into()) } else { Ok(()) }
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .about(translate!("printenv-about"))
        .override_usage(format_usage(&translate!("printenv-usage")))
        .infer_long_args(true);
    let cmd = sgcore::clap_localization::configure_localized_command(cmd)
        .arg(
            Arg::new(OPT_NULL)
                .short('0')
                .long(OPT_NULL)
                .help(translate!("printenv-help-null"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(ARG_VARIABLES)
                .action(ArgAction::Append)
                .num_args(1..)
        );
    stardust_output::add_json_args(cmd)
}

