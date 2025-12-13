// Copyright (C) 2025 Dmitry Kalashnikov

use clap::{Arg, ArgAction, Command as ClapCommand};
use serde::{Deserialize, Serialize};
use sgcore::{
    error::{UResult, USimpleError},
    format_usage,
    stardust_output::{self, StardustOutputOptions},
    translate,
};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use pcap::{Capture, Device};

static ARG_INTERFACE: &str = "interface";
static ARG_DURATION: &str = "duration";
static ARG_CONTINUOUS: &str = "continuous";

const DEFAULT_DURATION: u64 = 30;
const PACKET_SLEEP_MS: u64 = 10;
const HOSTNAME_TIMEOUT_SECS: &str = "1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
enum TrafficType {
    ARP,
    HTTPS,
    SMTP,
    POP3,
    IMAP,
}

impl std::fmt::Display for TrafficType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrafficType::ARP => write!(f, "ARP"),
            TrafficType::HTTPS => write!(f, "HTTPS"),
            TrafficType::SMTP => write!(f, "SMTP"),
            TrafficType::POP3 => write!(f, "POP3"),
            TrafficType::IMAP => write!(f, "IMAP"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Neighbor {
    ip: String,
    mac: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<String>,
    first_seen: f64,
    last_seen: f64,
    packet_count: usize,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    protocols: HashMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScanResult {
    neighbors: Vec<Neighbor>,
    count: usize,
    duration: f64,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err(USimpleError::new(
            1,
            "scan-neighbors is currently only supported on macos and linux".to_string(),
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
        
        check_root_privileges()?;
        
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
        .about(translate!("scan-neighbors-about"))
        .override_usage(format_usage(&translate!("scan-neighbors-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(ARG_INTERFACE)
                .short('i')
                .long("interface")
                .value_name("INTERFACE")
                .help(translate!("scan-neighbors-help-interface")),
        )
        .arg(
            Arg::new(ARG_DURATION)
                .short('d')
                .long("duration")
                .value_name("SECONDS")
                .help(translate!("scan-neighbors-help-duration"))
                .value_parser(clap::value_parser!(u64))
                .default_value("30"),
        )
        .arg(
            Arg::new(ARG_CONTINUOUS)
                .short('c')
                .long("continuous")
                .help(translate!("scan-neighbors-help-continuous"))
                .action(ArgAction::SetTrue),
        );

    stardust_output::add_json_args(cmd)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn check_root_privileges() -> UResult<()> {
    let current_uid = unsafe { libc::getuid() };
    
    if current_uid != 0 {
        return Err(USimpleError::new(
            1,
            "this command requires root privileges. please run with sudo.".to_string()
        ));
    }
    
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn produce(matches: &clap::ArgMatches) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "inet", "bpf"])?;

    let (interface, duration, continuous) = extract_config(matches);
    print_scan_info(&interface, duration, continuous);

    let neighbors = scan_neighbors(&interface, duration, continuous)?;
    print_neighbor_table(&neighbors);

    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn extract_config(matches: &clap::ArgMatches) -> (String, u64, bool) {
    let interface = if let Some(iface) = matches.get_one::<String>(ARG_INTERFACE) {
        iface.to_string()
    } else {
        detect_wifi_interface().unwrap_or_else(|| "wlan0".to_string())
    };
    
    let duration = matches
        .get_one::<u64>(ARG_DURATION)
        .copied()
        .unwrap_or(DEFAULT_DURATION);
    
    let continuous = matches.get_flag(ARG_CONTINUOUS);

    (interface, duration, continuous)
}

#[cfg(target_os = "linux")]
fn detect_wifi_interface() -> Option<String> {
    use std::path::Path;
    
    Device::list().ok()?
        .into_iter()
        .find(|d| {
            Path::new(&format!("/sys/class/net/{}/wireless", d.name)).exists()
        })
        .map(|d| d.name)
}

#[cfg(target_os = "macos")]
fn detect_wifi_interface() -> Option<String> {
    use std::process::Command;
    
    let output = Command::new("networksetup")
        .arg("-listallhardwareports")
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let text = String::from_utf8_lossy(&output.stdout);
    let mut next_is_device = false;
    
    for line in text.lines() {
        if line.contains("Wi-Fi") || line.contains("AirPort") {
            next_is_device = true;
        } else if next_is_device && line.starts_with("Device:") {
            return line.split(':').nth(1).map(|s| s.trim().to_string());
        }
    }
    
    None
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn print_scan_info(interface: &str, duration: u64, continuous: bool) {
    eprintln!("passively monitoring traffic on interface: {}", interface);
    if continuous {
        eprintln!("running continuously... press ctrl+c to stop");
    } else {
        eprintln!("duration: {} seconds", duration);
    }
    eprintln!("(no packets will be sent - completely passive)\n");
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn print_neighbor_table(neighbors: &[Neighbor]) {
    println!("\ndiscovered {} neighbors:", neighbors.len());
    println!("{:<15} {:<17} {:<20} {:<10} {:<30}", "ip", "mac", "hostname", "packets", "protocols");
    println!("{}", "-".repeat(100));
    
    for neighbor in neighbors {
        let hostname = neighbor.hostname.as_deref().unwrap_or("unknown");
        let protocols = format_protocols(&neighbor.protocols);
        
        println!(
            "{:<15} {:<17} {:<20} {:<10} {:<30}", 
            neighbor.ip, 
            neighbor.mac, 
            hostname,
            neighbor.packet_count,
            protocols
        );
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn format_protocols(protocols: &HashMap<String, usize>) -> String {
    if protocols.is_empty() {
        "none".to_string()
    } else {
        protocols
            .iter()
            .map(|(proto, count)| format!("{}({})", proto, count))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn produce_json(matches: &clap::ArgMatches, options: StardustOutputOptions) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "inet", "bpf"])?;

    let (interface, duration, continuous) = extract_config(matches);
    let start = Instant::now();
    let neighbors = scan_neighbors(&interface, duration, continuous)?;
    let elapsed = start.elapsed().as_secs_f64();

    let result = ScanResult { 
        count: neighbors.len(),
        neighbors, 
        duration: elapsed,
    };

    let json = if options.pretty {
        serde_json::to_string_pretty(&result).unwrap()
    } else {
        serde_json::to_string(&result).unwrap()
    };

    println!("{}", json);
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn scan_neighbors(interface: &str, duration: u64, continuous: bool) -> UResult<Vec<Neighbor>> {
    let device = find_network_device(interface)?;
    let mut capture = open_packet_capture(device)?;
    
    let mut neighbors: HashMap<String, Neighbor> = HashMap::new();
    let start_time = Instant::now();
    let timeout = Duration::from_secs(duration);
    
    loop {
        match capture.next_packet() {
            Ok(packet) => {
                let timestamp = get_current_timestamp();
                
                if let Some((neighbor, traffic_type)) = parse_arp_packet(&packet) {
                    update_or_create_neighbor(&mut neighbors, neighbor, traffic_type, timestamp);
                } else if let Some((ip, mac, traffic_type)) = parse_ip_packet(&packet) {
                    let neighbor = create_empty_neighbor(ip, mac);
                    update_or_create_neighbor(&mut neighbors, neighbor, traffic_type, timestamp);
                }
            }
            Err(pcap::Error::TimeoutExpired) => {
                std::thread::sleep(Duration::from_millis(PACKET_SLEEP_MS));
            }
            Err(e) => {
                eprintln!("warning: error capturing packet: {}", e);
            }
        }
        
        if !continuous && start_time.elapsed() >= timeout {
            break;
        }
    }
    
    let mut result: Vec<Neighbor> = neighbors.into_values().collect();
    result.sort_by(|a, b| a.ip.cmp(&b.ip));
    
    Ok(result)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn get_current_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn create_empty_neighbor(ip: String, mac: String) -> Neighbor {
    Neighbor {
        ip,
        mac,
        hostname: None,
        first_seen: 0.0,
        last_seen: 0.0,
        packet_count: 0,
        protocols: HashMap::new(),
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn update_or_create_neighbor(
    neighbors: &mut HashMap<String, Neighbor>,
    neighbor: Neighbor,
    traffic_type: TrafficType,
    timestamp: f64,
) {
    let key = format!("{}:{}", neighbor.ip, neighbor.mac);
    
    neighbors
        .entry(key)
        .and_modify(|n| {
            n.last_seen = timestamp;
            n.packet_count += 1;
            *n.protocols.entry(traffic_type.to_string()).or_insert(0) += 1;
        })
        .or_insert_with(|| {
            let hostname = resolve_hostname(&neighbor.ip);
            let mut protocols = HashMap::new();
            protocols.insert(traffic_type.to_string(), 1);
            Neighbor {
                ip: neighbor.ip,
                mac: neighbor.mac,
                hostname,
                first_seen: timestamp,
                last_seen: timestamp,
                packet_count: 1,
                protocols,
            }
        });
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn find_network_device(interface: &str) -> UResult<Device> {
    let all_devices = Device::list()
        .map_err(|e| USimpleError::new(1, format!("unable to list devices: {}", e)))?;
    
    all_devices
        .into_iter()
        .find(|d| d.name == interface)
        .ok_or_else(|| USimpleError::new(1, format!("interface '{}' not found", interface)))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn open_packet_capture(device: Device) -> UResult<Capture<pcap::Active>> {
    Capture::from_device(device)
        .map_err(|e| USimpleError::new(1, format!("unable to create capture: {}", e)))?
        .promisc(true)
        .snaplen(65535)
        .timeout(100)
        .open()
        .map_err(|e| USimpleError::new(1, format!("failed to open device: {}", e)))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn parse_arp_packet(packet: &pcap::Packet) -> Option<(Neighbor, TrafficType)> {
    let data = packet.data;
    
    if data.len() < 42 {
        return None;
    }
    
    let ethertype = u16::from_be_bytes([data[12], data[13]]);
    if ethertype != 0x0806 {
        return None;
    }
    
    let arp = &data[14..];
    
    let hw_type = u16::from_be_bytes([arp[0], arp[1]]);
    if hw_type != 1 {
        return None;
    }
    
    let proto_type = u16::from_be_bytes([arp[2], arp[3]]);
    if proto_type != 0x0800 {
        return None;
    }
    
    if arp[4] != 6 || arp[5] != 4 {
        return None;
    }
    
    let sender_mac = format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        arp[8], arp[9], arp[10], arp[11], arp[12], arp[13]
    );
    
    let sender_ip = format!(
        "{}.{}.{}.{}",
        arp[14], arp[15], arp[16], arp[17]
    );
    
    if sender_ip == "0.0.0.0" || sender_mac == "00:00:00:00:00:00" {
        return None;
    }
    
    let neighbor = create_empty_neighbor(sender_ip, sender_mac);
    Some((neighbor, TrafficType::ARP))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn parse_ip_packet(packet: &pcap::Packet) -> Option<(String, String, TrafficType)> {
    let data = packet.data;
    
    if data.len() < 54 {
        return None;
    }
    
    let src_mac = format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        data[6], data[7], data[8], data[9], data[10], data[11]
    );
    
    let ethertype = u16::from_be_bytes([data[12], data[13]]);
    if ethertype != 0x0800 {
        return None;
    }
    
    let ip = &data[14..];
    
    let version = (ip[0] >> 4) & 0x0F;
    if version != 4 {
        return None;
    }
    
    let protocol = ip[9];
    if protocol != 6 {
        return None;
    }
    
    let src_ip = format!(
        "{}.{}.{}.{}",
        ip[12], ip[13], ip[14], ip[15]
    );
    
    if src_ip.starts_with("127.") || src_ip.starts_with("169.254.") {
        return None;
    }
    
    let ihl = (ip[0] & 0x0F) as usize * 4;
    
    if ip.len() < ihl + 4 {
        return None;
    }
    let tcp = &ip[ihl..];
    
    let dst_port = u16::from_be_bytes([tcp[2], tcp[3]]);
    
    let traffic_type = match dst_port {
        443 => TrafficType::HTTPS,
        25 | 587 => TrafficType::SMTP,
        110 | 995 => TrafficType::POP3,
        143 | 993 => TrafficType::IMAP,
        _ => return None,
    };
    
    Some((src_ip, src_mac, traffic_type))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn resolve_hostname(ip: &str) -> Option<String> {
    use std::process::Command;
    
    let output = Command::new("host")
        .arg("-W")
        .arg(HOSTNAME_TIMEOUT_SECS)
        .arg(ip)
        .output()
        .ok()?;
    
    if !output.status.success() {
        return None;
    }
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|line| line.contains("domain name pointer"))?
        .split("domain name pointer")
        .nth(1)?
        .trim()
        .trim_end_matches('.')
        .to_string()
        .into()
}
