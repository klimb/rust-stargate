// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use super::super::scripting::{Expression, Statement, Value, Operator};
use super::{OpCode, BytecodeChunk};
use std::collections::HashMap;

pub struct Compiler {
    chunk: BytecodeChunk,
    var_slots: HashMap<String, u16>,
    current_line: usize,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            chunk: BytecodeChunk::new(),
            var_slots: HashMap::new(),
            current_line: 1,
        }
    }
    
    pub fn compile(&mut self, statements: Vec<Statement>) -> Result<BytecodeChunk, String> {
        for stmt in statements {
            self.compile_statement(stmt)?;
        }
        
        self.chunk.emit(OpCode::Halt, self.current_line);
        Ok(self.chunk.clone())
    }
    
    fn compile_statement(&mut self, stmt: Statement) -> Result<(), String> {
        match stmt {
            Statement::VarDecl(name, expr) => {
                self.compile_expression(expr)?;
                let slot = self.get_or_create_var_slot(name);
                self.chunk.emit_u16(OpCode::StoreVar, slot, self.current_line);
            }
            
            Statement::Assignment(name, expr) => {
                self.compile_expression(expr)?;
                let slot = self.var_slots.get(&name)
                    .copied()
                    .ok_or_else(|| format!("Undefined variable: {}", name))?;
                self.chunk.emit_u16(OpCode::StoreVar, slot, self.current_line);
            }
            
            Statement::ExprStmt(expr) => {
                self.compile_expression(expr)?;
                self.chunk.emit(OpCode::Pop, self.current_line);
            }
            
            Statement::If { condition, then_block, else_block } => {
                self.compile_expression(condition)?;
                
                let jump_to_else_pos = self.chunk.current_pos();
                self.chunk.emit_u16(OpCode::JumpIfFalse, 0, self.current_line);
                
                self.chunk.emit(OpCode::Pop, self.current_line);
                
                for stmt in then_block {
                    self.compile_statement(stmt)?;
                }
                
                if let Some(else_stmts) = else_block {
                    let jump_to_end_pos = self.chunk.current_pos();
                    self.chunk.emit_u16(OpCode::Jump, 0, self.current_line);
                    
                    let else_start = self.chunk.current_pos();
                    self.chunk.patch_jump(jump_to_else_pos, else_start);
                    
                    self.chunk.emit(OpCode::Pop, self.current_line);
                    
                    for stmt in else_stmts {
                        self.compile_statement(stmt)?;
                    }
                    
                    let end_pos = self.chunk.current_pos();
                    self.chunk.patch_jump(jump_to_end_pos, end_pos);
                } else {
                    let end_pos = self.chunk.current_pos();
                    self.chunk.patch_jump(jump_to_else_pos, end_pos);
                    
                    self.chunk.emit(OpCode::Pop, self.current_line);
                }
            }
            
            Statement::While { condition, body } => {
                let loop_start = self.chunk.current_pos();
                
                self.compile_expression(condition)?;
                
                let jump_to_end_pos = self.chunk.current_pos();
                self.chunk.emit_u16(OpCode::JumpIfFalse, 0, self.current_line);
                
                self.chunk.emit(OpCode::Pop, self.current_line);
                
                for stmt in body {
                    self.compile_statement(stmt)?;
                }
                
                let offset_back = (self.chunk.current_pos() - loop_start + 3) as i16;
                self.chunk.emit_u16(OpCode::Jump, (-offset_back) as u16, self.current_line);
                
                let end_pos = self.chunk.current_pos();
                self.chunk.patch_jump(jump_to_end_pos, end_pos);
                
                self.chunk.emit(OpCode::Pop, self.current_line);
            }
            
            Statement::Print(expr) => {
                self.compile_expression(expr)?;
                self.chunk.emit(OpCode::Print, self.current_line);
            }
            
            _ => {
                return Err(format!("Statement not yet supported in bytecode compiler: {:?}", stmt));
            }
        }
        
        Ok(())
    }
    
    fn compile_expression(&mut self, expr: Expression) -> Result<(), String> {
        match expr {
            Expression::Value(value) => {
                let const_idx = self.chunk.add_constant(value);
                self.chunk.emit_u16(OpCode::LoadConst, const_idx, self.current_line);
            }
            
            Expression::Variable(name) => {
                let slot = self.var_slots.get(&name)
                    .copied()
                    .ok_or_else(|| format!("Undefined variable: {}", name))?;
                self.chunk.emit_u16(OpCode::LoadVar, slot, self.current_line);
            }
            
            Expression::BinaryOp { left, op, right } => {
                self.compile_expression(*left)?;
                self.compile_expression(*right)?;
                
                let opcode = match op {
                    Operator::Add => OpCode::Add,
                    Operator::Sub => OpCode::Sub,
                    Operator::Mul => OpCode::Mul,
                    Operator::Div => OpCode::Div,
                    Operator::Mod => OpCode::Mod,
                    Operator::Eq => OpCode::Eq,
                    Operator::Ne => OpCode::Ne,
                    Operator::Lt => OpCode::Lt,
                    Operator::Gt => OpCode::Gt,
                    Operator::Le => OpCode::Le,
                    Operator::Ge => OpCode::Ge,
                    Operator::And => OpCode::And,
                    Operator::Or => OpCode::Or,
                    Operator::Not => {
                        return Err("Binary Not operator not supported".to_string());
                    }
                };
                
                self.chunk.emit(opcode, self.current_line);
            }
            
            Expression::ListLiteral(elements) => {
                for elem in &elements {
                    self.compile_expression(elem.clone())?;
                }
                
                let count = elements.len();
                assert!(count <= u8::MAX as usize, "List too large");
                self.chunk.emit_u8(OpCode::BuildList, count as u8, self.current_line);
            }
            
            _ => {
                return Err(format!("Expression not yet supported in bytecode compiler: {:?}", expr));
            }
        }
        
        Ok(())
    }
    
    fn get_or_create_var_slot(&mut self, name: String) -> u16 {
        if let Some(&slot) = self.var_slots.get(&name) {
            slot
        } else {
            let slot = self.chunk.add_var(name.clone());
            self.var_slots.insert(name, slot);
            slot
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}
