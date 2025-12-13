
use clap::{Arg, ArgAction, Command};
#[cfg(any(target_os = "linux"))]
use nix::errno::Errno;
#[cfg(any(target_os = "linux"))]
use nix::fcntl::{OFlag, open};
#[cfg(any(target_os = "linux"))]
use nix::sys::stat::Mode;
use std::path::Path;
use sgcore::display::Quotable;
#[cfg(any(target_os = "linux"))]
use sgcore::error::FromIo;
use sgcore::error::{SGResult, SGSimpleError};
use sgcore::format_usage;
use sgcore::translate;
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::json;

pub mod options {
    pub static FILE_SYSTEM: &str = "file-system";
    pub static DATA: &str = "data";
}

static ARG_FILES: &str = "files";
mod platform {
    use nix::unistd::sync;
    #[cfg(any(target_os = "linux"))]
    use nix::unistd::{fdatasync, syncfs};
    #[cfg(any(target_os = "linux"))]
    use std::fs::File;
    use sgcore::error::SGResult;

    pub fn do_sync() -> SGResult<()> {
        sync();
        Ok(())
    }

    #[cfg(any(target_os = "linux"))]
    pub fn do_syncfs(files: Vec<String>) -> SGResult<()> {
        for path in files {
            let f = File::open(path).unwrap();
            syncfs(f)?;
        }
        Ok(())
    }

    #[cfg(any(target_os = "linux"))]
    pub fn do_fdatasync(files: Vec<String>) -> SGResult<()> {
        for path in files {
            let f = File::open(path).unwrap();
            fdatasync(f)?;
        }
        Ok(())
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;
    let mut opts = StardustOutputOptions::from_matches(&matches);
    if !matches.contains_id("stardust_output") {
        opts.stardust_output = true;
    }
    let files: Vec<String> = matches
        .get_many::<String>(ARG_FILES)
        .map(|v| v.map(ToString::to_string).collect())
        .unwrap_or_default();

    if matches.get_flag(options::DATA) && files.is_empty() {
        return Err(SGSimpleError::new(
            1,
            translate!("sync-error-data-needs-argument")
        ));
    }

    for f in &files {
        #[cfg(any(target_os = "linux"))]
        {
            let path = Path::new(&f);
            if let Err(e) = open(path, OFlag::O_NONBLOCK, Mode::empty()) {
                if e != Errno::EACCES || (e == Errno::EACCES && path.is_dir()) {
                    e.map_err_context(
                        || translate!("sync-error-opening-file", "file" => f.quote())
                    )?;
                }
            }
        }
        #[cfg(not(any(target_os = "linux")))]
        {
            if !Path::new(&f).exists() {
                return Err(SGSimpleError::new(
                    1,
                    translate!("sync-error-no-such-file", "file" => f.quote())
                ));
            }
        }
    }

    let file_system_sync = matches.get_flag(options::FILE_SYSTEM);
    let data_sync = matches.get_flag(options::DATA);

    #[allow(clippy::if_same_then_else)]
    if file_system_sync {
        #[cfg(any(target_os = "linux"))]
        syncfs(files.clone())?;
    } else {
        sync()?;
    }

    if opts.stardust_output {
        let output = json!({
            "operation": if file_system_sync { "file_system" } else if data_sync { "data" } else { "sync" },
            "files": files,
            "success": true,
        });
        stardust_output::output(opts, output, || Ok(()))?;
    }

    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("sync-about"))
        .override_usage(format_usage(&translate!("sync-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILE_SYSTEM)
                .long(options::FILE_SYSTEM)
                .conflicts_with(options::DATA)
                .help(translate!("sync-help-file-system"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::DATA)
                .short('d')
                .long(options::DATA)
                .conflicts_with(options::FILE_SYSTEM)
                .help(translate!("sync-help-data"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(ARG_FILES)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::AnyPath)
        );

    stardust_output::add_json_args(cmd)
}

fn sync() -> SGResult<()> {
    platform::do_sync()
}

#[cfg(any(target_os = "linux"))]
fn syncfs(files: Vec<String>) -> SGResult<()> {
    platform::do_syncfs(files)
}

#[cfg(any(target_os = "linux"))]
fn fdatasync(files: Vec<String>) -> SGResult<()> {
    platform::do_fdatasync(files)
}

