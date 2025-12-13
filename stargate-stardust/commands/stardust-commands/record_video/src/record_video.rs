

use clap::{Arg, ArgMatches, Command as ClapCommand};
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command as ProcessCommand;
use sgcore::{
    error::{SGResult, SGSimpleError},
    format_usage,
    stardust_output::{self, StardustOutputOptions},
};

static DURATION_ARG: &str = "duration";
static OUTPUT_ARG: &str = "output";
static NO_TRANSCRIBE_ARG: &str = "no-transcription";
#[cfg(feature = "transcription")]
static MODEL_PATH_ARG: &str = "model-path";

const MAX_DURATION_SECONDS: u32 = 60;
const DEFAULT_DURATION_SECONDS: &str = "5";

fn get_record_dir() -> String {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    format!("{}/.stargate/record-video", home)
}

fn ensure_record_dir() -> SGResult<String> {
    let dir = get_record_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| SGSimpleError::new(1, format!("failed to create directory: {}", e)))?;
    Ok(dir)
}

const VIDEO_WIDTH: &str = "1280x720";
const VIDEO_FRAMERATE: &str = "30";
const AUDIO_CODEC: &str = "pcm_s16le";
const AUDIO_SAMPLE_RATE: &str = "16000";
const AUDIO_CHANNELS: &str = "1";
const AUDIO_FILE_EXTENSION: &str = ".wav";

#[cfg(target_os = "macos")]
const MACOS_FILE_EXTENSION: &str = ".mov";
#[cfg(target_os = "macos")]
const MACOS_VIDEO_INPUT: &str = "avfoundation";
#[cfg(target_os = "macos")]
const MACOS_DEVICE_INPUT: &str = "0:0";

#[cfg(target_os = "linux")]
const LINUX_DEVICE_INPUT: &str = "0";

#[cfg(target_os = "linux")]
const LINUX_FILE_EXTENSION: &str = ".mp4";
#[cfg(target_os = "linux")]
const LINUX_VIDEO_INPUT: &str = "v4l2";
#[cfg(target_os = "linux")]
const LINUX_CAMERA_DEVICE: &str = "/dev/video0";

#[cfg(feature = "transcription")]
fn get_default_model_path() -> String {
    let home = std::env::var("HOME").expect("HOME environment variable not set");
    format!("{}/.stargate/models/vosk-model-small-en-us-0.15", home)
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordVideoResult {
    transcript: String,
    duration: f64,
    word_count: usize,
    success: bool,
    video_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err(SGSimpleError::new(
            1,
            "record-video is only available on macos and linux".to_string(),
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
        .about("record video from the default camera")
        .override_usage(format_usage("record-video [options]"))
        .infer_long_args(true)
        .arg(
            Arg::new(DURATION_ARG)
                .short('d')
                .long("duration")
                .value_name("seconds")
                .help(format!("duration to record in seconds (max {})", MAX_DURATION_SECONDS))
                .default_value(DEFAULT_DURATION_SECONDS)
                .value_parser(clap::value_parser!(u32)),
        )
        .arg(
            Arg::new(OUTPUT_ARG)
                .short('o')
                .long("output")
                .value_name("file")
                .help("output file path (if not specified, uses temporary file)"),
        )
        .arg(
            Arg::new(NO_TRANSCRIBE_ARG)
                .long("no-transcription")
                .help("disable automatic transcription of audio")
                .action(clap::ArgAction::SetTrue)
        );

    #[cfg(feature = "transcription")]
    let cmd = cmd
        .arg(
            Arg::new(MODEL_PATH_ARG)
                .short('m')
                .long("model-path")
                .value_name("path")
                .help("path to vosk model directory (defaults to ~/.stargate/models/vosk-model-small-en-us-0.15)")
        );

    stardust_output::add_json_args(cmd)
}

#[cfg(target_os = "macos")]
fn produce(matches: &ArgMatches) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "proc", "exec"])?;

    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS);

    let output_file = matches
        .get_one::<String>(OUTPUT_ARG)
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let dir = ensure_record_dir().expect("failed to create record directory");
            format!("{}/record_video_{}{}", dir, std::process::id(), MACOS_FILE_EXTENSION)
        });

    let record_result = ProcessCommand::new("ffmpeg")
        .args([
            "-f", MACOS_VIDEO_INPUT,
            "-video_size", VIDEO_WIDTH,
            "-framerate", VIDEO_FRAMERATE,
            "-i", MACOS_DEVICE_INPUT,
            "-t", &duration.to_string(),
            "-y",
            &output_file,
        ])
        .output();

    if !record_result.is_ok() || !record_result.as_ref().unwrap().status.success() {
        return Err(SGSimpleError::new(
            1,
            "failed to record video. install ffmpeg: brew install ffmpeg".to_string(),
        ));
    }

    #[cfg(feature = "transcription")]
    let transcript = if matches.get_flag(NO_TRANSCRIBE_ARG) {
        String::new()
    } else {
        let model_path = matches
            .get_one::<String>(MODEL_PATH_ARG)
            .map(|s| s.to_string())
            .unwrap_or_else(get_default_model_path);
        extract_and_transcribe(&output_file, &model_path)?
    };

    #[cfg(not(feature = "transcription"))]
    let transcript = String::new();

    println!("{}", transcript);
    Ok(())
}

#[cfg(target_os = "macos")]
fn produce_json(matches: &ArgMatches, options: StardustOutputOptions) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "proc", "exec"])?;

    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS);

    let output_file = matches
        .get_one::<String>(OUTPUT_ARG)
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let dir = ensure_record_dir().expect("failed to create record directory");
            format!("{}/record_video_{}{}", dir, std::process::id(), MACOS_FILE_EXTENSION)
        });

    let record_result = ProcessCommand::new("ffmpeg")
        .args([
            "-f", MACOS_VIDEO_INPUT,
            "-video_size", VIDEO_WIDTH,
            "-framerate", VIDEO_FRAMERATE,
            "-i", MACOS_DEVICE_INPUT,
            "-t", &duration.to_string(),
            "-y",
            &output_file,
        ])
        .output();

    let result = if !record_result.is_ok() || !record_result.as_ref().unwrap().status.success() {
        RecordVideoResult {
            transcript: String::new(),
            duration: duration as f64,
            word_count: 0,
            success: false,
            video_file: output_file,
            error: Some("failed to record video. install ffmpeg: brew install ffmpeg".to_string()),
        }
    } else if matches.get_flag(NO_TRANSCRIBE_ARG) {
        RecordVideoResult {
            transcript: String::new(),
            duration: duration as f64,
            word_count: 0,
            success: true,
            video_file: output_file,
            error: None,
        }
    } else {
        #[cfg(feature = "transcription")]
        let (transcript, word_count) = if matches.get_flag(NO_TRANSCRIBE_ARG) {
            (String::new(), 0)
        } else {
            let model_path = matches
                .get_one::<String>(MODEL_PATH_ARG)
                .map(|s| s.to_string())
                .unwrap_or_else(get_default_model_path);
            match extract_and_transcribe(&output_file, &model_path) {
                Ok(transcript) => {
                    let word_count = transcript.split_whitespace().count();
                    (transcript, word_count)
                }
                Err(_) => (String::new(), 0),
            }
        };

        #[cfg(not(feature = "transcription"))]
        let (transcript, word_count) = (String::new(), 0);

        RecordVideoResult {
            transcript,
            duration: duration as f64,
            word_count,
            success: true,
            video_file: output_file,
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

#[cfg(target_os = "linux")]
fn produce(matches: &ArgMatches) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "proc", "exec"])?;

    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS);

    let output_file = matches
        .get_one::<String>(OUTPUT_ARG)
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let dir = ensure_record_dir().expect("failed to create record directory");
            format!("{}/record_video_{}{}", dir, std::process::id(), LINUX_FILE_EXTENSION)
        });

    let record_result = ProcessCommand::new("ffmpeg")
        .args([
            "-f", LINUX_VIDEO_INPUT,
            "-video_size", VIDEO_WIDTH,
            "-framerate", VIDEO_FRAMERATE,
            "-i", LINUX_DEVICE_INPUT,
            "-t", &duration.to_string(),
            "-y",
            &output_file,
        ])
        .output();

    if !record_result.is_ok() || !record_result.as_ref().unwrap().status.success() {
        return Err(SGSimpleError::new(
            1,
            "failed to record video. install ffmpeg or check camera access.".to_string(),
        ));
    }
    #[cfg(feature = "transcription")]
    let transcript = if matches.get_flag(NO_TRANSCRIBE_ARG) {
        String::new()
    } else {
        let model_path = matches
            .get_one::<String>(MODEL_PATH_ARG)
            .map(|s| s.to_string())
            .unwrap_or_else(get_default_model_path);
        extract_and_transcribe_linux(&output_file, &model_path)?
    };

    #[cfg(not(feature = "transcription"))]
    let transcript = String::new();

    println!("{}", transcript);
    Ok(())
}

#[cfg(target_os = "linux")]
fn produce_json(matches: &ArgMatches, options: StardustOutputOptions) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "wpath", "cpath", "proc", "exec"])?;

    let duration: u32 = *matches.get_one::<u32>(DURATION_ARG).unwrap();
    let duration = duration.min(MAX_DURATION_SECONDS);

    let output_file = matches
        .get_one::<String>(OUTPUT_ARG)
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let dir = ensure_record_dir().expect("failed to create record directory");
            format!("{}/record_video_{}{}", dir, std::process::id(), LINUX_FILE_EXTENSION)
        });

    let record_result = ProcessCommand::new("ffmpeg")
        .args([
            "-f", LINUX_VIDEO_INPUT,
            "-video_size", VIDEO_WIDTH,
            "-framerate", VIDEO_FRAMERATE,
            "-i", LINUX_DEVICE_INPUT,
            "-t", &duration.to_string(),
            "-y",
            &output_file,
        ])
        .output();

    let result = if !record_result.is_ok() || !record_result.as_ref().unwrap().status.success() {
        RecordVideoResult {
            transcript: String::new(),
            duration: duration as f64,
            word_count: 0,
            success: false,
            video_file: output_file,
            error: Some("failed to record video. install ffmpeg or check camera access.".to_string()),
        }
    } else {
        #[cfg(feature = "transcription")]
        let (transcript, word_count) = if matches.get_flag(NO_TRANSCRIBE_ARG) {
            (String::new(), 0)
        } else {
            let model_path = matches
                .get_one::<String>(MODEL_PATH_ARG)
                .map(|s| s.to_string())
                .unwrap_or_else(get_default_model_path);
            match extract_and_transcribe_linux(&output_file, &model_path) {
                Ok(transcript) => {
                    let word_count = transcript.split_whitespace().count();
                    (transcript, word_count)
                }
                Err(_) => (String::new(), 0),
            }
        };

        #[cfg(not(feature = "transcription"))]
        let (transcript, word_count) = (String::new(), 0);

        RecordVideoResult {
            transcript,
            duration: duration as f64,
            word_count,
            success: true,
            video_file: output_file,
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
fn extract_and_transcribe(video_file: &str, model_path: &str) -> SGResult<String> {
    let dir = ensure_record_dir()?;
    let audio_file = format!("{}/record_video_audio_{}{}", dir, std::process::id(), AUDIO_FILE_EXTENSION);

    let extract_result = ProcessCommand::new("ffmpeg")
        .args([
            "-i", video_file,
            "-vn",
            "-acodec", AUDIO_CODEC,
            "-ar", AUDIO_SAMPLE_RATE,
            "-ac", AUDIO_CHANNELS,
            "-y",
            &audio_file,
        ])
        .output();

    if !extract_result.is_ok() || !extract_result.as_ref().unwrap().status.success() {
        return Ok("no audio in video".to_string());
    }

    let transcript = transcribe_audio_vosk(&audio_file, model_path)?;

    let _ = std::fs::remove_file(&audio_file);

    Ok(transcript)
}

#[cfg(all(target_os = "macos", feature = "transcription"))]
fn transcribe_audio_vosk(audio_file: &str, model_path: &str) -> SGResult<String> {
    vosk::set_log_level(vosk::LogLevel::Error);

    let model = vosk::Model::new(model_path)
        .ok_or_else(|| SGSimpleError::new(1, "failed to load vosk model".to_string()))?;

    let mut recognizer = vosk::Recognizer::new(&model, 16000.0)
        .ok_or_else(|| SGSimpleError::new(1, "failed to create recognizer".to_string()))?;

    let mut reader = hound::WavReader::open(audio_file)
        .map_err(|e| SGSimpleError::new(1, format!("failed to open audio file: {}", e)))?;

    let spec = reader.spec();

    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(SGSimpleError::new(1, "audio must be 16-bit PCM format".to_string()));
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

#[cfg(all(target_os = "linux", feature = "transcription"))]
fn extract_and_transcribe_linux(video_file: &str, model_path: &str) -> SGResult<String> {
    let dir = ensure_record_dir()?;
    let audio_file = format!("{}/record_video_audio_{}{}", dir, std::process::id(), AUDIO_FILE_EXTENSION);

    let extract_result = ProcessCommand::new("ffmpeg")
        .args([
            "-i", video_file,
            "-vn",
            "-acodec", AUDIO_CODEC,
            "-ar", AUDIO_SAMPLE_RATE,
            "-ac", AUDIO_CHANNELS,
            "-y",
            &audio_file,
        ])
        .output();

    if !extract_result.is_ok() || !extract_result.as_ref().unwrap().status.success() {
        return Ok("no audio in video".to_string());
    }

    let transcript = transcribe_audio_vosk(&audio_file, model_path)?;

    let _ = std::fs::remove_file(&audio_file);

    Ok(transcript)
}

#[cfg(all(target_os = "linux", feature = "transcription"))]
fn transcribe_audio_vosk(audio_file: &str, model_path: &str) -> SGResult<String> {
    let model = vosk::Model::new(model_path)
        .ok_or_else(|| SGSimpleError::new(1, "failed to load vosk model".to_string()))?;

    let mut recognizer = vosk::Recognizer::new(&model, 16000.0)
        .ok_or_else(|| SGSimpleError::new(1, "failed to create recognizer".to_string()))?;

    let mut reader = hound::WavReader::open(audio_file)
        .map_err(|e| SGSimpleError::new(1, format!("failed to open audio file: {}", e)))?;

    let spec = reader.spec();

    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(SGSimpleError::new(1, "audio must be 16-bit PCM format".to_string()));
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

