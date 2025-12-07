use sgtests::new_ucmd;
#[cfg(any(target_vendor = "apple", target_os = "linux"))]
use sgtests::{util::TestScenario, util_name};

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails_with_code(1);
}

#[test]
fn test_users_no_arg() {
    new_ucmd!().succeeds();
}

#[test]
#[cfg(any(target_vendor = "apple", target_os = "linux"))]
#[test]
#[cfg(target_os = "openbsd")]
fn test_users_check_name_openbsd() {
    new_ucmd!()
        .args(&["openbsd_utmp"])
        .succeeds()
        .stdout_contains("test");
}
