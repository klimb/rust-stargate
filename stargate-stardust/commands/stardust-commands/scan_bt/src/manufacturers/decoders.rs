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
        _ => None,
    }
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
