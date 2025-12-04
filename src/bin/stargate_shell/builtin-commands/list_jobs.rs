use crate::stargate_shell::jobs::{list_jobs, JobStatus};

pub fn execute() -> Result<(), String> {
    let jobs = list_jobs();
    
    if jobs.is_empty() {
        return Ok(());
    }
    
    for (id, command, status) in jobs {
        let status_str = match status {
            JobStatus::Running => "Running",
            JobStatus::Stopped => "Stopped",
            JobStatus::Done(code) => &format!("Done({})", code),
        };
        println!("[{}] {:10} {}", id, status_str, command);
    }
    
    Ok(())
}
