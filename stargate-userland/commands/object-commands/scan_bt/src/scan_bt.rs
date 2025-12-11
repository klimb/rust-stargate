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

const DEFAULT_TIMEOUT_SECONDS: &str = "10";
const MAX_TIMEOUT_SECONDS: u32 = 60;

#[derive(Debug, Serialize, Deserialize)]
struct BluetoothDevice {
    name: String,
    address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rssi: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_type: Option<String>,
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
            "scan-bt is only available on macos and linux".to_string(),
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
        .about("scan for nearby bluetooth devices")
        .override_usage(format_usage("scan-bt [options]"))
        .infer_long_args(true)
        .arg(
            Arg::new(TIMEOUT_ARG)
                .short('t')
                .long("timeout")
                .value_name("seconds")
                .help(format!("scan timeout in seconds (max {})", MAX_TIMEOUT_SECONDS))
                .default_value(DEFAULT_TIMEOUT_SECONDS)
                .value_parser(clap::value_parser!(u32)),
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
        for device in devices {
            if let Some(rssi) = device.rssi {
                println!("{} ({}) - rssi: {}", device.name, device.address, rssi);
            } else {
                println!("{} ({})", device.name, device.address);
            }
        }
    }
    
    Ok(())
}

#[cfg(target_os = "linux")]
fn produce_json(matches: &ArgMatches, options: JsonOutputOptions) -> UResult<()> {
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc", "exec"])?;
    
    let timeout: u32 = *matches.get_one::<u32>(TIMEOUT_ARG).unwrap();
    let timeout = timeout.min(MAX_TIMEOUT_SECONDS);
    
    let result = match scan_bluetooth_linux(timeout) {
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
    let scan_output = ProcessCommand::new("timeout")
        .args([
            &timeout.to_string(),
            "bluetoothctl",
            "scan", "on"
        ])
        .output();

    std::thread::sleep(std::time::Duration::from_secs(1));

    let devices_output = ProcessCommand::new("bluetoothctl")
        .args(["devices"])
        .output();

    let _ = ProcessCommand::new("bluetoothctl")
        .args(["scan", "off"])
        .output();

    if !scan_output.is_ok() && !devices_output.is_ok() {
        return Err(USimpleError::new(
            1,
            "failed to scan bluetooth devices. ensure bluetoothctl is installed".to_string(),
        ));
    }

    let mut devices = Vec::new();

    if let Ok(output) = devices_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.starts_with("Device ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let address = parts[1].to_string();
                    let name = parts[2..].join(" ");
                    
                    devices.push(BluetoothDevice {
                        name,
                        address,
                        rssi: None,
                        device_type: None,
                    });
                }
            }
        }
    }

    Ok(devices)
}
