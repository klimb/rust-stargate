pub use self::unix::{
    Pid,
    ProcessChecker,
    //stdin_is_bad_fd, stdin_is_pipe_or_fifo, supports_pid_checks, Pid, ProcessChecker,
    supports_pid_checks,
};
mod unix;


