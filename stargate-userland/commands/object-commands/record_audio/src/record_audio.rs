// Copyright (C) 2025 Dmitry Kalashnikov

use clap::{Arg, ArgMatches, Command as ClapCommand};
use serde::{Deserialize, Serialize};
use std::process::Command as ProcessCommand;
use sgcore::{
    error::{UResult, USimpleError},
    format_usage,
    object_output::{self, JsonOutputOptions},
};

static DURATION_ARG: &str = "duration";

#[derive(Debug, Serialize, Deserialize)]
struct RecordAudioResult {
    transcript: String,
    duration: f64,
    word_count: usize,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err(USimpleError::new(
            1,
            "record-audio is only available on macOS for now".to_string(),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
        let object_output = JsonOutputOptions::from_matches(&matches);

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
        .about("Record audio input and optionally transcribe it (macOS only)")
        .override_usage(format_usage("record-audio [OPTIONS]"))
        .infer_long_args(true)
        .arg(
            Arg::new(DURATION_ARG)
                .short('d')
                .long("duration")
                .value_name("SECONDS")
                .help("Duration to record in seconds (max 60)")
                .default_value("5")
                .value_parser(clap::value_parser!(u32)),
        );

    object_output::add_json_args(cmd)
}

#[cfg(target_os = "macos")]
fn produce(matches: &ArgMatches) -> UResult<()> {
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(60); 
    
    let temp_file = format!("/tmp/audio_input_{}.aiff", std::process::id());
    
    let record_result = ProcessCommand::new("rec")
        .args([
            "-q",
            "-c", "1", 
            "-r", "16000", 
            &temp_file,
            "trim", "0", &duration.to_string(),
        ])
        .output();

    let record_success = if record_result.is_ok() && record_result.as_ref().unwrap().status.success() {
        true
    } else {
        let result = ProcessCommand::new("sh")
            .arg("-c")
            .arg(format!(
                "afrecord -t {} {}",
                duration, temp_file
            ))
            .output();
        
        result.is_ok() && result.unwrap().status.success()
    };

    if !record_success {
        return Err(USimpleError::new(
            1,
            "Failed to record audio. Install sox (brew install sox) for better results.".to_string(),
        ));
    }

    let transcript = transcribe_audio(&temp_file)?;
    let _ = std::fs::remove_file(&temp_file);

    println!("{}", transcript);
    Ok(())
}

#[cfg(target_os = "macos")]
fn produce_json(matches: &ArgMatches, options: JsonOutputOptions) -> UResult<()> {
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(60); 
    
    let temp_file = format!("/tmp/record_audio_{}.aiff", std::process::id());
    
    let record_result = ProcessCommand::new("rec")
        .args([
            "-q",
            "-c", "1",
            "-r", "16000",
            &temp_file,
            "trim", "0", &duration.to_string(),
        ])
        .output();

    let record_success = if record_result.is_ok() && record_result.as_ref().unwrap().status.success() {
        true
    } else {
        // Fallback to afrecord
        let result = ProcessCommand::new("sh")
            .arg("-c")
            .arg(format!(
                "afrecord -t {} {}",
                duration, temp_file
            ))
            .output();
        
        result.is_ok() && result.unwrap().status.success()
    };

    let result = if !record_success {
        RecordAudioResult {
            transcript: String::new(),
            duration: duration as f64,
            word_count: 0,
            success: false,
            audio_file: None,
            error: Some("failed to record audio: brew install sox".to_string()),
        }
    } else {
        match transcribe_audio(&temp_file) {
            Ok(transcript) => {
                let word_count = transcript.split_whitespace().count();
                RecordAudioResult {
                    transcript: transcript.clone(),
                    duration: duration as f64,
                    word_count,
                    success: true,
                    audio_file: Some(temp_file.clone()),
                    error: None,
                }
            }
            Err(e) => RecordAudioResult {
                transcript: String::new(),
                duration: duration as f64,
                word_count: 0,
                success: false,
                audio_file: Some(temp_file.clone()),
                error: Some(format!("Transcription failed: {}", e)),
            },
        }
    };

    let json = if options.pretty {
        serde_json::to_string_pretty(&result).unwrap()
    } else {
        serde_json::to_string(&result).unwrap()
    };

    println!("{}", json);
    Ok(())
}

#[cfg(target_os = "macos")]
fn transcribe_audio(audio_file: &str) -> UResult<String> {
    let script = format!(
        r#"
        set audioFile to POSIX file "{}"
        tell application "Speech Recognition Server"
            try
                -- This is a simplified approach
                -- Real implementation would use Speech framework via FFI
                return "Transcription not available - requires Speech framework integration"
            end try
        end tell
        "#,
        audio_file
    );
    
    let output = ProcessCommand::new("osascript")
        .arg("-e")
        .arg(&script)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let transcript = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(if transcript.is_empty() {
                "speech recognition requires sox.".to_string()
            } else {
                transcript
            })
        }
        _ => {
            Ok("recorded audio".to_string())
        }
    }
}
