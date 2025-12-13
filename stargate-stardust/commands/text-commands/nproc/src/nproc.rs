

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::{env, thread};
use sgcore::display::Quotable;
use sgcore::error::{SGResult, SGSimpleError};
use sgcore::format_usage;
use sgcore::translate;
use sgcore::stardust_output::{self, StardustOutputOptions};

#[cfg(any(target_os = "linux"))]
pub const _SC_NPROCESSORS_CONF: libc::c_int = 83;
#[cfg(target_vendor = "apple")]
pub const _SC_NPROCESSORS_CONF: libc::c_int = libc::_SC_NPROCESSORS_CONF;
#[cfg(target_os = "freebsd")]
pub const _SC_NPROCESSORS_CONF: libc::c_int = 57;

static OPT_ALL: &str = "all";
static OPT_IGNORE: &str = "ignore";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;
    let object_output = StardustOutputOptions::from_matches(&matches);

    let cores = compute_cores(&matches)?;

    if object_output.stardust_output {
        let output = serde_json::json!({
            "nproc": cores,
            "flags": {
                "all": matches.get_flag(OPT_ALL),
                "ignore": matches.get_one::<String>(OPT_IGNORE).map(|s| s.as_str())
            }
        });
        stardust_output::output(object_output, output, || Ok(()))?;
    } else {
        println!("{cores}");
    }

    Ok(())
}

fn compute_cores(matches: &ArgMatches) -> SGResult<usize> {

    let ignore = match matches.get_one::<String>(OPT_IGNORE) {
        Some(numstr) => match numstr.trim().parse::<usize>() {
            Ok(num) => num,
            Err(e) => {
                return Err(SGSimpleError::new(
                    1,
                    translate!("nproc-error-invalid-number", "value" => numstr.quote(), "error" => e)
                ));
            }
        },
        None => 0,
    };

    let limit = match env::var("OMP_THREAD_LIMIT") {
        Ok(threads) => match threads.parse() {
            Ok(0) | Err(_) => usize::MAX,
            Ok(n) => n,
        },
        Err(_) => usize::MAX,
    };

    let mut cores = if matches.get_flag(OPT_ALL) {
        num_cpus_all()
    } else {
        match env::var("OMP_NUM_THREADS") {
            Ok(threads) => {
                match threads.split_terminator(',').next() {
                    None => available_parallelism(),
                    Some(s) => match s.parse() {
                        Ok(0) | Err(_) => available_parallelism(),
                        Ok(n) => n,
                    },
                }
            }
            Err(_) => available_parallelism(),
        }
    };

    cores = std::cmp::min(limit, cores);
    if cores <= ignore {
        cores = 1;
    } else {
        cores -= ignore;
    }

    Ok(cores)
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("nproc-about"))
        .override_usage(format_usage(&translate!("nproc-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(OPT_ALL)
                .long(OPT_ALL)
                .help(translate!("nproc-help-all"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(OPT_IGNORE)
                .long(OPT_IGNORE)
                .value_name("N")
                .help(translate!("nproc-help-ignore"))
        );

    stardust_output::add_json_args(cmd)
}

#[cfg(any(
    target_os = "linux",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "netbsd"
))]
fn num_cpus_all() -> usize {
    let nprocs = unsafe { libc::sysconf(_SC_NPROCESSORS_CONF) };
    if nprocs == 1 {
        available_parallelism()
    } else if nprocs > 0 {
        nprocs as usize
    } else {
        1
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "netbsd"
)))]
fn num_cpus_all() -> usize {
    available_parallelism()
}

/// In some cases, [`thread::available_parallelism`]() may return an Err
/// In this case, we will return 1 (like GNU)
fn available_parallelism() -> usize {
    match thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 1,
    }
}

