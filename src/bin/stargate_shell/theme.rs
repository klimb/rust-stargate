// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

pub struct Theme {
    pub completion_color: &'static str,
}

impl Theme {
    pub const fn default() -> Self {
        Self {
            completion_color: "\x1b[34m",
        }
    }
    
    pub const fn reset() -> &'static str {
        "\x1b[0m"
    }
    
    pub fn colorize(&self, text: &str) -> String {
        format!("{}{}{}", self.completion_color, text, Self::reset())
    }
}

pub const DEFAULT_THEME: Theme = Theme::default();
