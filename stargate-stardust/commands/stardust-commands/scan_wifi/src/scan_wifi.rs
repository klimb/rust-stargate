// Copyright (C) 2025 Dmitry Kalashnikov

use clap::{Arg, Command};
use sgcore::error::UResult;
use sgcore::format_usage;
use sgcore::translate;
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::json;

#[cfg(target_os = "macos")]
use std::process::{Command as ProcessCommand, Stdio};

static ARG_INTERFACE: &str = "interface";
static ARG_DURATION: &str = "duration";
static ARG_CHANNEL: &str = "channel";

const DEFAULT_INTERFACE_MACOS: &str = "en0"; // will look for alfas later

#[derive(Debug, Clone)]
struct WifiNetwork {
    bssid: String,
    ssid: String,
    channel: String,
    signal_strength: String,
    encryption: String,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    
    #[cfg(target_os = "macos")]
    check_root_privileges()?;
    
    //sgcore::pledge::apply_pledge(&["stdio", "proc", "exec", "rpath"])?;
    let opts = StardustOutputOptions::from_matches(&matches);
    
    let interface = matches.get_one::<String>(ARG_INTERFACE)
        .map(|s| s.as_str())
        .unwrap_or(DEFAULT_INTERFACE_MACOS);
    let duration = matches.get_one::<u64>(ARG_DURATION).copied().unwrap_or(15);
    let channel = matches.get_one::<String>(ARG_CHANNEL).map(|s| s.as_str());
    
    if !opts.stardust_output {
        eprintln!("scanning for WiFi networks on interface: {}", interface);
        eprintln!("duration: {} seconds", duration);
        if let Some(ch) = channel {
            eprintln!("Channel: {}", ch);
        }
        eprintln!("this requires aircrack-ng to be installed and root privileges.");
        eprintln!();
    }
    
    let networks = scan_wifi_networks(interface, duration, channel)?;
    
    if opts.stardust_output {
        output_json(&networks, opts)?;
    } else {
        output_text(&networks);
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
fn check_root_privileges() -> UResult<()> {
    let current_uid = unsafe { libc::getuid() };
    let is_root = current_uid == 0;
    
    if !is_root {
        return Err(sgcore::error::USimpleError::new(
            1,
            "This command requires root privileges. Please run with sudo.".to_string()
        ));
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
fn scan_wifi_networks(_interface: &str, duration: u64, channel: Option<&str>) -> UResult<Vec<WifiNetwork>> {
    let airport_path = "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport";
    
    let mut cmd = ProcessCommand::new(airport_path);
    cmd.arg("-s");
    
    if let Some(ch) = channel {
        cmd.arg("--channel").arg(ch);
    }
    
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    std::thread::sleep(std::time::Duration::from_secs(duration));
    
    let output = cmd.output()
        .map_err(|e| sgcore::error::USimpleError::new(
            1, 
            format!("Failed to execute airport command: {}. Make sure you have the necessary permissions.", e)
        ))?;
    
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(sgcore::error::USimpleError::new(
            1,
            format!("airport command failed: {}", error_msg)
        ));
    }
    
    let output_text = String::from_utf8_lossy(&output.stdout);
    parse_airport_output(&output_text)
}

#[cfg(target_os = "macos")]
fn parse_airport_output(output: &str) -> UResult<Vec<WifiNetwork>> {
    let mut networks = Vec::new();
    let lines: Vec<&str> = output.lines().collect();
    
    if lines.is_empty() {
        return Ok(networks);
    }
    
    for line in lines.iter().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        
        let mac_pattern = regex::Regex::new(r"([0-9a-fA-F]{1,2}:[0-9a-fA-F]{1,2}:[0-9a-fA-F]{1,2}:[0-9a-fA-F]{1,2}:[0-9a-fA-F]{1,2}:[0-9a-fA-F]{1,2})").unwrap();
        
        if let Some(mac_match) = mac_pattern.find(line) {
            let ssid_part = &line[..mac_match.start()];
            let rest_part = &line[mac_match.end()..];
            
            let ssid = ssid_part.trim();
            let ssid = if ssid.is_empty() {
                "<hidden>".to_string()
            } else {
                ssid.to_string()
            };
            
            let bssid = mac_match.as_str().to_string();
            
            let parts: Vec<&str> = rest_part.split_whitespace().collect();
            if parts.len() < 5 {
                continue;
            }
            
            let rssi = parts[0].to_string();
            let channel_info = parts[1].to_string();
            let security = parts[4..].join(" ");
            
            networks.push(WifiNetwork {
                bssid,
                ssid,
                channel: channel_info,
                signal_strength: rssi,
                encryption: security,
            });
        }
    }
    
    Ok(networks)
}

#[cfg(not(target_os = "macos"))]
fn scan_wifi_networks(_interface: &str, _duration: u64, _channel: Option<&str>) -> UResult<Vec<WifiNetwork>> {
    Err(sgcore::error::USimpleError::new(
        1,
        "scan-wifi is currently only supported on macOS".to_string()
    ))
}

fn output_json(networks: &[WifiNetwork], opts: StardustOutputOptions) -> UResult<()> {
    let network_list: Vec<_> = networks.iter().map(|n| {
        json!({
            "ssid": n.ssid,
            "bssid": n.bssid,
            "channel": n.channel,
            "signal_strength": n.signal_strength,
            "encryption": n.encryption
        })
    }).collect();
    
    let output = json!({
        "networks": network_list,
        "count": networks.len()
    });
    
    stardust_output::output(opts, output, || Ok(()))?;
    Ok(())
}

fn output_text(networks: &[WifiNetwork]) {
    println!("SSID\t\t\tBSSID\t\t\tCHANNEL\tSIGNAL\tENCRYPTION");
    println!("{}", "=".repeat(100));
    
    for network in networks {
        println!("{}\t{}\t{}\t{}\t{}",
            truncate_string(&network.ssid, 20),
            network.bssid,
            network.channel,
            network.signal_strength,
            truncate_string(&network.encryption, 20));
    }
    
    println!("\nTotal networks found: {}", networks.len());
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len-3])
    }
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("scan-wifi-about"))
        .override_usage(format_usage(&translate!("scan-wifi-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(ARG_INTERFACE)
                .short('i')
                .long("interface")
                .value_name("INTERFACE")
                .help(translate!("scan-wifi-help-interface"))
                .default_value(DEFAULT_INTERFACE_MACOS)
        )
        .arg(
            Arg::new(ARG_DURATION)
                .short('d')
                .long("duration")
                .value_name("SECONDS")
                .help(translate!("scan-wifi-help-duration"))
                .value_parser(clap::value_parser!(u64))
                .default_value("15")
        )
        .arg(
            Arg::new(ARG_CHANNEL)
                .short('c')
                .long("channel")
                .value_name("CHANNEL")
                .help(translate!("scan-wifi-help-channel"))
        );
    
    stardust_output::add_json_args(cmd)
}
