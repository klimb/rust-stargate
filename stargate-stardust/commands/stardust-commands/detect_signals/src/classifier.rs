// Copyright (c) 2025 Dmitry Kalashnikov.

struct SignalBand {
    freq_min: u64,
    freq_max: u64,
    signal_type: &'static str,
    classification: &'static str,
    description_template: &'static str,
    precision: usize,
}

#[derive(Clone)]
struct BandwidthVariant {
    bandwidth_threshold: Option<(u64, bool)>,
    signal_type: &'static str,
    classification: &'static str,
    description_template: &'static str,
    precision: usize,
}

struct BandwidthDependentSignal {
    freq_min: u64,
    freq_max: u64,
    variants: Vec<BandwidthVariant>,
}

static SIGNAL_BANDS: &[SignalBand] = &[
    // AM/Shortwave Radio
    SignalBand { freq_min: 530_000,       freq_max: 1_710_000,     signal_type: "AM Radio",        classification: "Radio",     description_template: "AM Radio Station @ MHz",           precision: 3 },
    SignalBand { freq_min: 2_300_000,     freq_max: 2_495_000,     signal_type: "Shortwave 120m",  classification: "Radio",     description_template: "Shortwave Broadcast 120m @ MHz",   precision: 3 },
    SignalBand { freq_min: 3_200_000,     freq_max: 3_400_000,     signal_type: "Shortwave 90m",   classification: "Radio",     description_template: "Shortwave Broadcast 90m @ MHz",    precision: 3 },
    SignalBand { freq_min: 3_500_000,     freq_max: 4_000_000,     signal_type: "Amateur 80m",     classification: "Ham Radio", description_template: "Amateur Radio 80m Band @ MHz",     precision: 3 },
    SignalBand { freq_min: 4_750_000,     freq_max: 5_060_000,     signal_type: "Shortwave 60m",   classification: "Radio",     description_template: "Shortwave Broadcast 60m @ MHz",    precision: 3 },
    SignalBand { freq_min: 5_900_000,     freq_max: 6_200_000,     signal_type: "Shortwave 49m",   classification: "Radio",     description_template: "Shortwave Broadcast 49m @ MHz",    precision: 3 },
    SignalBand { freq_min: 7_000_000,     freq_max: 7_300_000,     signal_type: "Amateur 40m",     classification: "Ham Radio", description_template: "Amateur Radio 40m Band @ MHz",     precision: 3 },
    SignalBand { freq_min: 9_400_000,     freq_max: 9_900_000,     signal_type: "Shortwave 31m",   classification: "Radio",     description_template: "Shortwave Broadcast 31m @ MHz",    precision: 3 },
    SignalBand { freq_min: 10_100_000,    freq_max: 10_150_000,    signal_type: "Amateur 30m",     classification: "Ham Radio", description_template: "Amateur Radio 30m Band @ MHz",     precision: 3 },
    SignalBand { freq_min: 11_600_000,    freq_max: 12_100_000,    signal_type: "Shortwave 25m",   classification: "Radio",     description_template: "Shortwave Broadcast 25m @ MHz",    precision: 3 },
    SignalBand { freq_min: 13_570_000,    freq_max: 13_870_000,    signal_type: "Shortwave 22m",   classification: "Radio",     description_template: "Shortwave Broadcast 22m @ MHz",    precision: 3 },
    SignalBand { freq_min: 14_000_000,    freq_max: 14_350_000,    signal_type: "Amateur 20m",     classification: "Ham Radio", description_template: "Amateur Radio 20m Band @ MHz",     precision: 3 },
    SignalBand { freq_min: 15_100_000,    freq_max: 15_800_000,    signal_type: "Shortwave 19m",   classification: "Radio",     description_template: "Shortwave Broadcast 19m @ MHz",    precision: 3 },
    SignalBand { freq_min: 18_068_000,    freq_max: 18_168_000,    signal_type: "Amateur 17m",     classification: "Ham Radio", description_template: "Amateur Radio 17m Band @ MHz",     precision: 3 },
    SignalBand { freq_min: 21_000_000,    freq_max: 21_450_000,    signal_type: "Amateur 15m",     classification: "Ham Radio", description_template: "Amateur Radio 15m Band @ MHz",     precision: 3 },
    SignalBand { freq_min: 24_890_000,    freq_max: 24_990_000,    signal_type: "Amateur 12m",     classification: "Ham Radio", description_template: "Amateur Radio 12m Band @ MHz",     precision: 3 },
    SignalBand { freq_min: 28_000_000,    freq_max: 29_700_000,    signal_type: "Amateur 10m",     classification: "Ham Radio", description_template: "Amateur Radio 10m Band @ MHz",     precision: 3 },
    SignalBand { freq_min: 26_960_000,    freq_max: 27_410_000,    signal_type: "CB Radio",        classification: "Radio",     description_template: "Citizens Band Radio @ MHz",        precision: 3 },
    
    // Marine & Emergency
    SignalBand { freq_min: 156_000_000,   freq_max: 162_000_000,   signal_type: "Marine VHF",      classification: "Maritime",  description_template: "Marine VHF Radio @ MHz",           precision: 3 },
    SignalBand { freq_min: 156_800_000,   freq_max: 156_800_000,   signal_type: "Marine Ch16",     classification: "Maritime",  description_template: "Marine Emergency Channel 16",     precision: 3 },
    
    // FM Radio & TV
    SignalBand { freq_min: 76_000_000,    freq_max: 90_000_000,    signal_type: "FM Japan",        classification: "Radio",     description_template: "FM Radio (Japan Band) @ MHz",      precision: 1 },
    SignalBand { freq_min: 88_000_000,    freq_max: 108_000_000,   signal_type: "FM Broadcast",    classification: "Radio",     description_template: "FM Radio Station @ MHz",           precision: 1 },
    SignalBand { freq_min: 87_500_000,    freq_max: 108_000_000,   signal_type: "FM OIRT",         classification: "Radio",     description_template: "FM Radio (OIRT) @ MHz",            precision: 1 },
    
    // Aviation
    SignalBand { freq_min: 108_000_000,   freq_max: 118_000_000,   signal_type: "VOR/ILS",         classification: "Aviation",  description_template: "Aviation Navigation @ MHz",        precision: 3 },
    SignalBand { freq_min: 118_000_000,   freq_max: 137_000_000,   signal_type: "Airband",         classification: "Aviation",  description_template: "Aircraft Communication @ MHz",     precision: 3 },
    SignalBand { freq_min: 121_500_000,   freq_max: 121_500_000,   signal_type: "Aviation ELT",    classification: "Aviation",  description_template: "Aviation Emergency (121.5 MHz)",   precision: 3 },
    SignalBand { freq_min: 243_000_000,   freq_max: 243_000_000,   signal_type: "Military ELT",    classification: "Aviation",  description_template: "Military Emergency (243 MHz)",     precision: 0 },
    SignalBand { freq_min: 960_000_000,   freq_max: 1_215_000_000, signal_type: "TACAN/DME",       classification: "Aviation",  description_template: "Aviation TACAN/DME @ MHz",         precision: 0 },
    SignalBand { freq_min: 1_030_000_000, freq_max: 1_090_000_000, signal_type: "SSR",             classification: "Aviation",  description_template: "Secondary Surveillance Radar",     precision: 0 },
    SignalBand { freq_min: 1_090_000_000, freq_max: 1_090_000_000, signal_type: "ADS-B",           classification: "Aviation",  description_template: "Aircraft Transponder (ADS-B)",     precision: 0 },
    
    // Amateur Radio VHF/UHF
    SignalBand { freq_min: 50_000_000,    freq_max: 54_000_000,    signal_type: "Amateur 6m",      classification: "Ham Radio", description_template: "Amateur Radio 6m Band @ MHz",      precision: 3 },
    SignalBand { freq_min: 144_000_000,   freq_max: 148_000_000,   signal_type: "Amateur 2m",      classification: "Ham Radio", description_template: "Amateur Radio 2m Band @ MHz",      precision: 3 },
    SignalBand { freq_min: 219_000_000,   freq_max: 225_000_000,   signal_type: "Amateur 1.25m",   classification: "Ham Radio", description_template: "Amateur Radio 1.25m Band @ MHz",   precision: 0 },
    SignalBand { freq_min: 420_000_000,   freq_max: 450_000_000,   signal_type: "Amateur 70cm",    classification: "Ham Radio", description_template: "Amateur Radio 70cm Band @ MHz",    precision: 0 },
    SignalBand { freq_min: 902_000_000,   freq_max: 928_000_000,   signal_type: "Amateur 33cm",    classification: "Ham Radio", description_template: "Amateur Radio 33cm Band @ MHz",    precision: 0 },
    SignalBand { freq_min: 1_240_000_000, freq_max: 1_300_000_000, signal_type: "Amateur 23cm",    classification: "Ham Radio", description_template: "Amateur Radio 23cm Band @ MHz",    precision: 0 },
    
    // Weather & NOAA
    SignalBand { freq_min: 162_400_000,   freq_max: 162_550_000,   signal_type: "NOAA Weather",    classification: "Broadcast", description_template: "NOAA Weather Radio @ MHz",         precision: 3 },
    SignalBand { freq_min: 137_000_000,   freq_max: 138_000_000,   signal_type: "Weather Sat",     classification: "Satellite", description_template: "Weather Satellite APT @ MHz",      precision: 3 },
    
    // TV Broadcast
    SignalBand { freq_min: 54_000_000,    freq_max: 88_000_000,    signal_type: "VHF Low TV",      classification: "Broadcast", description_template: "VHF Low Television @ MHz",         precision: 0 },
    SignalBand { freq_min: 174_000_000,   freq_max: 240_000_000,   signal_type: "VHF TV",          classification: "Broadcast", description_template: "VHF Television @ MHz",             precision: 0 },
    SignalBand { freq_min: 470_000_000,   freq_max: 890_000_000,   signal_type: "UHF TV",          classification: "Broadcast", description_template: "UHF Television @ MHz",             precision: 0 },
    
    // ISM & Remote Control
    SignalBand { freq_min: 13_553_000,    freq_max: 13_567_000,    signal_type: "ISM 13.56MHz",    classification: "Remote",    description_template: "RFID NFC @ MHz",                  precision: 3 },
    SignalBand { freq_min: 26_957_000,    freq_max: 27_283_000,    signal_type: "ISM 27MHz",       classification: "Remote",    description_template: "RC Toys / Baby Monitors",         precision: 3 },
    SignalBand { freq_min: 40_660_000,    freq_max: 40_700_000,    signal_type: "ISM 40MHz",       classification: "Remote",    description_template: "RC Aircraft @ MHz",               precision: 3 },
    SignalBand { freq_min: 72_000_000,    freq_max: 73_000_000,    signal_type: "RC 72MHz",        classification: "Remote",    description_template: "RC Aircraft 72MHz @ MHz",         precision: 3 },
    SignalBand { freq_min: 315_000_000,   freq_max: 316_000_000,   signal_type: "ISM 315MHz",      classification: "Remote",    description_template: "Car Key / Remote Control",        precision: 0 },
    SignalBand { freq_min: 433_050_000,   freq_max: 434_790_000,   signal_type: "ISM 433MHz",      classification: "Remote",    description_template: "Garage Opener / Weather Station", precision: 0 },
    SignalBand { freq_min: 868_000_000,   freq_max: 869_000_000,   signal_type: "ISM 868MHz",      classification: "Remote",    description_template: "IoT / Smart Home Device",         precision: 0 },
    SignalBand { freq_min: 902_000_000,   freq_max: 928_000_000,   signal_type: "ISM 915MHz",      classification: "Remote",    description_template: "Zigbee / Z-Wave / RFID",          precision: 0 },
    
    // PMR & FRS Radios
    SignalBand { freq_min: 446_000_000,   freq_max: 446_200_000,   signal_type: "PMR446",          classification: "Radio",     description_template: "PMR446 Walkie-Talkie @ MHz",      precision: 0 },
    SignalBand { freq_min: 462_000_000,   freq_max: 467_000_000,   signal_type: "FRS/GMRS",        classification: "Radio",     description_template: "FRS/GMRS Radio @ MHz",            precision: 0 },
    
    // Public Safety & Emergency
    SignalBand { freq_min: 150_000_000,   freq_max: 174_000_000,   signal_type: "VHF Public",      classification: "Emergency", description_template: "Public Safety VHF @ MHz",          precision: 0 },
    SignalBand { freq_min: 450_000_000,   freq_max: 470_000_000,   signal_type: "UHF Public",      classification: "Emergency", description_template: "Public Safety UHF @ MHz",          precision: 0 },
    SignalBand { freq_min: 806_000_000,   freq_max: 824_000_000,   signal_type: "Public Safety",   classification: "Emergency", description_template: "Public Safety 800MHz @ MHz",       precision: 0 },
    
    // Cellular Networks (2G/3G/4G/5G)
    SignalBand { freq_min: 617_000_000,   freq_max: 652_000_000,   signal_type: "LTE Band 71",     classification: "Cellular",  description_template: "5G LTE 600MHz @ MHz",             precision: 0 },
    SignalBand { freq_min: 698_000_000,   freq_max: 806_000_000,   signal_type: "LTE 700MHz",      classification: "Cellular",  description_template: "4G LTE 700MHz @ MHz",             precision: 0 },
    SignalBand { freq_min: 806_000_000,   freq_max: 824_000_000,   signal_type: "IDEN 800",        classification: "Cellular",  description_template: "Nextel iDEN @ MHz",               precision: 0 },
    SignalBand { freq_min: 824_000_000,   freq_max: 849_000_000,   signal_type: "CDMA 850",        classification: "Cellular",  description_template: "CDMA Cell Tower @ MHz",           precision: 0 },
    SignalBand { freq_min: 851_000_000,   freq_max: 869_000_000,   signal_type: "GSM 850",         classification: "Cellular",  description_template: "GSM 850 Cell Tower @ MHz",        precision: 0 },
    SignalBand { freq_min: 869_000_000,   freq_max: 894_000_000,   signal_type: "GSM 900",         classification: "Cellular",  description_template: "GSM 900 Cell Tower @ MHz",        precision: 0 },
    SignalBand { freq_min: 1_710_000_000, freq_max: 1_785_000_000, signal_type: "DCS 1800",        classification: "Cellular",  description_template: "DCS 1800 Cell Tower @ MHz",       precision: 0 },
    SignalBand { freq_min: 1_805_000_000, freq_max: 1_880_000_000, signal_type: "GSM 1800",        classification: "Cellular",  description_template: "GSM 1800 Cell Tower @ MHz",       precision: 0 },
    SignalBand { freq_min: 1_850_000_000, freq_max: 1_910_000_000, signal_type: "PCS 1900",        classification: "Cellular",  description_template: "PCS 1900 Cell Tower @ MHz",       precision: 0 },
    SignalBand { freq_min: 1_920_000_000, freq_max: 1_980_000_000, signal_type: "UMTS 2100",       classification: "Cellular",  description_template: "3G UMTS 2100 @ MHz",              precision: 0 },
    SignalBand { freq_min: 2_110_000_000, freq_max: 2_200_000_000, signal_type: "UMTS/LTE",        classification: "Cellular",  description_template: "4G LTE Cell Tower @ MHz",         precision: 0 },
    SignalBand { freq_min: 2_500_000_000, freq_max: 2_690_000_000, signal_type: "LTE 2.5GHz",      classification: "Cellular",  description_template: "4G LTE 2.5GHz @ MHz",             precision: 0 },
    SignalBand { freq_min: 3_400_000_000, freq_max: 3_800_000_000, signal_type: "5G C-Band",       classification: "Cellular",  description_template: "5G C-Band @ MHz",                 precision: 0 },
    
    // WiFi & Bluetooth
    SignalBand { freq_min: 5_150_000_000, freq_max: 5_350_000_000, signal_type: "WiFi 5GHz Low",   classification: "Wireless",  description_template: "WiFi 5GHz (Ch 36-64) @ MHz",      precision: 0 },
    SignalBand { freq_min: 5_470_000_000, freq_max: 5_725_000_000, signal_type: "WiFi 5GHz Mid",   classification: "Wireless",  description_template: "WiFi 5GHz (Ch 100-144) @ MHz",    precision: 0 },
    SignalBand { freq_min: 5_725_000_000, freq_max: 5_850_000_000, signal_type: "WiFi 5GHz High",  classification: "Wireless",  description_template: "WiFi 5GHz (Ch 149-165) @ MHz",    precision: 0 },
    SignalBand { freq_min: 5_925_000_000, freq_max: 7_125_000_000, signal_type: "WiFi 6E",         classification: "Wireless",  description_template: "WiFi 6E 6GHz @ MHz",              precision: 0 },
    
    // Satellite (GPS) Communications
    SignalBand { freq_min: 1_525_000_000, freq_max: 1_559_000_000, signal_type: "Inmarsat",        classification: "Satellite", description_template: "Inmarsat Sat Phone @ MHz",        precision: 0 },
    SignalBand { freq_min: 1_610_000_000, freq_max: 1_626_500_000, signal_type: "Iridium",         classification: "Satellite", description_template: "Iridium Sat Phone @ MHz",         precision: 0 },
    SignalBand { freq_min: 1_575_420_000, freq_max: 1_575_420_000, signal_type: "GPS L1",          classification: "Satellite", description_template: "GPS L1 (1575.42 MHz)",            precision: 3 },
    SignalBand { freq_min: 1_227_600_000, freq_max: 1_227_600_000, signal_type: "GPS L2",          classification: "Satellite", description_template: "GPS L2 (1227.6 MHz)",             precision: 3 },
    SignalBand { freq_min: 1_176_450_000, freq_max: 1_176_450_000, signal_type: "GPS L5",          classification: "Satellite", description_template: "GPS L5 (1176.45 MHz)",            precision: 3 },
    SignalBand { freq_min: 1_598_000_000, freq_max: 1_606_000_000, signal_type: "GLONASS L1",      classification: "Satellite", description_template: "GLONASS L1 @ MHz",                precision: 0 },
    SignalBand { freq_min: 1_559_000_000, freq_max: 1_591_000_000, signal_type: "Galileo E1",      classification: "Satellite", description_template: "Galileo E1 @ MHz",                precision: 0 },
    
    // Radar 
    SignalBand { freq_min: 1_215_000_000, freq_max: 1_400_000_000, signal_type: "L-Band Radar",    classification: "Radar",     description_template: "L-Band Radar @ MHz",              precision: 0 },
    SignalBand { freq_min: 2_700_000_000, freq_max: 3_700_000_000, signal_type: "S-Band Radar",    classification: "Radar",     description_template: "S-Band Weather Radar @ MHz",      precision: 0 },
    SignalBand { freq_min: 5_250_000_000, freq_max: 5_925_000_000, signal_type: "C-Band Radar",    classification: "Radar",     description_template: "C-Band Radar @ MHz",              precision: 0 },
    
    // Wireless Microphones & Audio
    SignalBand { freq_min: 174_000_000,   freq_max: 216_000_000,   signal_type: "Wireless Mic",    classification: "Audio",     description_template: "Wireless Microphone @ MHz",       precision: 0 },
    SignalBand { freq_min: 470_000_000,   freq_max: 698_000_000,   signal_type: "Wireless Mic",    classification: "Audio",     description_template: "Wireless Microphone @ MHz",       precision: 0 },
];

const ISM_2_4_GHZ_VARIANTS: &[BandwidthVariant] = &[
    BandwidthVariant { bandwidth_threshold: Some((20_000_000, true)),  signal_type: "WiFi 2.4GHz", classification: "Wireless", description_template: "WiFi Router @ MHz",       precision: 0 },
    BandwidthVariant { bandwidth_threshold: Some((2_000_000, false)),  signal_type: "Bluetooth",   classification: "Wireless", description_template: "Bluetooth Device @ MHz",  precision: 0 },
    BandwidthVariant { bandwidth_threshold: None,                      signal_type: "ISM 2.4GHz",  classification: "Wireless", description_template: "2.4 GHz Device @ MHz",    precision: 0 },
];

fn get_bandwidth_dependent_signals() -> Vec<BandwidthDependentSignal> {
    vec![
        BandwidthDependentSignal {
            freq_min: 2_400_000_000,
            freq_max: 2_500_000_000,
            variants: ISM_2_4_GHZ_VARIANTS.to_vec(),
        },
    ]
}

impl SignalBand {
    fn matches(&self, freq_hz: u64) -> bool {
        freq_hz >= self.freq_min && freq_hz <= self.freq_max
    }

    fn format_description(&self, freq_mhz: f64) -> String {
        if self.description_template.contains("@") {
            self.description_template.replace("@", &format!("{:.prec$}", freq_mhz, prec = self.precision))
        } else {
            self.description_template.to_string()
        }
    }
}

impl BandwidthDependentSignal {
    fn matches(&self, freq_hz: u64) -> bool {
        freq_hz >= self.freq_min && freq_hz <= self.freq_max
    }

    fn classify(&self, bandwidth: u64, freq_mhz: f64) -> (String, String, String) {
        for variant in &self.variants {
            let matches = match variant.bandwidth_threshold {
                Some((threshold, greater_than)) => {
                    if greater_than {
                        bandwidth > threshold
                    } else {
                        bandwidth < threshold
                    }
                }
                None => true,
            };

            if matches {
                let description = if variant.description_template.contains("@") {
                    variant.description_template.replace("@", &format!("{:.prec$}", freq_mhz, prec = variant.precision))
                } else {
                    variant.description_template.to_string()
                };
                return (
                    variant.signal_type.to_string(),
                    variant.classification.to_string(),
                    description,
                );
            }
        }

        ("Unknown".to_string(), "Unclassified".to_string(), format!("Signal @ {:.1} MHz", freq_mhz))
    }
}

pub fn classify_signal(freq_hz: u64, bandwidth: u64) -> (String, String, String) {
    let freq_mhz = freq_hz as f64 / 1_000_000.0;

    for bw_signal in &get_bandwidth_dependent_signals() {
        if bw_signal.matches(freq_hz) {
            return bw_signal.classify(bandwidth, freq_mhz);
        }
    }

    for band in SIGNAL_BANDS {
        if band.matches(freq_hz) {
            return (
                band.signal_type.to_string(),
                band.classification.to_string(),
                band.format_description(freq_mhz),
            );
        }
    }

    ("Unknown".to_string(), "Unclassified".to_string(), format!("Signal @ {:.1} MHz", freq_mhz))
}

pub struct SignalBandInfo {
    pub freq_min: u64,
    pub freq_max: u64,
    pub signal_type: &'static str,
    pub classification: &'static str,
}

pub fn get_all_signal_bands() -> Vec<SignalBandInfo> {
    SIGNAL_BANDS.iter().map(|band| SignalBandInfo {
        freq_min: band.freq_min,
        freq_max: band.freq_max,
        signal_type: band.signal_type,
        classification: band.classification,
    }).collect()
}

pub struct PresetInfo {
    pub freq_min: u64,
    pub freq_max: u64,
    pub threshold: f64,
    pub duration: u64,
}

pub fn get_preset_by_type(signal_type: &str) -> Option<PresetInfo> {
    SIGNAL_BANDS.iter()
        .find(|band| band.signal_type == signal_type)
        .map(|band| PresetInfo {
            freq_min: band.freq_min,
            freq_max: band.freq_max,
            threshold: -30.0,
            duration: 5,
        })
}

pub fn get_preset_by_classification(classification: &str) -> Option<PresetInfo> {
    // Find the range spanning all bands of this classification
    let matching_bands: Vec<_> = SIGNAL_BANDS.iter()
        .filter(|band| band.classification == classification)
        .collect();
    
    if matching_bands.is_empty() {
        return None;
    }
    
    let freq_min = matching_bands.iter().map(|b| b.freq_min).min().unwrap();
    let freq_max = matching_bands.iter().map(|b| b.freq_max).max().unwrap();
    
    Some(PresetInfo {
        freq_min,
        freq_max,
        threshold: -30.0,
        duration: 10,
    })
}