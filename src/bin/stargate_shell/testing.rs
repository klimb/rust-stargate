// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use super::scripting::*;
use std::collections::HashMap;

/// Unit testing state and functionality
pub struct TestRunner {
    pub test_functions: Vec<String>,
    pub test_passed: usize,
    pub test_failed: usize,
    pub ut_enabled: bool,
}

impl TestRunner {
    pub fn new() -> Self {
        TestRunner {
            test_functions: Vec::new(),
            test_passed: 0,
            test_failed: 0,
            ut_enabled: false,
        }
    }

    /// Enable the ut module and create the ut object with assertion methods
    pub fn enable_ut_module(&mut self) -> Value {
        self.ut_enabled = true;
        self.create_ut_instance()
    }

    /// Register a test function
    pub fn register_test(&mut self, name: String) {
        self.test_functions.push(name);
    }

    /// Check if test mode is enabled
    pub fn is_enabled(&self) -> bool {
        self.ut_enabled
    }

    /// Check if there are any tests to run
    pub fn has_tests(&self) -> bool {
        !self.test_functions.is_empty()
    }

    /// Create or update the ut instance with current stats
    pub fn create_ut_instance(&self) -> Value {
        let mut ut_fields = HashMap::new();
        ut_fields.insert("assert_equals".to_string(), Value::String("assert_equals".to_string()));
        ut_fields.insert("assert_not_equals".to_string(), Value::String("assert_not_equals".to_string()));
        ut_fields.insert("assert_true".to_string(), Value::String("assert_true".to_string()));
        
        // Add stats as formatted string
        let stats = format!("Passed: {}, Failed: {}, Total: {}", 
            self.test_passed, self.test_failed, self.test_passed + self.test_failed);
        ut_fields.insert("stats".to_string(), Value::String(stats));
        
        // Add healthy as boolean (true if no failures)
        ut_fields.insert("healthy".to_string(), Value::Bool(self.test_failed == 0));
        
        Value::Instance {
            class_name: "UT".to_string(),
            fields: ut_fields,
        }
    }
}
