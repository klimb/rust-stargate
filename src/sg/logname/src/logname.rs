// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (ToDO) getlogin userlogin

use clap::Command;
use std::ffi::CStr;
use sgcore::translate;
use sgcore::{error::UResult, show_error};

fn get_userlogin() -> Option<String> {
    unsafe {
        let login: *const libc::c_char = libc::getlogin();
        if login.is_null() {
            None
        } else {
            Some(String::from_utf8_lossy(CStr::from_ptr(login).to_bytes()).to_string())
        }
    }
}

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let _ = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;

    match get_userlogin() {
        Some(userlogin) => println!("{userlogin}"),
        None => show_error!("{}", translate!("logname-error-no-login-name")),
    }

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .override_usage(sgcore::util_name())
        .about(translate!("logname-about"))
        .infer_long_args(true)
}
