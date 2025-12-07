// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

pub fn is_exit_command(input: &str) -> bool {
    input == "exit" || input == "quit"
}
