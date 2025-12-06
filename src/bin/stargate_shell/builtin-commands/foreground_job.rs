// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use crate::stargate_shell::jobs::bring_to_foreground;

pub fn execute(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("foreground-job: no job specified".to_string());
    }
    
    let job_id_str = args[0].trim_start_matches('%');
    let job_id: usize = job_id_str.parse()
        .map_err(|_| format!("foreground-job: invalid job id: {}", job_id_str))?;
    
    match bring_to_foreground(job_id) {
        Ok(exit_code) => {
            if exit_code != 0 {
                return Err(format!("job exited with code:{}", exit_code));
            }
            Ok(())
        }
        Err(e) => Err(format!("foreground-job: {}", e))
    }
}
