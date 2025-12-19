// Copyright (c) 2025 Dmitry Kalashnikov.

use clap::{Arg, Command};
use sgcore::error::SGResult;
use sgcore::stardust_output::StardustOutputOptions;
use serde_json::json;
use std::process::{Command as ProcessCommand, Stdio};
use std::io;

static ARG_FREQUENCY: &str = "frequency";
static ARG_PPM: &str = "ppm";
static ARG_SQUELCH: &str = "squelch";
static ARG_GAIN: &str = "gain";

const DEFAULT_PPM: i32 = 0;
const DEFAULT_SQUELCH: i32 = 0;
const DEFAULT_GAIN: &str = "auto";

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    sgmain_impl(args)
}

fn sgmain_impl(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;

    let opts = StardustOutputOptions::from_matches(&matches);

    let frequency_str = matches.get_one::<String>(ARG_FREQUENCY)
        .ok_or_else(|| sgcore::error::SGSimpleError::new(1, "frequency is required".to_string()))?;
    
    let frequency = parse_frequency(frequency_str)?;
    
    let ppm = matches.get_one::<i32>(ARG_PPM)
        .copied()
        .unwrap_or(DEFAULT_PPM);
    
    let squelch = matches.get_one::<i32>(ARG_SQUELCH)
        .copied()
        .unwrap_or(DEFAULT_SQUELCH);
    
    let gain = matches.get_one::<String>(ARG_GAIN)
        .map(|s| s.as_str())
        .unwrap_or(DEFAULT_GAIN);

    if opts.stardust_output {
        output_json(frequency, ppm, squelch, gain)?;
    } else {
        play_radio(frequency, ppm, squelch, gain)?;
    }

    Ok(())
}

fn parse_frequency(freq_str: &str) -> SGResult<f64> {
    if let Ok(freq) = freq_str.parse::<f64>() {
        if freq >= 80.0 && freq <= 110.0 {
            return Ok(freq);
        }
        if freq >= 80_000_000.0 {
            return Ok(freq / 1_000_000.0);
        }
    }
    
    Err(sgcore::error::SGSimpleError::new(
        1,
        format!("invalid frequency: {}. Use format like 91.3 (MHz) or 91300000 (Hz)", freq_str)
    ))
}

fn command_exists(cmd: &str) -> bool {
    ProcessCommand::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn play_radio(frequency: f64, ppm: i32, squelch: i32, gain: &str) -> SGResult<()> {
    if !command_exists("rtl_fm") {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            "rtl-sdr is not installed. Linux: sudo apt-get install rtl-sdr\n  macOS: brew install librtlsdr".to_string()
        ));
    }
    
    let audio_player = if command_exists("play") {
        "play"
    } else if command_exists("aplay") {
        "aplay"
    } else {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            "no audio player found. Install sox (Linux: sudo apt-get install sox, macOS: brew install sox)".to_string()
        ));
    };

    println!("FM Radio Player using RTL-SDR");
    println!("==============================");
    println!("Frequency: {:.1} MHz", frequency);
    println!("PPM correction: {}", ppm);
    println!("Squelch level: {}", squelch);
    println!("Gain: {}", gain);
    println!();
    println!("Playing audio... Press Ctrl+C to stop");
    println!();

    let freq_hz = (frequency * 1_000_000.0) as u32;

    let mut rtl_cmd = ProcessCommand::new("rtl_fm");
    rtl_cmd
        .arg("-f")
        .arg(freq_hz.to_string())
        .arg("-M")
        .arg("wbfm")
        .arg("-s")
        .arg("200000")
        .arg("-r")
        .arg("48000")
        .arg("-l")
        .arg(squelch.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if ppm != 0 {
        rtl_cmd.arg("-p").arg(ppm.to_string());
    }

    if gain != "auto" {
        rtl_cmd.arg("-g").arg(gain);
    }

    let mut audio_cmd = if audio_player == "play" {
        let mut cmd = ProcessCommand::new("play");
        cmd.arg("-t")
            .arg("raw")
            .arg("-r")
            .arg("48000")
            .arg("-e")
            .arg("s")
            .arg("-b")
            .arg("16")
            .arg("-c")
            .arg("1")
            .arg("-V1")
            .arg("-");
        cmd
    } else {
        let mut cmd = ProcessCommand::new("aplay");
        cmd.arg("-r")
            .arg("48000")
            .arg("-f")
            .arg("S16_LE")
            .arg("-t")
            .arg("raw")
            .arg("-c")
            .arg("1");
        cmd
    };
    
    audio_cmd.stdin(Stdio::piped()).stderr(Stdio::piped());

    let mut rtl_process = rtl_cmd.spawn()
        .map_err(|e| sgcore::error::SGSimpleError::new(1, format!("failed to start rtl_fm: {}", e)))?;

    let rtl_stdout = rtl_process.stdout.take()
        .ok_or_else(|| sgcore::error::SGSimpleError::new(1, "failed to capture rtl_fm stdout".to_string()))?;

    let mut audio_process = audio_cmd.spawn()
        .map_err(|e| sgcore::error::SGSimpleError::new(1, format!("failed to start audio player: {}", e)))?;

    let mut audio_stdin = audio_process.stdin.take()
        .ok_or_else(|| sgcore::error::SGSimpleError::new(1, "failed to capture audio player stdin".to_string()))?;

    let pipe_handle = std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(rtl_stdout);
        io::copy(&mut reader, &mut audio_stdin)
    });

    let rtl_result = rtl_process.wait();
    let audio_result = audio_process.wait();
    let _ = pipe_handle.join();

    if let Err(e) = rtl_result {
        eprintln!("rtl_fm error: {}", e);
    }
    if let Err(e) = audio_result {
        eprintln!("audio player error: {}", e);
    }

    Ok(())
}

fn output_json(frequency: f64, ppm: i32, squelch: i32, gain: &str) -> SGResult<()> {
    let output = json!({
        "command": "listen-radio",
        "frequency_mhz": frequency,
        "frequency_hz": (frequency * 1_000_000.0) as u64,
        "ppm_correction": ppm,
        "squelch_level": squelch,
        "gain": gain,
        "status": "starting"
    });

    let json_str = serde_json::to_string_pretty(&output)
        .unwrap_or_else(|_| "{}".to_string());
    println!("{}", json_str);
    
    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(clap::crate_version!())
        .about("play audio from FM radio stations using RTL-SDR")
        .arg(
            Arg::new(ARG_FREQUENCY)
                .value_name("FREQUENCY")
                .required(true)
                .help("FM frequency in MHz (e.g., 91.3) or Hz")
        )
        .arg(
            Arg::new(ARG_PPM)
                .long("ppm")
                .short('p')
                .value_name("PPM")
                .default_value("0")
                .value_parser(clap::value_parser!(i32))
                .help("PPM frequency correction")
        )
        .arg(
            Arg::new(ARG_SQUELCH)
                .long("squelch")
                .short('s')
                .value_name("LEVEL")
                .default_value("0")
                .value_parser(clap::value_parser!(i32))
                .help("Squelch level (0-100, higher values cut out weaker signals)")
        )
        .arg(
            Arg::new(ARG_GAIN)
                .long("gain")
                .short('g')
                .value_name("GAIN")
                .default_value("auto")
                .help("Tuner gain (auto or 0-50, default: auto)")
        );

    sgcore::stardust_output::add_json_args(cmd)
}
