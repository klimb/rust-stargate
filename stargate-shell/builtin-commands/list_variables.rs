// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

// list-variables built-in command
use super::super::scripting::Value;
use std::collections::HashMap;
use serde_json::json;

pub fn execute(variables: &HashMap<String, Value>, args: &str) -> Result<(), String> {
    let is_object_output = args.contains("-o") || args.contains("--obj");
    
    // Extract filter (everything that's not a flag)
    let filter = args.split_whitespace()
        .filter(|&s| s != "-o" && s != "--obj")
        .collect::<Vec<_>>()
        .join(" ");
    
    // Collect all variables including instance fields
    let mut var_list: Vec<(String, String, String)> = Vec::new();
    
    for (name, value) in variables.iter() {
        match value {
            Value::Instance { class_name, fields } => {
                // Add the instance itself
                var_list.push((name.clone(), format_value(value), get_type_name(value)));
                
                // Add all instance fields with class prefix
                for (field_name, field_value) in fields.iter() {
                    let prefixed_name = format!("{}.{}", name, field_name);
                    var_list.push((prefixed_name, format_value(field_value), get_type_name(field_value)));
                }
            },
            _ => {
                var_list.push((name.clone(), format_value(value), get_type_name(value)));
            }
        }
    }
    
    // Sort by variable name
    var_list.sort_by(|a, b| a.0.cmp(&b.0));
    
    // Apply filter if provided
    if !filter.is_empty() {
        var_list.retain(|(name, _, _)| name.contains(&filter));
    }
    
    if var_list.is_empty() {
        if is_object_output {
            println!("{}", json!({"variables": [], "count": 0}));
        } else {
            if filter.is_empty() {
                println!("No variables defined.");
            } else {
                println!("No variables matching '{}'.", filter);
            }
        }
    } else if is_object_output {
        let entries: Vec<_> = var_list.iter().map(|(name, value, type_name)| {
            json!({
                "name": name,
                "value": value,
                "type": type_name
            })
        }).collect();
        
        let output = json!({
            "variables": entries,
            "count": entries.len()
        });
        println!("{}", output);
    } else {
        // Display as a formatted table
        if var_list.is_empty() {
            println!("No variables defined.");
            return Ok(());
        }
        
        // Calculate column widths
        let max_name_len = var_list.iter().map(|(n, _, _)| n.len()).max().unwrap_or(4).max(4);
        let max_type_len = var_list.iter().map(|(_, _, t)| t.len()).max().unwrap_or(4).max(4);
        
        // Print header
        println!("{:<width_name$}  {:<width_type$}  Value", 
                 "Name", "Type",
                 width_name = max_name_len,
                 width_type = max_type_len);
        println!("{}", "-".repeat(max_name_len + max_type_len + 50));
        
        // Print each variable
        for (name, value, type_name) in &var_list {
            println!("{:<width_name$}  {:<width_type$}  {}", 
                     name, type_name, value,
                     width_name = max_name_len,
                     width_type = max_type_len);
        }
    }
    
    Ok(())
}

fn get_type_name(value: &Value) -> String {
    match value {
        Value::String(_) => "string".to_string(),
        Value::SmallInt(_) => "int".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::Bool(_) => "bool".to_string(),
        Value::None => "none".to_string(),
        Value::List(_) => "list".to_string(),
        Value::Dict(_) => "dict".to_string(),
        Value::Set(_) => "set".to_string(),
        Value::Object(_) => "object".to_string(),
        Value::Instance { class_name, .. } => class_name.clone(),
        Value::Closure { .. } => "closure".to_string(),
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        Value::SmallInt(i) => i.to_string(),
        Value::String(s) => format!("\"{}\"", s),
        Value::Bool(b) => b.to_string(),
        Value::None => "none".to_string(),
        Value::List(items) => {
            let formatted_items: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", formatted_items.join(", "))
        },
        Value::Dict(map) => {
            let formatted_items: Vec<String> = map.iter()
                .map(|(k, v)| format!("{}: {}", format_value(k), format_value(v)))
                .collect();
            format!("{{{}}}", formatted_items.join(", "))
        },
        Value::Set(items) => {
            let formatted_items: Vec<String> = items.iter().map(format_value).collect();
            format!("set({})", formatted_items.join(", "))
        },
        Value::Object(obj) => {
            format!("<object: {}>", obj)
        },
        Value::Instance { class_name, .. } => {
            format!("<{} instance>", class_name)
        },
        Value::Closure { .. } => {
            "<closure>".to_string()
        },
    }
}
