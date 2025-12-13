

use clap::{Arg, ArgAction, Command};
use serde_json::json;
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::translate;
use sgcore::{
    display::Quotable,
    entries::{Locate, Passwd, get_groups_gnu, gid2grp},
    error::{SGError, SGResult},
    format_usage, show,
};
use std::collections::HashMap;
use thiserror::Error;

mod options {
    pub const USERS: &str = "USERNAME";
}

#[derive(Debug, Error)]
enum GroupsError {
    #[error("{message}", message = translate!("groups-error-fetch"))]
    GetGroupsFailed,

    #[error("{message} {gid}", message = translate!("groups-error-notfound"), gid = .0)]
    GroupNotFound(u32),

    #[error("{user}: {message}", user = .0.quote(), message = translate!("groups-error-user"))]
    UserNotFound(String),
}

impl SGError for GroupsError {}

fn infallible_gid2grp(gid: &u32) -> String {
    match gid2grp(*gid) {
        Ok(grp) => grp,
        Err(_) => {
            show!(GroupsError::GroupNotFound(*gid));
            gid.to_string()
        }
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "getpw"])?;

    let json_output_options = StardustOutputOptions::from_matches(&matches);

    let users: Vec<String> = matches
        .get_many::<String>(options::USERS)
        .map(|v| v.map(ToString::to_string).collect())
        .unwrap_or_default();

    if users.is_empty() {
        let Ok(gids) = get_groups_gnu(None) else {
            return Err(GroupsError::GetGroupsFailed.into());
        };
        let groups: Vec<String> = gids.iter().map(infallible_gid2grp).collect();

        if json_output_options.stardust_output {
            let output = json!({ "groups": groups });
            stardust_output::output(json_output_options, output, || Ok(()))?;
        } else {
            println!("{}", groups.join(" "));
        }
        return Ok(());
    }

    if json_output_options.stardust_output {
        let mut user_groups: HashMap<String, Vec<String>> = HashMap::new();
        for user in users {
            match Passwd::locate(user.as_str()) {
                Ok(p) => {
                    let groups: Vec<String> = p.belongs_to().iter().map(infallible_gid2grp).collect();
                    user_groups.insert(user, groups);
                }
                Err(_) => {
                    show!(GroupsError::UserNotFound(user.clone()));
                    user_groups.insert(user, vec![]);
                }
            }
        }
        let json_value = serde_json::to_value(&user_groups)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        stardust_output::output(json_output_options, json_value, || Ok(()))?;
    } else {
        for user in users {
            match Passwd::locate(user.as_str()) {
                Ok(p) => {
                    let groups: Vec<String> = p.belongs_to().iter().map(infallible_gid2grp).collect();
                    println!("{user} : {}", groups.join(" "));
                }
                Err(_) => {
                    show!(GroupsError::UserNotFound(user));
                }
            }
        }
    }
    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("groups-about"))
        .override_usage(format_usage(&translate!("groups-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::USERS)
                .action(ArgAction::Append)
                .value_name(options::USERS)
                .value_hint(clap::ValueHint::Username)
        );
    stardust_output::add_json_args(cmd)
}

