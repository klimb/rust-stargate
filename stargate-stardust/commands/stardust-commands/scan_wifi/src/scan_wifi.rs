// Copyright (c) 2025 Dmitry Kalashnikov.

use clap::{Arg, Command};
use sgcore::error::SGResult;
use sgcore::format_usage;
use sgcore::translate;
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::json;

#[cfg(target_os = "macos")]
use std::process::{Command as ProcessCommand, Stdio};

static ARG_INTERFACE: &str = "interface";
static ARG_DURATION: &str = "duration";
static ARG_CHANNEL: &str = "channel";

#[cfg(target_os = "macos")]
const DEFAULT_INTERFACE: &str = "en0";

#[cfg(target_os = "linux")]
const DEFAULT_INTERFACE: &str = "wlan0";

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const DEFAULT_INTERFACE: &str = "wlan0";

#[derive(Debug, Clone)]
struct ClientInfo {
    mac: String,
    signal: String,
    packets: usize,
}

#[derive(Debug, Clone)]
struct WifiNetwork {
    bssid: String,
    ssid: String,
    channel: String,
    signal_strength: String,
    encryption: String,
    clients: Option<usize>,
    packets: Option<usize>,
    beacons: Option<usize>,
    client_details: Vec<ClientInfo>,
    distance_meters: Option<f64>,
    distance: Option<String>,
    proximity: Option<String>,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;

    // Handle --schema flag
    if stardust_output::self_describe(&matches, sgcore::schema!(
        "networks" => "array", "List of detected WiFi networks with their properties";
        "count" => "integer", "Total number of networks found";
    ))? {
        return Ok(());
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    check_root_privileges()?;

    let opts = StardustOutputOptions::from_matches(&matches);

    let detected_interface = detect_wireless_interface();
    let interface_from_args = matches.get_one::<String>(ARG_INTERFACE);
    let interface = if interface_from_args.is_some() {
        interface_from_args.unwrap().as_str()
    } else if let Some(ref iface) = detected_interface {
        iface.as_str()
    } else {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            "No external WiFi adapter detected. scan-wifi requires an external USB WiFi adapter (wlx* on Linux). Please plug in an external WiFi adapter and specify it with --interface.".to_string()
        ));
    };

    #[cfg(target_os = "linux")]
    if !interface.starts_with("wlx") {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            format!("scan-wifi only works with external WiFi adapters (wlx*). Interface '{}' appears to be built-in. Please plug in an external USB WiFi adapter.", interface)
        ));
    }

    #[cfg(target_os = "macos")]
    if interface.starts_with("en") && !detected_interface.is_none() {
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(&["SPUSBDataType"])
            .output()
        {
            let usb_info = String::from_utf8_lossy(&output.stdout);
            if !usb_info.contains(interface) {
                return Err(sgcore::error::SGSimpleError::new(
                    1,
                    format!("scan-wifi only works with external USB WiFi adapters. Interface '{}' appears to be built-in. Please plug in an external USB WiFi adapter.", interface)
                ));
            }
        }
    }

    let duration = matches.get_one::<u64>(ARG_DURATION).copied().unwrap_or(15);
    let channel = matches.get_one::<String>(ARG_CHANNEL).map(|s| s.as_str());

    if !opts.stardust_output {
        if interface_from_args.is_none() {
            eprintln!("auto-detected external WiFi adapter: {}", interface);
        } else {
            eprintln!("using WiFi adapter: {}", interface);
        }
        eprintln!("duration: {} seconds", duration);
        if let Some(ch) = channel {
            eprintln!("Channel: {}", ch);
        }
        eprintln!("this requires root privileges and iw/wireless-tools (or airodump-ng).");
        eprintln!();
    }

    let mut networks = {
        #[cfg(target_os = "linux")]
        {
            scan_wifi_detailed(interface, duration, channel)
                .or_else(|_| scan_wifi_networks(interface, duration, channel))?
        }
        #[cfg(not(target_os = "linux"))]
        {
            scan_wifi_networks(interface, duration, channel)?
        }
    };

    networks.sort_by(|a, b| {
        let sig_a = parse_signal_strength(&a.signal_strength);
        let sig_b = parse_signal_strength(&b.signal_strength);
        sig_b.partial_cmp(&sig_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    if opts.stardust_output {
        output_json(&networks, opts)?;
    } else {
        output_text(&networks);
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn detect_wireless_interface() -> Option<String> {
    use std::process::{Command as ProcessCommand, Stdio};

    let iw_paths = ["/usr/sbin/iw", "/sbin/iw", "/usr/bin/iw"];

    for iw_path in &iw_paths {
        if let Ok(output) = ProcessCommand::new(iw_path)
            .arg("dev")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let mut wlx_interfaces = Vec::new();

                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("Interface ") {
                        let parts: Vec<&str> = trimmed.split_whitespace().collect();
                        if parts.len() >= 2 {
                            let iface = parts[1].to_string();
                            if iface.starts_with("wlx") {
                                wlx_interfaces.push(iface);
                            }
                        }
                    }
                }

                wlx_interfaces.sort();
                if !wlx_interfaces.is_empty() {
                    return Some(wlx_interfaces[0].clone());
                }
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
        let mut wlx_interfaces = Vec::new();

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("wlx") {
                let wireless_path = format!("/sys/class/net/{}/wireless", name_str);
                if std::path::Path::new(&wireless_path).exists() {
                    wlx_interfaces.push(name_str.to_string());
                }
            }
        }

        wlx_interfaces.sort();
        if !wlx_interfaces.is_empty() {
            return Some(wlx_interfaces[0].clone());
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn detect_wireless_interface() -> Option<String> {
    use std::process::{Command as ProcessCommand, Stdio};

    let output = ProcessCommand::new("networksetup")
        .arg("-listallhardwareports")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let mut is_usb_wifi = false;
        for line in text.lines() {
            if line.contains("USB") && (line.contains("Wi-Fi") || line.contains("AirPort")) {
                is_usb_wifi = true;
            } else if is_usb_wifi && line.starts_with("Device: ") {
                let device = line.trim_start_matches("Device: ").trim();
                return Some(device.to_string());
            } else if line.starts_with("Hardware Port:") {
                is_usb_wifi = false;
            }
        }
    }

    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn detect_wireless_interface() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn check_root_privileges() -> SGResult<()> {
    let current_uid = unsafe { libc::getuid() };
    let is_root = current_uid == 0;

    if !is_root {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            "This command requires root privileges. Please run with sudo.".to_string()
        ));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn scan_wifi_networks(_interface: &str, duration: u64, channel: Option<&str>) -> SGResult<Vec<WifiNetwork>> {
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
        .map_err(|e| sgcore::error::SGSimpleError::new(
            1,
            format!("Failed to execute airport command: {}. Make sure you have the necessary permissions.", e)
        ))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(sgcore::error::SGSimpleError::new(
            1,
            format!("airport command failed: {}", error_msg)
        ));
    }

    let output_text = String::from_utf8_lossy(&output.stdout);
    parse_airport_output(&output_text)
}

#[cfg(target_os = "macos")]
fn parse_airport_output(output: &str) -> SGResult<Vec<WifiNetwork>> {
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

            let rssi_str = parts[0].to_string();
            let channel_info = parts[1].to_string();
            let security = parts[4..].join(" ");

            let rssi_value = parse_signal_strength(&rssi_str) as i16;
            let distance_m = estimate_distance(rssi_value, None);
            let distance_string = distance_to_string(distance_m);
            let proximity_string = distance_to_proximity(distance_m);

            networks.push(WifiNetwork {
                bssid,
                ssid,
                channel: channel_info,
                signal_strength: rssi_str,
                encryption: security,
                clients: None,
                packets: None,
                beacons: None,
                client_details: Vec::new(),
                distance_meters: Some(distance_m),
                distance: Some(distance_string),
                proximity: Some(proximity_string.to_string()),
            });
        }
    }

    Ok(networks)
}

#[cfg(target_os = "linux")]
fn check_root_privileges() -> SGResult<()> {
    let current_uid = unsafe { libc::getuid() };
    let is_root = current_uid == 0;

    if !is_root {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            "This command requires root privileges. Please run with sudo.".to_string()
        ));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn scan_wifi_networks(interface: &str, _duration: u64, _channel: Option<&str>) -> SGResult<Vec<WifiNetwork>> {
    use std::process::{Command as ProcessCommand, Stdio};

    check_root_privileges()?;

    let iw_paths = ["/usr/sbin/iw", "/sbin/iw", "/usr/bin/iw", "iw"];
    let mut iw_cmd = None;
    for path in &iw_paths {
        if std::path::Path::new(path).exists() || *path == "iw" {
            iw_cmd = Some(*path);
            break;
        }
    }

    if let Some(iw_path) = iw_cmd {
        let output = ProcessCommand::new(iw_path)
            .arg("dev")
            .arg(interface)
            .arg("scan")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| sgcore::error::SGSimpleError::new(
                1,
                format!("Failed to execute iw: {}", e)
            ))?;

        if output.status.success() {
            let output_text = String::from_utf8_lossy(&output.stdout);
            return parse_iw_output(&output_text);
        }
    }

    let output = ProcessCommand::new("iwlist")
        .arg(interface)
        .arg("scan")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| sgcore::error::SGSimpleError::new(
            1,
            format!("Failed to execute wireless scan. Please install iw or wireless-tools: sudo apt install iw wireless-tools\nError: {}", e)
        ))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(sgcore::error::SGSimpleError::new(
            1,
            format!("Wireless scan failed: {}. Try: sudo apt install iw wireless-tools", error_msg)
        ));
    }

    let output_text = String::from_utf8_lossy(&output.stdout);
    parse_iwlist_output(&output_text)
}

#[cfg(target_os = "linux")]
fn scan_wifi_detailed(interface: &str, duration: u64, _channel: Option<&str>) -> SGResult<Vec<WifiNetwork>> {
    use std::process::{Command as ProcessCommand, Stdio};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    check_root_privileges()?;

    // Safety check: only allow monitor mode on external WiFi cards
    if !interface.starts_with("wlx") {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            format!("Monitor mode scanning only supported on external WiFi adapters (wlx*). Interface '{}' appears to be built-in. Use --interface wlx<device> or remove --interface to auto-detect external adapter.", interface)
        ));
    }

    let airmon_paths = ["/usr/sbin/airmon-ng", "/sbin/airmon-ng", "/usr/bin/airmon-ng", "airmon-ng"];
    let mut airmon_cmd = None;
    for path in &airmon_paths {
        if std::path::Path::new(path).exists() || *path == "airmon-ng" {
            airmon_cmd = Some(*path);
            break;
        }
    }

    let airmon_path = airmon_cmd.ok_or_else(|| {
        sgcore::error::SGSimpleError::new(
            1,
            "airmon-ng not found. Install with: sudo apt install aircrack-ng".to_string()
        )
    })?;

    let airodump_paths = ["/usr/sbin/airodump-ng", "/sbin/airodump-ng", "/usr/bin/airodump-ng", "airodump-ng"];
    let mut airodump_cmd = None;
    for path in &airodump_paths {
        if std::path::Path::new(path).exists() || *path == "airodump-ng" {
            airodump_cmd = Some(*path);
            break;
        }
    }

    let airodump_path = airodump_cmd.ok_or_else(|| {
        sgcore::error::SGSimpleError::new(
            1,
            "airodump-ng not found. Install with: sudo apt install aircrack-ng".to_string()
        )
    })?;

    // Save current connection state for restoration
    let connection_info = save_connection_state(interface);

    let _ = ProcessCommand::new(airmon_path)
        .arg("stop")
        .arg(interface)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    std::thread::sleep(std::time::Duration::from_secs(1));

    let _ = ProcessCommand::new(airmon_path)
        .arg("start")
        .arg(interface)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    std::thread::sleep(std::time::Duration::from_secs(3));

    let monitor_iface = interface.to_string();

    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let scan_dir = format!("{}/.stargate/scan-wifi", home_dir);
    fs::create_dir_all(&scan_dir)
        .map_err(|e| sgcore::error::SGSimpleError::new(
            1,
            format!("Failed to create directory {}: {}", scan_dir, e)
        ))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let output_prefix = format!("{}/scan-wifi-{}", scan_dir, timestamp);
    let csv_file = format!("{}-01.csv", output_prefix);

    eprintln!("Starting airodump-ng on {} for {} seconds...", monitor_iface, duration);

    let mut child = ProcessCommand::new(airodump_path)
        .arg(&monitor_iface)
        .arg("--write")
        .arg(&output_prefix)
        .arg("--output-format")
        .arg("csv")
        .arg("--background")
        .arg("1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| sgcore::error::SGSimpleError::new(
            1,
            format!("Failed to start airodump-ng: {}. Make sure {} is in monitor mode", e, monitor_iface)
        ))?;

    std::thread::sleep(std::time::Duration::from_secs(duration));

    let _ = child.kill();
    let _ = child.wait();

    std::thread::sleep(std::time::Duration::from_secs(2));

    if !std::path::Path::new(&csv_file).exists() {
        return Ok(Vec::new());
    }
    let csv_content = fs::read_to_string(&csv_file)
        .map_err(|e| sgcore::error::SGSimpleError::new(
            1,
            format!("Failed to read airodump-ng output: {}", e)
        ))?;

    let networks = parse_airodump_csv(&csv_content)?;

    // Stop monitor mode
    let _ = ProcessCommand::new(airmon_path)
        .arg("stop")
        .arg(&monitor_iface)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    std::thread::sleep(std::time::Duration::from_secs(2));

    // Restore connection if there was one
    restore_connection_state(interface, &connection_info);

    let _ = fs::remove_file(&csv_file);
    let cap_file = format!("{}-01.cap", output_prefix);
    let _ = fs::remove_file(&cap_file);
    let kismet_file = format!("{}-01.kismet.csv", output_prefix);
    let _ = fs::remove_file(&kismet_file);
    let netxml_file = format!("{}-01.kismet.netxml", output_prefix);
    let _ = fs::remove_file(&netxml_file);

    Ok(networks)
}

#[cfg(target_os = "linux")]
fn parse_airodump_csv(content: &str) -> SGResult<Vec<WifiNetwork>> {
    let mut networks = Vec::new();
    let mut in_ap_section = false;
    let mut in_station_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("BSSID") {
            in_ap_section = true;
            in_station_section = false;
            continue;
        }

        if trimmed.starts_with("Station MAC") {
            in_ap_section = false;
            in_station_section = true;
            continue;
        }

        if trimmed.is_empty() {
            if in_ap_section {
                in_ap_section = false;
            }
            continue;
        }

        if in_ap_section {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() < 14 {
                continue;
            }

            let bssid = parts[0].to_string();
            if bssid.is_empty() {
                continue;
            }

            let channel = parts[3].trim().to_string();
            let signal = format!("{} dBm", parts[8].trim());
            let beacons = parts[9].trim().parse::<usize>().ok();
            let packets = parts[10].trim().parse::<usize>().ok();
            let encryption = format!("{} {}", parts[5].trim(), parts[6].trim()).trim().to_string();
            let ssid = if parts.len() > 13 && !parts[13].trim().is_empty() {
                parts[13].trim().to_string()
            } else {
                "<hidden>".to_string()
            };

            let rssi_value = parse_signal_strength(&signal) as i16;
            let distance_m = estimate_distance(rssi_value, None);
            let distance_string = distance_to_string(distance_m);
            let proximity_string = distance_to_proximity(distance_m);

            networks.push(WifiNetwork {
                bssid,
                ssid,
                channel,
                signal_strength: signal,
                encryption,
                clients: None,
                packets,
                beacons,
                client_details: Vec::new(),
                distance_meters: Some(distance_m),
                distance: Some(distance_string),
                proximity: Some(proximity_string.to_string()),
            });
        } else if in_station_section {
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() < 6 {
                continue;
            }

            let client_mac = parts[0].to_string();
            if client_mac.is_empty() {
                continue;
            }

            let bssid = parts[5].to_string();
            if bssid == "(not associated)" || bssid.is_empty() {
                continue;
            }

            let signal = parts[3].to_string();
            let packets = parts[4].trim().parse::<usize>().unwrap_or(0);

            let client_info = ClientInfo {
                mac: client_mac,
                signal: format!("{} dBm", signal),
                packets,
            };

            for network in &mut networks {
                if network.bssid == bssid {
                    network.client_details.push(client_info.clone());
                    break;
                }
            }
        }
    }

    for network in &mut networks {
        network.clients = Some(network.client_details.len());
    }

    Ok(networks)
}

#[cfg(target_os = "macos")]
fn scan_wifi_detailed(_interface: &str, _duration: u64, _channel: Option<&str>) -> SGResult<Vec<WifiNetwork>> {
    Err(sgcore::error::SGSimpleError::new(
        1,
        "--detailed mode is only supported on Linux with airodump-ng".to_string()
    ))
}

#[cfg(target_os = "linux")]
fn parse_iw_output(output: &str) -> SGResult<Vec<WifiNetwork>> {
    let mut networks = Vec::new();
    let mut current_network: Option<WifiNetwork> = None;

    for line in output.lines() {
        let line = line.trim();

        if line.starts_with("BSS ") {
            if let Some(network) = current_network.take() {
                networks.push(network);
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let bssid = parts[1].trim_end_matches('(').to_string();
                current_network = Some(WifiNetwork {
                    bssid,
                    ssid: String::new(),
                    channel: String::new(),
                    signal_strength: String::new(),
                    encryption: "Open".to_string(),
                    clients: None,
                    packets: None,
                    beacons: None,
                    client_details: Vec::new(),
                    distance_meters: None,
                    distance: None,
                    proximity: None,
                });
            }
        } else if line.starts_with("SSID: ") {
            if let Some(ref mut network) = current_network {
                let ssid = line[6..].trim();
                network.ssid = if ssid.is_empty() {
                    "<hidden>".to_string()
                } else {
                    ssid.to_string()
                };
            }
        } else if line.starts_with("signal: ") {
            if let Some(ref mut network) = current_network {
                let signal = line[8..].trim();
                network.signal_strength = signal.to_string();
            }
        } else if line.starts_with("DS Parameter set: channel ") {
            if let Some(ref mut network) = current_network {
                let channel = line[27..].trim();
                network.channel = channel.to_string();
            }
        } else if line.starts_with("* primary channel: ") {
            if let Some(ref mut network) = current_network {
                if network.channel.is_empty() {
                    let channel = line[19..].trim();
                    network.channel = channel.to_string();
                }
            }
        } else if line.contains("RSN:") || line.contains("WPA:") {
            if let Some(ref mut network) = current_network {
                if line.contains("RSN:") {
                    network.encryption = "WPA2/WPA3".to_string();
                } else if network.encryption == "Open" {
                    network.encryption = "WPA".to_string();
                }
            }
        } else if line.starts_with("* Authentication suites:") {
            if let Some(ref mut network) = current_network {
                if line.contains("PSK") {
                    if network.encryption == "Open" {
                        network.encryption = "WPA-PSK".to_string();
                    }
                }
            }
        }
    }

    if let Some(mut network) = current_network {
        if !network.signal_strength.is_empty() {
            let rssi_value = parse_signal_strength(&network.signal_strength) as i16;
            let distance_m = estimate_distance(rssi_value, None);
            network.distance_meters = Some(distance_m);
            network.distance = Some(distance_to_string(distance_m));
            network.proximity = Some(distance_to_proximity(distance_m).to_string());
        }
        networks.push(network);
    }

    Ok(networks)
}

#[cfg(target_os = "linux")]
fn parse_iwlist_output(output: &str) -> SGResult<Vec<WifiNetwork>> {
    let mut networks = Vec::new();
    let mut current_network: Option<WifiNetwork> = None;

    for line in output.lines() {
        let line = line.trim();

        if line.starts_with("Cell ") {
            if let Some(network) = current_network.take() {
                networks.push(network);
            }

            if let Some(addr_start) = line.find("Address: ") {
                let bssid = line[addr_start + 9..].split_whitespace().next().unwrap_or("").to_string();
                current_network = Some(WifiNetwork {
                    bssid,
                    ssid: String::new(),
                    channel: String::new(),
                    signal_strength: String::new(),
                    encryption: String::new(),
                    clients: None,
                    packets: None,
                    beacons: None,
                    client_details: Vec::new(),
                    distance_meters: None,
                    distance: None,
                    proximity: None,
                });
            }
        } else if line.starts_with("Channel:") {
            if let Some(ref mut network) = current_network {
                network.channel = line.split(':').nth(1).unwrap_or("").trim().to_string();
            }
        } else if line.starts_with("Quality=") || line.contains("Signal level=") {
            if let Some(ref mut network) = current_network {
                if let Some(signal_pos) = line.find("Signal level=") {
                    let signal_part = &line[signal_pos + 13..];
                    let signal = signal_part.split_whitespace().next().unwrap_or("");
                    network.signal_strength = signal.to_string();
                }
            }
        } else if line.starts_with("ESSID:") {
            if let Some(ref mut network) = current_network {
                let essid = line.split(':').nth(1).unwrap_or("").trim().trim_matches('"');
                network.ssid = if essid.is_empty() {
                    "<hidden>".to_string()
                } else {
                    essid.to_string()
                };
            }
        } else if line.contains("Encryption key:") {
            if let Some(ref mut network) = current_network {
                let encrypted = line.contains("on");
                if encrypted {
                    network.encryption = "Encrypted".to_string();
                } else {
                    network.encryption = "Open".to_string();
                }
            }
        } else if line.starts_with("IE: IEEE 802.11i/WPA2") {
            if let Some(ref mut network) = current_network {
                network.encryption = "WPA2".to_string();
            }
        } else if line.starts_with("IE: WPA") {
            if let Some(ref mut network) = current_network {
                if network.encryption == "Encrypted" {
                    network.encryption = "WPA".to_string();
                }
            }
        }
    }

    if let Some(mut network) = current_network {
        if !network.signal_strength.is_empty() {
            let rssi_value = parse_signal_strength(&network.signal_strength) as i16;
            let distance_m = estimate_distance(rssi_value, None);
            network.distance_meters = Some(distance_m);
            network.distance = Some(distance_to_string(distance_m));
            network.proximity = Some(distance_to_proximity(distance_m).to_string());
        }
        networks.push(network);
    }

    Ok(networks)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn scan_wifi_networks(_interface: &str, _duration: u64, _channel: Option<&str>) -> SGResult<Vec<WifiNetwork>> {
    Err(sgcore::error::SGSimpleError::new(
        1,
        "scan-wifi is currently only supported on macOS and Linux".to_string()
    ))
}

fn output_json(networks: &[WifiNetwork], opts: StardustOutputOptions) -> SGResult<()> {
    let network_list: Vec<_> = networks.iter().map(|n| {
        let mut obj = json!({
            "ssid": n.ssid,
            "bssid": n.bssid,
            "channel": n.channel,
            "signal_strength": n.signal_strength,
            "encryption": n.encryption
        });

        if let Some(clients) = n.clients {
            obj["clients"] = json!(clients);
        }
        if let Some(packets) = n.packets {
            obj["packets"] = json!(packets);
        }
        if let Some(beacons) = n.beacons {
            obj["beacons"] = json!(beacons);
        }
        if let Some(distance_m) = n.distance_meters {
            obj["distance_meters"] = json!(distance_m);
        }
        if let Some(ref distance) = n.distance {
            obj["distance"] = json!(distance);
        }
        if let Some(ref proximity) = n.proximity {
            obj["proximity"] = json!(proximity);
        }

        if !n.client_details.is_empty() {
            let clients_json: Vec<_> = n.client_details.iter().map(|c| {
                json!({
                    "mac": c.mac,
                    "signal": c.signal,
                    "packets": c.packets
                })
            }).collect();
            obj["client_details"] = json!(clients_json);
        }

        obj
    }).collect();

    let output = json!({
        "networks": network_list,
        "count": networks.len()
    });

    stardust_output::output(opts, output, || Ok(()))?;
    Ok(())
}

fn output_text(networks: &[WifiNetwork]) {
    let has_detailed = networks.iter().any(|n| n.beacons.is_some() || n.packets.is_some());

    if has_detailed {
        println!("SSID\t\t\tBSSID\t\t\tCH\tSIGNAL\t\tDISTANCE\tPROXIMITY\tBEACONS\tPACKETS\tCLIENTS\tENCRYPTION");
        println!("{}", "=".repeat(140));

        for network in networks {
            let distance_str = network.distance.as_deref().unwrap_or("-");
            let proximity_str = network.proximity.as_deref().unwrap_or("-");
            
            println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                truncate_string(&network.ssid, 20),
                network.bssid,
                network.channel,
                truncate_string(&network.signal_strength, 10),
                truncate_string(distance_str, 10),
                truncate_string(proximity_str, 10),
                network.beacons.map(|b| b.to_string()).unwrap_or_else(|| "-".to_string()),
                network.packets.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string()),
                network.clients.map(|c| c.to_string()).unwrap_or_else(|| "0".to_string()),
                truncate_string(&network.encryption, 15));

            if !network.client_details.is_empty() {
                for client in &network.client_details {
                    println!("  └─ Client: {} (Signal: {}, Packets: {})",
                        client.mac, client.signal, client.packets);
                }
            }
        }
    } else {
        for network in networks {
            let rssi_value = parse_signal_strength(&network.signal_strength) as i16;
            let quality = rssi_to_quality(rssi_value);
            let distance_str = network.distance.as_deref().unwrap_or("N/A");
            let proximity_str = network.proximity.as_deref().unwrap_or("N/A");
            
            println!("{} ({}) - {} - {}, {}, {} ({})",
                network.ssid,
                network.bssid,
                network.encryption,
                network.signal_strength,
                quality,
                distance_str,
                proximity_str);
        }
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

fn parse_signal_strength(signal_str: &str) -> f64 {
    signal_str
        .trim()
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(-100.0)
}

fn estimate_distance(rssi: i16, tx_power: Option<i8>) -> f64 {
    let measured_power = tx_power.unwrap_or(0) as f64;
    let n = 2.7;
    let exponent = (measured_power - rssi as f64) / (10.0 * n);
    10_f64.powf(exponent)
}

fn distance_to_string(distance_meters: f64) -> String {
    if distance_meters < 1.0 {
        format!("{:.0} cm", distance_meters * 100.0)
    } else if distance_meters < 10.0 {
        format!("{:.1} m", distance_meters)
    } else if distance_meters < 100.0 {
        format!("{:.0} m", distance_meters)
    } else {
        "100+ m".to_string()
    }
}

fn distance_to_proximity(distance_meters: f64) -> &'static str {
    if distance_meters < 1.0 {
        "Immediate"
    } else if distance_meters < 3.0 {
        "Very Close"
    } else if distance_meters < 10.0 {
        "Near"
    } else if distance_meters < 30.0 {
        "Far"
    } else {
        "Very Far"
    }
}

fn rssi_to_quality(rssi: i16) -> &'static str {
    if rssi >= -50 {
        "Excellent"
    } else if rssi >= -60 {
        "Good"
    } else if rssi >= -70 {
        "Fair"
    } else {
        "Weak"
    }
}

#[cfg(target_os = "linux")]
struct ConnectionInfo {
    was_up: bool,
    ssid: Option<String>,
    nm_was_running: bool,
}

#[cfg(target_os = "linux")]
fn save_connection_state(interface: &str) -> ConnectionInfo {
    use std::process::{Command as ProcessCommand, Stdio};
    
    let nm_running = ProcessCommand::new("systemctl")
        .args(&["is-active", "NetworkManager"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    
    let mut info = ConnectionInfo {
        was_up: false,
        ssid: None,
        nm_was_running: nm_running,
    };
    
    // Check if interface is up
    if let Ok(output) = ProcessCommand::new("ip")
        .args(&["link", "show", interface])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        info.was_up = text.contains("state UP");
    }
    
    // Try to get current SSID
    let iw_paths = ["/usr/sbin/iw", "/sbin/iw", "/usr/bin/iw", "iw"];
    for iw_path in &iw_paths {
        if let Ok(output) = ProcessCommand::new(iw_path)
            .arg("dev")
            .arg(interface)
            .arg("link")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if line.contains("SSID:") {
                        if let Some(ssid) = line.split("SSID:").nth(1) {
                            info.ssid = Some(ssid.trim().to_string());
                            break;
                        }
                    }
                }
                break;
            }
        }
    }
    
    info
}

#[cfg(target_os = "linux")]
fn restore_connection_state(interface: &str, info: &ConnectionInfo) {
    use std::process::{Command as ProcessCommand, Stdio};
    
    if info.nm_was_running {
        let nm_running = ProcessCommand::new("systemctl")
            .args(&["is-active", "NetworkManager"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        
        if !nm_running {
            let _ = ProcessCommand::new("systemctl")
                .args(&["start", "NetworkManager"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    }
    
    if info.was_up {
        let _ = ProcessCommand::new("ip")
            .args(&["link", "set", interface, "up"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    
    if let Some(ssid) = &info.ssid {
        let _ = ProcessCommand::new("nmcli")
            .args(&["device", "connect", interface])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        
        std::thread::sleep(std::time::Duration::from_secs(1));
        
        let _ = ProcessCommand::new("nmcli")
            .args(&["device", "wifi", "connect", ssid, "ifname", interface])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
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

