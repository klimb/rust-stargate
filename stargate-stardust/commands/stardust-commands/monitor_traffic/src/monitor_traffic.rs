

use clap::{Arg, ArgAction, Command};
use sgcore::error::SGResult;
use sgcore::format_usage;
use sgcore::translate;
use sgcore::stardust_output::{self, StardustOutputOptions};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
use pcap::{Capture, Device, Packet};

static ARG_INTERFACE: &str = "interface";
static ARG_COUNT: &str = "count";
static ARG_VERBOSE: &str = "verbose";

const DEFAULT_INTERFACE_MACOS: &str = "en0";

#[derive(Debug, Clone)]
struct PacketInfo {
    timestamp: f64,
    interface: String,
    protocol: String,
    src_addr: String,
    dst_addr: String,
    length: usize,
    summary: String,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;

    #[cfg(target_os = "macos")]
    check_root_privileges()?;

    sgcore::pledge::apply_pledge(&["stdio", "inet", "rpath", "bpf"])?;
    let opts = StardustOutputOptions::from_matches(&matches);

    let interface = matches.get_one::<String>(ARG_INTERFACE)
        .map(|s| s.as_str())
        .unwrap_or(DEFAULT_INTERFACE_MACOS);
    let count = matches.get_one::<usize>(ARG_COUNT).copied();
    let verbose = matches.get_flag(ARG_VERBOSE);

    if !opts.stardust_output {
        eprintln!("Monitoring traffic on interface: {}", interface);
        eprintln!("Press Ctrl+C to stop...");
        eprintln!();
    }

    let packets = capture_packets(interface, count, verbose)?;

    if opts.stardust_output {
        output_json(&packets, opts)?;
    } else {
        output_text(&packets, verbose);
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn check_root_privileges() -> SGResult<()> {
    let current_uid = unsafe { libc::getuid() };
    let is_root = current_uid == 0;

    if !is_root {
        return Err(sgcore::error::SGSimpleError::new(
            1,
            "This command requires root privileges. Please run it with sudo on your mac.".to_string()
        ));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn find_network_device(interface: &str) -> SGResult<Device> {
    if interface == "any" {
        let default_device = Device::lookup()
            .map_err(|e| sgcore::error::SGSimpleError::new(1, format!("failed to locate the default device: {}", e)))?
            .ok_or_else(|| sgcore::error::SGSimpleError::new(1, "no network devices found".to_string()))?;
        return Ok(default_device);
    }

    let all_devices = Device::list()
        .map_err(|e| sgcore::error::SGSimpleError::new(1, format!("unable to list devices: {}", e)))?;

    all_devices
        .into_iter()
        .find(|d| d.name == interface)
        .ok_or_else(|| sgcore::error::SGSimpleError::new(1, format!("interface '{}' not found", interface)))
}

#[cfg(target_os = "macos")]
fn open_packet_capture(device: Device) -> SGResult<Capture<pcap::Active>> {
    let max_packet_size = 65535;
    let promiscuous_mode = true;

    Capture::from_device(device)
        .map_err(|e| sgcore::error::SGSimpleError::new(1, format!("unable to capture: {}", e)))?
        .promisc(promiscuous_mode)
        .snaplen(max_packet_size)
        .open()
        .map_err(|e| sgcore::error::SGSimpleError::new(1, format!("failed to open device: {}", e)))?
        .setnonblock()
        .map_err(|e| sgcore::error::SGSimpleError::new(1, format!("failed to set non-blocking mode: {}", e)))
}

#[cfg(target_os = "macos")]
fn capture_packets(interface: &str, max_count: Option<usize>, _verbose: bool) -> SGResult<Vec<PacketInfo>> {
    let device = find_network_device(interface)?;
    let mut capture = open_packet_capture(device)?;

    let default_packet_count = 10;
    let target_count = max_count.unwrap_or(default_packet_count);
    let mut packets = Vec::new();
    let mut captured_count = 0;

    while captured_count < target_count {
        match capture.next_packet() {
            Ok(packet) => {
                let current_timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);

                let packet_info = parse_packet(&packet, interface, current_timestamp);
                packets.push(packet_info);
                captured_count += 1;
            }
            Err(pcap::Error::TimeoutExpired) => {
                let retry_delay = std::time::Duration::from_millis(10);
                std::thread::sleep(retry_delay);
                continue;
            }
            Err(e) => {
                return Err(sgcore::error::SGSimpleError::new(1, format!("error capturing packet: {}", e)));
            }
        }
    }

    Ok(packets)
}

#[cfg(target_os = "macos")]
fn parse_packet(packet: &Packet, interface: &str, timestamp: f64) -> PacketInfo {
    let data = packet.data;
    let length = data.len();
    let minimum_ethernet_frame_size = 14;

    if length < minimum_ethernet_frame_size {
        return PacketInfo {
            timestamp,
            interface: interface.to_string(),
            protocol: "UNKNOWN".to_string(),
            src_addr: "N/A".to_string(),
            dst_addr: "N/A".to_string(),
            length,
            summary: "Packet too short".to_string(),
        };
    }

    let ethertype_offset = 12;
    let ethertype = u16::from_be_bytes([data[ethertype_offset], data[ethertype_offset + 1]]);

    const ETHERTYPE_IPV4: u16 = 0x0800;
    const ETHERTYPE_IPV6: u16 = 0x86DD;
    const ETHERTYPE_ARP: u16 = 0x0806;

    match ethertype {
        ETHERTYPE_IPV4 => parse_ipv4_packet(data, interface, timestamp, length),
        ETHERTYPE_IPV6 => parse_ipv6_packet(data, interface, timestamp, length),
        ETHERTYPE_ARP => PacketInfo {
            timestamp,
            interface: interface.to_string(),
            protocol: "ARP".to_string(),
            src_addr: "N/A".to_string(),
            dst_addr: "N/A".to_string(),
            length,
            summary: "ARP packet".to_string(),
        },
        _ => PacketInfo {
            timestamp,
            interface: interface.to_string(),
            protocol: format!("0x{:04X}", ethertype),
            src_addr: "N/A".to_string(),
            dst_addr: "N/A".to_string(),
            length,
            summary: format!("EtherType: 0x{:04X}", ethertype),
        },
    }
}

#[cfg(target_os = "macos")]
fn parse_ipv4_packet(data: &[u8], interface: &str, timestamp: f64, length: usize) -> PacketInfo {
    const MINIMUM_IPV4_PACKET_SIZE: usize = 34;

    if data.len() < MINIMUM_IPV4_PACKET_SIZE {
        return PacketInfo {
            timestamp,
            interface: interface.to_string(),
            protocol: "IPv4".to_string(),
            src_addr: "N/A".to_string(),
            dst_addr: "N/A".to_string(),
            length,
            summary: "IPv4 packet too short".to_string(),
        };
    }

    const ETHERNET_HEADER_SIZE: usize = 14;
    const IP_PROTOCOL_OFFSET: usize = 9;
    const IP_SOURCE_OFFSET: usize = 12;
    const IP_DEST_OFFSET: usize = 16;

    let ip_header = &data[ETHERNET_HEADER_SIZE..];
    let protocol = ip_header[IP_PROTOCOL_OFFSET];
    let src_ip = format!("{}.{}.{}.{}",
        ip_header[IP_SOURCE_OFFSET], ip_header[IP_SOURCE_OFFSET + 1],
        ip_header[IP_SOURCE_OFFSET + 2], ip_header[IP_SOURCE_OFFSET + 3]);
    let dst_ip = format!("{}.{}.{}.{}",
        ip_header[IP_DEST_OFFSET], ip_header[IP_DEST_OFFSET + 1],
        ip_header[IP_DEST_OFFSET + 2], ip_header[IP_DEST_OFFSET + 3]);

    const PROTOCOL_ICMP: u8 = 1;
    const PROTOCOL_TCP: u8 = 6;
    const PROTOCOL_UDP: u8 = 17;

    let (protocol_name, summary) = match protocol {
        PROTOCOL_ICMP => ("ICMP".to_string(), format!("{} → {} ICMP", src_ip, dst_ip)),
        PROTOCOL_TCP => {
            let ip_header_length_mask = 0x0F;
            let words_to_bytes = 4;
            let ip_header_length = (ip_header[0] & ip_header_length_mask) as usize * words_to_bytes;
            let minimum_tcp_header_size = 4;

            if ip_header.len() >= ip_header_length + minimum_tcp_header_size {
                let tcp_header = &ip_header[ip_header_length..];
                let src_port = u16::from_be_bytes([tcp_header[0], tcp_header[1]]);
                let dst_port = u16::from_be_bytes([tcp_header[2], tcp_header[3]]);
                ("TCP".to_string(), format!("{}:{} → {}:{}", src_ip, src_port, dst_ip, dst_port))
            } else {
                ("TCP".to_string(), format!("{} → {} TCP", src_ip, dst_ip))
            }
        }
        PROTOCOL_UDP => {
            let ip_header_length_mask = 0x0F;
            let words_to_bytes = 4;
            let ip_header_length = (ip_header[0] & ip_header_length_mask) as usize * words_to_bytes;
            let minimum_udp_header_size = 4;

            if ip_header.len() >= ip_header_length + minimum_udp_header_size {
                let udp_header = &ip_header[ip_header_length..];
                let src_port = u16::from_be_bytes([udp_header[0], udp_header[1]]);
                let dst_port = u16::from_be_bytes([udp_header[2], udp_header[3]]);
                ("UDP".to_string(), format!("{}:{} → {}:{}", src_ip, src_port, dst_ip, dst_port))
            } else {
                ("UDP".to_string(), format!("{} → {} UDP", src_ip, dst_ip))
            }
        }
        _ => (format!("IP({})", protocol), format!("{} → {} Protocol {}", src_ip, dst_ip, protocol)),
    };

    PacketInfo {
        timestamp,
        interface: interface.to_string(),
        protocol: protocol_name,
        src_addr: src_ip.clone(),
        dst_addr: dst_ip.clone(),
        length,
        summary,
    }
}

#[cfg(target_os = "macos")]
fn format_ipv6_address_abbreviated(addr_bytes: &[u8]) -> String {
    format!("{}:{:02x}{:02x}:...:{:02x}{:02x}",
        u16::from_be_bytes([addr_bytes[0], addr_bytes[1]]),
        addr_bytes[2], addr_bytes[3],
        addr_bytes[14], addr_bytes[15])
}

#[cfg(target_os = "macos")]
fn parse_ipv6_packet(data: &[u8], interface: &str, timestamp: f64, length: usize) -> PacketInfo {
    const MINIMUM_IPV6_PACKET_SIZE: usize = 54;

    if data.len() < MINIMUM_IPV6_PACKET_SIZE {
        return PacketInfo {
            timestamp,
            interface: interface.to_string(),
            protocol: "IPv6".to_string(),
            src_addr: "N/A".to_string(),
            dst_addr: "N/A".to_string(),
            length,
            summary: "IPv6 packet too short".to_string(),
        };
    }

    const ETHERNET_HEADER_SIZE: usize = 14;
    const IPV6_NEXT_HEADER_OFFSET: usize = 6;

    let ip_header = &data[ETHERNET_HEADER_SIZE..];
    let next_header = ip_header[IPV6_NEXT_HEADER_OFFSET];

    let src_ip = format_ipv6_address_abbreviated(&ip_header[8..24]);
    let dst_ip = format_ipv6_address_abbreviated(&ip_header[24..40]);

    const IPV6_PROTOCOL_TCP: u8 = 6;
    const IPV6_PROTOCOL_UDP: u8 = 17;
    const IPV6_PROTOCOL_ICMPV6: u8 = 58;

    let protocol_name = match next_header {
        IPV6_PROTOCOL_TCP => "TCP".to_string(),
        IPV6_PROTOCOL_UDP => "UDP".to_string(),
        IPV6_PROTOCOL_ICMPV6 => "ICMPv6".to_string(),
        _ => format!("IPv6({})", next_header),
    };

    PacketInfo {
        timestamp,
        interface: interface.to_string(),
        protocol: protocol_name,
        src_addr: src_ip.clone(),
        dst_addr: dst_ip.clone(),
        length,
        summary: format!("{} → {}", src_ip, dst_ip),
    }
}

#[cfg(not(target_os = "macos"))]
fn capture_packets(_interface: &str, _max_count: Option<usize>, _verbose: bool) -> SGResult<Vec<PacketInfo>> {
    Err(sgcore::error::SGSimpleError::new(
        1,
        "monitor-traffic is currently only supported on macOS".to_string()
    ))
}

fn output_json(packets: &[PacketInfo], opts: StardustOutputOptions) -> SGResult<()> {
    let packet_list: Vec<_> = packets.iter().map(|p| {
        json!({
            "timestamp": p.timestamp,
            "interface": p.interface,
            "protocol": p.protocol,
            "source": p.src_addr,
            "destination": p.dst_addr,
            "length": p.length,
            "summary": p.summary
        })
    }).collect();

    let output = json!({
        "packets": packet_list,
        "count": packets.len()
    });

    stardust_output::output(opts, output, || Ok(()))?;
    Ok(())
}

fn output_text(packets: &[PacketInfo], verbose: bool) {
    println!("TIME\t\tPROTO\tSOURCE\t\t\tDESTINATION\t\tLEN");
    println!("{}", "=".repeat(80));

    for packet in packets {
        let time = format_timestamp(packet.timestamp);

        if verbose {
            println!("{}\t{}\t{}\t{}\t{}",
                time, packet.protocol, packet.src_addr, packet.dst_addr, packet.length);
            println!("  Interface: {} | {}", packet.interface, packet.summary);
            println!();
        } else {
            println!("{}\t{}\t{}\t{}\t{}",
                time, packet.protocol, packet.src_addr, packet.dst_addr, packet.length);
        }
    }

    println!("\nTotal packets captured: {}", packets.len());
}

fn format_timestamp(timestamp: f64) -> String {
    let decimal_places = 3;
    format!("{:.precision$}", timestamp, precision = decimal_places)
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("monitor-traffic-about"))
        .override_usage(format_usage(&translate!("monitor-traffic-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(ARG_INTERFACE)
                .short('i')
                .long("interface")
                .value_name("INTERFACE")
                .help(translate!("monitor-traffic-help-interface"))
                .default_value(DEFAULT_INTERFACE_MACOS)
        )
        .arg(
            Arg::new(ARG_COUNT)
                .short('c')
                .long("count")
                .value_name("NUM")
                .help(translate!("monitor-traffic-help-count"))
                .value_parser(clap::value_parser!(usize))
        )
        .arg(
            Arg::new(ARG_VERBOSE)
                .short('v')
                .long("verbose")
                .help(translate!("monitor-traffic-help-verbose"))
                .action(ArgAction::SetTrue)
        );

    stardust_output::add_json_args(cmd)
}

