use clap::{Arg, ArgAction, Command, value_parser};
use libc::mkfifo;
use std::ffi::CString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use sgcore::display::Quotable;
use sgcore::error::{SGResult, SGSimpleError};
use sgcore::translate;

use sgcore::{format_usage, show};

mod options {
    pub static MODE: &str = "mode";
    pub static CONTEXT: &str = "context";
    pub static FIFO: &str = "fifo";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath"])?;

    let mode = calculate_mode(matches.get_one::<String>(options::MODE))
        .map_err(|e| SGSimpleError::new(1, translate!("mkfifo-error-invalid-mode", "error" => e)))?;

    let fifos: Vec<String> = match matches.get_many::<String>(options::FIFO) {
        Some(v) => v.cloned().collect(),
        None => {
            return Err(SGSimpleError::new(
                1,
                translate!("mkfifo-error-missing-operand")
            ));
        }
    };

    for f in fifos {
        let err = unsafe {
            let name = CString::new(f.as_bytes()).unwrap();
            mkfifo(name.as_ptr(), 0o666)
        };
        if err == -1 {
            show!(SGSimpleError::new(
                1,
                translate!("mkfifo-error-cannot-create-fifo", "path" => f.quote())
            ));
        }

        if let Err(e) = fs::set_permissions(&f, fs::Permissions::from_mode(mode)) {
            return Err(SGSimpleError::new(
                1,
                translate!("mkfifo-error-cannot-set-permissions", "path" => f.quote(), "error" => e)
            ));
        }

    }

    Ok(())
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .override_usage(format_usage(&translate!("mkfifo-usage")))
        .about(translate!("mkfifo-about"))
        .infer_long_args(true)
        .arg(
            Arg::new(options::MODE)
                .short('m')
                .long(options::MODE)
                .help(translate!("mkfifo-help-mode"))
                .value_name("MODE")
        )
        .arg(
            Arg::new(options::CONTEXT)
                .long(options::CONTEXT)
                .value_name("CTX")
                .value_parser(value_parser!(String))
                .num_args(0..=1)
                .require_equals(true)
                .help(translate!("mkfifo-help-context"))
        )
        .arg(
            Arg::new(options::FIFO)
                .hide(true)
                .action(ArgAction::Append)
                .value_hint(clap::ValueHint::AnyPath)
        )
}

fn calculate_mode(mode_option: Option<&String>) -> Result<u32, String> {
    let umask = sgcore::mode::get_umask();
    let mut mode = 0o666;

    if let Some(m) = mode_option {
        if m.chars().any(|c| c.is_ascii_digit()) {
            mode = sgcore::mode::parse_numeric(mode, m, false)?;
        } else {
            for item in m.split(',') {
                mode = sgcore::mode::parse_symbolic(mode, item, umask, false)?;
            }
        }
    } else {
        mode &= !umask;
    }

    Ok(mode)
}

