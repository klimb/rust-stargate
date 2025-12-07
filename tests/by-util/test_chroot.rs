// spell-checker:ignore (words) araba newroot userspec chdir pwd's isroot

use sgtests::at_and_ucmd;
use sgtests::new_ucmd;
use sgtests::util::is_ci;
use sgtests::util::{TestScenario, run_ucmd_as_root};
use sgtests::util_name;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails_with_code(125);
}

#[test]
fn test_missing_operand() {
    let result = new_ucmd!().fails_with_code(125);

    assert!(
        result
            .stderr_str()
            .starts_with("error: the following required arguments were not provided")
    );

    assert!(result.stderr_str().contains("<newroot>"));
}

#[test]
fn test_enter_chroot_fails() {
    // NOTE: since #2689 this test also ensures that we don't regress #2687
    let (at, mut ucmd) = at_and_ucmd!();

    at.mkdir("jail");

    let result = ucmd.arg("jail").fails_with_code(125);
    assert!(
        result
            .stderr_str()
            .starts_with("chroot: cannot chroot to 'jail': Operation not permitted (os error 1)")
    );
}

#[test]
fn test_no_such_directory() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.touch(at.plus_as_string("a"));

    ucmd.arg("a")
        .fails_with_code(125)
        .stderr_is("chroot: cannot change root directory to 'a': no such directory\n");
}

#[test]
fn test_preference_of_userspec() {
    let scene = TestScenario::new(util_name!());
    let result = scene.cmd("whoami").run();
    if is_ci() && result.stderr_str().contains("No such user/group") {
        // In the CI, some server are failing to return whoami.
        // As seems to be a configuration issue, ignoring it
        return;
    }
    println!("result.stdout = {}", result.stdout_str());
    println!("result.stderr = {}", result.stderr_str());
    let username = result.stdout_str().trim_end();

    let ts = TestScenario::new("id");
    let result = ts.cmd("id").arg("-g").arg("-n").run();
    println!("result.stdout = {}", result.stdout_str());
    println!("result.stderr = {}", result.stderr_str());

    if is_ci() && result.stderr_str().contains("cannot find name for user ID") {
        // In the CI, some server are failing to return id.
        // As seems to be a configuration issue, ignoring it
        return;
    }

    let group_name = result.stdout_str().trim_end();
    let (at, mut ucmd) = at_and_ucmd!();

    at.mkdir("a");

    // `--user` is an abbreviation of `--userspec`.
    let result = ucmd
        .arg("a")
        .arg("--user")
        .arg("fake")
        .arg("--groups")
        .arg("ABC,DEF")
        .arg(format!("--userspec={username}:{group_name}"))
        .fails_with_code(125);

    println!("result.stdout = {}", result.stdout_str());
    println!("result.stderr = {}", result.stderr_str());
}

#[test]
fn test_chroot_skip_chdir_not_root() {
    let (at, mut ucmd) = at_and_ucmd!();

    let dir = "foobar";
    at.mkdir(dir);

    ucmd.arg("--skip-chdir")
        .arg(dir)
        .fails_with_code(125)
        .stderr_contains("chroot: option --skip-chdir only permitted if NEWROOT is old '/'");
}

