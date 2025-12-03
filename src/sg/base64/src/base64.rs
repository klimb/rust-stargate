use clap::Command;
use sg_base32::base_common;
use sgcore::translate;
use sgcore::{encoding::Format, error::UResult};

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let format = Format::Base64;
    let (about, usage) = get_info();
    let config = base_common::parse_base_cmd_args(args, about, usage)?;
    let mut input = base_common::get_input(&config)?;
    base_common::handle_input(&mut input, format, config)
}

pub fn uu_app() -> Command {
    let (about, usage) = get_info();
    base_common::base_app(about, usage)
}

fn get_info() -> (&'static str, &'static str) {
    let about: &'static str = Box::leak(translate!("base64-about").into_boxed_str());
    let usage: &'static str = Box::leak(translate!("base64-usage").into_boxed_str());
    (about, usage)
}
