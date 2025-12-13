

use clap::{Arg, ArgAction, Command};
use platform_info::*;
use serde::Serialize;
use sgcore::stardust_output::{self, StardustOutputOptions};
use sgcore::translate;
use sgcore::{
    error::{SGResult, SGSimpleError},
    format_usage,
};

pub mod options {
    pub static ALL: &str = "all";
    pub static KERNEL_NAME: &str = "kernel-name";
    pub static NODENAME: &str = "nodename";
    pub static KERNEL_VERSION: &str = "kernel-version";
    pub static KERNEL_RELEASE: &str = "kernel-release";
    pub static MACHINE: &str = "machine";
    pub static PROCESSOR: &str = "processor";
    pub static HARDWARE_PLATFORM: &str = "hardware-platform";
    pub static OS: &str = "operating-system";
}

#[derive(Serialize)]
pub struct UNameOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_platform: Option<String>,
}

impl UNameOutput {
    fn display(&self) -> String {
        [
            self.kernel_name.as_ref(),
            self.nodename.as_ref(),
            self.kernel_release.as_ref(),
            self.kernel_version.as_ref(),
            self.machine.as_ref(),
            self.processor.as_ref(),
            self.hardware_platform.as_ref(),
            self.os.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(|name| name.as_str())
        .collect::<Vec<_>>()
        .join(" ")
    }

    pub fn new(opts: &Options) -> SGResult<Self> {
        let uname = PlatformInfo::new()
            .map_err(|_e| SGSimpleError::new(1, translate!("uname-error-cannot-get-system-name")))?;
        let none = !(opts.all
            || opts.kernel_name
            || opts.nodename
            || opts.kernel_release
            || opts.kernel_version
            || opts.machine
            || opts.os
            || opts.processor
            || opts.hardware_platform);

        let kernel_name = (opts.kernel_name || opts.all || none)
            .then(|| uname.sysname().to_string_lossy().to_string());

        let nodename =
            (opts.nodename || opts.all).then(|| uname.nodename().to_string_lossy().to_string());

        let kernel_release = (opts.kernel_release || opts.all)
            .then(|| uname.release().to_string_lossy().to_string());

        let kernel_version = (opts.kernel_version || opts.all)
            .then(|| uname.version().to_string_lossy().to_string());

        let machine =
            (opts.machine || opts.all).then(|| uname.machine().to_string_lossy().to_string());

        let os = (opts.os || opts.all).then(|| uname.osname().to_string_lossy().to_string());

        let processor = opts.processor.then(|| translate!("uname-unknown"));

        let hardware_platform = opts.hardware_platform.then(|| translate!("uname-unknown"));

        Ok(Self {
            kernel_name,
            nodename,
            kernel_release,
            kernel_version,
            machine,
            os,
            processor,
            hardware_platform,
        })
    }
}

pub struct Options {
    pub all: bool,
    pub kernel_name: bool,
    pub nodename: bool,
    pub kernel_version: bool,
    pub kernel_release: bool,
    pub machine: bool,
    pub processor: bool,
    pub hardware_platform: bool,
    pub os: bool,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    let json_output_options = StardustOutputOptions::from_matches(&matches);

    let options = Options {
        all: matches.get_flag(options::ALL),
        kernel_name: matches.get_flag(options::KERNEL_NAME),
        nodename: matches.get_flag(options::NODENAME),
        kernel_release: matches.get_flag(options::KERNEL_RELEASE),
        kernel_version: matches.get_flag(options::KERNEL_VERSION),
        machine: matches.get_flag(options::MACHINE),
        processor: matches.get_flag(options::PROCESSOR),
        hardware_platform: matches.get_flag(options::HARDWARE_PLATFORM),
        os: matches.get_flag(options::OS),
    };
    let output = UNameOutput::new(&options)?;

    if json_output_options.stardust_output {
        let json_value = serde_json::to_value(&output)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        stardust_output::output(json_output_options, json_value, || Ok(()))?;
    } else {
        println!("{}", output.display());
    }
    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("uname-about"))
        .override_usage(format_usage(&translate!("uname-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(options::ALL)
                .short('a')
                .long(options::ALL)
                .help(translate!("uname-help-all"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::KERNEL_NAME)
                .short('s')
                .long(options::KERNEL_NAME)
                .alias("sysname")
                .help(translate!("uname-help-kernel-name"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::NODENAME)
                .short('n')
                .long(options::NODENAME)
                .help(translate!("uname-help-nodename"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::KERNEL_RELEASE)
                .short('r')
                .long(options::KERNEL_RELEASE)
                .alias("release")
                .help(translate!("uname-help-kernel-release"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::KERNEL_VERSION)
                .short('v')
                .long(options::KERNEL_VERSION)
                .help(translate!("uname-help-kernel-version"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::MACHINE)
                .short('m')
                .long(options::MACHINE)
                .help(translate!("uname-help-machine"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::OS)
                .short('o')
                .long(options::OS)
                .help(translate!("uname-help-os"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(options::PROCESSOR)
                .short('p')
                .long(options::PROCESSOR)
                .help(translate!("uname-help-processor"))
                .action(ArgAction::SetTrue)
                .hide(true)
        )
        .arg(
            Arg::new(options::HARDWARE_PLATFORM)
                .short('i')
                .long(options::HARDWARE_PLATFORM)
                .help(translate!("uname-help-hardware-platform"))
                .action(ArgAction::SetTrue)
                .hide(true)
        );
    stardust_output::add_json_args(cmd)
}

