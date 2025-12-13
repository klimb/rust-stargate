

use clap::{Arg, ArgAction, Command};
use std::io::{IsTerminal, Write};
use sgcore::error::{SGResult, set_exit_code};
use sgcore::format_usage;
use sgcore::stardust_output::{self, StardustOutputOptions};

use sgcore::translate;

mod options {
    pub const SILENT: &str = "silent";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result_with_exit_code(sg_app(), args, 2)?;
    sgcore::pledge::apply_pledge(&["stdio", "tty"])?;
    let object_output = StardustOutputOptions::from_matches(&matches);

    let silent = matches.get_flag(options::SILENT);

    if silent {
        return if std::io::stdin().is_terminal() {
            Ok(())
        } else {
            Err(1.into())
        };
    }

    let name = nix::unistd::ttyname(std::io::stdin());

    if object_output.stardust_output {
        let output = match &name {
            Ok(n) => serde_json::json!({
                "tty": n.display().to_string(),
                "is_tty": true
            }),
            Err(_) => {
                set_exit_code(1);
                serde_json::json!({
                    "tty": null,
                    "is_tty": false,
                    "message": translate!("tty-not-a-tty")
                })
            }
        };
        stardust_output::output(object_output, output, || Ok(()))?;
    } else {
        let mut stdout = std::io::stdout();
        let write_result = match name {
            Ok(name) => writeln!(stdout, "{}", name.display()),
            Err(_) => {
                set_exit_code(1);
                writeln!(stdout, "{}", translate!("tty-not-a-tty"))
            }
        };

        if write_result.is_err() || stdout.flush().is_err() {
            std::process::exit(3);
        }
    }

    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .about(translate!("tty-about"))
        .override_usage(format_usage(&translate!("tty-usage")))
        .infer_long_args(true);
    let cmd = sgcore::clap_localization::configure_localized_command(cmd).arg(
        Arg::new(options::SILENT)
            .long(options::SILENT)
            .visible_alias("quiet")
            .short('s')
            .help(translate!("tty-help-silent"))
            .action(ArgAction::SetTrue)
    );

    stardust_output::add_json_args(cmd)
}

