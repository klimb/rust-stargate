// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

// This file is part of the stargate package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

/// Type of command input detected
#[derive(Debug, PartialEq)]
pub enum CommandType {
    ScriptStatement,
    ControlFlow,
    ChainedCommands,
    PropertyAccess,
    Pipeline,
}

impl CommandType {
    /// Detect the type of command from input string
    pub fn detect(input: &str) -> Self {
        // Check for chained commands first (but not in script statements)
        if input.contains("&&") 
            && !input.starts_with("let ") 
            && !input.starts_with("class ")
            && !Self::is_control_flow(input) {
            return Self::ChainedCommands;
        }
        
        // Control flow keywords
        if Self::is_control_flow(input) {
            return Self::ControlFlow;
        }
        
        // Script statements
        if Self::is_script_statement(input) {
            return Self::ScriptStatement;
        }
        
        // Property access
        if Self::has_property_access(input) {
            return Self::PropertyAccess;
        }
        
        // Default to pipeline
        Self::Pipeline
    }
    
    /// Check if input is a script statement
    pub fn is_script_statement(input: &str) -> bool {
        input.starts_with("let ") 
            || input.starts_with("class ")
            || input.starts_with("print ")
            || input.starts_with("if ")
            || input.starts_with("while ")
            || input.starts_with("for ")
            || input.starts_with("fn ")
            || input.starts_with("return ")
            || input.contains(" = ")
    }
    
    /// Check if input is a control flow statement
    pub fn is_control_flow(input: &str) -> bool {
        input.starts_with("if ") 
            || input.starts_with("while ") 
            || input.starts_with("for ") 
            || input.starts_with("fn ")
    }
    
    /// Check if input contains property access patterns
    pub fn has_property_access(input: &str) -> bool {
        let mut in_quotes = false;
        let chars: Vec<char> = input.chars().collect();
        
        for i in 0..chars.len() {
            if chars[i] == '"' {
                in_quotes = !in_quotes;
            } else if !in_quotes && chars[i] == '.' && i > 0 && i < chars.len() - 1 {
                let before = chars[i-1];
                let after = chars[i+1];
                if (before.is_alphanumeric() || before == '_' || before == ')' || before == ']')
                    && (after.is_alphanumeric() || after == '_') {
                    return true;
                }
            }
        }
        
        !in_quotes && input.contains('[') && input.contains(']')
    }
}
