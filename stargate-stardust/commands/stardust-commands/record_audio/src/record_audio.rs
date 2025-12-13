// Copyright (C) 2025 Dmitry Kalashnikov

use clap::{Arg, ArgMatches, Command as ClapCommand};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::process::Command as ProcessCommand;
use sgcore::{
    error::{UResult, USimpleError},
    format_usage,
    stardust_output::{self, StardustOutputOptions},
};

static DURATION_ARG: &str = "duration";
#[cfg(feature = "transcription")]
static NO_TRANSCRIBE_ARG: &str = "no-transcription";
#[cfg(feature = "transcription")]
static MODEL_PATH_ARG: &str = "model-path";

const MAX_DURATION_SECONDS: u32 = 60;
const DEFAULT_DURATION_SECONDS: &str = "5";

fn get_record_dir() -> String {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    format!("{}/.stargate/record-audio", home)
}

fn ensure_record_dir() -> UResult<String> {
    let dir = get_record_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| USimpleError::new(1, format!("failed to create directory: {}", e)))?;
    Ok(dir)
}

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
const LINUX_SAMPLE_RATE: u32 = 16000;
#[cfg(target_os = "linux")]
const U16_SAMPLE_OFFSET: i32 = 32768;

#[cfg(feature = "transcription")]
fn get_default_model_path() -> String {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    format!("{}/.stargate/models/vosk-model-small-en-us-0.15", home)
}

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
        let object_output = StardustOutputOptions::from_matches(&matches);

        if object_output.stardust_output {
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

    #[cfg(feature = "transcription")]
    let cmd = cmd
        .arg(
            Arg::new(NO_TRANSCRIBE_ARG)
                .long("no-transcription")
                .help("Disable automatic transcription")
                .action(clap::ArgAction::SetTrue)
        )
        .arg(
            Arg::new(MODEL_PATH_ARG)
                .short('m')
                .long("model-path")
                .value_name("PATH")
                .help("Path to Vosk model directory (defaults to ~/.stargate/models/vosk-model-small-en-us-0.15)")
        );

    stardust_output::add_json_args(cmd)
}

#[cfg(target_os = "macos")]
fn produce(matches: &ArgMatches) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "proc", "exec"])?;
    
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS); 
    
    let dir = ensure_record_dir()?;
    let temp_file = format!("{}/record_audio_{}{}", dir, std::process::id(), MACOS_FILE_EXTENSION);
    
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

    #[cfg(feature = "transcription")]
    {
        if !matches.get_flag(NO_TRANSCRIBE_ARG) {
            let model_path = matches.get_one::<String>(MODEL_PATH_ARG)
                .map(|s| s.as_str())
                .unwrap_or_else(|| {
                    static DEFAULT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                    DEFAULT.get_or_init(get_default_model_path)
                });
            
            let transcript = transcribe_audio_vosk(&temp_file, model_path)?;
            let _ = std::fs::remove_file(&temp_file);
            println!("{}", transcript);
            return Ok(());
        }
    }
    
    let _ = std::fs::remove_file(&temp_file);
    println!("");
    Ok(())
}

#[cfg(target_os = "macos")]
fn produce_json(matches: &ArgMatches, options: StardustOutputOptions) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "proc", "exec"])?;
    
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS); 
    
    let dir = ensure_record_dir()?;
    let temp_file = format!("{}/record_audio_{}{}", dir, std::process::id(), MACOS_FILE_EXTENSION);
    
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
        #[cfg(feature = "transcription")]
        let (transcript, word_count) = {
            if !matches.get_flag(NO_TRANSCRIBE_ARG) {
                let model_path = matches.get_one::<String>(MODEL_PATH_ARG)
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| {
                        static DEFAULT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                        DEFAULT.get_or_init(get_default_model_path)
                    });
                match transcribe_audio_vosk(&temp_file, model_path) {
                    Ok(transcript) => {
                        let word_count = transcript.split_whitespace().count();
                        (transcript, word_count)
                    }
                    Err(_) => (String::new(), 0),
                }
            } else {
                (String::new(), 0)
            }
        };

        #[cfg(not(feature = "transcription"))]
        let (transcript, word_count) = (String::new(), 0);

        RecordAudioResult {
            transcript,
            duration: duration as f64,
            word_count,
            success: true,
            audio_file: Some(temp_file.clone()),
            error: None,
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

#[cfg(all(target_os = "macos", feature = "transcription"))]
fn transcribe_audio_vosk(audio_file: &str, model_path: &str) -> UResult<String> {
    vosk::set_log_level(vosk::LogLevel::Error);
    
    let wav_file = format!("{}.wav", audio_file);
    
    let convert_result = ProcessCommand::new("sox")
        .args([
            audio_file,
            "-r", "16000",
            "-c", "1",
            "-b", "16",
            &wav_file,
        ])
        .output();
    
    if !convert_result.is_ok() || !convert_result.as_ref().unwrap().status.success() {
        return Err(USimpleError::new(1, "failed to convert audio to wav format".to_string()));
    }
    
    let model = vosk::Model::new(model_path)
        .ok_or_else(|| USimpleError::new(1, "failed to load vosk model".to_string()))?;
    
    let mut recognizer = vosk::Recognizer::new(&model, 16000.0)
        .ok_or_else(|| USimpleError::new(1, "failed to create recognizer".to_string()))?;
    
    let mut reader = hound::WavReader::open(&wav_file)
        .map_err(|e| USimpleError::new(1, format!("failed to open wav file: {}", e)))?;
    
    let spec = reader.spec();
    
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        let _ = std::fs::remove_file(&wav_file);
        return Err(USimpleError::new(1, "audio must be 16-bit pcm format".to_string()));
    }
    
    let samples: Vec<i16> = reader.samples::<i16>()
        .filter_map(|s| s.ok())
        .collect();
    
    for chunk in samples.chunks(4000) {
        recognizer.accept_waveform(chunk);
    }
    
    let result_json = recognizer.final_result();
    
    let transcript = result_json.single()
        .map(|s| s.text.to_string())
        .unwrap_or_default();
    
    let _ = std::fs::remove_file(&wav_file);
    
    Ok(transcript)
}

#[cfg(target_os = "linux")]
fn produce(matches: &ArgMatches) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "audio"])?;
    
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS);
    
    let dir = ensure_record_dir()?;
    let temp_file = format!("{}/record_audio_{}{}", dir, std::process::id(), LINUX_FILE_EXTENSION);
    
    record_audio_linux(&temp_file, duration as f32)?;
    
    #[cfg(feature = "transcription")]
    {
        if !matches.get_flag(NO_TRANSCRIBE_ARG) {
            let model_path = matches.get_one::<String>(MODEL_PATH_ARG)
                .map(|s| s.as_str())
                .unwrap_or_else(|| {
                    static DEFAULT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                    DEFAULT.get_or_init(get_default_model_path)
                });
            
            match transcribe_audio_linux(&temp_file, model_path) {
                Ok(transcript) => {
                    println!("{}", transcript);
                }
                Err(_) => {}
            }
        }
    }
    
    #[cfg(not(feature = "transcription"))]
    {
        let _ = temp_file;
    }
    
    Ok(())
}

#[cfg(target_os = "linux")]
fn produce_json(matches: &ArgMatches, options: StardustOutputOptions) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "audio"])?;
    
    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS);
    
    let dir = ensure_record_dir()?;
    let temp_file = format!("{}/record_audio_{}{}", dir, std::process::id(), LINUX_FILE_EXTENSION);
    
    let result = match record_audio_linux(&temp_file, duration as f32) {
        Ok(_samples) => {
            #[cfg(feature = "transcription")]
            let (transcript, word_count) = {
                if !matches.get_flag(NO_TRANSCRIBE_ARG) {
                    let model_path = matches.get_one::<String>(MODEL_PATH_ARG)
                        .map(|s| s.as_str())
                        .unwrap_or_else(|| {
                            static DEFAULT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                            DEFAULT.get_or_init(get_default_model_path)
                        });
                    
                    match transcribe_audio_linux(&temp_file, model_path) {
                        Ok(text) => {
                            let count = text.split_whitespace().count();
                            (text, count)
                        }
                        Err(_) => (String::new(), 0)
                    }
                } else {
                    (String::new(), 0)
                }
            };
            
            #[cfg(not(feature = "transcription"))]
            let (transcript, word_count) = (String::new(), 0);
            
            RecordAudioResult {
                transcript,
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

    // Use 16000 Hz mono for Vosk compatibility
    let sample_rate = LINUX_SAMPLE_RATE;
    let channels = 1;

    let config = cpal::StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

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

    let err_fn = |_err| {};

    // Try to build stream with i16 format (most common for audio input)
    let stream = device.build_input_stream(
        &config,
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
    .map_err(|e| USimpleError::new(1, format!("Failed to build input stream: {}", e)))?;

    stream.play()
        .map_err(|e| USimpleError::new(1, format!("Failed to start recording: {}", e)))?;

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

#[cfg(all(target_os = "linux", feature = "transcription"))]
fn transcribe_audio_linux(audio_file: &str, model_path: &str) -> UResult<String> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath"])?;
    
    vosk::set_log_level(vosk::LogLevel::Error);
    
    let model = vosk::Model::new(model_path)
        .ok_or_else(|| USimpleError::new(1, "Failed to load Vosk model".to_string()))?;
    
    let mut recognizer = vosk::Recognizer::new(&model, 16000.0)
        .ok_or_else(|| USimpleError::new(1, "Failed to create recognizer".to_string()))?;
    
    let mut reader = hound::WavReader::open(audio_file)
        .map_err(|e| USimpleError::new(1, format!("Failed to open WAV file: {}", e)))?;
    
    let spec = reader.spec();
    
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(USimpleError::new(1, "Audio must be 16-bit PCM format".to_string()));
    }
    
    let samples: Vec<i16> = reader.samples::<i16>()
        .filter_map(|s| s.ok())
        .collect();
    
    for chunk in samples.chunks(4000) {
        recognizer.accept_waveform(chunk);
    }
    
    let result_json = recognizer.final_result();
    
    let transcript = result_json.single()
        .map(|s| s.text.to_string())
        .unwrap_or_default();
    
    Ok(transcript)
}
