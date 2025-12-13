

#![no_main]
use libfuzzer_sys::fuzz_target;
use sg_sort::sgmain;

use rand::Rng;
use std::env;
use std::ffi::OsString;

use sgfuzz::CommandResult;
use sgfuzz::{compare_result, generate_and_run_uumain, generate_random_string, run_gnu_cmd};
static CMD_PATH: &str = "sort";

fn generate_sort_args() -> String {
    let mut rng = rand::rng();

    let arg_count = rng.random_range(1..=5);
    let mut args = Vec::new();

    for _ in 0..arg_count {
        match rng.random_range(0..=4) {
            0 => args.push(String::from("-r")),
            1 => args.push(String::from("-n")),
            2 => args.push(String::from("-f")),
            3 => args.push(generate_random_string(rng.random_range(1..=10))),
            _ => args.push(String::from("-k") + &rng.random_range(1..=5).to_string()),
        }
    }

    args.join(" ")
}

fn generate_random_lines(count: usize) -> String {
    let mut rng = rand::rng();
    let mut lines = Vec::new();

    for _ in 0..count {
        lines.push(generate_random_string(rng.random_range(1..=20)));
    }

    lines.join("\n")
}

fuzz_target!(|_data: &[u8]| {
    let sort_args = generate_sort_args();
    let mut args = vec![OsString::from("sort")];
    args.extend(sort_args.split_whitespace().map(OsString::from));

    let input_lines = generate_random_lines(10);

    let rust_result = generate_and_run_uumain(&args, uumain, Some(&input_lines));

    unsafe {
        env::set_var("LC_ALL", "C");
    }
    let gnu_result = match run_gnu_cmd(CMD_PATH, &args[1..], false, Some(&input_lines)) {
        Ok(result) => result,
        Err(error_result) => {
            eprintln!("Failed to run GNU command:");
            eprintln!("Stderr: {}", error_result.stderr);
            eprintln!("Exit Code: {}", error_result.exit_code);
            CommandResult {
                stdout: String::new(),
                stderr: error_result.stderr,
                exit_code: error_result.exit_code,
            }
        }
    };

    compare_result(
        "sort",
        &format!("{:?}", &args[1..]),
        None,
        &rust_result,
        &gnu_result,
        false,
    );
});

