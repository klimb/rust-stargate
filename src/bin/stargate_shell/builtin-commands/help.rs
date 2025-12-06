// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

pub fn execute() {
    println!("Stargate Shell Commands:");
    println!("  help                      - Show this help message");
    println!("  exit, quit                - Exit the shell");
    println!("  describe-command <cmd>    - Show help for a stargate command");
    println!("  <cmd> [args...]           - Execute a stargate command");
    println!("  <cmd> | <cmd> | ...       - Chain commands with JSON pipes");
    println!();
    println!("Scripting Language:");
    println!("  script <code>             - Execute inline script");
    println!("  script{{ ... }}             - Execute multi-line script block");
    println!();
    println!("  Variables:     let x = 5; let name = \"hello\";");
    println!("  Conditionals:  if x > 3 {{ print x; }} else {{ print \"small\"; }}");
    println!("  Functions:     fn add(a, b) {{ return a + b; }}");
    println!("  Commands:      exec \"ls -la\";");
    println!("  Substitution:  let files = $(ls);");
    println!("  Print:         print x;");
    println!();
    println!("  Operators:     +, -, *, /, ==, !=, <, >, <=, >=, &&, ||");
    println!();
    println!("Features:");
    println!("  Tab completion            - Press Tab to see/cycle through completions");
    println!("                              Works for commands, parameters (--flags), and options");
    println!("  Property completion       - Type command. and press Tab to see object properties");
    println!("                              Example: get-hostname.<TAB> shows 'flags' and 'hostname'");
    println!("                              Example: (list-directory).<TAB> shows 'entries', 'count'");
    println!("  Command hints             - Grayed suggestions appear as you type");
    println!("  Command history           - Use Up/Down arrows or Ctrl-P/Ctrl-N");
    println!("  Line editing              - Emacs-style keybindings (Ctrl-A, Ctrl-E, etc.)");
    println!();
    println!("Property Access in Scripts:");
    println!("  Object properties:        let host = (get-hostname).hostname;");
    println!("  Array indexing:           let first = (list-directory).entries[0];");
    println!("  Negative indexing:        let last = (list-directory).entries[-1];");
    println!("  Nested access:            let name = (list-directory).entries[0].name;");
    println!();
    println!("Examples:");
    println!("  describe-command list-directory");
    println!("  list-directory --long");
    println!("  list-directory | collect-count");
    println!("  script let x = 10; if x > 5 {{ print \"big\"; }} else {{ print \"small\"; }}");
    println!("  script fn factorial(n) {{ if n <= 1 {{ return 1; }} return n * factorial(n - 1); }} print factorial(5);");
    println!();
    println!("When using pipes (|), commands automatically use -o for JSON output");
    println!("and feed the JSON to the next command via stdin. ");
    println!("(Unless it's the last command)");
}
