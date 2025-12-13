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
static ARG_PHONE: &str = "phone";

const DEFAULT_FREQ_START: u32 = 88_000_000;
const DEFAULT_FREQ_END: u32 = 108_000_000;
const PHONE_FREQ_START: u32 = 800_000_000;
const PHONE_FREQ_END: u32 = 2_700_000_000;
const DEFAULT_BIN_SIZE: u32 = 125_000;
const PHONE_BIN_SIZE: u32 = 1_000_000;
const DEFAULT_GAIN: u32 = 40;
const DEFAULT_DURATION: u64 = 10;
const DEFAULT_PPM: i32 = 0;

const GSM_850_START: u64 = 806_000_000;
const GSM_850_END: u64 = 809_000_000;
const GSM_900_START: u64 = 870_000_000;
const GSM_900_END: u64 = 890_000_000;
const DCS_1800_START: u64 = 1_800_000_000;
const DCS_1800_END: u64 = 1_900_000_000;
const PCS_1900_START: u64 = 1_920_000_000;
const PCS_1900_END: u64 = 2_000_000_000;
const UMTS_LTE_START: u64 = 2_050_000_000;
const UMTS_LTE_END: u64 = 2_100_000_000;

#[derive(Debug, Clone)]
struct RFSignal {
    frequency: String,
    frequency_hz: u64,
    signal_strength: f64,
    signal_dbm: String,
    band: Option<String>,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    sgmain_impl(args)
}

fn sgmain_impl(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    
    let opts = StardustOutputOptions::from_matches(&matches);
    
    let phone_mode = matches.get_flag(ARG_PHONE);
    
    let (default_start, default_end, bin_size) = if phone_mode {
        (PHONE_FREQ_START, PHONE_FREQ_END, PHONE_BIN_SIZE)
    } else {
        (DEFAULT_FREQ_START, DEFAULT_FREQ_END, DEFAULT_BIN_SIZE)
    };
    
    let freq_start = matches.get_one::<u32>(ARG_FREQUENCY_START)
        .copied()
        .unwrap_or(default_start);
    let freq_end = matches.get_one::<u32>(ARG_FREQUENCY_END)
        .copied()
        .unwrap_or(default_end);
    let duration = matches.get_one::<u64>(ARG_DURATION)
        .copied()
        .unwrap_or(DEFAULT_DURATION);
    let ppm = matches.get_one::<i32>(ARG_PPM)
        .copied()
        .unwrap_or(DEFAULT_PPM);
    
    if !opts.stardust_output {
        eprintln!("RF Signal Scanner using RTL-SDR");
        eprintln!("================================");
        if phone_mode {
            eprintln!("Mode: Phone/Cellular detection (800 MHz - 2.7 GHz)");
        } else {
            eprintln!("Mode: FM Radio (88-108 MHz)");
        }
        eprintln!("Frequency range: {} Hz - {} Hz", freq_start, freq_end);
        eprintln!("Duration: {} seconds", duration);
        eprintln!("PPM correction: {}", ppm);
    }
    
    let signals = scan_rf_signals(freq_start, freq_end, duration, ppm, bin_size)?;
    
    if opts.stardust_output {
        output_json(&signals, opts)?;
    } else {
        output_text(&signals);
    }
    
    Ok(())
}

fn scan_rf_signals(freq_start: u32, freq_end: u32, duration: u64, ppm: i32, bin_size: u32) -> UResult<Vec<RFSignal>> {
    if !command_exists("rtl_power") {
        return Err(sgcore::error::USimpleError::new(
            1,
            "rtl-sdr is not installed. Linux: sudo apt-get install rtl-sdr\n  macOS: brew install librtlsdr".to_string()
        ));
    }
    
    eprintln!("Scanning RF spectrum...");
    eprintln!("This may take a moment...");
    
    let mut cmd = ProcessCommand::new("rtl_power");
    cmd.arg("-f")
        .arg(format!("{}:{}:{}", freq_start, freq_end, bin_size))
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
            band: detect_cellular_band(*freq_hz),
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

fn detect_cellular_band(freq_hz: u64) -> Option<String> {
    if freq_hz >= GSM_850_START && freq_hz <= GSM_850_END {
        Some("GSM 850".to_string())
    } else if freq_hz >= GSM_900_START && freq_hz <= GSM_900_END {
        Some("GSM 900".to_string())
    } else if freq_hz >= DCS_1800_START && freq_hz <= DCS_1800_END {
        Some("DCS 1800".to_string())
    } else if freq_hz >= PCS_1900_START && freq_hz <= PCS_1900_END {
        Some("PCS 1900".to_string())
    } else if freq_hz >= UMTS_LTE_START && freq_hz <= UMTS_LTE_END {
        Some("UMTS/LTE".to_string())
    } else {
        None
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
    println!("Detected RF Signals:");
    println!("====================");
    println!("{:<20} {:<15} {:<12}", "Frequency", "Signal Strength", "Band");
    println!("{:-<20} {:-<15} {:-<12}", "", "", "");
    
    for signal in signals.iter().take(50) {
        let band_str = signal.band.as_ref().map(|s| s.as_str()).unwrap_or("-");
        println!("{:<20} {:<15} {:<12}", signal.frequency, signal.signal_dbm, band_str);
    }
    
    if signals.len() > 50 {
        println!("... and {} more signals", signals.len() - 50);
    }
}

fn output_json(signals: &[RFSignal], _opts: StardustOutputOptions) -> UResult<()> {
    let json_signals: Vec<serde_json::Value> = signals.iter()
        .map(|s| {
            let mut obj = json!({
                "frequency": s.frequency,
                "frequency_hz": s.frequency_hz,
                "signal_strength_dbm": s.signal_strength,
            });
            if let Some(band) = &s.band {
                obj["band"] = json!(band);
            }
            obj
        })
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
            Arg::new(ARG_PHONE)
                .long("phone")
                .short('p')
                .action(clap::ArgAction::SetTrue)
                .help("Scan phone/cellular frequency bands (800 MHz - 2.7 GHz)")
        )
        .arg(
            Arg::new(ARG_FREQUENCY_START)
                .long("freq-start")
                .value_name("FREQUENCY")
                .value_parser(clap::value_parser!(u32))
                .help("Start frequency in Hz (default: 88 MHz for FM, 800 MHz for phone)")
        )
        .arg(
            Arg::new(ARG_FREQUENCY_END)
                .long("freq-end")
                .value_name("FREQUENCY")
                .value_parser(clap::value_parser!(u32))
                .help("End frequency in Hz (default: 108 MHz for FM, 2.7 GHz for phone)")
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
