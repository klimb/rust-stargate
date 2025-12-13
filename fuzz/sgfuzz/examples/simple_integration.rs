use std::ffi::OsString;
use sgfuzz::{CommandResult, run_gnu_cmd};

fn main() {
    println!("=== Simple Integration Testing sgfuzz Example ===");
    println!("This demonstrates how to use sgfuzz to compare against GNU tools");
    println!("without the complex file descriptor manipulation.\n");

    let test_cases = [
        (
            "echo test",
            "echo",
            vec![OsString::from("hello"), OsString::from("world")],
            None,
        ),
        (
            "echo with flag",
            "echo",
            vec![OsString::from("-n"), OsString::from("no-newline")],
            None,
        ),
        (
            "cat with input",
            "cat",
            vec![],
            Some("Hello from cat!\nLine 2\n"),
        ),
        ("sort basic", "sort", vec![], Some("zebra\napple\nbanana\n")),
        (
            "sort numeric",
            "sort",
            vec![OsString::from("-n")],
            Some("10\n2\n1\n20\n"),
        ),
    ];

    for (test_name, cmd, args, input) in test_cases {
        println!("--- {} ---", test_name);

        match run_gnu_cmd(cmd, &args, false, input) {
            Ok(gnu_result) => {
                println!("✓ GNU {} succeeded", cmd);
                println!(
                    "  Stdout: {:?}",
                    gnu_result.stdout.trim().replace('\n', "\\n")
                );
                println!("  Exit code: {}", gnu_result.exit_code);

            }
            Err(error_result) => {
                println!(
                    "⚠ GNU {} failed or not available: {}",
                    cmd, error_result.stderr
                );
                println!("  This is normal if GNU coreutils isn't installed");
            }
        }
        println!();
    }

    println!("=== Practical Example: Compare two echo implementations ===");

    let args = vec![OsString::from("hello"), OsString::from("world")];
    match run_gnu_cmd("echo", &args, false, None) {
        Ok(gnu_result) => {
            println!("GNU echo result: {:?}", gnu_result.stdout.trim());

            let our_result = CommandResult {
                stdout: "hello world\n".to_string(),
                stderr: String::new(),
                exit_code: 0,
            };

            if our_result.stdout.trim() == gnu_result.stdout.trim()
                && our_result.exit_code == gnu_result.exit_code
            {
                println!("✓ Our echo matches GNU echo!");
            } else {
                println!("✗ Our echo differs from GNU echo");
                println!("  Our result: {:?}", our_result.stdout.trim());
                println!("  GNU result: {:?}", gnu_result.stdout.trim());
            }
        }
        Err(_) => {
            println!("Cannot compare - GNU echo not available");
        }
    }

    println!("\n=== Example completed ===");
    println!("This approach is simpler and more reliable for integration testing.");
}

