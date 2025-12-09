// spell-checker:ignore procfs

use clap::{Arg, ArgAction, Command};
use sgcore::error::UResult;
use sgcore::format_usage;
use sgcore::translate;
use sgcore::object_output::{self, JsonOutputOptions};
use serde_json::json;
use std::collections::HashMap;
use std::sync::OnceLock;

#[cfg(target_os = "linux")]
use procfs::process::{Process, all_processes};

#[cfg(target_os = "macos")]
use sysinfo::{System, ProcessesToUpdate};

static ARG_ALL: &str = "all";
static ARG_FULL: &str = "full";

#[derive(Debug)]
struct ProcessInfo {
    pid: i32,
    ppid: i32,
    uid: u32,
    name: String,
    cmdline: Vec<String>,
    state: String,
    cpu_time: u64,
    mem_usage: u64,
}

#[cfg(target_os = "linux")]
impl ProcessInfo {
    fn from_process(process: &Process) -> Option<Self> {
        let stat = process.stat().ok()?;
        let cmdline = process.cmdline().ok().unwrap_or_default();
        let status = process.status().ok()?;
        
        // Calculate total CPU time in seconds
        let cpu_time = (stat.utime + stat.stime) / procfs::ticks_per_second();
        
        // Get memory usage in KB
        let mem_usage = status.vmrss.unwrap_or(0);
        
        // Get UID
        let uid = status.ruid;
        
        Some(ProcessInfo {
            pid: process.pid(),
            ppid: stat.ppid,
            uid,
            name: stat.comm.clone(),
            cmdline,
            state: format!("{:?}", stat.state()),
            cpu_time,
            mem_usage,
        })
    }
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(sg_app(), args)?;
    sgcore::pledge::apply_pledge(&["stdio", "rpath", "proc"])?;
    let opts = JsonOutputOptions::from_matches(&matches);
    
    let show_all = matches.get_flag(ARG_ALL);
    let show_full = matches.get_flag(ARG_FULL);
    
    let processes = collect_processes(show_all)?;
    
    if opts.object_output {
        output_json(processes, opts, show_full)?;
    } else {
        output_text(&processes, show_full);
    }
    
    Ok(())
}

#[cfg(target_os = "linux")]
fn collect_processes(show_all: bool) -> UResult<Vec<ProcessInfo>> {
    let mut processes = Vec::new();
    let current_pid = std::process::id() as i32;
    
    for prc in all_processes().map_err(|e| {
        sgcore::error::USimpleError::new(1, format!("Failed to read processes: {}", e))
    })? {
        let process = prc.map_err(|e| {
            sgcore::error::USimpleError::new(1, format!("Failed to read process: {}", e))
        })?;
        
        // Skip the current process (ps itself) unless --all is specified
        if !show_all && process.pid() == current_pid {
            continue;
        }
        
        if let Some(info) = ProcessInfo::from_process(&process) {
            processes.push(info);
        }
    }
    
    // Sort by PID
    processes.sort_by_key(|p| p.pid);
    
    Ok(processes)
}

#[cfg(target_os = "macos")]
fn collect_processes(show_all: bool) -> UResult<Vec<ProcessInfo>> {
    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    
    let current_pid = std::process::id() as i32;
    let mut processes = Vec::new();
    
    for (pid, process) in sys.processes() {
        let pid_i32 = pid.as_u32() as i32;
        
        // Skip the current process unless --all is specified
        if !show_all && pid_i32 == current_pid {
            continue;
        }
        
        let ppid = process.parent().map(|p| p.as_u32() as i32).unwrap_or(0);
        
        // Get UID - default to 0 if not available
        let uid = process.user_id()
            .and_then(|id| id.to_string().parse::<u32>().ok())
            .unwrap_or(0);
        
        let name = process.name().to_string_lossy().to_string();
        
        let cmdline: Vec<String> = process.cmd().iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect();
        
        let state = format!("{:?}", process.status());
        
        // CPU time in seconds
        let cpu_time = process.run_time();
        
        // Memory usage in KB
        let mem_usage = process.memory() / 1024;
        
        processes.push(ProcessInfo {
            pid: pid_i32,
            ppid,
            uid,
            name,
            cmdline,
            state,
            cpu_time,
            mem_usage,
        });
    }
    
    // Sort by PID
    processes.sort_by_key(|p| p.pid);
    
    Ok(processes)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn collect_processes(_show_all: bool) -> UResult<Vec<ProcessInfo>> {
    Err(sgcore::error::USimpleError::new(1, "list-processes is not supported on this platform"))
}

fn output_json(processes: Vec<ProcessInfo>, opts: JsonOutputOptions, show_full: bool) -> UResult<()> {
    let process_list: Vec<serde_json::Value> = processes.iter().map(|p| {
        let mut obj = json!({
            "pid": p.pid,
            "ppid": p.ppid,
            "uid": p.uid,
            "user": get_username(p.uid),
            "name": p.name,
            "state": p.state,
        });
        
        if show_full || !p.cmdline.is_empty() {
            obj["cmdline"] = json!(p.cmdline);
        }
        
        if show_full {
            obj["cpu_time"] = json!(p.cpu_time);
            obj["mem_kb"] = json!(p.mem_usage);
        }
        
        obj
    }).collect();
    
    let output = json!({
        "processes": process_list,
        "count": processes.len(),
    });
    
    object_output::output(opts, output, || Ok(()))?;
    Ok(())
}

fn get_username(uid: u32) -> String {
    static UID_CACHE: OnceLock<HashMap<u32, String>> = OnceLock::new();
    
    let cache = UID_CACHE.get_or_init(|| {
        let mut map = HashMap::new();
        // Read /etc/passwd to build UID to username mapping
        if let Ok(contents) = std::fs::read_to_string("/etc/passwd") {
            for line in contents.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    if let Ok(uid_val) = parts[2].parse::<u32>() {
                        map.insert(uid_val, parts[0].to_string());
                    }
                }
            }
        }
        map
    });
    
    cache.get(&uid).cloned().unwrap_or_else(|| uid.to_string())
}

fn output_text(processes: &[ProcessInfo], _show_full: bool) {
    println!("PID\tUSER\tCOMMAND");
    
    for p in processes {
        let user = get_username(p.uid);
        let command = if !p.cmdline.is_empty() {
            p.cmdline.join(" ")
        } else {
            format!("[{}]", p.name)
        };
        
        println!("{}\t{}\t{}", p.pid, user, command);
    }
}

pub fn sg_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("list-processes-about"))
        .override_usage(format_usage(&translate!("list-processes-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(ARG_ALL)
                .short('a')
                .long("all")
                .help(translate!("list-processes-help-all"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(ARG_FULL)
                .long("full")
                .help(translate!("list-processes-help-full"))
                .action(ArgAction::SetTrue)
        );
    
    object_output::add_json_args(cmd)
}
