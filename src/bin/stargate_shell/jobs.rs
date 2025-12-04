// Job control for background and foreground processes

use std::collections::HashMap;
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobStatus {
    Running,
    Stopped,
    Done(i32),
}

#[derive(Debug)]
pub struct Job {
    pub id: usize,
    pub pgid: Option<u32>,
    pub command: String,
    pub status: JobStatus,
    pub is_background: bool,
    child: Option<Child>,
}

impl Job {
    pub fn new(id: usize, command: String, is_background: bool, child: Child) -> Self {
        let pgid = child.id();
        Job {
            id,
            pgid: Some(pgid),
            command,
            status: JobStatus::Running,
            is_background,
            child: Some(child),
        }
    }

    pub fn check_status(&mut self) -> JobStatus {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(1);
                    self.status = JobStatus::Done(code);
                    self.status
                }
                Ok(None) => {
                    JobStatus::Running
                }
                Err(_) => {
                    self.status = JobStatus::Done(1);
                    self.status
                }
            }
        } else {
            self.status
        }
    }

    pub fn wait(&mut self) -> Result<ExitStatus, std::io::Error> {
        if let Some(mut child) = self.child.take() {
            child.wait()
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "No child process",
            ))
        }
    }
}

pub struct JobTable {
    jobs: HashMap<usize, Job>,
    next_id: usize,
}

impl JobTable {
    pub fn new() -> Self {
        JobTable {
            jobs: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn add_job(&mut self, command: String, is_background: bool, child: Child) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let job = Job::new(id, command, is_background, child);
        self.jobs.insert(id, job);
        id
    }

    pub fn get_job(&mut self, id: usize) -> Option<&mut Job> {
        self.jobs.get_mut(&id)
    }

    pub fn remove_job(&mut self, id: usize) -> Option<Job> {
        self.jobs.remove(&id)
    }

    pub fn list_jobs(&mut self) -> Vec<(usize, String, JobStatus)> {
        let mut result = Vec::new();
        for (id, job) in &mut self.jobs {
            job.check_status();
            result.push((*id, job.command.clone(), job.status));
        }
        result.sort_by_key(|(id, _, _)| *id);
        result
    }

    pub fn cleanup_done_jobs(&mut self) {
        let done_jobs: Vec<usize> = self
            .jobs
            .iter_mut()
            .filter_map(|(id, job)| {
                job.check_status();
                if matches!(job.status, JobStatus::Done(_)) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        for id in done_jobs {
            self.jobs.remove(&id);
        }
    }

    pub fn check_background_jobs(&mut self) -> Vec<String> {
        let mut messages = Vec::new();
        for (id, job) in &mut self.jobs {
            if job.is_background {
                let old_status = job.status;
                let new_status = job.check_status();
                
                if old_status != new_status {
                    match new_status {
                        JobStatus::Done(code) => {
                            messages.push(format!("job id [{}] finished with exit code [{}]  {}", id, code, job.command));
                        }
                        JobStatus::Stopped => {
                            messages.push(format!("[{}] Stopped {}", id, job.command));
                        }
                        _ => {}
                    }
                }
            }
        }
        messages
    }

    pub fn wait_for_job(&mut self, id: usize) -> Result<i32, String> {
        if let Some(job) = self.jobs.get_mut(&id) {
            match job.wait() {
                Ok(status) => {
                    let code = status.code().unwrap_or(1);
                    self.jobs.remove(&id);
                    Ok(code)
                }
                Err(e) => Err(format!("Failed to wait for job {}: {}", id, e)),
            }
        } else {
            Err(format!("No such job: {}", id))
        }
    }
}

lazy_static::lazy_static! {
    pub static ref JOB_TABLE: Arc<Mutex<JobTable>> = Arc::new(Mutex::new(JobTable::new()));
    pub static ref JOB_MONITOR: Arc<Mutex<Option<Sender<String>>>> = Arc::new(Mutex::new(None));
}

pub fn start_job_monitor() -> Receiver<String> {
    let (tx, rx) = channel();
    
    *JOB_MONITOR.lock().unwrap() = Some(tx.clone());
    
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(100));
            
            let messages = {
                let mut table = JOB_TABLE.lock().unwrap();
                let mut msgs = Vec::new();
                
                for (id, job) in &mut table.jobs {
                    if job.is_background {
                        let old_status = job.status;
                        let new_status = job.check_status();
                        
                        if old_status != new_status && matches!(new_status, JobStatus::Done(_)) {
                            match new_status {
                                JobStatus::Done(code) => {
                                    msgs.push(format!("[{}] Done (exit: {}) {}", id, code, job.command));
                                }
                                _ => {}
                            }
                        }
                    }
                }
                msgs
            };
            
            for msg in messages {
                if tx.send(msg).is_err() {
                    return;
                }
            }
        }
    });
    
    rx
}

pub fn add_background_job(command: String, child: Child) -> usize {
    let mut table = JOB_TABLE.lock().unwrap();
    table.add_job(command, true, child)
}

pub fn add_foreground_job(command: String, child: Child) -> usize {
    let mut table = JOB_TABLE.lock().unwrap();
    table.add_job(command, false, child)
}

pub fn list_jobs() -> Vec<(usize, String, JobStatus)> {
    let mut table = JOB_TABLE.lock().unwrap();
    table.list_jobs()
}

pub fn check_background_jobs() -> Vec<String> {
    let mut table = JOB_TABLE.lock().unwrap();
    table.check_background_jobs()
}

pub fn cleanup_done_jobs() {
    let mut table = JOB_TABLE.lock().unwrap();
    table.cleanup_done_jobs()
}

pub fn wait_for_job(id: usize) -> Result<i32, String> {
    let mut table = JOB_TABLE.lock().unwrap();
    table.wait_for_job(id)
}

pub fn bring_to_foreground(id: usize) -> Result<i32, String> {
    let mut table = JOB_TABLE.lock().unwrap();
    table.wait_for_job(id)
}
