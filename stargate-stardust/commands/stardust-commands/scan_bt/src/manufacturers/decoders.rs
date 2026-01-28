// Copyright (c) 2025 Dmitry Kalashnikov

use std::collections::HashMap;
use std::sync::OnceLock;

const APPLE_CONTINUITY_TYPES: &[(u8, &str)] = &[
    (0x01, "Apple Overflow Area"),
    (0x02, "iBeacon"),
    (0x03, "AirPrint"),
    (0x05, "AirDrop"),
    (0x06, "HomeKit"),
    (0x07, "AirPods/Proximity Pairing"),
    (0x08, "Hey Siri"),
    (0x09, "AirPlay Target"),
    (0x0A, "AirPlay Source"),
    (0x0B, "Magic Switch"),
    (0x0C, "Handoff"),
    (0x0D, "Tethering Target"),
    (0x0E, "Tethering Source"),
    (0x0F, "Nearby Info"),
    (0x10, "Nearby Action"),
    (0x12, "FindMy"),
    (0x13, "FindMy Notification"),
    (0x14, "Apple Audio"),
    (0x16, "Nearby Interaction"),
    (0x19, "Pairing Request"),
];

#[allow(dead_code)]
const APPLE_DEVICE_MODELS: &[(u8, &str)] = &[
    (0x01, "iPhone"),
    (0x02, "iPad"),
    (0x03, "iPod"),
    (0x04, "Mac"),
    (0x05, "Apple Watch"),
    (0x06, "Apple TV"),
    (0x07, "HomePod"),
    (0x08, "AirPods"),
    (0x09, "AirPods Pro"),
    (0x0A, "AirPods Max"),
    (0x0B, "AirTag"),
    (0x0C, "Apple Pencil"),
    (0x0D, "Magic Keyboard"),
    (0x0E, "Magic Mouse"),
    (0x0F, "Magic Trackpad"),
    (0x10, "Vision Pro"),
];

#[allow(dead_code)]
const APPLE_COLORS: &[(u8, &str)] = &[
    (0x00, "White/Silver"),
    (0x01, "Black"),
    (0x02, "Red"),
    (0x03, "Blue"),
    (0x04, "Pink"),
    (0x05, "Gray"),
    (0x06, "Silver"),
    (0x07, "Gold"),
    (0x08, "Rose Gold"),
    (0x09, "Space Gray"),
    (0x0A, "Green"),
    (0x0B, "Midnight"),
    (0x0C, "Purple"),
    (0x0D, "Starlight"),
    (0x0E, "Yellow"),
    (0x0F, "Orange"),
];

const AIRPODS_MODELS: &[(u16, &str)] = &[
    (0x0220, "AirPods"),
    (0x0320, "Powerbeats3"),
    (0x0520, "BeatsX"),
    (0x0620, "Beats Solo3"),
    (0x0920, "Beats Studio3"),
    (0x0A20, "AirPods Max"),
    (0x0B20, "Powerbeats Pro"),
    (0x0C20, "Beats Solo Pro"),
    (0x0D20, "Beats Fit Pro"),
    (0x0E20, "AirPods Pro"),
    (0x0F20, "AirPods (2nd gen)"),
    (0x1020, "AirPods (3rd gen)"),
    (0x1120, "Beats Studio Buds"),
    (0x1220, "AirPods Pro (2nd gen)"),
    (0x1320, "Beats Studio Buds+"),
    (0x1420, "AirPods (4th gen)"),
    (0x1520, "AirPods Pro (4th gen with ANC)"),
];

const GOOGLE_FAST_PAIR_DEVICES: &[(u32, &str)] = &[
    (0x00000E, "Pixel Buds A-Series"),
    (0x000043, "Sony WH-1000XM3"),
    (0x000048, "Bose QC35 II"),
    (0x000055, "Pixel Buds"),
    (0x000091, "Sony WF-1000XM3"),
    (0x0000C7, "Sony WH-1000XM4"),
    (0x0000D8, "Pixel Buds Pro"),
    (0x0000F0, "Bose QC45"),
    (0x000109, "Sony WH-1000XM5"),
    (0x000112, "Samsung Galaxy Buds2"),
    (0x000119, "Samsung Galaxy Buds Pro"),
    (0x000124, "Samsung Galaxy Buds Live"),
    (0x00012C, "Samsung Galaxy Buds FE"),
    (0x000132, "Samsung Galaxy Buds2 Pro"),
    (0x00015E, "Pixel Buds A-Series"),
    (0x0001B0, "Google Pixel Buds Pro 2"),
];

const GOOGLE_FLAGS: &[(u8, &str)] = &[
    (0x01, "Fast Pair"),
    (0x02, "Nearby Share"),
];

static APPLE_TYPE_MAP: OnceLock<HashMap<u8, &'static str>> = OnceLock::new();
#[allow(dead_code)]
static APPLE_DEVICE_MAP: OnceLock<HashMap<u8, &'static str>> = OnceLock::new();
#[allow(dead_code)]
static APPLE_COLOR_MAP: OnceLock<HashMap<u8, &'static str>> = OnceLock::new();
static AIRPODS_MODEL_MAP: OnceLock<HashMap<u16, &'static str>> = OnceLock::new();
static FAST_PAIR_MAP: OnceLock<HashMap<u32, &'static str>> = OnceLock::new();

fn get_apple_type_map() -> &'static HashMap<u8, &'static str> {
    APPLE_TYPE_MAP.get_or_init(|| APPLE_CONTINUITY_TYPES.iter().copied().collect())
}

#[allow(dead_code)]
fn get_apple_device_map() -> &'static HashMap<u8, &'static str> {
    APPLE_DEVICE_MAP.get_or_init(|| APPLE_DEVICE_MODELS.iter().copied().collect())
}

#[allow(dead_code)]
fn get_apple_color_map() -> &'static HashMap<u8, &'static str> {
    APPLE_COLOR_MAP.get_or_init(|| APPLE_COLORS.iter().copied().collect())
}

fn get_airpods_model_map() -> &'static HashMap<u16, &'static str> {
    AIRPODS_MODEL_MAP.get_or_init(|| AIRPODS_MODELS.iter().copied().collect())
}

fn get_fast_pair_map() -> &'static HashMap<u32, &'static str> {
    FAST_PAIR_MAP.get_or_init(|| GOOGLE_FAST_PAIR_DEVICES.iter().copied().collect())
}

pub fn decode_manufacturer_data(company_id: u16, data: &str) -> Option<String> {
    match company_id {
        0x004C => decode_apple_data(data),
        0x0006 => Some("Microsoft Beacon".to_string()),
        0x00E0 => decode_google_data(data),
        0x0075 => decode_samsung_data(data),
        0x00D2 => Some("Bose".to_string()),
        0x0059 => Some("Nordic Semiconductor".to_string()),
        0x000D => decode_texas_instruments(data),
        0x001D => Some("Qualcomm".to_string()),
        0x00E3 => Some("Samsung SmartThings".to_string()),
        0xFE95 => Some("Xiaomi MiBeacon".to_string()),
        0x0157 => Some("Huawei".to_string()),
        0x038F => Some("Xiaomi".to_string()),
        0x02E1 => Some("JBL".to_string()),
        0x00B4 => Some("LG Electronics".to_string()),
        0x001A => Some("Harman".to_string()),
        0x02E5 => Some("Sony Headphones".to_string()),
        0x012D => Some("Sony".to_string()),
        0x0131 => Some("DSEA (Sennheiser)".to_string()),
        0x0310 => Some("Bang & Olufsen".to_string()),
        0x0067 => Some("Belkin".to_string()),
        0x2400 => decode_charger_beacon(data),
        _ => None,
    }
}

fn decode_apple_data(hex: &str) -> Option<String> {
    if hex.len() < 4 {
        return None;
    }
    
    let bytes = hex_to_bytes(hex)?;
    if bytes.is_empty() {
        return None;
    }
    
    let type_byte = bytes[0];
    let type_map = get_apple_type_map();
    let type_name = type_map.get(&type_byte).copied();
    
    match type_byte {
        0x07 => decode_apple_proximity_pairing(&bytes),
        0x0F => decode_apple_nearby_info(&bytes),
        0x10 => decode_apple_nearby_action(&bytes),
        0x12 => decode_apple_findmy(&bytes),
        0x05 => decode_apple_airdrop(&bytes),
        0x0C => decode_apple_handoff(&bytes),
        0x02 => decode_apple_ibeacon(&bytes),
        _ => type_name.map(|name| name.to_string())
            .or_else(|| Some(format!("Apple Continuity (type 0x{:02x})", type_byte))),
    }
}

fn decode_apple_proximity_pairing(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 {
        return Some("AirPods/Beats (unknown model)".to_string());
    }
    
    let model_id = if bytes.len() >= 4 {
        ((bytes[3] as u16) << 8) | (bytes[2] as u16)
    } else {
        0
    };
    
    let model_map = get_airpods_model_map();
    let model_name = model_map.get(&model_id)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Apple Audio Device (0x{:04X})", model_id));
    
    if bytes.len() >= 6 {
        let status = bytes[4];
        let battery_info = decode_airpods_battery(bytes);
        if let Some(battery) = battery_info {
            return Some(format!("{} - {}", model_name, battery));
        }
        let charging = (status & 0x01) != 0;
        if charging {
            return Some(format!("{} (Charging)", model_name));
        }
    }
    
    Some(model_name)
}

fn decode_airpods_battery(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 7 {
        return None;
    }
    
    let left = (bytes[5] >> 4) & 0x0F;
    let right = bytes[5] & 0x0F;
    let case = (bytes[6] >> 4) & 0x0F;
    
    let format_battery = |val: u8| -> String {
        if val == 15 || val > 10 {
            "N/A".to_string()
        } else {
            format!("{}%", val * 10)
        }
    };
    
    Some(format!("L:{} R:{} Case:{}", 
        format_battery(left), 
        format_battery(right), 
        format_battery(case)))
}

fn decode_apple_nearby_info(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 3 {
        return Some("Apple Device (Nearby)".to_string());
    }
    
    let status = bytes[2];
    let action_code = status >> 4;
    let device_type_hint = status & 0x0F;
    
    let device_type = match device_type_hint {
        0x01..=0x03 => "iPhone",
        0x04 => "iPad",
        0x05..=0x06 => "Mac",
        0x07 => "Apple Watch",
        0x0A..=0x0B => "Apple TV",
        0x0E => "HomePod",
        _ => "Apple Device",
    };
    
    let action = match action_code {
        0x00 => "Idle",
        0x01 => "Active",
        0x02 => "Calling",
        0x03 => "Playing Audio",
        0x05 => "Facetime",
        0x07 => "Wi-Fi Connected",
        0x08 => "Navigating",
        0x09 => "Using iPhone",
        0x0A => "Homekit Active",
        0x0B => "Watching Video",
        _ => "",
    };
    
    if !action.is_empty() && action != "Idle" {
        Some(format!("{} ({})", device_type, action))
    } else {
        Some(device_type.to_string())
    }
}

fn decode_apple_nearby_action(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 {
        return Some("Apple Nearby Action".to_string());
    }
    
    let action_type = bytes[2];
    let action = match action_type {
        0x01 => "Apple TV Setup",
        0x02 => "HomePod Setup",
        0x04 => "Apple TV Keyboard",
        0x05 => "Apple TV Connecting",
        0x06 => "Apple TV Keyboard Setup",
        0x08 => "AirPods Handoff",
        0x09 => "Apple Watch Setup",
        0x0B => "iOS Setup",
        0x0D => "Instant Hotspot",
        0x0F => "Apple TV Audio Sync",
        0x13 => "AirPlay",
        _ => "Apple Nearby Action",
    };
    
    Some(action.to_string())
}

fn decode_apple_findmy(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 3 {
        return Some("FindMy Device".to_string());
    }
    
    let status = bytes[2];
    let is_separated = (status & 0x04) != 0;
    let has_audio = (status & 0x08) != 0;
    
    let mut info = "FindMy Device".to_string();
    
    if is_separated {
        info = format!("{} (Separated)", info);
    }
    if has_audio {
        info = format!("{} - Playing Sound", info);
    }
    
    Some(info)
}

fn decode_apple_airdrop(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 {
        return Some("AirDrop".to_string());
    }
    
    Some("AirDrop (Discoverable)".to_string())
}

fn decode_apple_handoff(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 4 {
        return Some("Handoff".to_string());
    }
    
    let activity_type = bytes.get(2).copied().unwrap_or(0);
    let activity = match activity_type >> 4 {
        0x00 => "Generic Activity",
        0x01 => "Notes",
        0x02 => "Safari",
        0x03 => "Mail",
        0x04 => "Messages",
        0x05 => "Maps",
        0x06 => "Reminders",
        0x07 => "Pages",
        0x08 => "Numbers",
        0x09 => "Keynote",
        _ => "Handoff Activity",
    };
    
    Some(format!("Handoff: {}", activity))
}

fn decode_apple_ibeacon(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 23 {
        return Some("iBeacon".to_string());
    }
    
    let uuid = format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[2], bytes[3], bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11],
        bytes[12], bytes[13], bytes[14], bytes[15], bytes[16], bytes[17]
    );
    
    let major = ((bytes[18] as u16) << 8) | (bytes[19] as u16);
    let minor = ((bytes[20] as u16) << 8) | (bytes[21] as u16);
    
    Some(format!("iBeacon UUID:{} Major:{} Minor:{}", uuid, major, minor))
}

fn decode_google_data(hex: &str) -> Option<String> {
    if hex.len() < 2 {
        return None;
    }
    
    let bytes = hex_to_bytes(hex)?;
    
    if bytes.len() >= 3 {
        let model_id = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);
        
        let fast_pair_map = get_fast_pair_map();
        if let Some(&device_name) = fast_pair_map.get(&model_id) {
            return Some(format!("Google Fast Pair: {}", device_name));
        }
    }
    
    let flags = bytes.get(0).copied().unwrap_or(0);
    
    for (flag_bit, name) in GOOGLE_FLAGS {
        if flags & flag_bit != 0 {
            return Some(name.to_string());
        }
    }
    
    Some("Google Beacon".to_string())
}

fn decode_samsung_data(data: &str) -> Option<String> {
    let bytes = hex_to_bytes(data)?;
    
    if bytes.is_empty() {
        return Some("Samsung Device".to_string());
    }
    
    let protocol = bytes.get(0).copied().unwrap_or(0);
    
    match protocol {
        0x01 => Some("Samsung Galaxy Wearable".to_string()),
        0x02 => Some("Samsung Smart Tag".to_string()),
        0x03 => Some("Samsung TV".to_string()),
        0x04 => Some("Samsung Galaxy Buds".to_string()),
        0x42 => {
            if bytes.len() >= 3 {
                let device_type = bytes.get(2).copied().unwrap_or(0);
                match device_type {
                    0x01 => Some("Samsung Galaxy Phone".to_string()),
                    0x02 => Some("Samsung Galaxy Watch".to_string()),
                    0x03 => Some("Samsung Galaxy Buds".to_string()),
                    0x04 => Some("Samsung Galaxy Tab".to_string()),
                    _ => Some("Samsung SmartThings".to_string()),
                }
            } else {
                Some("Samsung SmartThings Find".to_string())
            }
        }
        _ => Some("Samsung Device".to_string()),
    }
}

fn decode_texas_instruments(_data: &str) -> Option<String> {
    Some("TI SensorTag".to_string())
}

fn decode_charger_beacon(hex_str: &str) -> Option<String> {
    if hex_str.len() < 44 {
        return None;
    }
    
    let bytes = hex_to_bytes(hex_str)?;
    
    if bytes.len() < 22 {
        return None;
    }
    
    let msg_type = bytes[0];
    
    let mac1 = format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[2], bytes[3], bytes[4], 
        bytes[5], bytes[6], bytes[7]);
    
    let mac2 = format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[8], bytes[9], bytes[10], 
        bytes[11], bytes[12], bytes[13]);
    
    let own_mac = format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[16], bytes[17], bytes[18], 
        bytes[19], bytes[20], bytes[21]);
    
    Some(format!("Charger Beacon (type 0x{:02x}): paired devices [{}, {}], own MAC [{}]",
        msg_type, mac1, mac2, own_mac))
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() % 2 != 0 {
        return None;
    }
    
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let byte = u8::from_str_radix(&hex[i..i+2], 16).ok()?;
        bytes.push(byte);
    }
    
    Some(bytes)
}

#[allow(dead_code)]
pub fn identify_device_type(
    manufacturer_id: Option<u16>,
    manufacturer_data: Option<&str>,
    services: &[String],
    device_name: &str,
) -> Option<String> {
    if let Some(id) = manufacturer_id {
        match id {
            0x004C => {
                if let Some(data) = manufacturer_data {
                    if let Some(decoded) = decode_apple_data(data) {
                        return Some(decoded);
                    }
                }
                return Some("Apple Device".to_string());
            }
            0x0075 => return Some("Samsung".to_string()),
            0x00E0 => return Some("Google".to_string()),
            0x0006 => return Some("Microsoft".to_string()),
            0x00D2 => return Some("Bose".to_string()),
            0x012D => return Some("Sony".to_string()),
            _ => {}
        }
    }
    
    let name_lower = device_name.to_lowercase();
    if name_lower.contains("iphone") {
        return Some("iPhone".to_string());
    }
    if name_lower.contains("ipad") {
        return Some("iPad".to_string());
    }
    if name_lower.contains("macbook") || name_lower.contains("mac ") {
        return Some("Mac".to_string());
    }
    if name_lower.contains("airpods") {
        return Some("AirPods".to_string());
    }
    if name_lower.contains("apple watch") {
        return Some("Apple Watch".to_string());
    }
    if name_lower.contains("galaxy") {
        return Some("Samsung Galaxy".to_string());
    }
    if name_lower.contains("pixel") {
        return Some("Google Pixel".to_string());
    }
    
    for service in services {
        let service_lower = service.to_lowercase();
        if service_lower.contains("fe2c") || service_lower.contains("fe26") {
            return Some("Apple Device".to_string());
        }
        if service_lower.contains("fea0") {
            return Some("Samsung Device".to_string());
        }
    }
    
    None
}
