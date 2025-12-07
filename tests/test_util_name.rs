// spell-checker:ignore readelf

use sgtests::util::TestScenario;

use std::os::unix::fs::symlink as symlink_file;

use std::env;
pub const TESTS_BINARY: &str = env!("CARGO_BIN_EXE_stargate");

// Set the environment variable for any tests

// Use the ctor attribute to run this function before any tests
#[ctor::ctor]
fn init() {
    // No need for unsafe here
    unsafe {
        std::env::set_var("SGTESTS_BINARY_PATH", TESTS_BINARY);
    }
    // Print for debugging
    eprintln!("Setting SGTESTS_BINARY_PATH={TESTS_BINARY}");
}

#[test]
#[cfg(feature = "ls")]
fn execution_phrase_double() {
    use std::process::Command;

    let scenario = TestScenario::new("ls");
    if !scenario.bin_path.exists() {
        println!("Skipping test: Binary not found at {:?}", scenario.bin_path);
        return;
    }
    let output = Command::new(&scenario.bin_path)
        .arg("ls")
        .arg("--some-invalid-arg")
        .env("LANG", "en_US.UTF-8")
        .output()
        .unwrap();
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains(&"Usage: ls".to_string())
    );
}

#[test]
#[cfg(unix)]
fn util_invalid_name_help() {
    use std::process::{Command, Stdio};

    let scenario = TestScenario::new("invalid_name");
    if !scenario.bin_path.exists() {
        println!("Skipping test: Binary not found at {:?}", scenario.bin_path);
        return;
    }
    symlink_file(&scenario.bin_path, scenario.fixtures.plus("invalid_name")).unwrap();
    let child = Command::new(scenario.fixtures.plus("invalid_name"))
        .arg("--help")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stderr, b"");
    let output_str = String::from_utf8(output.stdout).unwrap();
    assert!(output_str.contains("(multi-call binary)"), "{output_str:?}");
    assert!(
        output_str.contains("Usage: invalid_name [function "),
        "{output_str:?}"
    );
}

// The exact set of permitted filenames depends on many factors. Non-UTF-8 strings
// work on very few platforms, but linux works, especially because it also increases
// the likelihood that a filesystem is being used that supports non-UTF-8 filenames.
#[cfg(target_os = "linux")]
fn util_non_utf8_name_help() {
    // Make sure we don't crash even if the util name is invalid UTF-8.
    use std::{
        ffi::OsStr,
        os::unix::ffi::OsStrExt,
        process::{Command, Stdio},
    };

    let scenario = TestScenario::new("invalid_name");
    let non_utf8_path = scenario.fixtures.plus(OsStr::from_bytes(b"\xff"));
    if !scenario.bin_path.exists() {
        println!("Skipping test: Binary not found at {:?}", scenario.bin_path);
        return;
    }

    symlink_file(&scenario.bin_path, &non_utf8_path).unwrap();
    let child = Command::new(&non_utf8_path)
        .arg("--help")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stderr, b"");
    let output_str = String::from_utf8(output.stdout).unwrap();
    assert!(output_str.contains("(multi-call binary)"), "{output_str:?}");
    assert!(
        output_str.contains("Usage: <unknown binary name> [function "),
        "{output_str:?}"
    );
}

#[test]
#[cfg(unix)]
fn util_invalid_name_invalid_command() {
    use std::process::{Command, Stdio};

    let scenario = TestScenario::new("invalid_name");
    symlink_file(&scenario.bin_path, scenario.fixtures.plus("invalid_name")).unwrap();
    if !scenario.bin_path.exists() {
        println!("Skipping test: Binary not found at {:?}", scenario.bin_path);
        return;
    }

    let child = Command::new(scenario.fixtures.plus("invalid_name"))
        .arg("definitely_invalid")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stderr, b"");
    assert_eq!(
        output.stdout,
        b"definitely_invalid: function/utility not found\n"
    );
}

#[test]
fn util_version() {
    use std::process::{Command, Stdio};

    let scenario = TestScenario::new("--version");
    if !scenario.bin_path.exists() {
        println!("Skipping test: Binary not found at {:?}", scenario.bin_path);
        return;
    }
    for arg in ["-V", "--version"] {
        let child = Command::new(&scenario.bin_path)
            .arg(arg)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stderr, b"");
        let output_str = String::from_utf8(output.stdout).unwrap();
        let ver = std::env::var("CARGO_PKG_VERSION").unwrap();
        let expected = format!(
            "This is stargate {}, built on Rust.\n\nCopyright (c) 2025 Dmitry Kalashnikov\n\nDual Licensed: Open-Source (non-commercial) / Commercial (proprietary use)\nCommercial use requires a Commercial License.\nSee LICENSE file or contact author for details.\n",
            ver
        );
        assert_eq!(expected, output_str);
    }
}

#[test]
#[cfg(target_env = "musl")]
fn test_musl_no_dynamic_deps() {
    use std::process::Command;

    let scenario = TestScenario::new("test_musl_no_dynamic_deps");
    if !scenario.bin_path.exists() {
        println!("Skipping test: Binary not found at {:?}", scenario.bin_path);
        return;
    }

    let output = Command::new("readelf")
        .arg("-d")
        .arg(&scenario.bin_path)
        .output()
        .expect("Failed to run readelf");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Static binaries should have no NEEDED entries (dynamic library dependencies)
    assert!(
        !stdout.contains("NEEDED"),
        "Found dynamic dependencies in musl binary:\n{}",
        stdout
    );
}
