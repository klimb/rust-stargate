// spell-checker:ignore (ToDO) sysv

use clap::{Arg, ArgAction, Command};
use std::ffi::OsString;
use std::fs::File;
use std::io::{ErrorKind, Read, Write, stdin, stdout};
use std::path::Path;
use serde_json::json;
use sgcore::display::Quotable;
use sgcore::error::{FromIo, UResult, USimpleError};
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::translate;

use sgcore::{format_usage, show};

fn bsd_sum(mut reader: impl Read) -> std::io::Result<(usize, u16)> {
    let mut buf = [0; 4096];
    let mut bytes_read = 0;
    let mut checksum: u16 = 0;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                bytes_read += n;
                checksum = buf[..n].iter().fold(checksum, |acc, &byte| {
                    let rotated = acc.rotate_right(1);
                    rotated.wrapping_add(u16::from(byte))
                });
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => (),
            Err(e) => return Err(e),
        }
    }

    // Report blocks read in terms of 1024-byte blocks.
    let blocks_read = bytes_read.div_ceil(1024);
    Ok((blocks_read, checksum))
}

fn sysv_sum(mut reader: impl Read) -> std::io::Result<(usize, u16)> {
    let mut buf = [0; 4096];
    let mut bytes_read = 0;
    let mut ret = 0u32;

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                bytes_read += n;
                ret = buf[..n]
                    .iter()
                    .fold(ret, |acc, &byte| acc.wrapping_add(u32::from(byte)));
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => (),
            Err(e) => return Err(e),
        }
    }

    ret = (ret & 0xffff) + (ret >> 16);
    ret = (ret & 0xffff) + (ret >> 16);

    // Report blocks read in terms of 512-byte blocks.
    let blocks_read = bytes_read.div_ceil(512);
    Ok((blocks_read, ret as u16))
}

fn open(name: &OsString) -> UResult<Box<dyn Read>> {
    if name == "-" {
        Ok(Box::new(stdin()) as Box<dyn Read>)
    } else {
        let path = Path::new(name);
        if path.is_dir() {
            return Err(USimpleError::new(
                2,
                translate!("sum-error-is-directory", "name" => name.to_string_lossy().maybe_quote())
            ));
        }
        // Silent the warning as we want to the error message
        if path.metadata().is_err() {
            return Err(USimpleError::new(
                2,
                translate!("sum-error-no-such-file-or-directory", "name" => name.to_string_lossy().maybe_quote())
            ));
        }
        let f = File::open(path).map_err_context(String::new)?;
        Ok(Box::new(f) as Box<dyn Read>)
    }
}

mod options {
    pub static FILE: &str = "file";
    pub static BSD_COMPATIBLE: &str = "r";
    pub static SYSTEM_V_COMPATIBLE: &str = "sysv";
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath"])?;
    let json_output_options = StardustOutputOptions::from_matches(&matches);

    let files: Vec<OsString> = match matches.get_many::<OsString>(options::FILE) {
        Some(v) => v.cloned().collect(),
        None => vec![OsString::from("-")],
    };

    let sysv = matches.get_flag(options::SYSTEM_V_COMPATIBLE);

    if json_output_options.stardust_output {
        let mut results = Vec::new();
        
        for file in &files {
            let reader = match open(file) {
                Ok(f) => f,
                Err(error) => {
                    results.push(json!({
                        "file": file.to_string_lossy(),
                        "error": error.to_string(),
                        "success": false
                    }));
                    continue;
                }
            };
            let (blocks, sum) = if sysv {
                sysv_sum(reader)
            } else {
                bsd_sum(reader)
            }?;
            
            results.push(json!({
                "file": file.to_string_lossy(),
                "checksum": sum,
                "blocks": blocks,
                "algorithm": if sysv { "sysv" } else { "bsd" },
                "success": true
            }));
        }
        
        let output = if results.len() == 1 {
            results.into_iter().next().unwrap()
        } else {
            json!({ "files": results })
        };
        
        stardust_output::output(json_output_options, output, || Ok(()))?;
    } else {
        let print_names = files.len() > 1 || files[0] != "-";
        let width = if sysv { 1 } else { 5 };

        for file in &files {
            let reader = match open(file) {
                Ok(f) => f,
                Err(error) => {
                    show!(error);
                    continue;
                }
            };
            let (blocks, sum) = if sysv {
                sysv_sum(reader)
            } else {
                bsd_sum(reader)
            }?;

            let mut stdout = stdout().lock();
            if print_names {
                writeln!(
                    stdout,
                    "{sum:0width$} {blocks:width$} {}",
                    file.to_string_lossy()
                )?;
            } else {
                writeln!(stdout, "{sum:0width$} {blocks:width$}")?;
            }
        }
    }
    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .override_usage(format_usage(&translate!("sum-usage")))
        .about(translate!("sum-about"))
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILE)
                .action(ArgAction::Append)
                .hide(true)
                .value_hint(clap::ValueHint::FilePath)
                .value_parser(clap::value_parser!(OsString))
        )
        .arg(
            Arg::new(options::BSD_COMPATIBLE)
                .short('r')
                .help(translate!("sum-help-bsd-compatible"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::SYSTEM_V_COMPATIBLE)
                .short('s')
                .long(options::SYSTEM_V_COMPATIBLE)
                .help(translate!("sum-help-sysv-compatible"))
                .action(ArgAction::SetTrue)
        );
    
    stardust_output::add_json_args(cmd)
}
