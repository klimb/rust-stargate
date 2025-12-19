// Copyright (c) 2025 Dmitry Kalashnikov.

use clap::{Arg, Command};
use sgcore::error::SGResult;
use sgcore::stardust_output::StardustOutputOptions;
use serde_json::json;
use std::process::{Command as ProcessCommand, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::Local;

static ARG_FREQUENCY_START: &str = "freq-start";
static ARG_FREQUENCY_END: &str = "freq-end";
static ARG_BIN_SIZE: &str = "bin-size";
static ARG_INTERVAL: &str = "interval";
static ARG_GAIN: &str = "gain";
static ARG_PPM: &str = "ppm";
static ARG_HEIGHT: &str = "height";
static ARG_MODE: &str = "mode";

const DEFAULT_FREQ_START: u32 = 88_000_000;
const DEFAULT_FREQ_END: u32 = 108_000_000;
const DEFAULT_BIN_SIZE: u32 = 125_000;
const DEFAULT_GAIN: u32 = 40;
const DEFAULT_PPM: i32 = 0;
const DEFAULT_INTERVAL: f32 = 0.5;
const DEFAULT_HEIGHT: usize = 20;

#[derive(Debug, Clone)]
struct SpectrumScan {
    timestamp: String,
    frequencies: Vec<FrequencyPower>,
    peak_frequency: u64,
    peak_power: f64,
    avg_power: f64,
}

#[derive(Debug, Clone)]
struct FrequencyPower {
    frequency_hz: u64,
    frequency_mhz: f64,
    power_db: f64,
    normalized: f64,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    sgmain_impl(args)
}

fn sgmain_impl(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    
    let opts = StardustOutputOptions::from_matches(&matches);
    
    let freq_start = matches.get_one::<u32>(ARG_FREQUENCY_START)
        .copied()
        .unwrap_or(DEFAULT_FREQ_START);
    
    let freq_end = matches.get_one::<u32>(ARG_FREQUENCY_END)
        .copied()
        .unwrap_or(DEFAULT_FREQ_END);
    
    let bin_size = matches.get_one::<u32>(ARG_BIN_SIZE)
        .copied()
        .unwrap_or(DEFAULT_BIN_SIZE);
    
    let interval = matches.get_one::<f32>(ARG_INTERVAL)
        .copied()
        .unwrap_or(DEFAULT_INTERVAL);
    
    let gain = matches.get_one::<u32>(ARG_GAIN)
        .copied()
        .unwrap_or(DEFAULT_GAIN);
    
    let ppm = matches.get_one::<i32>(ARG_PPM)
        .copied()
        .unwrap_or(DEFAULT_PPM);
    
    let height = matches.get_one::<usize>(ARG_HEIGHT)
        .copied()
        .unwrap_or(DEFAULT_HEIGHT);
    
    let mode = matches.get_one::<String>(ARG_MODE)
        .map(|s| s.as_str())
        .unwrap_or("waterfall");

    if !command_exists("rtl_power") {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            "rtl-sdr is not installed. Linux: sudo apt-get install rtl-sdr\n  macOS: brew install librtlsdr".to_string()
        ));
    }

    if opts.stardust_output {
        output_json(freq_start, freq_end, bin_size, interval, gain, ppm, mode)?;
    } else {
        monitor_spectrum(freq_start, freq_end, bin_size, interval, gain, ppm, height, mode)?;
    }
    
    Ok(())
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

fn monitor_spectrum(
    freq_start: u32,
    freq_end: u32,
    bin_size: u32,
    interval: f32,
    gain: u32,
    ppm: i32,
    height: usize,
    mode: &str,
) -> SGResult<()> {
    println!("\x1b[2J\x1b[H");
    println!("RF Spectrum Monitor using RTL-SDR");
    println!("==================================");
    println!("Frequency range: {:.2} MHz - {:.2} MHz", freq_start as f64 / 1_000_000.0, freq_end as f64 / 1_000_000.0);
    println!("Bin size: {} Hz", bin_size);
    println!("Refresh interval: {:.1}s", interval);
    println!("Gain: {}", gain);
    println!("Display mode: {}", mode);
    println!();
    println!("Press Ctrl+C to stop");
    println!();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    
    #[cfg(unix)]
    {
        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        }).map_err(|e| sgcore::error::SGSimpleError::new(1, format!("failed to set Ctrl+C handler: {}", e)))?;
    }

    let waterfall_history = Arc::new(Mutex::new(VecDeque::new()));
    
    while running.load(Ordering::SeqCst) {
        let mut rtl_cmd = ProcessCommand::new("rtl_power");
        rtl_cmd
            .arg("-f")
            .arg(format!("{}:{}:{}", freq_start, freq_end, bin_size))
            .arg("-g")
            .arg(gain.to_string())
            .arg("-i")
            .arg(format!("{}s", interval))
            .arg("-p")
            .arg(ppm.to_string())
            .arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut process = match rtl_cmd.spawn() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to start rtl_power: {}", e);
                break;
            }
        };

        let stdout = match process.stdout.take() {
            Some(s) => s,
            None => {
                eprintln!("Failed to capture rtl_power stdout");
                break;
            }
        };

        let reader = BufReader::new(stdout);
        
        for line in reader.lines() {
            if !running.load(Ordering::SeqCst) {
                let _ = process.kill();
                break;
            }
            
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            if let Ok(scan) = parse_rtl_power_line(&line) {
                if mode == "waterfall" {
                    display_waterfall(&scan, &waterfall_history, height);
                } else {
                    display_bars(&scan);
                }
            }
        }

        let _ = process.kill();
        
        if !running.load(Ordering::SeqCst) {
            break;
        }
    }
    
    println!("\n\nMonitoring stopped.");
    
    Ok(())
}

fn parse_rtl_power_line(line: &str) -> SGResult<SpectrumScan> {
    let parts: Vec<&str> = line.split(',').collect();
    
    if parts.len() < 6 {
        return Err(sgcore::error::SGSimpleError::new(1, "invalid rtl_power output format".to_string()));
    }
    
    let timestamp = format!("{} {}", parts[0], parts[1]);
    let freq_low: u64 = parts[2].trim().parse()
        .map_err(|_| sgcore::error::SGSimpleError::new(1, "invalid frequency".to_string()))?;
    let freq_high: u64 = parts[3].trim().parse()
        .map_err(|_| sgcore::error::SGSimpleError::new(1, "invalid frequency".to_string()))?;
    
    let power_values: Vec<f64> = parts[6..].iter()
        .filter_map(|s| s.trim().parse::<f64>().ok())
        .collect();
    
    if power_values.is_empty() {
        return Err(sgcore::error::SGSimpleError::new(1, "no power values found".to_string()));
    }
    
    let num_bins = power_values.len();
    let freq_step = (freq_high - freq_low) / num_bins as u64;
    
    let min_power = power_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_power = power_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let power_range = max_power - min_power;
    
    let mut frequencies = Vec::new();
    let mut peak_idx = 0;
    let mut peak_power = f64::NEG_INFINITY;
    
    for (i, &power) in power_values.iter().enumerate() {
        let freq_hz = freq_low + (i as u64 * freq_step);
        let normalized = if power_range > 0.0 {
            (power - min_power) / power_range
        } else {
            0.5
        };
        
        frequencies.push(FrequencyPower {
            frequency_hz: freq_hz,
            frequency_mhz: freq_hz as f64 / 1_000_000.0,
            power_db: power,
            normalized,
        });
        
        if power > peak_power {
            peak_power = power;
            peak_idx = i;
        }
    }
    
    let avg_power = power_values.iter().sum::<f64>() / power_values.len() as f64;
    let peak_frequency = frequencies[peak_idx].frequency_hz;
    
    Ok(SpectrumScan {
        timestamp,
        frequencies,
        peak_frequency,
        peak_power,
        avg_power,
    })
}

fn display_waterfall(scan: &SpectrumScan, history: &Arc<Mutex<VecDeque<SpectrumScan>>>, height: usize) {
    let mut hist = history.lock().unwrap();
    hist.push_back(scan.clone());
    
    while hist.len() > height {
        hist.pop_front();
    }
    
    print!("\x1b[2J\x1b[H");
    println!("RF Spectrum Monitor - Waterfall Display");
    println!("========================================");
    println!("Peak: {:.2} MHz @ {:.1} dB  |  Avg: {:.1} dB  |  Time: {}", 
             scan.peak_frequency as f64 / 1_000_000.0,
             scan.peak_power,
             scan.avg_power,
             Local::now().format("%H:%M:%S"));
    println!();
    
    let freq_start = scan.frequencies.first().unwrap().frequency_mhz;
    let freq_end = scan.frequencies.last().unwrap().frequency_mhz;
    
    println!("{:.1} MHz{:width$}{:.1} MHz", 
             freq_start, 
             "", 
             freq_end,
             width = scan.frequencies.len().saturating_sub(20));
    
    for scan_line in hist.iter() {
        for freq in &scan_line.frequencies {
            print!("{}", get_color_char(freq.normalized));
        }
        println!();
    }
    
    println!();
    print_legend();
    
    std::io::stdout().flush().unwrap();
}

fn display_bars(scan: &SpectrumScan) {
    print!("\x1b[2J\x1b[H");
    println!("RF Spectrum Monitor - Bar Display");
    println!("==================================");
    println!("Peak: {:.2} MHz @ {:.1} dB  |  Avg: {:.1} dB  |  Time: {}", 
             scan.peak_frequency as f64 / 1_000_000.0,
             scan.peak_power,
             scan.avg_power,
             Local::now().format("%H:%M:%S"));
    println!();
    
    let width = 80;
    let bins_per_char = (scan.frequencies.len() as f64 / width as f64).ceil() as usize;
    
    for row in (0..20).rev() {
        let threshold = row as f64 / 20.0;
        for chunk in scan.frequencies.chunks(bins_per_char) {
            let avg_normalized = chunk.iter().map(|f| f.normalized).sum::<f64>() / chunk.len() as f64;
            if avg_normalized >= threshold {
                print!("{}", get_bar_char(avg_normalized));
            } else {
                print!(" ");
            }
        }
        println!(" {:.0}%", threshold * 100.0);
    }
    
    println!("{}", "─".repeat(width + 2));
    
    let num_labels = 8;
    let step = scan.frequencies.len() / num_labels;
    for i in 0..num_labels {
        let idx = i * step;
        if idx < scan.frequencies.len() {
            print!("{:>10.1}", scan.frequencies[idx].frequency_mhz);
        }
    }
    println!("\n{:>40}", "Frequency (MHz)");
    
    std::io::stdout().flush().unwrap();
}

fn get_color_char(normalized: f64) -> &'static str {
    if normalized >= 0.9 { "\x1b[91m█\x1b[0m" }
    else if normalized >= 0.8 { "\x1b[93m█\x1b[0m" }
    else if normalized >= 0.7 { "\x1b[92m█\x1b[0m" }
    else if normalized >= 0.6 { "\x1b[96m█\x1b[0m" }
    else if normalized >= 0.5 { "\x1b[94m█\x1b[0m" }
    else if normalized >= 0.4 { "\x1b[95m█\x1b[0m" }
    else if normalized >= 0.3 { "\x1b[90m█\x1b[0m" }
    else if normalized >= 0.2 { "\x1b[90m▓\x1b[0m" }
    else if normalized >= 0.1 { "\x1b[90m░\x1b[0m" }
    else { " " }
}

fn get_bar_char(normalized: f64) -> &'static str {
    if normalized >= 0.9 { "\x1b[91m█\x1b[0m" }
    else if normalized >= 0.8 { "\x1b[93m█\x1b[0m" }
    else if normalized >= 0.7 { "\x1b[92m█\x1b[0m" }
    else if normalized >= 0.6 { "\x1b[96m▓\x1b[0m" }
    else if normalized >= 0.5 { "\x1b[94m▓\x1b[0m" }
    else if normalized >= 0.4 { "\x1b[95m▒\x1b[0m" }
    else if normalized >= 0.3 { "\x1b[90m▒\x1b[0m" }
    else { "\x1b[90m░\x1b[0m" }
}

fn print_legend() {
    println!("\nSignal Strength Legend:");
    print!("  Strongest: \x1b[91m█\x1b[0m  ");
    print!("Strong: \x1b[93m█\x1b[0m  ");
    print!("Medium: \x1b[92m█\x1b[0m  ");
    print!("Weak: \x1b[94m█\x1b[0m  ");
    println!("Weakest: \x1b[90m░\x1b[0m");
}

fn output_json(
    freq_start: u32,
    freq_end: u32,
    bin_size: u32,
    interval: f32,
    gain: u32,
    ppm: i32,
    mode: &str,
) -> SGResult<()> {
    let output = json!({
        "command": "monitor-spectrum",
        "frequency_range": {
            "start_hz": freq_start,
            "end_hz": freq_end,
            "start_mhz": freq_start as f64 / 1_000_000.0,
            "end_mhz": freq_end as f64 / 1_000_000.0
        },
        "bin_size_hz": bin_size,
        "interval_seconds": interval,
        "gain": gain,
        "ppm_correction": ppm,
        "display_mode": mode,
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
        .about("real-time RF spectrum monitor with waterfall display using RTL-SDR")
        .arg(
            Arg::new(ARG_FREQUENCY_START)
                .long("freq-start")
                .short('f')
                .value_name("HZ")
                .default_value("88000000")
                .value_parser(clap::value_parser!(u32))
                .help("Start frequency in Hz (default: 88 MHz)")
        )
        .arg(
            Arg::new(ARG_FREQUENCY_END)
                .long("freq-end")
                .short('e')
                .value_name("HZ")
                .default_value("108000000")
                .value_parser(clap::value_parser!(u32))
                .help("End frequency in Hz (default: 108 MHz)")
        )
        .arg(
            Arg::new(ARG_BIN_SIZE)
                .long("bin-size")
                .short('b')
                .value_name("HZ")
                .default_value("125000")
                .value_parser(clap::value_parser!(u32))
                .help("Frequency bin size in Hz")
        )
        .arg(
            Arg::new(ARG_INTERVAL)
                .long("interval")
                .short('i')
                .value_name("SECONDS")
                .default_value("0.5")
                .value_parser(clap::value_parser!(f32))
                .help("Refresh interval in seconds")
        )
        .arg(
            Arg::new(ARG_GAIN)
                .long("gain")
                .short('g')
                .value_name("GAIN")
                .default_value("40")
                .value_parser(clap::value_parser!(u32))
                .help("Tuner gain (0-50)")
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
            Arg::new(ARG_HEIGHT)
                .long("height")
                .short('h')
                .value_name("LINES")
                .default_value("20")
                .value_parser(clap::value_parser!(usize))
                .help("Waterfall display height in lines")
        )
        .arg(
            Arg::new(ARG_MODE)
                .long("mode")
                .short('m')
                .value_name("MODE")
                .default_value("waterfall")
                .value_parser(["waterfall", "bars"])
                .help("Display mode: waterfall or bars")
        );

    sgcore::stardust_output::add_json_args(cmd)
}
