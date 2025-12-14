// Copyright (c) 2025 Dmitry Kalashnikov

use std::collections::HashMap;
use std::sync::OnceLock;

const APPLE_CONTINUITY_TYPES: &[(u8, &str)] = &[
    (0x02, "iBeacon"),
    (0x05, "AirDrop"),
    (0x07, "AirPods"),
    (0x09, "AirPlay"),
    (0x0A, "HomeKit"),
    (0x0C, "Handoff"),
    (0x0F, "Nearby Info"),
    (0x10, "Nearby Action"),
    (0x12, "FindMy"),
    (0x13, "FindMy Notification"),
    (0x16, "Nearby Interaction"),
];

const GOOGLE_FLAGS: &[(u8, &str)] = &[
    (0x01, "Fast Pair"),
    (0x02, "Nearby Share"),
];

static APPLE_TYPE_MAP: OnceLock<HashMap<u8, &'static str>> = OnceLock::new();

fn get_apple_type_map() -> &'static HashMap<u8, &'static str> {
    APPLE_TYPE_MAP.get_or_init(|| {
        APPLE_CONTINUITY_TYPES.iter().copied().collect()
    })
}

pub fn decode_manufacturer_data(company_id: u16, data: &str) -> Option<String> {
    match company_id {
        0x004C => decode_apple_data(data),
        0x0006 => Some("Microsoft Beacon".to_string()),
        0x00E0 => decode_google_data(data),
        0x0075 => Some("Samsung Smart Beacon".to_string()),
        0xFE95 => Some("Xiaomi MiBeacon".to_string()),
        0x2400 => decode_charger_beacon(data),
        _ => None,
    }
}

fn decode_charger_beacon(hex_str: &str) -> Option<String> {
    if hex_str.len() < 44 {
        return None;
    }
    
    let mut data_bytes = Vec::new();
    for i in (0..hex_str.len()).step_by(2) {
        if i + 2 <= hex_str.len() {
            let byte = u8::from_str_radix(&hex_str[i..i+2], 16).ok()?;
            data_bytes.push(byte);
        }
    }
    
    if data_bytes.len() < 22 {
        return None;
    }
    
    let msg_type = data_bytes[0];
    
    let mac1 = format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        data_bytes[2], data_bytes[3], data_bytes[4], 
        data_bytes[5], data_bytes[6], data_bytes[7]);
    
    let mac2 = format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        data_bytes[8], data_bytes[9], data_bytes[10], 
        data_bytes[11], data_bytes[12], data_bytes[13]);
    
    let own_mac = format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        data_bytes[16], data_bytes[17], data_bytes[18], 
        data_bytes[19], data_bytes[20], data_bytes[21]);
    
    Some(format!("Charger Beacon (type 0x{:02x}): paired devices [{}, {}], own MAC [{}]", 
        msg_type, mac1, mac2, own_mac))
}

fn decode_apple_data(hex: &str) -> Option<String> {
    if hex.len() < 2 {
        return None;
    }
    
    let type_byte = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let type_map = get_apple_type_map();
    
    type_map.get(&type_byte)
        .map(|&name| name.to_string())
        .or_else(|| Some(format!("Apple Continuity (type 0x{:02x})", type_byte)))
}

fn decode_google_data(hex: &str) -> Option<String> {
    if hex.len() < 2 {
        return None;
    }
    
    let flags = u8::from_str_radix(&hex[0..2], 16).ok()?;
    
    for (flag_bit, name) in GOOGLE_FLAGS {
        if flags & flag_bit != 0 {
            return Some(name.to_string());
        }
    }
    
    Some("Google Beacon".to_string())
}
