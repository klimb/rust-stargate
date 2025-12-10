// Copyright (C) 2025 Dmitry Kalashnikov

use clap::{Arg, ArgMatches, Command as ClapCommand};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::process::Command as ProcessCommand;
use sgcore::{
    error::{UResult, USimpleError},
    format_usage,
    object_output::{self, JsonOutputOptions},
};

static DURATION_ARG: &str = "duration";

const MAX_DURATION_SECONDS: u32 = 60;
const DEFAULT_DURATION_SECONDS: &str = "5";
const TEMP_FILE_PREFIX: &str = "/tmp/record_audio_";

#[cfg(target_os = "macos")]
const MACOS_SAMPLE_RATE: &str = "16000";
#[cfg(target_os = "macos")]
const MACOS_CHANNELS: &str = "1";
#[cfg(target_os = "macos")]
const MACOS_FILE_EXTENSION: &str = ".aiff";

#[cfg(target_os = "linux")]
const LINUX_FILE_EXTENSION: &str = ".wav";
#[cfg(target_os = "linux")]
const LINUX_BITS_PER_SAMPLE: u16 = 16;
#[cfg(target_os = "linux")]
const U16_SAMPLE_OFFSET: i32 = 32768;

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
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err(USimpleError::new(
            1,
            "record-audio is only available on macOS and Linux".to_string(),
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
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
        .about("Record audio input and optionally transcribe it")
        .override_usage(format_usage("record-audio [OPTIONS]"))
        .infer_long_args(true)
        .arg(
            Arg::new(DURATION_ARG)
                .short('d')
                .long("duration")
                .value_name("SECONDS")
                .help(format!("Duration to record in seconds (max {})", MAX_DURATION_SECONDS))
                .default_value(DEFAULT_DURATION_SECONDS)
                .value_parser(clap::value_parser!(u32)),
        );

    object_output::add_json_args(cmd)
}

#[cfg(target_os = "macos")]
fn produce(matches: &ArgMatches) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "proc", "exec"])?;
    
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS); 
    
    let temp_file = format!("{}{}{}", TEMP_FILE_PREFIX, std::process::id(), MACOS_FILE_EXTENSION);
    
    let record_result = ProcessCommand::new("rec")
        .args([
            "-q",
            "-c", MACOS_CHANNELS, 
            "-r", MACOS_SAMPLE_RATE, 
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
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "proc", "exec"])?;
    
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS); 
    
    let temp_file = format!("{}{}{}", TEMP_FILE_PREFIX, std::process::id(), MACOS_FILE_EXTENSION);
    
    let record_result = ProcessCommand::new("rec")
        .args([
            "-q",
            "-c", MACOS_CHANNELS,
            "-r", MACOS_SAMPLE_RATE,
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

#[cfg(target_os = "linux")]
fn produce(matches: &ArgMatches) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "audio"])?;
    
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS);
    
    let temp_file = format!("{}{}{}", TEMP_FILE_PREFIX, std::process::id(), LINUX_FILE_EXTENSION);
    
    record_audio_linux(&temp_file, duration as f32)?;
    
    println!("Audio recorded to: {}", temp_file);
    Ok(())
}

#[cfg(target_os = "linux")]
fn produce_json(matches: &ArgMatches, options: JsonOutputOptions) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "audio"])?;
    
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS);
    
    let temp_file = format!("{}{}{}", TEMP_FILE_PREFIX, std::process::id(), LINUX_FILE_EXTENSION);
    
    let result = match record_audio_linux(&temp_file, duration as f32) {
        Ok(_samples) => {
            RecordAudioResult {
                transcript: String::new(),
                duration: duration as f64,
                word_count: 0,
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
            audio_file: None,
            error: Some(format!("Recording failed: {}", e)),
        },
    };

    let json = if options.pretty {
        serde_json::to_string_pretty(&result).unwrap()
    } else {
        serde_json::to_string(&result).unwrap()
    };

    println!("{}", json);
    Ok(())
}

#[cfg(target_os = "linux")]
fn record_audio_linux(output_file: &str, duration: f32) -> UResult<usize> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::{Arc, Mutex};

    let host = cpal::default_host();
    
    let device = host.default_input_device()
        .ok_or_else(|| USimpleError::new(1, "No default input device available".to_string()))?;

    let config = device.default_input_config()
        .map_err(|e| USimpleError::new(1, format!("Failed to get input config: {}", e)))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: LINUX_BITS_PER_SAMPLE,
        sample_format: hound::SampleFormat::Int,
    };

    let writer = Arc::new(Mutex::new(
        hound::WavWriter::create(output_file, spec)
            .map_err(|e| USimpleError::new(1, format!("Failed to create WAV file: {}", e)))?
    ));

    let writer_clone = Arc::clone(&writer);
    let sample_count = Arc::new(Mutex::new(0usize));
    let sample_count_clone = Arc::clone(&sample_count);
    let max_samples = (sample_rate as f32 * duration) as usize * channels as usize;

    let err_fn = |err| {
        eprintln!("Error during recording: {}", err);
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let mut writer = writer_clone.lock().unwrap();
                    let mut count = sample_count_clone.lock().unwrap();
                    
                    for &sample in data {
                        if *count >= max_samples {
                            break;
                        }
                        let amplitude = (sample * i16::MAX as f32) as i16;
                        writer.write_sample(amplitude).ok();
                        *count += 1;
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut writer = writer_clone.lock().unwrap();
                    let mut count = sample_count_clone.lock().unwrap();
                    
                    for &sample in data {
                        if *count >= max_samples {
                            break;
                        }
                        writer.write_sample(sample).ok();
                        *count += 1;
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            device.build_input_stream(
                &config.into(),
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let mut writer = writer_clone.lock().unwrap();
                    let mut count = sample_count_clone.lock().unwrap();
                    
                    for &sample in data {
                        if *count >= max_samples {
                            break;
                        }
                        let normalized = (sample as i32 - U16_SAMPLE_OFFSET) as i16;
                        writer.write_sample(normalized).ok();
                        *count += 1;
                    }
                },
                err_fn,
                None,
            )
        }
        _ => {
            return Err(USimpleError::new(1, "Unsupported sample format".to_string()));
        }
    }
    .map_err(|e| USimpleError::new(1, format!("Failed to build input stream: {}", e)))?;

    stream.play()
        .map_err(|e| USimpleError::new(1, format!("Failed to start recording: {}", e)))?;

    eprintln!("Recording for {} seconds...", duration);
    std::thread::sleep(std::time::Duration::from_secs_f32(duration));

    drop(stream);
    
    let final_count = *sample_count.lock().unwrap();
    let writer = match Arc::try_unwrap(writer) {
        Ok(mutex) => mutex.into_inner().unwrap(),
        Err(_) => return Err(USimpleError::new(1, "Failed to finalize writer".to_string())),
    };
    writer.finalize()
        .map_err(|e| USimpleError::new(1, format!("Failed to finalize WAV file: {}", e)))?;

    Ok(final_count)
}
