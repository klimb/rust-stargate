// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

pub mod vm;
pub mod compiler;

use super::scripting::Value;

pub use self::vm::VM;
pub use self::compiler::Compiler;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OpCode {
    LoadConst = 0,
    LoadVar = 1,
    StoreVar = 2,
    
    Add = 10,
    Sub = 11,
    Mul = 12,
    Div = 13,
    Mod = 14,
    
    Eq = 20,
    Ne = 21,
    Lt = 22,
    Gt = 23,
    Le = 24,
    Ge = 25,
    
    And = 30,
    Or = 31,
    Not = 32,
    
    Jump = 40,
    JumpIfFalse = 41,
    JumpIfTrue = 42,
    
    Call = 50,
    Return = 51,
    
    BuildList = 60,
    BuildDict = 61,
    BuildSet = 62,
    
    LoadIndex = 70,
    StoreIndex = 71,
    
    CallMethod = 80,
    
    Pop = 90,
    Dup = 91,
    
    Print = 100,
    
    Halt = 255,
}

#[derive(Debug, Clone)]
pub struct BytecodeChunk {
    pub code: Vec<u8>,
    pub constants: Vec<Value>,
    pub var_names: Vec<String>,
    pub lines: Vec<usize>,
}

impl BytecodeChunk {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            constants: Vec::new(),
            var_names: Vec::new(),
            lines: Vec::new(),
        }
    }
    
    pub fn add_constant(&mut self, value: Value) -> u16 {
        for (i, existing) in self.constants.iter().enumerate() {
            if existing == &value {
                return i as u16;
            }
        }
        
        let idx = self.constants.len();
        assert!(idx < u16::MAX as usize, "Too many constants");
        self.constants.push(value);
        idx as u16
    }
    
    pub fn add_var(&mut self, name: String) -> u16 {
        for (i, existing) in self.var_names.iter().enumerate() {
            if existing == &name {
                return i as u16;
            }
        }
        
        let idx = self.var_names.len();
        assert!(idx < u16::MAX as usize, "Too many variables");
        self.var_names.push(name);
        idx as u16
    }
    
    pub fn emit(&mut self, op: OpCode, line: usize) {
        self.code.push(op as u8);
        self.lines.push(line);
    }
    
    pub fn emit_u8(&mut self, op: OpCode, operand: u8, line: usize) {
        self.code.push(op as u8);
        self.code.push(operand);
        self.lines.push(line);
        self.lines.push(line);
    }
    
    pub fn emit_u16(&mut self, op: OpCode, operand: u16, line: usize) {
        self.code.push(op as u8);
        self.code.push((operand >> 8) as u8);
        self.code.push((operand & 0xFF) as u8);
        self.lines.push(line);
        self.lines.push(line);
        self.lines.push(line);
    }
    
    pub fn current_pos(&self) -> usize {
        self.code.len()
    }
    
    pub fn patch_jump(&mut self, pos: usize, target: usize) {
        let offset = (target - pos - 3) as u16;
        self.code[pos + 1] = (offset >> 8) as u8;
        self.code[pos + 2] = (offset & 0xFF) as u8;
    }
}

impl Default for BytecodeChunk {
    fn default() -> Self {
        Self::new()
    }
}
