// list-history built-in command
use std::time::SystemTime;
use std::io::{BufRead, BufReader};
use serde_json::json;

pub fn execute(args: &str, history_file: &str) -> Result<(), String> {
    let is_object_output = args.contains("-o") || args.contains("--obj");
    
    // Extract filter (everything that's not a flag)
    let filter = args.split_whitespace()
        .filter(|&s| s != "-o" && s != "--obj")
        .collect::<Vec<_>>()
        .join(" ");
    
    let history_with_ts = load_timestamped_history(history_file);
    
    if filter.is_empty() {
        // No filter, show all history
        if history_with_ts.is_empty() {
            if is_object_output {
                println!("{}", json!({"entries": [], "count": 0}));
            } else {
                println!("No command history available.");
            }
        } else if is_object_output {
            let entries: Vec<_> = history_with_ts.iter().map(|(timestamp, cmd)| {
                json!({
                    "timestamp": format_timestamp(*timestamp),
                    "timestamp_unix": timestamp.duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default().as_secs(),
                    "command": cmd
                })
            }).collect();
            
            let output = json!({
                "entries": entries,
                "count": entries.len()
            });
            println!("{}", output);
        } else {
            for (timestamp, cmd) in &history_with_ts {
                println!("{}\t{}", format_timestamp(*timestamp), cmd);
            }
        }
    } else {
        // Filter history
        let filtered: Vec<_> = history_with_ts.iter()
            .filter(|(_, cmd)| cmd.contains(&filter))
            .collect();
        
        if filtered.is_empty() {
            if is_object_output {
                println!("{}", json!({"entries": [], "count": 0}));
            } else {
                println!("No history entries matching '{}'", filter);
            }
        } else if is_object_output {
            let entries: Vec<_> = filtered.iter().map(|(timestamp, cmd)| {
                json!({
                    "timestamp": format_timestamp(*timestamp),
                    "timestamp_unix": timestamp.duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default().as_secs(),
                    "command": cmd
                })
            }).collect();
            
            let output = json!({
                "entries": entries,
                "count": entries.len()
            });
            println!("{}", output);
        } else {
            for (timestamp, cmd) in filtered {
                println!("{}\t{}", format_timestamp(*timestamp), cmd);
            }
        }
    }
    
    Ok(())
}

// Load history with timestamps from file (public for main to use)
pub fn load_timestamped_history(history_file: &str) -> Vec<(SystemTime, String)> {
    let mut history = Vec::new();
    
    if let Ok(file) = std::fs::File::open(history_file) {
        let reader = BufReader::new(file);
        
        for line in reader.lines().flatten() {
            // Format: timestamp|command
            if let Some(pos) = line.find('|') {
                let timestamp_str = &line[..pos];
                let command = &line[pos + 1..];
                
                if let Ok(secs) = timestamp_str.parse::<u64>() {
                    let timestamp = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
                    history.push((timestamp, command.to_string()));
                }
            }
        }
    }
    
    history
}

// Format timestamp as human-readable date (short format with minutes)
fn format_timestamp(timestamp: SystemTime) -> String {
    use std::time::Duration;
    
    let now = SystemTime::now();
    let duration = now.duration_since(timestamp).unwrap_or(Duration::from_secs(0));
    let secs = duration.as_secs();
    
    let elapsed_hours = secs / 3600;
    let elapsed_mins = (secs % 3600) / 60;
    
    // Less than 1 hour: show minutes
    if secs < 3600 {
        if secs < 60 {
            return "0m".to_string();
        }
        return format!("{}m", elapsed_mins);
    }
    
    // Less than 24 hours: show hours and minutes
    if secs < 86400 {
        if elapsed_mins == 0 {
            return format!("{}h", elapsed_hours);
        }
        return format!("{}h{}m", elapsed_hours, elapsed_mins);
    }
    
    // Days but less than a week: show days and hours
    let days = secs / 86400;
    if days < 7 {
        let remaining_hours = (secs % 86400) / 3600;
        if remaining_hours == 0 {
            return format!("{}d", days);
        }
        return format!("{}d{}h", days, remaining_hours);
    }
    
    // Weeks but less than a month: show weeks and days
    if days < 30 {
        let weeks = days / 7;
        let remaining_days = days % 7;
        if remaining_days == 0 {
            return format!("{}w", weeks);
        }
        return format!("{}w{}d", weeks, remaining_days);
    }
    
    // Months but less than a year: show months and weeks
    if days < 365 {
        let months = days / 30;
        let remaining_weeks = (days % 30) / 7;
        if remaining_weeks == 0 {
            return format!("{}mo", months);
        }
        return format!("{}mo{}w", months, remaining_weeks);
    }
    
    // Years: show years and months
    let years = days / 365;
    let remaining_months = (days % 365) / 30;
    if remaining_months == 0 {
        return format!("{}y", years);
    }
    format!("{}y{}mo", years, remaining_months)
}
