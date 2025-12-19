// Copyright (c) 2025 Dmitry Kalashnikov.

use clap::{Arg, ArgMatches, Command as ClapCommand};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::process::Command as ProcessCommand;
use sgcore::{
    error::{SGResult, SGSimpleError},
    format_usage,
    stardust_output::{self, StardustOutputOptions},
};

#[cfg(target_os = "linux")]
use crate::manufacturers::{get_manufacturer_name, decode_manufacturer_data};
#[cfg(target_os = "linux")]
use crate::manufacturer_capability::detect_capabilities;
#[cfg(target_os = "linux")]
use crate::bluetooth_specs::{decode_service_uuid, decode_appearance, rssi_to_quality, 
                              estimate_distance, distance_to_string, distance_to_proximity};

static TIMEOUT_ARG: &str = "timeout";
static NO_VERBOSE_ARG: &str = "no-verbose";

const DEFAULT_TIMEOUT_SECONDS: &str = "10";
const MAX_TIMEOUT_SECONDS: u32 = 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManufacturerInfo {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decoded: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BluetoothDevice {
    name: String,
    address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    advertised_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tx_power: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rssi: Option<i16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signal_quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance_meters: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proximity: Option<String>,
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    services: std::collections::HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer: Option<ManufacturerInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_data: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    appearance: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    appearance_name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    capabilities: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScanResult {
    devices: Vec<BluetoothDevice>,
    count: usize,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err(SGSimpleError::new(
            1,
            "scan-bt is currently only supported on macos and linux".to_string(),
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
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
        .about("scan nearby bluetooth devices")
        .override_usage(format_usage("scan-bt [-t seconds] [--json]"))
        .infer_long_args(true)
        .arg(
            Arg::new(TIMEOUT_ARG)
                .short('t')
                .long("timeout")
                .value_name("SECONDS")
                .help("how long to scan")
                .value_parser(clap::value_parser!(u32))
                .default_value(DEFAULT_TIMEOUT_SECONDS),
        )
        .arg(
            Arg::new(NO_VERBOSE_ARG)
                .long("no-verbose")
                .help("omit raw manufacturer/service data in json output")
                .action(clap::ArgAction::SetTrue),
        );

    stardust_output::add_json_args(cmd)
}

#[cfg(target_os = "macos")]
fn produce(matches: &ArgMatches) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;

    let timeout: u32 = *matches.get_one::<u32>(TIMEOUT_ARG).unwrap();
    let _timeout = timeout.min(MAX_TIMEOUT_SECONDS);

    let devices = scan_bluetooth_macos()?;

    if devices.is_empty() {
        println!("no bluetooth devices found");
    } else {
        for device in devices {
            if let Some(device_type) = device.device_type {
                println!("{} ({}) - {}", device.name, device.address, device_type);
            } else {
                println!("{} ({})", device.name, device.address);
            }
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn produce_json(matches: &ArgMatches, options: StardustOutputOptions) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;

    let timeout: u32 = *matches.get_one::<u32>(TIMEOUT_ARG).unwrap();
    let _timeout = timeout.min(MAX_TIMEOUT_SECONDS);

    let result = match scan_bluetooth_macos() {
        Ok(devices) => {
            let count = devices.len();
            ScanResult {
                devices,
                count,
                success: true,
                error: None,
            }
        }
        Err(e) => ScanResult {
            devices: vec![],
            count: 0,
            success: false,
            error: Some(e.to_string()),
        },
    };

    let json = if options.pretty {
        serde_json::to_string_pretty(&result).unwrap()
    } else {
        serde_json::to_string(&result).unwrap()
    };

    println!("{}", json);
    Ok(())
}

#[cfg(target_os = "linux")]
fn produce(matches: &ArgMatches) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;

    let timeout: u32 = *matches.get_one::<u32>(TIMEOUT_ARG).unwrap();
    let timeout = timeout.min(MAX_TIMEOUT_SECONDS);

    let devices = scan_bluetooth_linux(timeout)?;

    if devices.is_empty() {
        println!("no bluetooth devices found");
    } else {
        let mut devices = devices;
        devices.sort_by(|a, b| {
            match (a.rssi, b.rssi) {
                (Some(rssi_a), Some(rssi_b)) => rssi_b.cmp(&rssi_a),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.address.cmp(&b.address),
            }
        });
        for device in devices {
            let mut parts = vec![format!("{} ({})", device.name, device.address)];

            if let Some(ref manufacturer) = device.manufacturer {
                parts.push(manufacturer.name.clone());
            }

            if let Some(rssi) = device.rssi {
                let mut signal_parts = vec![format!("{} dBm", rssi)];
                
                if let Some(ref quality) = device.signal_quality {
                    signal_parts.push(quality.clone());
                }
                
                if let Some(ref distance) = device.distance {
                    if let Some(ref proximity) = device.proximity {
                        signal_parts.push(format!("{} ({})", distance, proximity));
                    } else {
                        signal_parts.push(distance.clone());
                    }
                }
                
                parts.push(signal_parts.join(", "));
            }

            if !device.services.is_empty() {
                let service_names: Vec<&String> = device.services.values().collect();
                parts.push(format!("[Services: {}]", service_names.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
            } else if !device.capabilities.is_empty() {
                parts.push(format!("[{}]", device.capabilities.join(", ")));
            }

            println!("{}", parts.join(" - "));
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn produce_json(matches: &ArgMatches, options: StardustOutputOptions) -> SGResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;

    let timeout: u32 = *matches.get_one::<u32>(TIMEOUT_ARG).unwrap();
    let timeout = timeout.min(MAX_TIMEOUT_SECONDS);
    let verbose = !matches.get_flag(NO_VERBOSE_ARG);

    let mut result = match scan_bluetooth_linux(timeout) {
        Ok(devices) => {
            let count = devices.len();
            ScanResult {
                devices,
                count,
                success: true,
                error: None,
            }
        }
        Err(e) => ScanResult {
            devices: vec![],
            count: 0,
            success: false,
            error: Some(e.to_string()),
        },
    };

    if !verbose {
        for d in &mut result.devices {
            if let Some(ref mut mfg) = d.manufacturer {
                mfg.data = None;
                mfg.decoded = None;
            }
            d.service_data = None;
        }
    }

    result.devices.sort_by(|a, b| {
        match (a.rssi, b.rssi) {
            (Some(rssi_a), Some(rssi_b)) => rssi_b.cmp(&rssi_a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.address.cmp(&b.address),
        }
    });

    let json = if options.pretty {
        serde_json::to_string_pretty(&result).unwrap()
    } else {
        serde_json::to_string(&result).unwrap()
    };

    println!("{}", json);
    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_not_connected_devices(not_conn_array: &serde_json::Value) -> Vec<BluetoothDevice> {
    let mut devices = Vec::new();

    if let Some(array) = not_conn_array.as_array() {
        for device_wrapper in array {
            if let Some(obj) = device_wrapper.as_object() {
                for (key, device) in obj {
                    let name = key.to_string();

                    let address = device.get("device_address")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    devices.push(BluetoothDevice {
                        name,
                        address,
                        advertised_name: None,
                        device_type: None,
                        address_type: None,
                        tx_power: None,
                        rssi: None,
                        signal_quality: None,
                        distance_meters: None,
                        distance: None,
                        proximity: None,
                        services: std::collections::HashMap::new(),
                        manufacturer: None,
                        service_data: None,
                        appearance: None,
                        appearance_name: None,
                        capabilities: vec![],
                    });
                }
            }
        }
    }

    devices
}

#[cfg(target_os = "macos")]
fn parse_cached_devices(cache_array: &serde_json::Value) -> Vec<BluetoothDevice> {
    let mut devices = Vec::new();

    if let Some(array) = cache_array.as_array() {
        for device in array {
            let name = device.get("device_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let address = device.get("device_address")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let device_type = device.get("device_minorType")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            devices.push(BluetoothDevice {
                name,
                address,
                advertised_name: None,
                device_type,
                address_type: None,
                tx_power: None,
                rssi: None,
                signal_quality: None,
                distance_meters: None,
                distance: None,
                proximity: None,
                services: std::collections::HashMap::new(),
                manufacturer: None,
                service_data: None,
                appearance: None,
                appearance_name: None,
                capabilities: vec![],
            });
        }
    }

    devices
}

#[cfg(target_os = "macos")]
fn parse_connected_devices(conn_array: &serde_json::Value) -> Vec<BluetoothDevice> {
    let mut devices = Vec::new();

    if let Some(array) = conn_array.as_array() {
        for device in array {
            let name = device.get("device_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let address = device.get("device_address")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let device_type = device.get("device_minorType")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            devices.push(BluetoothDevice {
                name: format!("{} (connected)", name),
                address,
                advertised_name: None,
                device_type,
                address_type: None,
                tx_power: None,
                rssi: None,
                signal_quality: None,
                distance_meters: None,
                distance: None,
                proximity: None,
                services: std::collections::HashMap::new(),
                manufacturer: None,
                service_data: None,
                appearance: None,
                appearance_name: None,
                capabilities: vec![],
            });
        }
    }

    devices
}

#[cfg(target_os = "macos")]
fn parse_bluetooth_json(json_str: &str) -> Vec<BluetoothDevice> {
    let mut devices = Vec::new();

    let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return devices;
    };

    let Some(bt_data) = json.get("SPBluetoothDataType") else {
        return devices;
    };

    let Some(array) = bt_data.as_array() else {
        return devices;
    };

    for item in array {
        if let Some(device_not_connected) = item.get("device_not_connected") {
            devices.extend(parse_not_connected_devices(device_not_connected));
        }

        if let Some(device_cache) = item.get("device_cache") {
            devices.extend(parse_cached_devices(device_cache));
        }

        if let Some(connected) = item.get("device_connected") {
            devices.extend(parse_connected_devices(connected));
        }
    }

    devices
}

#[cfg(target_os = "macos")]
fn scan_bluetooth_macos() -> SGResult<Vec<BluetoothDevice>> {
    let output = ProcessCommand::new("system_profiler")
        .args(["SPBluetoothDataType", "-json"])
        .output();

    if !output.is_ok() || !output.as_ref().unwrap().status.success() {
        return Err(SGSimpleError::new(
            1,
            "failed to scan bluetooth devices".to_string(),
        ));
    }

    let output = output.unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let devices = parse_bluetooth_json(&stdout);

    Ok(devices)
}

#[cfg(target_os = "linux")]
fn get_rssi_via_btmon(timeout: u32) -> std::collections::HashMap<String, i16> {
    use std::process::{Command as ProcessCommand, Stdio};
    use std::io::{BufRead, BufReader};
    use std::thread;
    use std::time::{Duration, Instant};
    
    let mut rssi_map = std::collections::HashMap::new();
    
    let mut child = match ProcessCommand::new("btmon")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("note: btmon not available, try: sudo apt install bluez-tools");
                return rssi_map;
            }
        };
    
    let start = Instant::now();
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut current_address: Option<String> = None;
        
        for line in reader.lines().flatten() {
            if start.elapsed() > Duration::from_secs(timeout as u64) {
                break;
            }
            
            let line = line.trim();
            
            if line.starts_with("Address: ") {
                if let Some(addr_part) = line.split_whitespace().nth(1) {
                    current_address = Some(addr_part.to_uppercase());
                }
            }
            
            if line.starts_with("RSSI: ") {
                if let Some(ref addr) = current_address {
                    if let Some(rssi_str) = line.split_whitespace().nth(1) {
                        if let Ok(rssi) = rssi_str.parse::<i16>() {
                            rssi_map.insert(addr.clone(), rssi);
                        }
                    }
                }
            }
        }
    }
    
    let _ = child.kill();
    rssi_map
}

#[cfg(target_os = "linux")]
fn scan_bluetooth_linux(timeout_secs: u32) -> SGResult<Vec<BluetoothDevice>> {
    use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter};
    use btleplug::platform::Manager;
    use std::collections::HashMap;
    use futures::stream::StreamExt;

    eprintln!("scanning for bluetooth devices ({}s)...", timeout_secs);
    
    let btmon_timeout = timeout_secs;
    let btmon_handle = std::thread::spawn(move || {
        get_rssi_via_btmon(btmon_timeout)
    });

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| SGSimpleError::new(1, format!("failed to create async runtime: {}", e)))?;

    let devices = runtime.block_on(async {
        let manager = match Manager::new().await {
            Ok(m) => m,
            Err(e) => {
                eprintln!("failed to initialize bluetooth manager: {}. ensure bluetooth is enabled", e);
                return Vec::new();
            }
        };

        let adapters = match manager.adapters().await {
            Ok(a) => a,
            Err(e) => {
                eprintln!("failed to get bluetooth adapters: {}", e);
                return Vec::new();
            }
        };

        if adapters.is_empty() {
            eprintln!("no bluetooth adapters found");
            return Vec::new();
        }

        let adapter = adapters[0].clone();
        
        let mut events = match adapter.events().await {
            Ok(e) => e,
            Err(e) => {
                eprintln!("failed to subscribe to events: {}", e);
                return Vec::new();
            }
        };
        
        if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
            eprintln!("failed to start scan: {}", e);
            return Vec::new();
        }

        let adapter_clone = adapter.clone();
        let scan_task = tokio::spawn(async move {
            let mut discovered = HashMap::new();
            while let Some(event) = events.next().await {
                match event {
                    CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => {
                        if let Ok(peripherals) = adapter_clone.peripherals().await {
                            for p in peripherals {
                                if p.id() == id {
                                    if let Ok(Some(props)) = p.properties().await {
                                        if let Some(rssi) = props.rssi {
                                            discovered.insert(p.address().to_string(), rssi);
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            discovered
        });

        // Poll for devices multiple times during scan to capture fresh RSSI
        let mut all_peripherals = Vec::new();
        let poll_interval = 2; // Poll every 2 seconds
        let polls = (timeout_secs / poll_interval).max(1);
        
        for i in 0..polls {
            tokio::time::sleep(tokio::time::Duration::from_secs(poll_interval as u64)).await;
            if let Ok(peripherals) = adapter.peripherals().await {
                eprintln!("poll {}/{}: found {} devices", i + 1, polls, peripherals.len());
                all_peripherals = peripherals;
            }
        }

        let peripherals = all_peripherals;


        let rssi_from_events = match tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            scan_task
        ).await {
            Ok(Ok(map)) => {
                eprintln!("captured {} RSSI values from events", map.len());
                map
            },
            _ => HashMap::new(),
        };
        
        adapter.stop_scan().await.ok();
        
        let mut out = Vec::new();
        for peripheral in peripherals {
            let properties = peripheral.properties().await.ok().flatten();
            if let Some(props) = properties {
                let advertised_name = props.local_name.clone();
                let mut name = props.local_name.unwrap_or_else(|| "unknown".to_string());
                let address = props.address.to_string();
                let address_type = props.address_type.as_ref().map(|t| format!("{:?}", t));
                let tx_power = props.tx_power_level.map(|v| v as i8);
                let services = props.services
                    .into_iter()
                    .map(|u| u.to_string())
                    .collect::<Vec<String>>();

                let manufacturer_data = {
                    let mut map = std::collections::HashMap::new();
                    for (id, bytes) in props.manufacturer_data.into_iter() {
                        let hex = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
                        map.insert(format!("{:04x}", id), hex);
                    }
                    if map.is_empty() { None } else { Some(map) }
                };

                let service_data = {
                    let mut map = std::collections::HashMap::new();
                    for (uuid, bytes) in props.service_data.into_iter() {
                        let hex = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
                        map.insert(uuid.to_string(), hex);
                    }
                    if map.is_empty() { None } else { Some(map) }
                };

                let manufacturer = if let Some(ref m) = manufacturer_data {
                    if let Some((id_str, data_hex)) = m.iter().next() {
                        if let Ok(company_id) = u16::from_str_radix(id_str, 16) {
                            let mfg_name = get_manufacturer_name(company_id)
                                .unwrap_or("Unknown")
                                .to_string();
                            let decoded = decode_manufacturer_data(company_id, data_hex);

                            if name == "unknown" {
                                name = format!("{} device", mfg_name.to_lowercase());
                            }

                            Some(ManufacturerInfo {
                                id: id_str.clone(),
                                name: mfg_name,
                                data: Some(data_hex.clone()),
                                decoded,
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                let rssi = rssi_from_events.get(&address)
                    .copied()
                    .or(props.rssi);
                let signal_quality = rssi.map(|r| rssi_to_quality(r).to_string());
                
                let distance_meters = rssi.map(|r| estimate_distance(r, tx_power));
                let distance = distance_meters.map(distance_to_string);
                let proximity = distance_meters.map(distance_to_proximity).map(|s| s.to_string());
                
                let appearance = None;
                let appearance_name = appearance.and_then(decode_appearance);
                
                let services_map: std::collections::HashMap<String, String> = services.iter()
                    .map(|uuid| {
                        let decoded = decode_service_uuid(uuid)
                            .unwrap_or_else(|| "Unknown".to_string());
                        (uuid.clone(), decoded)
                    })
                    .collect();

                let capabilities = detect_capabilities(&services, tx_power);

                out.push(BluetoothDevice {
                    name,
                    address,
                    advertised_name,
                    device_type: None,
                    address_type,
                    tx_power,
                    rssi,
                    signal_quality,
                    distance_meters,
                    distance,
                    proximity,
                    services: services_map,
                    manufacturer,
                    service_data,
                    appearance,
                    appearance_name,
                    capabilities,
                });
            }
        }

        out
    });

    let rssi_from_btmon = btmon_handle.join().unwrap_or_default();
    eprintln!("btmon captured {} RSSI values", rssi_from_btmon.len());
    
    let mut enriched_devices = Vec::new();
    for mut device in devices {
        if device.rssi.is_none() {
            if let Some(&rssi) = rssi_from_btmon.get(&device.address.to_uppercase()) {
                device.rssi = Some(rssi);
                device.signal_quality = Some(rssi_to_quality(rssi).to_string());
                let distance_meters = estimate_distance(rssi, device.tx_power);
                device.distance_meters = Some(distance_meters);
                device.distance = Some(distance_to_string(distance_meters));
                device.proximity = Some(distance_to_proximity(distance_meters).to_string());
            }
        }
        enriched_devices.push(device);
    }
    
    let devices = enriched_devices;

    if devices.is_empty() {
        eprintln!("try: sudo systemctl start bluetooth");
    } else {
        let rssi_count = devices.iter().filter(|d| d.rssi.is_some()).count();
        if rssi_count == 0 {
            eprintln!("note: no RSSI values available (bluetooth hardware/driver limitation)");
            eprintln!("      RSSI enables distance estimation and signal quality reporting");
        } else {
            eprintln!("captured RSSI for {} of {} devices", rssi_count, devices.len());
        }
    }

    Ok(devices)
}

