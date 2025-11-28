// Tab completion, hints, and validation for the shell
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;

use super::commands::{get_stargate_commands, get_command_parameters, SHELL_COMMANDS};

const DESCRIBE_COMMAND_PREFIX: &str = "describe-command ";

pub struct StargateCompletion {
    commands: Vec<String>,
}

impl StargateCompletion {
    pub fn new() -> Self {
        let mut commands = get_stargate_commands();
        commands.extend(SHELL_COMMANDS.iter().map(|s| s.to_string()));
        commands.sort();
        commands.dedup();
        Self { commands }
    }
}

impl Helper for StargateCompletion {}

impl Completer for StargateCompletion {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line = &line[..pos];
        
        // Special handling for "describe-command "
        if let Some(rest) = line.strip_prefix(DESCRIBE_COMMAND_PREFIX) {
            let matches: Vec<Pair> = self.commands
                .iter()
                .filter(|cmd| !SHELL_COMMANDS.contains(&cmd.as_str())) // Exclude shell builtins
                .filter(|cmd| cmd.starts_with(rest))
                .map(|cmd| Pair {
                    display: cmd.clone(),
                    replacement: cmd.clone(),
                })
                .collect();
            
            return Ok((DESCRIBE_COMMAND_PREFIX.len(), matches));
        }
        
        // Find the start of the current word
        let start = line.rfind(|c: char| c.is_whitespace() || c == '|')
            .map(|i| i + 1)
            .unwrap_or(0);
        
        let prefix = &line[start..];
        
        if prefix.is_empty() {
            return Ok((start, vec![]));
        }

        // Check if we're completing a parameter (starts with -)
        if prefix.starts_with('-') {
            // Extract the command name (first word after | or at start)
            let cmd_start = line[..start].rfind('|')
                .map(|i| i + 1)
                .unwrap_or(0);
            
            let cmd_part = line[cmd_start..start].trim();
            let cmd_name = cmd_part.split_whitespace().next().unwrap_or("");
            
            // Get parameter completions for this command
            if !cmd_name.is_empty() && !SHELL_COMMANDS.contains(&cmd_name) {
                let params = get_command_parameters(cmd_name);
                let matches: Vec<Pair> = params
                    .into_iter()
                    .filter(|param| param.starts_with(prefix))
                    .map(|param| Pair {
                        display: param.clone(),
                        replacement: param,
                    })
                    .collect();
                
                return Ok((start, matches));
            }
        }

        // Regular command completion
        let matches: Vec<Pair> = self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|cmd| Pair {
                display: cmd.clone(),
                replacement: cmd.clone(),
            })
            .collect();

        Ok((start, matches))
    }
}

impl Hinter for StargateCompletion {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        if pos < line.len() {
            return None;
        }
        
        // Find the start of the current word
        let start = line.rfind(|c: char| c.is_whitespace() || c == '|')
            .map(|i| i + 1)
            .unwrap_or(0);
        
        let prefix = &line[start..];
        
        if prefix.len() < 2 {
            return None;
        }
        
        // Find the first matching command
        self.commands
            .iter()
            .find(|cmd| cmd.starts_with(prefix) && cmd.len() > prefix.len())
            .map(|cmd| cmd[prefix.len()..].to_string())
    }
}

impl Highlighter for StargateCompletion {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        Cow::Borrowed(line)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _forced: bool) -> bool {
        false
    }
}

impl Validator for StargateCompletion {}
