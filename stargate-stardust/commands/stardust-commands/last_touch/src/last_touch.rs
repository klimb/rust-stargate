

use clap::{Arg, ArgAction, Command};
use sgcore::translate;
use sgcore::{
    error::SGResult,
    show_error,
    stardust_output::{self, StardustOutputOptions},
};
use std::thread;
use std::time::Duration;

#[cfg(target_vendor = "apple")]
use core_graphics::event_source::CGEventSourceStateID;

#[cfg(target_vendor = "apple")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(
        stateID: u32,
        eventType: u32,
    ) -> f64;
}

#[cfg(target_vendor = "apple")]
const K_CG_ANY_INPUT_EVENT_TYPE: u32 = !0;

#[cfg(target_vendor = "apple")]
const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_RIGHT_MOUSE_DRAGGED: u32 = 7;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_KEY_DOWN: u32 = 10;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_KEY_UP: u32 = 11;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_SCROLL_WHEEL: u32 = 22;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_TABLET_POINTER: u32 = 23;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;
#[cfg(target_vendor = "apple")]
const K_CG_EVENT_OTHER_MOUSE_DRAGGED: u32 = 27;

#[cfg(target_vendor = "apple")]
#[derive(Debug, Clone)]
struct EventInfo {
    seconds: f64,
    event_type: String,
}

#[cfg(target_vendor = "apple")]
fn get_seconds_since_last_event() -> Option<EventInfo> {
    unsafe {
        let state = CGEventSourceStateID::CombinedSessionState as u32;

        let events = vec![
            (K_CG_EVENT_KEY_DOWN, "keyboard"),
            (K_CG_EVENT_KEY_UP, "keyboard"),
            (K_CG_EVENT_FLAGS_CHANGED, "keyboard"),
            (K_CG_EVENT_LEFT_MOUSE_DOWN, "mouse_click"),
            (K_CG_EVENT_LEFT_MOUSE_UP, "mouse_click"),
            (K_CG_EVENT_RIGHT_MOUSE_DOWN, "mouse_click"),
            (K_CG_EVENT_RIGHT_MOUSE_UP, "mouse_click"),
            (K_CG_EVENT_OTHER_MOUSE_DOWN, "mouse_click"),
            (K_CG_EVENT_OTHER_MOUSE_UP, "mouse_click"),
            (K_CG_EVENT_MOUSE_MOVED, "mouse_move"),
            (K_CG_EVENT_LEFT_MOUSE_DRAGGED, "mouse_drag"),
            (K_CG_EVENT_RIGHT_MOUSE_DRAGGED, "mouse_drag"),
            (K_CG_EVENT_OTHER_MOUSE_DRAGGED, "mouse_drag"),
            (K_CG_EVENT_SCROLL_WHEEL, "scroll"),
            (K_CG_EVENT_TABLET_POINTER, "tablet"),
        ];

        let mut most_recent: Option<(f64, &str)> = None;

        for (event_type, name) in events {
            let seconds = CGEventSourceSecondsSinceLastEventType(state, event_type);
            if let Some((min_seconds, _)) = most_recent {
                if seconds < min_seconds {
                    most_recent = Some((seconds, name));
                }
            } else {
                most_recent = Some((seconds, name));
            }
        }

        most_recent.map(|(seconds, event_type)| EventInfo {
            seconds,
            event_type: event_type.to_string(),
        })
    }
}

#[cfg(not(target_vendor = "apple"))]
#[derive(Debug, Clone)]
struct EventInfo {
    seconds: f64,
    event_type: String,
}

#[cfg(not(target_vendor = "apple"))]
fn get_seconds_since_last_event() -> Option<EventInfo> {
    None
}

fn format_duration_human_readable(seconds: f64) -> String {
    let total_seconds = seconds as u64;

    if total_seconds < 60 {
        format!("{} seconds ago", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let secs = total_seconds % 60;
        if secs == 0 {
            format!("{} minute{} ago", minutes, if minutes == 1 { "" } else { "s" })
        } else {
            format!("{} minute{} {} second{} ago",
                minutes, if minutes == 1 { "" } else { "s" },
                secs, if secs == 1 { "" } else { "s" })
        }
    } else if total_seconds < 86400 {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        if minutes == 0 {
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else {
            format!("{} hour{} {} minute{} ago",
                hours, if hours == 1 { "" } else { "s" },
                minutes, if minutes == 1 { "" } else { "s" })
        }
    } else {
        let days = total_seconds / 86400;
        let hours = (total_seconds % 86400) / 3600;
        if hours == 0 {
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        } else {
            format!("{} day{} {} hour{} ago",
                days, if days == 1 { "" } else { "s" },
                hours, if hours == 1 { "" } else { "s" })
        }
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    #[cfg(not(target_vendor = "apple"))]
    {
        return Ok(());
    }

    #[cfg(target_vendor = "apple")]
    {
    let object_output = StardustOutputOptions::from_matches(&matches);

    let number: usize = matches.get_one::<String>("number")
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    if object_output.stardust_output {
        let mut samples = Vec::new();
        for _ in 0..number {
            if let Some(event_info) = get_seconds_since_last_event() {
                use chrono::Utc;
                let now = Utc::now();
                let last_event_time = now - chrono::Duration::milliseconds((event_info.seconds * 1000.0) as i64);

                samples.push(serde_json::json!({
                    "seconds_since_last_event": event_info.seconds,
                    "event_type": event_info.event_type,
                    "human_readable": format_duration_human_readable(event_info.seconds),
                    "last_event_timestamp": last_event_time.to_rfc3339(),
                    "current_timestamp": now.to_rfc3339()
                }));

                if samples.len() < number {
                    thread::sleep(Duration::from_millis(500));
                }
            }
        }

        let output = if number == 1 {
            samples.into_iter().next().unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!(samples)
        };

        stardust_output::output(object_output, output, || Ok(()))?;
    } else {
        for i in 0..number {
            match get_seconds_since_last_event() {
                Some(event_info) => {
                    use chrono::Local;
                    let now = Local::now();
                    let timestamp = now.format("%H:%M:%S");
                    let human_readable = format_duration_human_readable(event_info.seconds);

                    if number == 1 {
                        println!("{} ({})", human_readable, event_info.event_type);
                    } else {
                        println!("[{}] {} ({})", timestamp, human_readable, event_info.event_type);
                    }

                    if i < number - 1 {
                        thread::sleep(Duration::from_millis(500));
                    }
                }
                None => {
                    show_error!("{}", translate!("last_touch-error-cannot-get-idle-time"));
                    break;
                }
            }
        }
    }

    Ok(())
    }
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .override_usage(sgcore::util_name())
        .about(translate!("last_touch-about"))
        .infer_long_args(true)
        .arg(
            Arg::new("number")
                .short('n')
                .long("number")
                .help(translate!("last_touch-help-number"))
                .value_name("NUM")
                .default_value("20")
                .action(ArgAction::Set)
        );

    stardust_output::add_json_args(cmd)
}

