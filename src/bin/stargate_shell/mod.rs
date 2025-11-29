// Stargate Shell - Interactive shell for chaining stargate commands
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
pub mod scripting;
pub mod interpreter;
pub mod ui;

pub use completion::StargateCompletion;
pub use execution::execute_pipeline;
pub use interpreter::{execute_script, execute_script_with_interpreter, Interpreter};
pub use ui::{describe_command, print_banner, print_help};
