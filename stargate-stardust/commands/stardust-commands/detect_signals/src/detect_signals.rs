// Copyright (c) 2025 Dmitry Kalashnikov.

use clap::{Arg, Command};
use sgcore::error::SGResult;
use sgcore::stardust_output::StardustOutputOptions;
use serde_json::json;
use std::process::{Command as ProcessCommand, Stdio};
use std::collections::HashMap;
use chrono::Local;
use crate::classifier::classify_signal;

fn parse_frequency(input: &str) -> Result<u64, String> {
    let input = input.trim().to_lowercase();
    
    if let Some(val) = input.strip_suffix("ghz") {
        let num: f64 = val.trim().parse()
            .map_err(|_| format!("Invalid frequency: {}", input))?;
        Ok((num * 1_000_000_000.0) as u64)
    } else if let Some(val) = input.strip_suffix("mhz") {
        let num: f64 = val.trim().parse()
            .map_err(|_| format!("Invalid frequency: {}", input))?;
        Ok((num * 1_000_000.0) as u64)
    } else {
        input.parse::<u64>()
            .map_err(|_| format!("Invalid frequency: {}. Use Hz, MHz, or GHz (e.g., 88MHz, 1.7GHz)", input))
    }
}

static ARG_FREQUENCY_START: &str = "freq-start";
static ARG_FREQUENCY_END: &str = "freq-end";
static ARG_THRESHOLD: &str = "threshold";
static ARG_DURATION: &str = "duration";
static ARG_PPM: &str = "ppm";
static ARG_PRESET: &str = "preset";

const DEFAULT_FREQ_START: u64 = 24_000_000;
const DEFAULT_FREQ_END: u64 = 1_700_000_000;
const DEFAULT_THRESHOLD: f64 = -30.0;
const DEFAULT_DURATION: u64 = 10;
const DEFAULT_PPM: i32 = 0;
const BIN_SIZE: u32 = 1_000_000;

use crate::classifier::{get_preset_by_type, get_preset_by_classification, PresetInfo};

struct ScanPreset {
    name: &'static str,
    lookup_key: PresetLookup,
    description: &'static str,
    threshold_override: Option<f64>,
    duration_override: Option<u64>,
}

enum PresetLookup {
    ByType(&'static str),
    ByClassification(&'static str),
    Custom { freq_start: u64, freq_end: u64 },
}

static SCAN_PRESETS: &[ScanPreset] = &[
    ScanPreset {
        name: "fm-radio",
        lookup_key: PresetLookup::ByType("FM Broadcast"),
        description: "FM Broadcast Radio (88-108 MHz)",
        threshold_override: None,
        duration_override: None,
    },
    ScanPreset {
        name: "am-radio",
        lookup_key: PresetLookup::ByType("AM Radio"),
        description: "AM Broadcast Radio (530-1710 kHz)",
        threshold_override: None,
        duration_override: None,
    },
    ScanPreset {
        name: "ham-2m",
        lookup_key: PresetLookup::ByType("Amateur 2m"),
        description: "Amateur Radio 2m Band (144-148 MHz)",
        threshold_override: None,
        duration_override: None,
    },
    ScanPreset {
        name: "ham-70cm",
        lookup_key: PresetLookup::ByType("Amateur 70cm"),
        description: "Amateur Radio 70cm Band (420-450 MHz)",
        threshold_override: None,
        duration_override: None,
    },
    ScanPreset {
        name: "ham-all",
        lookup_key: PresetLookup::ByClassification("Ham Radio"),
        description: "All Amateur Radio Bands",
        threshold_override: None,
        duration_override: Some(10),
    },
    ScanPreset {
        name: "marine",
        lookup_key: PresetLookup::ByType("Marine VHF"),
        description: "Marine VHF Radio (156-162 MHz)",
        threshold_override: None,
        duration_override: None,
    },
    ScanPreset {
        name: "iot",
        lookup_key: PresetLookup::ByType("ISM 433MHz"),
        description: "ISM 433 MHz Band (IoT devices)",
        threshold_override: None,
        duration_override: None,
    },
    ScanPreset {
        name: "ism-all",
        lookup_key: PresetLookup::Custom { freq_start: 315_000_000, freq_end: 928_000_000 },
        description: "All ISM Bands (315/433/868/915 MHz)",
        threshold_override: None,
        duration_override: Some(8),
    },
    ScanPreset {
        name: "wifi-2g",
        lookup_key: PresetLookup::Custom { freq_start: 2_400_000_000, freq_end: 2_500_000_000 },
        description: "WiFi 2.4 GHz Band",
        threshold_override: Some(-40.0),
        duration_override: None,
    },
    ScanPreset {
        name: "wifi-5g",
        lookup_key: PresetLookup::Custom { freq_start: 5_150_000_000, freq_end: 5_850_000_000 },
        description: "WiFi 5 GHz Band",
        threshold_override: Some(-40.0),
        duration_override: None,
    },
    ScanPreset {
        name: "gps",
        lookup_key: PresetLookup::Custom { freq_start: 1_570_000_000, freq_end: 1_580_000_000 },
        description: "GPS/Galileo Satellites (1.57-1.58 GHz)",
        threshold_override: Some(-40.0),
        duration_override: None,
    },
    ScanPreset {
        name: "glonass",
        lookup_key: PresetLookup::ByType("GLONASS L1"),
        description: "GLONASS Satellites (1.598-1.61 GHz)",
        threshold_override: Some(-40.0),
        duration_override: None,
    },
    ScanPreset {
        name: "weather-sat",
        lookup_key: PresetLookup::ByType("Weather Sat"),
        description: "NOAA Weather Satellites (137-138 MHz)",
        threshold_override: Some(-35.0),
        duration_override: None,
    },
    ScanPreset {
        name: "cb-radio",
        lookup_key: PresetLookup::ByType("CB Radio"),
        description: "Citizens Band Radio (26.96-27.41 MHz)",
        threshold_override: None,
        duration_override: None,
    },
    ScanPreset {
        name: "shortwave",
        lookup_key: PresetLookup::Custom { freq_start: 3_000_000, freq_end: 30_000_000 },
        description: "Shortwave Radio Bands (3-30 MHz)",
        threshold_override: None,
        duration_override: Some(10),
    },
    ScanPreset {
        name: "uhf-wide",
        lookup_key: PresetLookup::Custom { freq_start: 400_000_000, freq_end: 470_000_000 },
        description: "UHF Wide Scan (400-470 MHz)",
        threshold_override: None,
        duration_override: Some(8),
    },
    ScanPreset {
        name: "all",
        lookup_key: PresetLookup::Custom { freq_start: 24_000_000, freq_end: 1_700_000_000 },
        description: "Full RTL-SDR Range (24 MHz - 1.7 GHz)",
        threshold_override: None,
        duration_override: Some(15),
    },
];

fn get_preset(name: &str) -> Option<PresetInfo> {
    let preset = SCAN_PRESETS.iter().find(|p| p.name == name)?;
    
    let mut info = match &preset.lookup_key {
        PresetLookup::ByType(signal_type) => get_preset_by_type(signal_type)?,
        PresetLookup::ByClassification(classification) => get_preset_by_classification(classification)?,
        PresetLookup::Custom { freq_start, freq_end } => PresetInfo {
            freq_min: *freq_start,
            freq_max: *freq_end,
            threshold: -30.0,
            duration: 5,
        },
    };
    
    if let Some(threshold) = preset.threshold_override {
        info.threshold = threshold;
    }
    if let Some(duration) = preset.duration_override {
        info.duration = duration;
    }
    
    Some(info)
}

fn list_presets() {
    println!("Available Scan Presets");
    println!("======================");
    println!();
    
    for preset in SCAN_PRESETS {
        println!("  {:<15} - {}", preset.name, preset.description);
    }
    
    println!();
    println!("Usage: stargate detect-signals --preset <name>");
    println!("Example: stargate detect-signals --preset fm-radio");
}

#[derive(Debug, Clone)]
struct DetectedSignal {
    frequency_hz: u64,
    frequency_mhz: f64,
    power_dbm: f64,
    bandwidth_hz: u64,
    signal_type: String,
    classification: String,
    description: String,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    sgmain_impl(args)
}

fn sgmain_impl(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    
    let opts = StardustOutputOptions::from_matches(&matches);
    
    // Check if preset is specified
    if let Some(preset_name) = matches.get_one::<String>(ARG_PRESET) {
        if preset_name == "list" {
            list_presets();
            return Ok(());
        }
        
        let preset_info = get_preset(preset_name)
            .ok_or_else(|| sgcore::error::SGSimpleError::new(
                1, 
                format!("Unknown preset '{}'. Use --preset list to see available presets.", preset_name)
            ))?;
        
        let preset_desc = SCAN_PRESETS.iter()
            .find(|p| p.name == preset_name)
            .map(|p| p.description)
            .unwrap_or(preset_name);
        
        println!("Using preset: {} - {}", preset_name, preset_desc);
        println!();
        
        if !command_exists("rtl_power") {
            return Err(sgcore::error::SGSimpleError::new(
                1,
                "rtl-sdr is not installed. Linux: sudo apt-get install rtl-sdr\n  macOS: brew install librtlsdr".to_string()
            ));
        }
        
        let ppm = matches.get_one::<i32>(ARG_PPM).copied().unwrap_or(DEFAULT_PPM);
        let signals = detect_signals(preset_info.freq_min, preset_info.freq_max, preset_info.threshold, preset_info.duration, ppm)?;
        
        if opts.stardust_output {
            output_json(&signals)?;
        } else {
            output_text(&signals);
        }
        
        return Ok(());
    }
    
    // Check if no frequency arguments provided - show all bands
    let show_all_bands = matches.get_one::<String>(ARG_FREQUENCY_START).is_none() 
        && matches.get_one::<String>(ARG_FREQUENCY_END).is_none();
    
    if show_all_bands {
        list_all_signal_bands();
        return Ok(());
    }
    
    let freq_start = if let Some(val) = matches.get_one::<String>(ARG_FREQUENCY_START) {
        parse_frequency(val)
            .map_err(|e| sgcore::error::SGSimpleError::new(1, e))?
    } else {
        DEFAULT_FREQ_START
    };
    
    let freq_end = if let Some(val) = matches.get_one::<String>(ARG_FREQUENCY_END) {
        parse_frequency(val)
            .map_err(|e| sgcore::error::SGSimpleError::new(1, e))?
    } else {
        DEFAULT_FREQ_END
    };
    
    let threshold = matches.get_one::<f64>(ARG_THRESHOLD)
        .copied()
        .unwrap_or(DEFAULT_THRESHOLD);
    
    let duration = matches.get_one::<u64>(ARG_DURATION)
        .copied()
        .unwrap_or(DEFAULT_DURATION);
    
    let ppm = matches.get_one::<i32>(ARG_PPM)
        .copied()
        .unwrap_or(DEFAULT_PPM);

    if !command_exists("rtl_power") {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            "rtl-sdr is not installed. Linux: sudo apt-get install rtl-sdr\n  macOS: brew install librtlsdr".to_string()
        ));
    }

    let signals = detect_signals(freq_start, freq_end, threshold, duration, ppm)?;

    if opts.stardust_output {
        output_json(&signals)?;
    } else {
        output_text(&signals);
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

fn list_all_signal_bands() {
    use crate::classifier::get_all_signal_bands;
    
    println!("RF Signal Classification Database");
    println!("==================================");
    println!();
    
    let bands = get_all_signal_bands();
    let total = bands.len();
    
    let mut current_category = "";
    for band in &bands {
        let category = band.classification;
        if category != current_category {
            if !current_category.is_empty() {
                println!();
            }
            println!("{}:", category);
            current_category = category;
        }
        
        let freq_start_mhz = band.freq_min as f64 / 1_000_000.0;
        let freq_end_mhz = band.freq_max as f64 / 1_000_000.0;
        
        println!("  {:<25} {:>10.3} - {:<10.3} MHz", 
            band.signal_type, 
            freq_start_mhz, 
            freq_end_mhz
        );
    }
    
    println!();
    println!("Total signal types: {}", total);
    println!();
    println!("QUICK SCAN PRESETS:");
    println!();
    for preset in SCAN_PRESETS {
        println!("  detect-signals --preset {:<12}  # {}", preset.name, preset.description);
    }
    println!();
    println!("Or use --freq-start and --freq-end to scan a custom frequency range.");
}

fn detect_signals(
    freq_start: u64,
    freq_end: u64,
    threshold: f64,
    duration: u64,
    ppm: i32,
) -> SGResult<Vec<DetectedSignal>> {
    println!("Intelligent RF Signal Detector");
    println!("==============================");
    println!("Scanning: {:.1} MHz - {:.1} MHz", freq_start as f64 / 1_000_000.0, freq_end as f64 / 1_000_000.0);
    println!("Threshold: {} dBm", threshold);
    println!("Duration: {}s", duration);
    println!();
    println!("Scanning spectrum...");

    let mut cmd = ProcessCommand::new("rtl_power");
    cmd.arg("-f")
        .arg(format!("{}:{}:{}", freq_start, freq_end, BIN_SIZE))
        .arg("-g")
        .arg("40")
        .arg("-i")
        .arg(format!("{}s", duration))
        .arg("-1")
        .arg("-")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    if ppm != 0 {
        cmd.arg("-p").arg(ppm.to_string());
    }

    let output = cmd.output()
        .map_err(|e| sgcore::error::SGSimpleError::new(1, format!("failed to run rtl_power: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let signals = parse_and_classify(&stdout, threshold)?;

    Ok(signals)
}

fn parse_and_classify(output: &str, threshold: f64) -> SGResult<Vec<DetectedSignal>> {
    let mut signal_map: HashMap<u64, (f64, u64)> = HashMap::new();

    for line in output.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 6 {
            continue;
        }

        let freq_low: u64 = match parts[2].trim().parse() {
            Ok(f) => f,
            Err(_) => continue,
        };
        
        let freq_high: u64 = match parts[3].trim().parse() {
            Ok(f) => f,
            Err(_) => continue,
        };

        let power_values: Vec<f64> = parts[6..].iter()
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect();

        if power_values.is_empty() {
            continue;
        }

        let num_bins = power_values.len();
        let freq_step = (freq_high - freq_low) / num_bins as u64;

        for (i, &power) in power_values.iter().enumerate() {
            if power > threshold {
                let freq_hz = freq_low + (i as u64 * freq_step);
                let freq_mhz = freq_hz / 1_000_000;
                
                signal_map.entry(freq_mhz * 1_000_000)
                    .and_modify(|(max_power, count)| {
                        *max_power = max_power.max(power);
                        *count += 1;
                    })
                    .or_insert((power, 1));
            }
        }
    }

    let mut signals: Vec<DetectedSignal> = signal_map.iter()
        .map(|(&freq_hz, &(power, count))| {
            let freq_mhz = freq_hz as f64 / 1_000_000.0;
            let bandwidth = count as u64 * BIN_SIZE as u64;
            let (signal_type, classification, description) = classify_signal(freq_hz, bandwidth);
            
            DetectedSignal {
                frequency_hz: freq_hz,
                frequency_mhz: freq_mhz,
                power_dbm: power,
                bandwidth_hz: bandwidth,
                signal_type,
                classification,
                description,
            }
        })
        .collect();

    signals.sort_by(|a, b| b.power_dbm.partial_cmp(&a.power_dbm).unwrap());

    Ok(signals)
}

fn output_text(signals: &[DetectedSignal]) {
    println!();
    println!("Detected Signals:");
    println!("=================");
    
    if signals.is_empty() {
        println!("No signals detected above threshold.");
        return;
    }

    println!();
    println!("{:<5} {:<15} {:<20} {:<15} {:<20} {}", 
             "#", "Frequency", "Type", "Classification", "Power", "Description");
    println!("{:-<5} {:-<15} {:-<20} {:-<15} {:-<20} {:-<40}", "", "", "", "", "", "");

    for (idx, signal) in signals.iter().enumerate() {
        println!("{:<5} {:<15} {:<20} {:<15} {:<20} {}", 
                 idx + 1,
                 format!("{:.2} MHz", signal.frequency_mhz),
                 signal.signal_type,
                 signal.classification,
                 format!("{:.1} dBm", signal.power_dbm),
                 signal.description);
    }

    println!();
    println!("Total signals detected: {}", signals.len());
    println!("Scan completed at: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
}

fn output_json(signals: &[DetectedSignal]) -> SGResult<()> {
    let signals_json: Vec<_> = signals.iter().enumerate().map(|(idx, s)| {
        json!({
            "index": idx + 1,
            "frequency_hz": s.frequency_hz,
            "frequency_mhz": s.frequency_mhz,
            "power_dbm": s.power_dbm,
            "bandwidth_hz": s.bandwidth_hz,
            "signal_type": s.signal_type,
            "classification": s.classification,
            "description": s.description,
        })
    }).collect();

    let output = json!({
        "command": "detect-signals",
        "timestamp": Local::now().to_rfc3339(),
        "total_signals": signals.len(),
        "signals": signals_json,
    });

    let json_str = serde_json::to_string_pretty(&output)
        .unwrap_or_else(|_| "{}".to_string());
    println!("{}", json_str);
    
    Ok(())
}

pub fn sg_app() -> Command {
    let preset_names: Vec<&str> = SCAN_PRESETS.iter().map(|p| p.name).collect();
    let preset_help = format!(
        "Use a preset scan configuration\n\nAvailable presets:\n{}",
        SCAN_PRESETS.iter()
            .map(|p| format!("  {:<15} - {}", p.name, p.description))
            .collect::<Vec<_>>()
            .join("\n")
    );
    
    let cmd = Command::new(sgcore::util_name())
        .version(clap::crate_version!())
        .about("intelligent RF signal detector and classifier using RTL-SDR")
        .after_help("EXAMPLES:\n  detect-signals --preset fm-radio\n  detect-signals --freq-start 88MHz --freq-end 108MHz\n  detect-signals --preset gps\n  detect-signals  (list band classifications)")
        .arg(
            Arg::new(ARG_PRESET)
                .long("preset")
                .short('P')
                .value_name("NAME")
                .value_parser(preset_names)
                .help(&preset_help)
        )
        .arg(
            Arg::new(ARG_FREQUENCY_START)
                .long("freq-start")
                .short('f')
                .value_name("FREQ")
                .help("Start frequency (e.g., 88MHz, 1.7GHz, 24000000)")
                .conflicts_with(ARG_PRESET)
        )
        .arg(
            Arg::new(ARG_FREQUENCY_END)
                .long("freq-end")
                .short('e')
                .value_name("FREQ")
                .help("End frequency (e.g., 108MHz, 1.9GHz, 1700000000)")
                .conflicts_with(ARG_PRESET)
        )
        .arg(
            Arg::new(ARG_THRESHOLD)
                .long("threshold")
                .short('t')
                .value_name("DBM")
                .default_value("-30")
                .value_parser(clap::value_parser!(f64))
                .help("Signal detection threshold in dBm")
        )
        .arg(
            Arg::new(ARG_DURATION)
                .long("duration")
                .short('d')
                .value_name("SECONDS")
                .default_value("10")
                .value_parser(clap::value_parser!(u64))
                .help("Scan duration in seconds")
        )
        .arg(
            Arg::new(ARG_PPM)
                .long("ppm")
                .short('p')
                .value_name("PPM")
                .default_value("0")
                .value_parser(clap::value_parser!(i32))
                .help("PPM frequency correction")
        );

    sgcore::stardust_output::add_json_args(cmd)
}
