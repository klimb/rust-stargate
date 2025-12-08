// spell-checker:ignore (ToDO) getlogin userlogin

use clap::Command;
use std::ffi::CStr;
use sgcore::translate;
use sgcore::{
    error::UResult,
    show_error,
    object_output::{self, JsonOutputOptions},
};

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
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;
    let object_output = JsonOutputOptions::from_matches(&matches);

    match get_userlogin() {
        Some(userlogin) => {
            if object_output.object_output {
                let output = serde_json::json!({
                    "username": userlogin
                });
                object_output::output(object_output, output, || Ok(()))?;
            } else {
                println!("{userlogin}");
            }
        }
        None => show_error!("{}", translate!("get_username-error-no-login-name")),
    }

    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .override_usage(sgcore::util_name())
        .about(translate!("get_username-about"))
        .infer_long_args(true);
    
    object_output::add_json_args(cmd)
}
