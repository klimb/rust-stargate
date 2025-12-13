// Copyright (C) 2025 Dmitry Kalashnikov

use clap::{Arg, ArgMatches, Command as ClapCommand};
use serde::{Deserialize, Serialize};
use sgcore::{
    error::{UResult, USimpleError},
    format_usage,
    stardust_output::{self, StardustOutputOptions},
    translate,
};
use std::process::Command as ProcessCommand;

static NETWORK_ARG: &str = "network";

#[derive(Debug, Serialize, Deserialize)]
struct Host {
    ip: String,
    mac: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScanResult {
    hosts: Vec<Host>,
    count: usize,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err(USimpleError::new(
            1,
            "scan-arp is currently only supported on macos".to_string(),
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
        let object_output = StardustOutputOptions::from_matches(&matches);

        if object_output.object_output {
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
        .about(translate!("scan-arp-about"))
        .override_usage(format_usage(&translate!("scan-arp-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(NETWORK_ARG)
                .short('n')
                .long("network")
                .value_name("CIDR")
                .help(translate!("scan-arp-help-network")),
        );

    stardust_output::add_json_args(cmd)
}

#[cfg(target_os = "macos")]
fn produce(matches: &ArgMatches) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;

    let hosts = scan_network_macos(matches)?;

    for host in &hosts {
        if let Some(hostname) = &host.hostname {
            println!("{} {} {}", host.ip, host.mac, hostname);
        } else {
            println!("{} {}", host.ip, host.mac);
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn produce_json(matches: &ArgMatches, options: StardustOutputOptions) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;

    let hosts = scan_network_macos(matches)?;
    let count = hosts.len();

    let result = ScanResult { hosts, count };

    let json = if options.pretty {
        serde_json::to_string_pretty(&result).unwrap()
    } else {
        serde_json::to_string(&result).unwrap()
    };

    println!("{}", json);
    Ok(())
}

#[cfg(target_os = "macos")]
fn scan_network_macos(matches: &ArgMatches) -> UResult<Vec<Host>> {
    let network = matches.get_one::<String>(NETWORK_ARG);

    if let Some(cidr) = network {
        scan_cidr_range(cidr)
    } else {
        scan_arp_cache()
    }
}

#[cfg(target_os = "macos")]
fn scan_arp_cache() -> UResult<Vec<Host>> {
    let output = ProcessCommand::new("arp")
        .arg("-a")
        .output()
        .map_err(|e| USimpleError::new(1, format!("failed to run arp: {}", e)))?;

    if !output.status.success() {
        return Err(USimpleError::new(1, "arp command failed".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut hosts = Vec::new();

    for line in stdout.lines() {
        if let Some(host) = parse_arp_line(line) {
            hosts.push(host);
        }
    }

    Ok(hosts)
}

#[cfg(target_os = "macos")]
fn parse_arp_line(line: &str) -> Option<Host> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    if parts.len() < 4 {
        return None;
    }

    let hostname = if parts[0].starts_with('?') {
        None
    } else {
        Some(parts[0].trim_matches(|c| c == '(' || c == ')').to_string())
    };

    let ip = parts[1]
        .trim_matches(|c| c == '(' || c == ')')
        .to_string();

    let mac = parts[3].to_string();

    if mac == "(incomplete)" || ip.is_empty() {
        return None;
    }

    Some(Host {
        ip,
        mac,
        hostname,
    })
}

#[cfg(target_os = "macos")]
fn scan_cidr_range(cidr: &str) -> UResult<Vec<Host>> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return Err(USimpleError::new(1, "invalid cidr format".to_string()));
    }

    let base_ip = parts[0];
    let prefix: u32 = parts[1]
        .parse()
        .map_err(|_| USimpleError::new(1, "invalid prefix length".to_string()))?;

    if prefix > 30 || prefix < 8 {
        return Err(USimpleError::new(
            1,
            "prefix must be between 8 and 30".to_string(),
        ));
    }

    let ip_parts: Vec<u32> = base_ip
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    if ip_parts.len() != 4 {
        return Err(USimpleError::new(1, "invalid ip address".to_string()));
    }

    let base_addr = (ip_parts[0] << 24) | (ip_parts[1] << 16) | (ip_parts[2] << 8) | ip_parts[3];
    let host_count = 1u32 << (32 - prefix);
    let network_addr = base_addr & (u32::MAX << (32 - prefix));

    for i in 1..host_count.min(254) {
        let addr = network_addr | i;
        let ip = format!(
            "{}.{}.{}.{}",
            (addr >> 24) & 0xFF,
            (addr >> 16) & 0xFF,
            (addr >> 8) & 0xFF,
            addr & 0xFF
        );

        let _ = ProcessCommand::new("ping")
            .args(["-c", "1", "-W", "100", &ip])
            .output();
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    scan_arp_cache()
}
