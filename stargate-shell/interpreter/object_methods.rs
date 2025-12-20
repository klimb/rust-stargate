// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use super::Interpreter;
use super::super::scripting::{Expression, Value};

impl Interpreter {
    pub fn value_to_display_string(&mut self, value: Value) -> Result<String, String> {
        if let Value::Instance { class_name, fields } = &value {
            if let Some(&has_to_string) = self.object_methods_cache.get(class_name) {
                if has_to_string {
                    let method_call = Expression::MethodCall {
                        object: Box::new(Expression::Value(value.clone())),
                        method: "to_string".to_string(),
                        args: vec![],
                    };
                    let result = self.eval_expression(method_call)?;
                    return Ok(result.to_string());
                }
            } else {
                let mut current_class = Some(class_name.clone());
                let mut found = false;
                
                while let Some(ref cls) = current_class {
                    if let Some((parent, _, methods)) = self.classes.get(cls) {
                        for (_access, method_name, params, _) in methods {
                            if method_name == "to_string" && params.is_empty() {
                                found = true;
                                break;
                            }
                        }
                        if found {
                            break;
                        }
                        current_class = parent.clone();
                    } else {
                        break;
                    }
                }
                
                self.object_methods_cache.insert(class_name.clone(), found);
                
                if found {
                    let method_call = Expression::MethodCall {
                        object: Box::new(Expression::Value(value.clone())),
                        method: "to_string".to_string(),
                        args: vec![],
                    };
                    let result = self.eval_expression(method_call)?;
                    return Ok(result.to_string());
                }
            }
        }
        
        Ok(value.to_string())
    }
}
