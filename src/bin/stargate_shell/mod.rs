// Stargate Shell - Interactive shell for chaining stargate commands
//
// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.
//
// This module provides an interactive shell with features like:
// - Command history and readline support
// - Tab completion for commands and parameters
// - Pipeline support with automatic JSON conversion
// - Emacs-style keybindings

pub mod commands;
pub mod completion;
pub mod execution;
pub mod parsing;
pub mod path;
pub mod path_completion;
pub mod scripting;
pub mod interpreter;
pub mod testing;
pub mod theme;
pub mod ui;
pub mod jobs;
pub mod bytecode;

// Shell main modules
pub mod command_type;
pub mod executor;
pub mod piped_input;
pub mod repl_handlers;

#[path = "builtin-commands/mod.rs"]
pub mod builtin_commands;

pub use completion::StargateCompletion;
pub use execution::execute_pipeline;
pub use interpreter::{execute_script_with_path, execute_stargate_script, Interpreter};
pub use ui::{describe_command, print_help};
pub use jobs::start_job_monitor;
