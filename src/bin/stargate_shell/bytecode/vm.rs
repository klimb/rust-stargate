use super::super::scripting::Value;
use super::{OpCode, BytecodeChunk};
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct CallFrame {
    return_addr: usize,
    base_ptr: usize,
    func_name: String,
}

pub struct VM {
    stack: Vec<Value>,
    call_stack: Vec<CallFrame>,
    globals: HashMap<String, Value>,
    ip: usize,
}

impl VM {
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(256),
            call_stack: Vec::with_capacity(32),
            globals: HashMap::new(),
            ip: 0,
        }
    }
    
    /// Execute a bytecode chunk
    pub fn execute(&mut self, chunk: &BytecodeChunk) -> Result<Value, String> {
        self.ip = 0;
        
        // Local variables array (indexed by slot)
        let mut locals: Vec<Value> = vec![Value::None; chunk.var_names.len()];
        
        loop {
            if self.ip >= chunk.code.len() {
                break;
            }
            
            let opcode = chunk.code[self.ip];
            self.ip += 1;
            
            match opcode {
                op if op == OpCode::LoadConst as u8 => {
                    let idx = self.read_u16(chunk);
                    let value = chunk.constants[idx as usize].clone();
                    self.stack.push(value);
                }
                
                op if op == OpCode::LoadVar as u8 => {
                    let slot = self.read_u16(chunk);
                    let value = locals[slot as usize].clone();
                    self.stack.push(value);
                }
                
                op if op == OpCode::StoreVar as u8 => {
                    let slot = self.read_u16(chunk);
                    let value = self.stack.pop()
                        .ok_or("Stack underflow in STORE_VAR")?;
                    
                    // Optimization: reuse existing value slot if possible
                    locals[slot as usize] = value;
                }
                
                op if op == OpCode::Add as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (left, right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => {
                            a.checked_add(b)
                                .map(Value::SmallInt)
                                .unwrap_or_else(|| Value::Number((a as f64) + (b as f64)))
                        }
                        (Value::SmallInt(a), Value::Number(b)) => Value::Number((a as f64) + b),
                        (Value::Number(a), Value::SmallInt(b)) => Value::Number(a + (b as f64)),
                        (Value::Number(a), Value::Number(b)) => Value::Number(a + b),
                        (Value::String(a), Value::String(b)) => Value::String(format!("{}{}", a, b)),
                        _ => return Err("Invalid operands for +".to_string()),
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::Sub as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (left, right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => {
                            a.checked_sub(b)
                                .map(Value::SmallInt)
                                .unwrap_or_else(|| Value::Number((a as f64) - (b as f64)))
                        }
                        (Value::SmallInt(a), Value::Number(b)) => Value::Number((a as f64) - b),
                        (Value::Number(a), Value::SmallInt(b)) => Value::Number(a - (b as f64)),
                        (l, r) => Value::Number(l.to_number() - r.to_number()),
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::Mul as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (left, right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => {
                            a.checked_mul(b)
                                .map(Value::SmallInt)
                                .unwrap_or_else(|| Value::Number((a as f64) * (b as f64)))
                        }
                        (Value::SmallInt(a), Value::Number(b)) => Value::Number((a as f64) * b),
                        (Value::Number(a), Value::SmallInt(b)) => Value::Number(a * (b as f64)),
                        (l, r) => Value::Number(l.to_number() * r.to_number()),
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::Div as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (left, right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => {
                            if b == 0 {
                                return Err("Division by zero".to_string());
                            } else if a % b == 0 {
                                Value::SmallInt(a / b)
                            } else {
                                Value::Number((a as f64) / (b as f64))
                            }
                        }
                        (l, r) => {
                            let divisor = r.to_number();
                            if divisor == 0.0 {
                                return Err("Division by zero".to_string());
                            }
                            Value::Number(l.to_number() / divisor)
                        }
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::Mod as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (left, right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => {
                            if b == 0 {
                                return Err("Modulo by zero".to_string());
                            }
                            Value::SmallInt(a % b)
                        }
                        (l, r) => {
                            let divisor = r.to_number();
                            if divisor == 0.0 {
                                return Err("Modulo by zero".to_string());
                            }
                            Value::Number(l.to_number() % divisor)
                        }
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::Eq as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    self.stack.push(Value::Bool(left == right));
                }
                
                op if op == OpCode::Ne as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    self.stack.push(Value::Bool(left != right));
                }
                
                op if op == OpCode::Lt as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (&left, &right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => Value::Bool(a < b),
                        _ => Value::Bool(left.to_number() < right.to_number()),
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::Gt as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (&left, &right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => Value::Bool(a > b),
                        _ => Value::Bool(left.to_number() > right.to_number()),
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::Le as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (&left, &right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => Value::Bool(a <= b),
                        _ => Value::Bool(left.to_number() <= right.to_number()),
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::Ge as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    
                    let result = match (&left, &right) {
                        (Value::SmallInt(a), Value::SmallInt(b)) => Value::Bool(a >= b),
                        _ => Value::Bool(left.to_number() >= right.to_number()),
                    };
                    
                    self.stack.push(result);
                }
                
                op if op == OpCode::And as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    self.stack.push(Value::Bool(left.to_bool() && right.to_bool()));
                }
                
                op if op == OpCode::Or as u8 => {
                    let right = self.stack.pop().ok_or("Stack underflow")?;
                    let left = self.stack.pop().ok_or("Stack underflow")?;
                    self.stack.push(Value::Bool(left.to_bool() || right.to_bool()));
                }
                
                op if op == OpCode::Not as u8 => {
                    let value = self.stack.pop().ok_or("Stack underflow")?;
                    self.stack.push(Value::Bool(!value.to_bool()));
                }
                
                op if op == OpCode::Jump as u8 => {
                    let offset = self.read_u16(chunk) as i16;
                    self.ip = (self.ip as isize + offset as isize) as usize;
                }
                
                op if op == OpCode::JumpIfFalse as u8 => {
                    let offset = self.read_u16(chunk) as i16;
                    let condition = self.stack.last()
                        .ok_or("Stack underflow in JUMP_IF_FALSE")?;
                    
                    if !condition.to_bool() {
                        self.ip = (self.ip as isize + offset as isize) as usize;
                    }
                }
                
                op if op == OpCode::Pop as u8 => {
                    self.stack.pop().ok_or("Stack underflow in POP")?;
                }
                
                op if op == OpCode::BuildList as u8 => {
                    let count = self.read_u8(chunk) as usize;
                    let start = self.stack.len() - count;
                    let items = self.stack.drain(start..).collect();
                    self.stack.push(Value::List(items));
                }
                
                op if op == OpCode::Print as u8 => {
                    let value = self.stack.pop().ok_or("Stack underflow in PRINT")?;
                    println!("{}", value.to_string());
                }
                
                op if op == OpCode::Halt as u8 => {
                    break;
                }
                
                _ => {
                    return Err(format!("Unknown opcode: {}", opcode));
                }
            }
        }
        
        // Return top of stack or None
        Ok(self.stack.pop().unwrap_or(Value::None))
    }
    
    #[inline]
    fn read_u8(&mut self, chunk: &BytecodeChunk) -> u8 {
        let value = chunk.code[self.ip];
        self.ip += 1;
        value
    }
    
    #[inline]
    fn read_u16(&mut self, chunk: &BytecodeChunk) -> u16 {
        let high = chunk.code[self.ip] as u16;
        let low = chunk.code[self.ip + 1] as u16;
        self.ip += 2;
        (high << 8) | low
    }
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}
