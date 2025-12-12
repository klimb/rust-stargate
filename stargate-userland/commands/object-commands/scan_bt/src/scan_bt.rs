// Copyright (C) 2025 Dmitry Kalashnikov

use clap::{Arg, ArgMatches, Command as ClapCommand};
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command as ProcessCommand;
use sgcore::{
    error::{UResult, USimpleError},
    format_usage,
    object_output::{self, JsonOutputOptions},
};

static TIMEOUT_ARG: &str = "timeout";
static NO_VERBOSE_ARG: &str = "no-verbose";

const DEFAULT_TIMEOUT_SECONDS: &str = "10";
const MAX_TIMEOUT_SECONDS: u32 = 60;

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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    services: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer_data: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_data: Option<std::collections::HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    appearance: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    likely_iphone: Option<bool>,
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
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        return Err(USimpleError::new(
            1,
            "scan-bt is currently only supported on macos and linux".to_string(),
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
        let object_output = JsonOutputOptions::from_matches(&matches);

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

    object_output::add_json_args(cmd)
}

#[cfg(target_os = "macos")]
fn produce(matches: &ArgMatches) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;
    
    let timeout: u32 = *matches.get_one::<u32>(TIMEOUT_ARG).unwrap();
    let _timeout = timeout.min(MAX_TIMEOUT_SECONDS);
    
    let devices = scan_bluetooth_macos()?;
    
    if devices.is_empty() {
        println!("no bluetooth devices found");
    } else {
        for device in devices {
            if let Some(rssi) = device.rssi {
                println!("{} ({}) - rssi: {}", device.name, device.address, rssi);
            } else if let Some(device_type) = device.device_type {
                println!("{} ({}) - {}", device.name, device.address, device_type);
            } else {
                println!("{} ({})", device.name, device.address);
            }
        }
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
fn produce_json(matches: &ArgMatches, options: JsonOutputOptions) -> UResult<()> {
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
fn produce(matches: &ArgMatches) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;
    
    let timeout: u32 = *matches.get_one::<u32>(TIMEOUT_ARG).unwrap();
    let timeout = timeout.min(MAX_TIMEOUT_SECONDS);
    
    let devices = scan_bluetooth_linux(timeout)?;
    
    if devices.is_empty() {
        println!("no bluetooth devices found");
    } else {
        let mut devices = devices;
        devices.sort_by(|a, b| a.address.cmp(&b.address));
        for device in devices {
            println!("{} ({})", device.name, device.address);
        }
    }
    
    Ok(())
}

#[cfg(target_os = "linux")]
fn produce_json(matches: &ArgMatches, options: JsonOutputOptions) -> UResult<()> {
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
            d.manufacturer_data = None;
            d.service_data = None;
        }
    }

    result.devices.sort_by(|a, b| a.address.cmp(&b.address));
    
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
                    
                    let rssi = device.get("device_rssi")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<i32>().ok());
                    
                    devices.push(BluetoothDevice {
                        name,
                        address,
                        rssi,
                        device_type: None,
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
                rssi: None,
                device_type,
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
                rssi: None,
                device_type,
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
fn scan_bluetooth_macos() -> UResult<Vec<BluetoothDevice>> {
    let output = ProcessCommand::new("system_profiler")
        .args(["SPBluetoothDataType", "-json"])
        .output();

    if !output.is_ok() || !output.as_ref().unwrap().status.success() {
        return Err(USimpleError::new(
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
fn scan_bluetooth_linux(timeout: u32) -> UResult<Vec<BluetoothDevice>> {
    use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
    use btleplug::platform::Manager;
    use uuid::Uuid;
    
    eprintln!("scanning for bluetooth devices ({}s)...", timeout);
    
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| USimpleError::new(1, format!("failed to create async runtime: {}", e)))?;
    
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

        let adapter = &adapters[0];
        if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
            eprintln!("failed to start scan: {}", e);
            return Vec::new();
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(timeout as u64)).await;

        let peripherals = match adapter.peripherals().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("failed to get devices: {}", e);
                return Vec::new();
            }
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

                let mut likely_iphone = None;
                if let Some(ref m) = manufacturer_data {
                    if m.contains_key("004c") {
                        if name == "unknown" {
                            name = "apple device".to_string();
                        }
                        use btleplug::api::AddressType;
                        let is_random_addr = matches!(props.address_type, Some(AddressType::Random));
                        likely_iphone = Some(is_random_addr);
                    }
                }

                out.push(BluetoothDevice {
                    name,
                    address,
                    advertised_name,
                    device_type: None,
                    address_type,
                    tx_power,
                    services,
                    manufacturer_data,
                    service_data,
                    appearance: None,
                    likely_iphone,
                });
            }
        }

        out
    });
    
    if devices.is_empty() {
        eprintln!("try: sudo systemctl start bluetooth");
    }
    
    Ok(devices)
}
