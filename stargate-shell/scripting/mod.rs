// Scripting language parser for stargate-shell
//
// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

mod value;
mod ast;
mod parser;

pub use value::Value;
pub use ast::{Statement, Expression, Operator, AccessModifier};
pub use parser::Parser;
