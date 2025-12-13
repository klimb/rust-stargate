

use sgcore::display::Quotable;
use sgcore::entries;
use sgcore::error::{FromIo, SGResult, SGSimpleError};
use sgcore::format_usage;
use sgcore::perms::{GidUidOwnerFilter, IfFrom, chown_base, options};
use sgcore::translate;

use clap::{Arg, ArgAction, ArgMatches, Command};

use std::fs;
use std::os::unix::fs::MetadataExt;

fn parse_gid_from_str(group: &str) -> Result<u32, String> {
    if let Some(gid_str) = group.strip_prefix(':') {
        gid_str
            .parse::<u32>()
            .map_err(|_| translate!("change_group-error-invalid-group-id", "gid_str" => gid_str))
    } else {
        match entries::grp2gid(group) {
            Ok(g) => Ok(g),
            Err(_) => group
                .parse::<u32>()
                .map_err(|_| translate!("change_group-error-invalid-group", "group" => group)),
        }
    }
}

fn get_dest_gid(matches: &ArgMatches) -> SGResult<(Option<u32>, String)> {
    let mut raw_group = String::new();
    let dest_gid = if let Some(file) = matches.get_one::<std::ffi::OsString>(options::REFERENCE) {
        let path = std::path::Path::new(file);
        fs::metadata(path)
            .map(|meta| {
                let gid = meta.gid();
                raw_group = entries::gid2grp(gid).unwrap_or_else(|_| gid.to_string());
                Some(gid)
            })
            .map_err_context(
                || translate!("change_group-error-failed-to-get-attributes", "file" => path.quote())
            )?
    } else {
        let group = matches
            .get_one::<String>(options::ARG_GROUP)
            .map(|s| s.as_str())
            .unwrap_or_default();
        raw_group = group.to_string();
        if group.is_empty() {
            None
        } else {
            match parse_gid_from_str(group) {
                Ok(g) => Some(g),
                Err(e) => return Err(SGSimpleError::new(1, e)),
            }
        }
    };
    Ok((dest_gid, raw_group))
}

fn parse_gid_and_uid(matches: &ArgMatches) -> SGResult<GidUidOwnerFilter> {
    let (dest_gid, raw_group) = get_dest_gid(matches)?;

    let filter = if let Some(from_group) = matches.get_one::<String>(options::FROM) {
        match parse_gid_from_str(from_group) {
            Ok(g) => IfFrom::Group(g),
            Err(_) => {
                return Err(SGSimpleError::new(
                    1,
                    translate!("change_group-error-invalid-user", "from_group" => from_group)
                ));
            }
        }
    } else {
        IfFrom::All
    };

    Ok(GidUidOwnerFilter {
        dest_gid,
        dest_uid: None,
        raw_owner: raw_group,
        filter,
    })
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "fattr"])?;
    chown_base(sg_app(), args, options::ARG_GROUP, parse_gid_and_uid, true)
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .about(translate!("change_group-about"))
        .override_usage(format_usage(&translate!("change_group-usage")))
        .infer_long_args(true);
    sgcore::clap_localization::configure_localized_command(cmd)
        .disable_help_flag(true)
        .arg(
            Arg::new(options::HELP)
                .long(options::HELP)
                .help(translate!("change_group-help-print-help"))
                .action(ArgAction::Help)
        )
        .arg(
            Arg::new(options::verbosity::CHANGES)
                .short('c')
                .long(options::verbosity::CHANGES)
                .help(translate!("change_group-help-changes"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::verbosity::SILENT)
                .short('f')
                .long(options::verbosity::SILENT)
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::verbosity::QUIET)
                .long(options::verbosity::QUIET)
                .help(translate!("change_group-help-quiet"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::verbosity::VERBOSE)
                .short('v')
                .long(options::verbosity::VERBOSE)
                .help(translate!("change_group-help-verbose"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::preserve_root::PRESERVE)
                .long(options::preserve_root::PRESERVE)
                .help(translate!("change_group-help-preserve-root"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::preserve_root::NO_PRESERVE)
                .long(options::preserve_root::NO_PRESERVE)
                .help(translate!("change_group-help-no-preserve-root"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::REFERENCE)
                .long(options::REFERENCE)
                .value_name("RFILE")
                .value_hint(clap::ValueHint::FilePath)
                .value_parser(clap::value_parser!(std::ffi::OsString))
                .help(translate!("change_group-help-reference"))
        )
        .arg(
            Arg::new(options::FROM)
                .long(options::FROM)
                .value_name("GROUP")
                .help(translate!("change_group-help-from"))
        )
        .arg(
            Arg::new(options::RECURSIVE)
                .short('R')
                .long(options::RECURSIVE)
                .help(translate!("change_group-help-recursive"))
                .action(ArgAction::SetTrue)
        )
        .args(sgcore::perms::common_args())
}

