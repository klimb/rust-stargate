

#![no_main]
use libfuzzer_sys::fuzz_target;
use sg_expr::sgmain;

use rand::Rng;
use rand::prelude::IndexedRandom;
use std::{env, ffi::OsString};

use sgfuzz::CommandResult;
use sgfuzz::{compare_result, generate_and_run_uumain, generate_random_string, run_gnu_cmd};
static CMD_PATH: &str = "expr";

fn generate_expr(max_depth: u32) -> String {
    let mut rng = rand::rng();
    let ops = [
        "+", "-", "*", "/", "%", "<", ">", "=", "&", "|", "!=", "<=", ">=", ":", "index", "length",
        "substr",
    ];

    let mut expr = String::new();
    let mut depth = 0;
    let mut last_was_operator = false;

    while depth <= max_depth {
        if last_was_operator || depth == 0 {
            expr.push_str(&rng.random_range(1..=100).to_string());
            last_was_operator = false;
        } else {
            if rng.random_bool(0.9) {
                let op = *ops.choose(&mut rng).unwrap();
                expr.push_str(&format!(" {op} "));
                last_was_operator = true;
            }
            else {
                let random_str = generate_random_string(rng.random_range(1..=10));
                expr.push_str(&random_str);
                last_was_operator = false;
            }
        }
        depth += 1;
    }

    if last_was_operator {
        expr.push_str(&rng.random_range(1..=100).to_string());
    }

    expr
}

fuzz_target!(|_data: &[u8]| {
    let mut rng = rand::rng();
    let expr = generate_expr(rng.random_range(0..=20));
    let mut args = vec![OsString::from("expr")];
    args.extend(expr.split_whitespace().map(OsString::from));

    unsafe {
        env::set_var("LC_COLLATE", "C");
    }
    let rust_result = generate_and_run_uumain(&args, uumain, None);

    let gnu_result = match run_gnu_cmd(CMD_PATH, &args[1..], false, None) {
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
        "expr",
        &format!("{:?}", &args[1..]),
        None,
        &rust_result,
        &gnu_result,
        false,
    );
});

