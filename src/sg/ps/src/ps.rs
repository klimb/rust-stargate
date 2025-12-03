// spell-checker:ignore procfs

use clap::{Arg, ArgAction, Command};
use sgcore::error::UResult;
use sgcore::format_usage;
use sgcore::translate;
use sgcore::object_output::{self, JsonOutputOptions};
use serde_json::json;
use procfs::process::{Process, all_processes};

static ARG_ALL: &str = "all";
static ARG_FULL: &str = "full";

#[derive(Debug)]
struct ProcessInfo {
    pid: i32,
    ppid: i32,
    name: String,
    cmdline: Vec<String>,
    state: String,
    cpu_time: u64,
    mem_usage: u64,
}

impl ProcessInfo {
    fn from_process(process: &Process) -> Option<Self> {
        let stat = process.stat().ok()?;
        let cmdline = process.cmdline().ok().unwrap_or_default();
        let status = process.status().ok()?;
        
        // Calculate total CPU time in seconds
        let cpu_time = (stat.utime + stat.stime) / procfs::ticks_per_second();
        
        // Get memory usage in KB
        let mem_usage = status.vmrss.unwrap_or(0);
        
        Some(ProcessInfo {
            pid: process.pid(),
            ppid: stat.ppid,
            name: stat.comm.clone(),
            cmdline,
            state: format!("{:?}", stat.state()),
            cpu_time,
            mem_usage,
        })
    }
}

#[sgcore::main]
pub fn uumain(args: impl sgcore::Args) -> UResult<()> {
    let matches = sgcore::clap_localization::handle_clap_result(uu_app(), args)?;
    let mut opts = JsonOutputOptions::from_matches(&matches);
    
    // Object output is the default for this command
    // Only use default if user didn't explicitly set the flag
    if matches.value_source("object_output") != Some(clap::parser::ValueSource::CommandLine) {
        opts.object_output = true;
    }
    
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

fn output_json(processes: Vec<ProcessInfo>, opts: JsonOutputOptions, show_full: bool) -> UResult<()> {
    let process_list: Vec<serde_json::Value> = processes.iter().map(|p| {
        let mut obj = json!({
            "pid": p.pid,
            "ppid": p.ppid,
            "name": p.name,
            "state": p.state,
        });
        
        if show_full {
            obj["cmdline"] = json!(p.cmdline);
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

fn output_text(processes: &[ProcessInfo], show_full: bool) {
    if show_full {
        println!("{:>7} {:>7} {:>10} {:>10} {:<20} {}", 
                 "PID", "PPID", "CPU(s)", "MEM(KB)", "STATE", "NAME");
    } else {
        println!("{:>7} {:>7} {:<20} {}", 
                 "PID", "PPID", "STATE", "NAME");
    }
    
    for p in processes {
        if show_full {
            let cmd = if !p.cmdline.is_empty() {
                p.cmdline.join(" ")
            } else {
                format!("[{}]", p.name)
            };
            println!("{:>7} {:>7} {:>10} {:>10} {:<20} {}", 
                     p.pid, p.ppid, p.cpu_time, p.mem_usage, p.state, cmd);
        } else {
            println!("{:>7} {:>7} {:<20} {}", 
                     p.pid, p.ppid, p.state, p.name);
        }
    }
}

pub fn uu_app() -> Command {
    let cmd = Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("ps-about"))
        .override_usage(format_usage(&translate!("ps-usage")))
        .infer_long_args(true)
        .arg(
            Arg::new(ARG_ALL)
                .short('a')
                .long("all")
                .help(translate!("ps-help-all"))
                .action(ArgAction::SetTrue)
        )
        .arg(
            Arg::new(ARG_FULL)
                .long("full")
                .help(translate!("ps-help-full"))
                .action(ArgAction::SetTrue)
        );
    
    object_output::add_json_args(cmd)
}
