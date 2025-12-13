


use std::process::Command;

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use sgcore::{
    error::{SGResult, SGSimpleError},
    format_usage,
    stardust_output::{self, StardustOutputOptions},
};

static TEXT_ARG: &str = "text";
static VOICE_FLAG: &str = "voice";
static RATE_FLAG: &str = "rate";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err(SGSimpleError::new(
            1,
            "say-text is only available on macOS".to_string(),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
        let object_output = StardustOutputOptions::from_matches(&matches);

        if object_output.object_output {
            produce_json(&matches, object_output)
        } else {
            produce(&matches)
        }
    }
}

pub fn sg_app() -> ClapCommand {
    let cmd = ClapCommand::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about("Speak text using macOS text-to-speech (macOS only)")
        .override_usage(format_usage("say-text [OPTIONS] <TEXT>"))
        .infer_long_args(true)
        .arg(
            Arg::new(TEXT_ARG)
                .help("The text to speak")
                .required(true)
                .num_args(1..)
                .value_name("TEXT"),
        )
        .arg(
            Arg::new(VOICE_FLAG)
                .short('v')
                .long("voice")
                .help("Voice to use (e.g., 'Alex', 'Samantha')")
                .value_name("VOICE")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(RATE_FLAG)
                .short('r')
                .long("rate")
                .help("Speech rate in words per minute (default: 175)")
                .value_name("RATE")
                .action(ArgAction::Set),
        );

    stardust_output::add_json_args(cmd)
}

#[cfg(target_os = "macos")]
fn produce(matches: &ArgMatches) -> SGResult<()> {
    let text_parts: Vec<&String> = matches
        .get_many::<String>(TEXT_ARG)
        .unwrap()
        .collect();
    let text = text_parts.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");

    let mut cmd = Command::new("say");
    cmd.arg(&text);

    if let Some(voice) = matches.get_one::<String>(VOICE_FLAG) {
        cmd.arg("-v").arg(voice);
    }

    if let Some(rate) = matches.get_one::<String>(RATE_FLAG) {
        cmd.arg("-r").arg(rate);
    }

    let status = cmd
        .status()
        .map_err(|e| SGSimpleError::new(1, format!("Failed to execute 'say' command: {}", e)))?;

    if !status.success() {
        return Err(SGSimpleError::new(
            status.code().unwrap_or(1),
            "Speech synthesis failed".to_string(),
        ));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn produce_json(matches: &ArgMatches, object_output: StardustOutputOptions) -> SGResult<()> {
    let text_parts: Vec<&String> = matches
        .get_many::<String>(TEXT_ARG)
        .unwrap()
        .collect();
    let text = text_parts.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");

    let voice = matches
        .get_one::<String>(VOICE_FLAG)
        .map(|s| s.to_string());
    let rate = matches
        .get_one::<String>(RATE_FLAG)
        .map(|s| s.to_string());

    let mut cmd = Command::new("say");
    cmd.arg(&text);

    if let Some(ref v) = voice {
        cmd.arg("-v").arg(v);
    }

    if let Some(ref r) = rate {
        cmd.arg("-r").arg(r);
    }

    let status = cmd
        .status()
        .map_err(|e| SGSimpleError::new(1, format!("Failed to execute 'say' command: {}", e)))?;

    let success = status.success();
    let exit_code = status.code().unwrap_or(1);

    let output = serde_json::json!({
        "text": text,
        "voice": voice,
        "rate": rate,
        "success": success,
        "exit_code": if success { 0 } else { exit_code }
    });

    stardust_output::output(object_output, output, || Ok(()))?;

    if !success {
        return Err(SGSimpleError::new(
            exit_code,
            "Speech synthesis failed".to_string(),
        ));
    }

    Ok(())
}

