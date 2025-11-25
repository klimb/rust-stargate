// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use uutests::new_ucmd;
use tempfile::NamedTempFile;

#[test]
fn test_help() {
    for help_flg in ["-h", "--help"] {
        new_ucmd!()
            .arg(help_flg)
            .succeeds()
            .no_stderr()
            .stdout_contains("Usage:");
    }
}

#[test]
fn test_version() {
    for version_flg in ["-V", "--version"] {
        assert!(
            new_ucmd!()
                .arg(version_flg)
                .succeeds()
                .no_stderr()
                .stdout_str()
                .starts_with("find-text")
        );
    }
}

#[test]
fn test_search_single_match() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello World\nGoodbye\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("Hello")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("Hello World");
}

#[test]
fn test_search_multiple_matches() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello World\nHello there\nGoodbye Hello\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("Hello")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("Hello World")
        .stdout_contains("Hello there")
        .stdout_contains("Goodbye Hello");
}

#[test]
fn test_search_no_matches() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello World\nGoodbye\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("NotFound")
        .arg(&file_path)
        .succeeds()
        .stdout_is("");
}

#[test]
fn test_search_case_sensitive() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello\nhello\nHELLO\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("Hello")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("Hello")
        .stdout_does_not_contain("hello");
}

#[test]
fn test_search_case_insensitive() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello\nhello\nHELLO\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("-i")
        .arg("hello")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("Hello")
        .stdout_contains("hello")
        .stdout_contains("HELLO");
}

#[test]
fn test_search_case_insensitive_long_flag() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello\nhello\nHELLO\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("--ignore-case")
        .arg("hello")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("Hello")
        .stdout_contains("hello")
        .stdout_contains("HELLO");
}

#[test]
fn test_search_partial_match() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"The quick fox\nA slower fox\nFoxy lady\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("fox")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("quick fox")
        .stdout_contains("slower fox")
        .stdout_contains("Foxy lady");
}

#[test]
fn test_search_special_characters() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"test@example.com\nuser@domain.org\nno email here\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("@")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("test@example.com")
        .stdout_contains("user@domain.org")
        .stdout_does_not_contain("no email");
}

#[test]
fn test_search_numbers() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Version 1.0\nVersion 2.0\nRelease 1.5\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("1")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("1.0")
        .stdout_contains("1.5");
}

#[test]
fn test_search_empty_file() {
    let file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = file.path().to_string_lossy().to_string();

    new_ucmd!()
        .arg("pattern")
        .arg(&file_path)
        .succeeds()
        .stdout_is("");
}

#[test]
fn test_search_preserves_whitespace() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"  Leading spaces\nTrailing spaces  \n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("spaces")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("  Leading spaces")
        .stdout_contains("Trailing spaces  ");
}

#[test]
fn test_search_unicode_content() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all("Hello 世界\nПривет мир\n".as_bytes())
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("世界")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("世界");
}

#[test]
fn test_search_long_lines() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let long_line = format!("{}needle{}", "a".repeat(500), "b".repeat(500));
    file.write_all(long_line.as_bytes())
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("needle")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("needle");
}

#[test]
fn test_search_many_lines() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    let mut content = String::new();
    for i in 0..100 {
        content.push_str(&format!("Line {}\n", i));
    }
    content.push_str("Target line\n");
    file.write_all(content.as_bytes())
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("Target")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("Target line");
}

#[test]
fn test_json_output() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello World\nHello there\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("-o")
        .arg("Hello")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("\"pattern\":\"Hello\"")
        .stdout_contains("\"found\":true")
        .stdout_contains("\"total_matches\":2");
}

#[test]
fn test_json_output_long_flag() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello World\nHello there\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("--obj")
        .arg("Hello")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("\"pattern\":\"Hello\"")
        .stdout_contains("\"found\":true")
        .stdout_contains("\"total_matches\":2");
}

#[test]
fn test_json_output_no_matches() {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(b"Hello World\n")
        .expect("Failed to write to temp file");
    file.flush().expect("Failed to flush temp file");

    let file_path = file.path().to_string_lossy().to_string();
    new_ucmd!()
        .arg("-o")
        .arg("NotFound")
        .arg(&file_path)
        .succeeds()
        .stdout_contains("\"pattern\":\"NotFound\"")
        .stdout_contains("\"found\":false")
        .stdout_contains("\"total_matches\":0");
}

#[test]
fn test_multiple_files() {
    let mut file1 = NamedTempFile::new().expect("Failed to create temp file");
    file1
        .write_all(b"Hello World\n")
        .expect("Failed to write to temp file");
    file1.flush().expect("Failed to flush temp file");

    let mut file2 = NamedTempFile::new().expect("Failed to create temp file");
    file2
        .write_all(b"Goodbye World\nHello again\n")
        .expect("Failed to write to temp file");
    file2.flush().expect("Failed to flush temp file");

    let file1_path = file1.path().to_string_lossy().to_string();
    let file2_path = file2.path().to_string_lossy().to_string();

    new_ucmd!()
        .arg("Hello")
        .arg(&file1_path)
        .arg(&file2_path)
        .succeeds()
        .stdout_contains(&file1_path)
        .stdout_contains(&file2_path)
        .stdout_contains("Hello World")
        .stdout_contains("Hello again");
}

#[test]
fn test_no_pattern_error() {
    new_ucmd!().fails();
}

#[test]
fn test_no_files_error() {
    new_ucmd!().arg("pattern").fails();
}
