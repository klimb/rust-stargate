use clap::{Arg, Command};
use sgcore::error::UResult;
use sgcore::stardust_output::StardustOutputOptions;
use serde_json::json;
use std::process::{Command as ProcessCommand, Stdio};
use std::collections::HashMap;

static ARG_FREQUENCY_START: &str = "freq-start";
static ARG_FREQUENCY_END: &str = "freq-end";
static ARG_DURATION: &str = "duration";
static ARG_PPM: &str = "ppm";

const DEFAULT_FREQ_START: u32 = 88_000_000;
const DEFAULT_FREQ_END: u32 = 108_000_000;
const DEFAULT_BIN_SIZE: u32 = 125_000;
const DEFAULT_GAIN: u32 = 40;
const DEFAULT_DURATION: u64 = 10;
const DEFAULT_PPM: i32 = 0;

#[derive(Debug, Clone)]
struct RFSignal {
    frequency: String,
    frequency_hz: u64,
    signal_strength: f64,
    signal_dbm: String,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    sgmain_impl(args)
}

fn sgmain_impl(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    
    let opts = StardustOutputOptions::from_matches(&matches);
    
    let freq_start = matches.get_one::<u32>(ARG_FREQUENCY_START)
        .copied()
        .unwrap_or(DEFAULT_FREQ_START);
    let freq_end = matches.get_one::<u32>(ARG_FREQUENCY_END)
        .copied()
        .unwrap_or(DEFAULT_FREQ_END);
    let duration = matches.get_one::<u64>(ARG_DURATION)
        .copied()
        .unwrap_or(DEFAULT_DURATION);
    let ppm = matches.get_one::<i32>(ARG_PPM)
        .copied()
        .unwrap_or(DEFAULT_PPM);
    
    if !opts.stardust_output {
        eprintln!("RF Signal Scanner using RTL-SDR");
        eprintln!("================================");
        eprintln!("Frequency range: {} Hz - {} Hz", freq_start, freq_end);
        eprintln!("Duration: {} seconds", duration);
        eprintln!("PPM correction: {}", ppm);
    }
    
    let signals = scan_rf_signals(freq_start, freq_end, duration, ppm)?;
    
    if opts.stardust_output {
        output_json(&signals, opts)?;
    } else {
        output_text(&signals);
    }
    
    Ok(())
}

fn scan_rf_signals(freq_start: u32, freq_end: u32, duration: u64, ppm: i32) -> UResult<Vec<RFSignal>> {
    if !command_exists("rtl_power") {
        return Err(sgcore::error::USimpleError::new(
            1,
            "rtl-sdr is not installed. Linux: sudo apt-get install rtl-sdr\n  macOS: brew install librtlsdr".to_string()
        ));
    }
    
    eprintln!("scanning RF spectrum...");
    eprintln!("(this may take a moment...)");
    
    let mut cmd = ProcessCommand::new("rtl_power");
    cmd.arg("-f")
        .arg(format!("{}:{}:{}", freq_start, freq_end, DEFAULT_BIN_SIZE))
        .arg("-g")
        .arg(DEFAULT_GAIN.to_string())
        .arg("-i")
        .arg(format!("{}s", duration))
        .arg("-1")
        .arg("-")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    if ppm != 0 {
        cmd.arg("-p").arg(ppm.to_string());
    }
    
    let signals = match cmd.output() {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                for line in stderr.lines() {
                    if line.contains("found") || line.contains("Using") || line.contains("Detected") {
                        eprintln!("{}", line);
                    }
                }
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_rtl_power_output(&stdout, freq_start)?
        }
        Err(e) => {
            return Err(sgcore::error::USimpleError::new(
                1,
                format!("failed to run rtl_power: {}", e)
            ));
        }
    };
    
    let mut sorted_signals = signals;
    sorted_signals.sort_by(|a, b| b.signal_strength.partial_cmp(&a.signal_strength).unwrap_or(std::cmp::Ordering::Equal));
    
    Ok(sorted_signals)
}

fn parse_rtl_power_output(output: &str, _freq_start: u32) -> UResult<Vec<RFSignal>> {
    let mut signals = Vec::new();
    let mut freq_map: HashMap<u64, f64> = HashMap::new();
    
    for line in output.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 7 {
            continue;
        }
        
        let freq_low = parts[2].parse::<f64>().ok();
        let freq_high = parts[3].parse::<f64>().ok();
        let step = parts[4].parse::<f64>().ok();
        
        if let (Some(low), Some(high), Some(step_hz)) = (freq_low, freq_high, step) {
            for (idx, power_str) in parts[6..].iter().enumerate() {
                if let Ok(power) = power_str.parse::<f64>() {
                    if power > -100.0 {
                        let freq_hz = (low + (idx as f64 * step_hz)) as u64;
                        freq_map.entry(freq_hz)
                            .and_modify(|existing| *existing = existing.max(power))
                            .or_insert(power);
                    }
                }
            }
        }
    }
    
    for (freq_hz, power) in freq_map.iter() {
        signals.push(RFSignal {
            frequency: format_frequency(*freq_hz),
            frequency_hz: *freq_hz,
            signal_strength: *power,
            signal_dbm: format!("{:.1} dBm", power),
        });
    }
    
    signals.sort_by(|a, b| b.signal_strength.partial_cmp(&a.signal_strength).unwrap_or(std::cmp::Ordering::Equal));
    
    Ok(signals)
}

fn format_frequency(hz: u64) -> String {
    if hz >= 1_000_000_000 {
        format!("{:.2} GHz", hz as f64 / 1_000_000_000.0)
    } else if hz >= 1_000_000 {
        format!("{:.2} MHz", hz as f64 / 1_000_000.0)
    } else if hz >= 1_000 {
        format!("{:.2} kHz", hz as f64 / 1_000.0)
    } else {
        format!("{} Hz", hz)
    }
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

fn output_text(signals: &[RFSignal]) {
    if signals.is_empty() {
        println!("no RF signals detected in the specified frequency range.");
        return;
    }
    
    println!();
    println!("detected RF signals:");
    println!("====================");
    println!("{:<20} {:<15}", "Frequency", "Signal Strength");
    println!("{:-<20} {:-<15}", "", "");
    
    for signal in signals.iter().take(50) {
        println!("{:<20} {:<15}", signal.frequency, signal.signal_dbm);
    }
    
    if signals.len() > 50 {
        println!("... and {} more signals", signals.len() - 50);
    }
}

fn output_json(signals: &[RFSignal], _opts: StardustOutputOptions) -> UResult<()> {
    let json_signals: Vec<serde_json::Value> = signals.iter()
        .map(|s| json!({
            "frequency": s.frequency,
            "frequency_hz": s.frequency_hz,
            "signal_strength_dbm": s.signal_strength,
        }))
        .collect();
    
    let output = json!({
        "scan_type": "rf_spectrum",
        "signals_detected": signals.len(),
        "signals": json_signals
    });
    
    let json_str = serde_json::to_string_pretty(&output)
        .unwrap_or_else(|_| "{}".to_string());
    println!("{}", json_str);
    Ok(())
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(clap::crate_version!())
        .about("scan for radio transmissions using RTL-SDR and display signal strength")
        .arg(
            Arg::new(ARG_FREQUENCY_START)
                .long("freq-start")
                .value_name("FREQUENCY")
                .default_value("88000000")
                .value_parser(clap::value_parser!(u32))
                .help("Start frequency in Hz (default: 88 MHz - FM radio band)")
        )
        .arg(
            Arg::new(ARG_FREQUENCY_END)
                .long("freq-end")
                .value_name("FREQUENCY")
                .default_value("108000000")
                .value_parser(clap::value_parser!(u32))
                .help("End frequency in Hz (default: 108 MHz - FM radio band)")
        )
        .arg(
            Arg::new(ARG_DURATION)
                .long("duration")
                .short('d')
                .value_name("SECONDS")
                .default_value("10")
                .value_parser(clap::value_parser!(u64))
                .help("Scan duration in seconds (default: 10)")
        )
        .arg(
            Arg::new(ARG_PPM)
                .long("ppm")
                .value_name("PPM")
                .default_value("0")
                .value_parser(clap::value_parser!(i32))
                .help("PPM frequency correction")
        );
    
    sgcore::stardust_output::add_json_args(cmd)
}
