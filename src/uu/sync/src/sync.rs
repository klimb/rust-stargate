// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

/* Last synced with: sync (GNU coreutils) 8.13 */

use clap::{Arg, ArgAction, Command};
#[cfg(any(target_os = "linux", target_os = "android"))]
use nix::errno::Errno;
#[cfg(any(target_os = "linux", target_os = "android"))]
use nix::fcntl::{OFlag, open};
#[cfg(any(target_os = "linux", target_os = "android"))]
use nix::sys::stat::Mode;
use std::path::Path;
use uucore::display::Quotable;
#[cfg(any(target_os = "linux", target_os = "android"))]
use uucore::error::FromIo;
use uucore::error::{UResult, USimpleError};
use uucore::format_usage;
use uucore::translate;

pub mod options {
    pub static FILE_SYSTEM: &str = "file-system";
    pub static DATA: &str = "data";
}

static ARG_FILES: &str = "files";

#[cfg(unix)]
mod platform {
    use nix::unistd::sync;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use nix::unistd::{fdatasync, syncfs};
    #[cfg(any(target_os = "linux", target_os = "android"))]
    use std::fs::File;
    use uucore::error::UResult;

    pub fn do_sync() -> UResult<()> {
        sync();
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn do_syncfs(files: Vec<String>) -> UResult<()> {
        for path in files {
            let f = File::open(path).unwrap();
            syncfs(f)?;
        }
        Ok(())
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn do_fdatasync(files: Vec<String>) -> UResult<()> {
        for path in files {
            let f = File::open(path).unwrap();
            fdatasync(f)?;
        }
        Ok(())
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
    let files: Vec<String> = matches
        .get_many::<String>(ARG_FILES)
        .map(|v| v.map(ToString::to_string).collect())
        .unwrap_or_default();

    if matches.get_flag(options::DATA) && files.is_empty() {
        return Err(USimpleError::new(
            1,
            translate!("sync-error-data-needs-argument"),
        ));
    }

    for f in &files {
        // Use the Nix open to be able to set the NONBLOCK flags for fifo files
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            let path = Path::new(&f);
            if let Err(e) = open(path, OFlag::O_NONBLOCK, Mode::empty()) {
                if e != Errno::EACCES || (e == Errno::EACCES && path.is_dir()) {
                    e.map_err_context(
                        || translate!("sync-error-opening-file", "file" => f.quote()),
                    )?;
                }
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            if !Path::new(&f).exists() {
                return Err(USimpleError::new(
                    1,
                    translate!("sync-error-no-such-file", "file" => f.quote()),
                ));
            }
        }
    }

    #[allow(clippy::if_same_then_else)]
    if matches.get_flag(options::FILE_SYSTEM) {
        #[cfg(any(target_os = "linux"))]
        syncfs(files)?;
    } else {
        sync()?;
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .help_template(uucore::localized_help_template(uucore::util_name()))
        .about(translate!("sync-about"))
        .override_usage(format_usage(&translate!("sync-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILE_SYSTEM)
                .short('f')
                .long(options::FILE_SYSTEM)
                .conflicts_with(options::DATA)
                .help(translate!("sync-help-file-system"))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::DATA)
                .short('d')
                .long(options::DATA)
                .conflicts_with(options::FILE_SYSTEM)
                .help(translate!("sync-help-data"))
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(ARG_FILES)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::AnyPath),
        )
}

fn sync() -> UResult<()> {
    platform::do_sync()
}

#[cfg(any(target_os = "linux"))]
fn syncfs(files: Vec<String>) -> UResult<()> {
    platform::do_syncfs(files)
}

#[cfg(any(target_os = "linux"))]
fn fdatasync(files: Vec<String>) -> UResult<()> {
    platform::do_fdatasync(files)
}
