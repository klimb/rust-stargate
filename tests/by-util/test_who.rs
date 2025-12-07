// spell-checker:ignore (flags) runlevel mesg

use sgtests::new_ucmd;
use sgtests::unwrap_or_return;
use sgtests::util::{TestScenario, expected_result, gnu_cmd_result};
use sgtests::util_name;
#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails_with_code(1);
}

#[test]
#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn test_boot() {
    let ts = TestScenario::new(util_name!());
    for opt in ["-b", "--boot", "--b"] {
        let expected_stdout = unwrap_or_return!(expected_result(&ts, &[opt])).stdout_move_str();
        ts.ucmd().arg(opt).succeeds().stdout_is(expected_stdout);
    }
}

#[cfg(unix)]
#[test]
#[cfg(unix)]
#[test]
#[cfg(unix)]
#[test]
#[cfg(not(target_os = "openbsd"))]
fn test_login() {
    let ts = TestScenario::new(util_name!());
    for opt in ["-l", "--login", "--log"] {
        let expected_stdout = unwrap_or_return!(expected_result(&ts, &[opt])).stdout_move_str();
        ts.ucmd().arg(opt).succeeds().stdout_is(expected_stdout);
    }
}

#[cfg(unix)]
#[test]
#[cfg(not(target_os = "openbsd"))]
fn test_m() {
    let ts = TestScenario::new(util_name!());
    let expected_stdout = unwrap_or_return!(expected_result(&ts, &["-m"])).stdout_move_str();
    ts.ucmd().arg("-m").succeeds().stdout_is(expected_stdout);
}

#[cfg(unix)]
#[test]
#[cfg(not(target_os = "openbsd"))]
fn test_process() {
    let ts = TestScenario::new(util_name!());
    for opt in ["-p", "--process", "--p"] {
        let expected_stdout = unwrap_or_return!(expected_result(&ts, &[opt])).stdout_move_str();
        ts.ucmd().arg(opt).succeeds().stdout_is(expected_stdout);
    }
}

#[cfg(unix)]
#[test]
#[cfg(not(target_os = "openbsd"))]
fn test_runlevel() {
    let ts = TestScenario::new(util_name!());
    for opt in ["-r", "--runlevel", "--r"] {
        let expected_stdout = unwrap_or_return!(expected_result(&ts, &[opt])).stdout_move_str();
        ts.ucmd().arg(opt).succeeds().stdout_is(expected_stdout);

        #[cfg(not(target_os = "linux"))]
        ts.ucmd().arg(opt).succeeds().stdout_is("");
    }
}

#[cfg(unix)]
#[test]
#[cfg(not(target_os = "openbsd"))]
fn test_time() {
    let ts = TestScenario::new(util_name!());
    for opt in ["-t", "--time", "--t"] {
        let expected_stdout = unwrap_or_return!(expected_result(&ts, &[opt])).stdout_move_str();
        ts.ucmd().arg(opt).succeeds().stdout_is(expected_stdout);
    }
}

#[cfg(unix)]
#[test]
#[cfg(unix)]
#[test]
#[cfg(not(target_os = "openbsd"))]
fn test_arg1_arg2() {
    let args = ["am", "i"];
    let ts = TestScenario::new(util_name!());
    let expected_stdout = unwrap_or_return!(expected_result(&ts, &args)).stdout_move_str();
    ts.ucmd().args(&args).succeeds().stdout_is(expected_stdout);
}

#[test]
fn test_too_many_args() {
    const EXPECTED: &str =
        "error: unexpected value 'u' for '[FILE]...' found; no more were expected";

    let args = ["am", "i", "u"];
    new_ucmd!().args(&args).fails().stderr_contains(EXPECTED);
}

#[cfg(unix)]
#[test]
#[cfg(unix)]
#[test]
#[cfg(unix)]
#[test]
#[cfg(not(target_os = "openbsd"))]
fn test_dead() {
    let ts = TestScenario::new(util_name!());
    for opt in ["-d", "--dead", "--de"] {
        let expected_stdout = unwrap_or_return!(expected_result(&ts, &[opt])).stdout_move_str();
        ts.ucmd().arg(opt).succeeds().stdout_is(expected_stdout);
    }
}
