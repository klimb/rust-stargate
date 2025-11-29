// Scripting language parser for stargate-shell

use serde_json;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
    Object(serde_json::Value), // JSON object/array
    Instance {
        class_name: String,
        fields: std::collections::HashMap<String, Value>,
    },
    List(Vec<Value>),
    Dict(std::collections::HashMap<Value, Value>),
}

impl Value {
    pub fn to_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Object(obj) => obj.to_string(),
            Value::Instance { class_name, .. } => format!("<{} instance>", class_name),
            Value::List(items) => {
                let items_str: Vec<String> = items.iter().map(|v| v.to_string()).collect();
                format!("[{}]", items_str.join(", "))
            }
            Value::Dict(map) => {
                let mut pairs: Vec<String> = map.iter()
                    .map(|(k, v)| format!("{}: {}", k.to_string(), v.to_string()))
                    .collect();
                pairs.sort();
                format!("{{{}}}", pairs.join(", "))
            }
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Null => false,
            Value::Object(_) => true,
            Value::Instance { .. } => true,
            Value::List(items) => !items.is_empty(),
            Value::Dict(map) => !map.is_empty(),
        }
    }

    pub fn to_number(&self) -> f64 {
        match self {
            Value::Number(n) => *n,
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
            Value::String(s) => s.parse().unwrap_or(0.0),
            Value::Null => 0.0,
            Value::Object(_) => 0.0,
            Value::Instance { .. } => 0.0,
            Value::List(items) => items.len() as f64,
            Value::Dict(map) => map.len() as f64,
        }
    }
}

// Manual implementation of PartialEq for Value (handles f64 comparison)
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Object(a), Value::Object(b)) => a == b,
            (Value::Instance { class_name: c1, fields: f1 }, Value::Instance { class_name: c2, fields: f2 }) => {
                c1 == c2 && f1 == f2
            }
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Dict(a), Value::Dict(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

// Manual implementation of Hash for Value (handles f64 hashing)
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::String(s) => {
                0u8.hash(state);
                s.hash(state);
            }
            Value::Number(n) => {
                1u8.hash(state);
                // Hash the bits of the f64 to handle it properly
                n.to_bits().hash(state);
            }
            Value::Bool(b) => {
                2u8.hash(state);
                b.hash(state);
            }
            Value::Null => {
                3u8.hash(state);
            }
            Value::Object(obj) => {
                4u8.hash(state);
                obj.to_string().hash(state);
            }
            Value::Instance { class_name, fields } => {
                5u8.hash(state);
                class_name.hash(state);
                // Hash fields in a deterministic order
                let mut pairs: Vec<_> = fields.iter().collect();
                pairs.sort_by_key(|(k, _)| k.as_str());
                for (k, v) in pairs {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Value::List(items) => {
                6u8.hash(state);
                for item in items {
                    item.hash(state);
                }
            }
            Value::Dict(map) => {
                7u8.hash(state);
                // Hash dict entries in a deterministic order
                let mut pairs: Vec<_> = map.iter().collect();
                pairs.sort_by_key(|(k, _)| k.to_string());
                for (k, v) in pairs {
                    k.hash(state);
                    v.hash(state);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    VarDecl(String, Expression),
    Assignment(String, Expression),
    IndexAssignment {
        object: String,
        index: Expression,
        value: Expression,
    },
    If {
        condition: Expression,
        then_block: Vec<Statement>,
        else_block: Option<Vec<Statement>>,
    },
    For {
        var_name: String,
        iterable: Expression,
        body: Vec<Statement>,
    },
    FunctionDef {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
        annotations: Vec<String>, // e.g., ["test"]
    },
    ClassDef {
        name: String,
        parent: Option<String>, // parent class name for inheritance
        fields: Vec<(String, Expression)>, // field name and default value
        methods: Vec<(String, Vec<String>, Vec<Statement>)>, // method name, params, body
    },
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
    Command(String),
    Return(Expression),
    Print(Expression),
    Exit(Option<Expression>), // exit with optional status code
    Assert {
        condition: Expression,
        message: Option<Expression>,
    },
    Use(String), // use statement for imports (e.g., "use ut;")
    ExprStmt(Expression), // Statement that just evaluates an expression (for method calls)
}

#[derive(Debug, Clone)]
pub enum Expression {
    Value(Value),
    Variable(String),
    BinaryOp {
        left: Box<Expression>,
        op: Operator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: Operator,
        operand: Box<Expression>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
    NewInstance {
        class_name: String,
    },
    CommandOutput(String),
    InterpolatedString(String), // String with {var} placeholders
    PropertyAccess {
        object: Box<Expression>,
        property: String,
    },
    MethodCall {
        object: Box<Expression>,
        method: String,
        args: Vec<Expression>,
    },
    IndexAccess {
        object: Box<Expression>,
        index: Box<Expression>,
    },
    Pipeline {
        input: Box<Expression>,
        command: String,
    },
    ListLiteral(Vec<Expression>),
    DictLiteral(Vec<(Expression, Expression)>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    Not,
}

pub struct Parser {
    tokens: Vec<String>,
    pos: usize,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        let tokens = Self::tokenize(input);
        Parser { tokens, pos: 0 }
    }

    fn tokenize(input: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut in_comment = false;
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            if in_comment {
                if ch == '\n' {
                    in_comment = false;
                }
                continue;
            }
            
            if in_string {
                if ch == '"' {
                    // Keep the quotes to mark it as a string literal
                    tokens.push(format!("\"{}\"", current));
                    current.clear();
                    in_string = false;
                } else {
                    current.push(ch);
                }
            } else {
                match ch {
                    '#' => {
                        if !current.is_empty() {
                            tokens.push(current.clone());
                            current.clear();
                        }
                        in_comment = true;
                    }
                    '"' => {
                        if !current.is_empty() {
                            tokens.push(current.clone());
                            current.clear();
                        }
                        in_string = true;
                    }
                    ' ' | '\t' | '\n' => {
                        if !current.is_empty() {
                            tokens.push(current.clone());
                            current.clear();
                        }
                    }
                    '(' | ')' | '{' | '}' | ';' | ',' | '=' | '+' | '*' | '/' | '<' | '>' | '!' | '[' | ']' | '.' | '|' | '&' | ':' => {
                        // Special handling for '.' - check if it's part of a number
                        if ch == '.' && !current.is_empty() && current.chars().all(|c| c.is_numeric()) && chars.peek().map(|c| c.is_numeric()).unwrap_or(false) {
                            // This is a decimal point in a number like 3.14
                            current.push(ch);
                            continue;
                        }
                        
                        if !current.is_empty() {
                            tokens.push(current.clone());
                            current.clear();
                        }
                        // Handle multi-char operators
                        if (ch == '=' || ch == '!' || ch == '<' || ch == '>') && chars.peek() == Some(&'=') {
                            let next = chars.next().unwrap();
                            tokens.push(format!("{}{}", ch, next));
                        } else if ch == '&' && chars.peek() == Some(&'&') {
                            let next = chars.next().unwrap();
                            tokens.push(format!("{}{}", ch, next));
                        } else if ch == '|' && chars.peek() == Some(&'|') {
                            let next = chars.next().unwrap();
                            tokens.push(format!("{}{}", ch, next));
                        } else if ch == '.' && chars.peek() == Some(&'.') {
                            // Handle .. as a single token (parent directory)
                            let next = chars.next().unwrap();
                            tokens.push("..".to_string());
                        } else {
                            tokens.push(ch.to_string());
                        }
                    }
                    '-' => {
                        // Only treat as operator if it looks like subtraction
                        // (preceded by whitespace/operator and followed by digit or whitespace)
                        let next_ch = chars.peek();
                        let is_operator = !current.is_empty() && 
                                         (next_ch == Some(&' ') || next_ch.map(|c| c.is_numeric()).unwrap_or(false));
                        
                        if is_operator {
                            if !current.is_empty() {
                                tokens.push(current.clone());
                                current.clear();
                            }
                            tokens.push("-".to_string());
                        } else {
                            // Part of an identifier like "list-directory"
                            current.push(ch);
                        }
                    }
                    _ => current.push(ch),
                }
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
    }

    fn peek(&self) -> Option<&String> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<String> {
        if self.pos < self.tokens.len() {
            let token = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Statement>, String> {
        let mut statements = Vec::new();

        while self.peek().is_some() {
            statements.push(self.parse_statement()?);
        }

        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        // Check for annotations like [test]
        let mut annotations = Vec::new();
        while self.peek().map(|s| s.as_str()) == Some("[") {
            self.advance(); // consume '['
            let annotation = self.advance().ok_or("Expected annotation name")?;
            annotations.push(annotation);
            self.expect("]")?;
        }
        
        let token = self.peek().ok_or("Unexpected end of input")?.clone();

        match token.as_str() {
            "use" => self.parse_use(),
            "let" => self.parse_var_decl(),
            "if" => self.parse_if(),
            "for" => self.parse_for(),
            "fn" => self.parse_function_def_with_annotations(annotations),
            "class" => self.parse_class_def(),
            "return" => self.parse_return(),
            "exit" => self.parse_exit(),
            "print" => self.parse_print(),
            "assert" => self.parse_assert(),
            "exec" => self.parse_command(),
            _ => {
                // Check if it's a pipeline (contains command names followed by |)
                // Look ahead to see if there's a pipe in the next few tokens
                let mut lookahead = self.pos;
                let mut found_pipe = false;
                while lookahead < self.tokens.len() {
                    let tok = &self.tokens[lookahead];
                    if tok == ";" || tok == "{" || tok == "}" {
                        break;
                    }
                    if tok == "|" {
                        found_pipe = true;
                        break;
                    }
                    lookahead += 1;
                }
                
                if found_pipe {
                    return self.parse_pipeline();
                }
                
                // Check if it's an assignment, function call, or method call
                // First check for index assignment: var[idx] = value
                if self.tokens.get(self.pos + 1).map(|s| s.as_str()) == Some("[") {
                    // Look ahead to find the = after ]
                    let mut lookahead = self.pos + 2; // After var and [
                    let mut bracket_depth = 1;
                    while lookahead < self.tokens.len() && bracket_depth > 0 {
                        let tok = &self.tokens[lookahead];
                        if tok == "[" {
                            bracket_depth += 1;
                        } else if tok == "]" {
                            bracket_depth -= 1;
                        }
                        lookahead += 1;
                    }
                    // Check if next token is =
                    if self.tokens.get(lookahead).map(|s| s.as_str()) == Some("=") {
                        return self.parse_assignment();
                    }
                    // Not an assignment, fall through to other checks
                }
                
                if self.tokens.get(self.pos + 1).map(|s| s.as_str()) == Some("=") {
                    self.parse_assignment()
                } else if self.tokens.get(self.pos + 1).map(|s| s.as_str()) == Some("(") {
                    let stmt = Statement::FunctionCall {
                        name: token.clone(),
                        args: {
                            self.advance();
                            self.parse_args()?
                        },
                    };
                    self.expect(";")?;
                    Ok(stmt)
                } else if self.tokens.get(self.pos + 1).map(|s| s.as_str()) == Some(".") {
                    // This is a method call like ut.assert_equals(...)
                    let expr = self.parse_expression()?;
                    self.expect(";")?;
                    Ok(Statement::ExprStmt(expr))
                } else {
                    // Treat as a command (could be a single stargate command)
                    self.parse_pipeline()
                }
            }
        }
    }

    fn parse_var_decl(&mut self) -> Result<Statement, String> {
        self.expect("let")?;
        let name = self.advance().ok_or("Expected variable name")?;
        self.expect("=")?;
        
        // Parse expression (which now handles pipes as operators)
        let expr = self.parse_expression()?;
        
        self.expect(";")?;
        Ok(Statement::VarDecl(name, expr))
    }

    fn parse_assignment(&mut self) -> Result<Statement, String> {
        let name = self.advance().ok_or("Expected variable name")?;
        
        // Check if this is an index assignment like: list[0] = value
        if self.peek().map(|s| s.as_str()) == Some("[") {
            self.advance(); // consume '['
            let index = self.parse_expression()?;
            self.expect("]")?;
            self.expect("=")?;
            let value = self.parse_expression()?;
            self.expect(";")?;
            return Ok(Statement::IndexAssignment {
                object: name,
                index,
                value,
            });
        }
        
        self.expect("=")?;
        
        // Parse expression (which now handles pipes as operators)
        let expr = self.parse_expression()?;
        
        self.expect(";")?;
        Ok(Statement::Assignment(name, expr))
    }

    fn parse_if(&mut self) -> Result<Statement, String> {
        self.expect("if")?;
        let condition = self.parse_expression()?;
        self.expect("{")?;
        let then_block = self.parse_block()?;
        
        let else_block = if self.peek().map(|s| s.as_str()) == Some("else") {
            self.advance();
            self.expect("{")?;
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(Statement::If {
            condition,
            then_block,
            else_block,
        })
    }

    fn parse_for(&mut self) -> Result<Statement, String> {
        self.expect("for")?;
        let var_name = self.advance().ok_or("Expected variable name after 'for'")?;
        self.expect("in")?;
        let iterable = self.parse_expression()?;
        self.expect("{")?;
        let body = self.parse_block()?;

        Ok(Statement::For {
            var_name,
            iterable,
            body,
        })
    }

    fn parse_function_def(&mut self) -> Result<Statement, String> {
        self.parse_function_def_with_annotations(Vec::new())
    }

    fn parse_function_def_with_annotations(&mut self, annotations: Vec<String>) -> Result<Statement, String> {
        self.expect("fn")?;
        let name = self.advance().ok_or("Expected function name")?;
        self.expect("(")?;
        
        let mut params = Vec::new();
        while self.peek().map(|s| s.as_str()) != Some(")") {
            params.push(self.advance().ok_or("Expected parameter name")?);
            if self.peek().map(|s| s.as_str()) == Some(",") {
                self.advance();
            }
        }
        self.expect(")")?;
        self.expect("{")?;
        let body = self.parse_block()?;

        Ok(Statement::FunctionDef { name, params, body, annotations })
    }

    fn parse_use(&mut self) -> Result<Statement, String> {
        self.expect("use")?;
        let module = self.advance().ok_or("Expected module name")?;
        self.expect(";")?;
        Ok(Statement::Use(module))
    }

    fn parse_class_def(&mut self) -> Result<Statement, String> {
        self.expect("class")?;
        let name = self.advance().ok_or("Expected class name")?;
        
        // Check for optional "extends ParentClass"
        let parent = if self.peek().map(|s| s.as_str()) == Some("extends") {
            self.advance(); // consume "extends"
            Some(self.advance().ok_or("Expected parent class name")?)
        } else {
            None
        };
        
        self.expect("{")?;
        
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        
        while self.peek().map(|s| s.as_str()) != Some("}") {
            if self.peek().is_none() {
                return Err("Unexpected end of class definition".to_string());
            }
            
            let token = self.peek().unwrap().clone();
            if token == "let" {
                // Parse field
                self.advance(); // consume "let"
                let field_name = self.advance().ok_or("Expected field name")?;
                self.expect("=")?;
                let default_value = self.parse_expression()?;
                self.expect(";")?;
                fields.push((field_name, default_value));
            } else if token == "fn" {
                // Parse method
                self.advance(); // consume "fn"
                let method_name = self.advance().ok_or("Expected method name")?;
                self.expect("(")?;
                
                let mut params = Vec::new();
                while self.peek().map(|s| s.as_str()) != Some(")") {
                    params.push(self.advance().ok_or("Expected parameter name")?);
                    if self.peek().map(|s| s.as_str()) == Some(",") {
                        self.advance();
                    }
                }
                self.expect(")")?;
                self.expect("{")?;
                let body = self.parse_block()?;
                methods.push((method_name, params, body));
            } else {
                return Err(format!("Unexpected token in class definition: {}", token));
            }
        }
        
        self.expect("}")?;
        Ok(Statement::ClassDef { name, parent, fields, methods })
    }

    fn parse_return(&mut self) -> Result<Statement, String> {
        self.expect("return")?;
        let expr = self.parse_expression()?;
        self.expect(";")?;
        Ok(Statement::Return(expr))
    }

    fn parse_exit(&mut self) -> Result<Statement, String> {
        self.expect("exit")?;
        // Check if there's an expression after exit
        if self.peek() == Some(&";".to_string()) {
            // exit with default code 0
            self.expect(";")?;
            Ok(Statement::Exit(None))
        } else {
            // exit with specific code
            let expr = self.parse_expression()?;
            self.expect(";")?;
            Ok(Statement::Exit(Some(expr)))
        }
    }

    fn parse_print(&mut self) -> Result<Statement, String> {
        self.expect("print")?;
        let expr = self.parse_expression()?;
        self.expect(";")?;
        Ok(Statement::Print(expr))
    }

    fn parse_assert(&mut self) -> Result<Statement, String> {
        self.expect("assert")?;
        let condition = self.parse_expression()?;
        
        let message = if self.peek() == Some(&",".to_string()) {
            self.expect(",")?;
            Some(self.parse_expression()?)
        } else {
            None
        };
        
        self.expect(";")?;
        Ok(Statement::Assert { condition, message })
    }

    fn parse_command(&mut self) -> Result<Statement, String> {
        self.expect("exec")?;
        let cmd = self.parse_expression()?;
        self.expect(";")?;
        if let Expression::Value(Value::String(s)) = cmd {
            Ok(Statement::Command(s))
        } else {
            Err("exec requires a string argument".to_string())
        }
    }

    fn parse_pipeline(&mut self) -> Result<Statement, String> {
        // Parse a direct pipeline like: list-directory | slice-object -f name;
        let mut pipeline = String::new();
        
        while self.peek().is_some() {
            let token = self.peek().unwrap();
            if token == ";" {
                break;
            }
            pipeline.push_str(&self.advance().unwrap());
            pipeline.push(' ');
        }
        
        // Consume semicolon if present
        if self.peek() == Some(&";".to_string()) {
            self.advance();
        }
        
        Ok(Statement::Command(pipeline.trim().to_string()))
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, String> {
        let mut statements = Vec::new();
        
        while self.peek().map(|s| s.as_str()) != Some("}") {
            if self.peek().is_none() {
                return Err("Unexpected end of block".to_string());
            }
            statements.push(self.parse_statement()?);
        }
        
        self.expect("}")?;
        Ok(statements)
    }

    pub fn parse_expression(&mut self) -> Result<Expression, String> {
        self.parse_pipeline_op()
    }

    fn parse_pipeline_op(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_or()?;

        // Handle pipeline operator (|) as expression-level operator
        while self.peek().map(|s| s.as_str()) == Some("|") {
            // Check if this looks like a pipeline with commands (not just |)
            if self.pos + 1 < self.tokens.len() {
                let next_token = &self.tokens[self.pos + 1];
                // If next token looks like a command, parse as pipeline
                if next_token.contains('-') || next_token.chars().all(|c| c.is_alphanumeric() || c == '-') {
                    self.advance(); // consume '|'
                    
                    // Collect tokens for just this stage of the pipeline (until next | or ;)
                    let mut pipeline_parts = Vec::new();
                    while self.peek().is_some() {
                        let peek_val = self.peek().map(|s| s.as_str());
                        if peek_val == Some(";") || peek_val == Some(")") || peek_val == Some("|") {
                            break;
                        }
                        pipeline_parts.push(self.advance().unwrap());
                    }
                    
                    let command = pipeline_parts.join(" ");
                    left = Expression::Pipeline {
                        input: Box::new(left),
                        command,
                    };
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn contains_pipe_before_semicolon(&self) -> bool {
        let mut lookahead = self.pos;
        while lookahead < self.tokens.len() {
            let token = &self.tokens[lookahead];
            if token == ";" {
                return false;
            }
            if token == "|" {
                return true;
            }
            lookahead += 1;
        }
        false
    }

    fn looks_like_command(&self) -> bool {
        // Check if the next token looks like a stargate command (contains hyphens)
        if let Some(token) = self.tokens.get(self.pos) {
            // Exclude string literals (they have quotes)
            if token.starts_with('"') && token.ends_with('"') {
                return false;
            }
            // Commands typically have hyphens like "list-directory", "get-hostname"
            // But exclude negative numbers
            return token.contains('-') && !token.starts_with('-') && token.len() > 1;
        }
        false
    }

    fn parse_pipeline_expr(&mut self) -> Result<Expression, String> {
        let mut pipeline_tokens = Vec::new();
        while self.peek().is_some() && self.peek().map(|s| s.as_str()) != Some(";") {
            pipeline_tokens.push(self.advance().ok_or("Unexpected end")?);
        }
        let pipeline_str = pipeline_tokens.join(" ");
        Ok(Expression::CommandOutput(pipeline_str))
    }

    fn parse_or(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_and()?;

        while self.peek().map(|s| s.as_str()) == Some("||") {
            self.advance();
            let right = self.parse_and()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: Operator::Or,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_comparison()?;

        while self.peek().map(|s| s.as_str()) == Some("&&") {
            self.advance();
            let right = self.parse_comparison()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op: Operator::And,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_additive()?;

        while let Some(token) = self.peek() {
            let op = match token.as_str() {
                "==" => Operator::Eq,
                "!=" => Operator::Ne,
                "<" => Operator::Lt,
                ">" => Operator::Gt,
                "<=" => Operator::Le,
                ">=" => Operator::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_multiplicative()?;

        while let Some(token) = self.peek() {
            let op = match token.as_str() {
                "+" => Operator::Add,
                "-" => Operator::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expression, String> {
        let mut left = self.parse_postfix()?;

        while let Some(token) = self.peek() {
            let op = match token.as_str() {
                "*" => Operator::Mul,
                "/" => Operator::Div,
                "%" => Operator::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_postfix()?;
            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_postfix(&mut self) -> Result<Expression, String> {
        let mut expr = self.parse_unary()?;

        loop {
            match self.peek().map(|s| s.as_str()) {
                Some(".") => {
                    self.advance(); // consume '.'
                    let property = self.advance().ok_or("Expected property name after '.'")?;
                    
                    // Check if this is a method call (property followed by '(')
                    if self.peek().map(|s| s.as_str()) == Some("(") {
                        self.advance(); // consume '('
                        let mut args = Vec::new();
                        
                        if self.peek().map(|s| s.as_str()) != Some(")") {
                            loop {
                                args.push(self.parse_expression()?);
                                if self.peek().map(|s| s.as_str()) == Some(",") {
                                    self.advance(); // consume ','
                                } else {
                                    break;
                                }
                            }
                        }
                        
                        self.expect(")")?;
                        expr = Expression::MethodCall {
                            object: Box::new(expr),
                            method: property,
                            args,
                        };
                    } else {
                        expr = Expression::PropertyAccess {
                            object: Box::new(expr),
                            property,
                        };
                    }
                }
                Some("[") => {
                    self.advance(); // consume '['
                    let index = self.parse_expression()?;
                    self.expect("]")?;
                    expr = Expression::IndexAccess {
                        object: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expression, String> {
        // Handle unary operators like !
        if self.peek().map(|s| s.as_str()) == Some("!") {
            self.advance(); // consume '!'
            let operand = self.parse_unary()?; // Allow chaining: !!x
            return Ok(Expression::UnaryOp {
                op: Operator::Not,
                operand: Box::new(operand),
            });
        }
        
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expression, String> {
        let token = self.advance().ok_or("Unexpected end of expression")?;

        if token == "(" {
            // Look ahead to see if this is a command (contains hyphens or pipes)
            let mut lookahead = self.pos;
            let mut looks_like_command = false;
            let mut paren_depth = 1;
            
            while lookahead < self.tokens.len() && paren_depth > 0 {
                let t = &self.tokens[lookahead];
                if t == "(" {
                    paren_depth += 1;
                } else if t == ")" {
                    paren_depth -= 1;
                } else if t == "|" || (t.contains('-') && !t.starts_with('-') && t.len() > 1) {
                    // Contains pipe or has hyphens (but not negative numbers)
                    looks_like_command = true;
                }
                lookahead += 1;
            }
            
            if looks_like_command {
                // Parse as command output
                let mut cmd = String::new();
                while self.peek().map(|s| s.as_str()) != Some(")") {
                    let tok = self.advance().ok_or("Expected command")?;
                    if !cmd.is_empty() {
                        cmd.push(' ');
                    }
                    cmd.push_str(&tok);
                }
                self.expect(")")?;
                return Ok(Expression::CommandOutput(cmd));
            } else {
                // Parse as expression
                let expr = self.parse_expression()?;
                self.expect(")")?;
                return Ok(expr);
            }
        }

        if token == "[" {
            // Parse list literal: [expr, expr, ...]
            let mut elements = Vec::new();
            
            // Handle empty list
            if self.peek().map(|s| s.as_str()) == Some("]") {
                self.advance(); // consume ']'
                return Ok(Expression::ListLiteral(elements));
            }
            
            // Parse comma-separated expressions
            loop {
                elements.push(self.parse_expression()?);
                
                if self.peek().map(|s| s.as_str()) == Some(",") {
                    self.advance(); // consume ','
                } else {
                    break;
                }
            }
            
            self.expect("]")?;
            return Ok(Expression::ListLiteral(elements));
        }

        if token == "{" {
            // Parse dictionary literal: {key: value, key: value, ...}
            let mut pairs = Vec::new();
            
            // Handle empty dict
            if self.peek().map(|s| s.as_str()) == Some("}") {
                self.advance(); // consume '}'
                return Ok(Expression::DictLiteral(pairs));
            }
            
            // Parse comma-separated key: value pairs
            loop {
                // Parse key expression
                let key = self.parse_expression()?;
                
                // Expect colon
                self.expect(":")?;
                
                // Parse value expression
                let value = self.parse_expression()?;
                
                pairs.push((key, value));
                
                if self.peek().map(|s| s.as_str()) == Some(",") {
                    self.advance(); // consume ','
                } else {
                    break;
                }
            }
            
            self.expect("}")?;
            return Ok(Expression::DictLiteral(pairs));
        }

        if token == "$" {
            // Command output substitution: $(command)
            self.expect("(")?;
            let mut cmd = String::new();
            while self.peek().map(|s| s.as_str()) != Some(")") {
                cmd.push_str(&self.advance().ok_or("Expected command")?);
                cmd.push(' ');
            }
            self.expect(")")?;
            return Ok(Expression::CommandOutput(cmd.trim().to_string()));
        }

        // Check if it's a function call
        if self.peek().map(|s| s.as_str()) == Some("(") {
            let args = self.parse_args()?;
            return Ok(Expression::FunctionCall { name: token, args });
        }

        // Try to parse as number
        if let Ok(num) = token.parse::<f64>() {
            return Ok(Expression::Value(Value::Number(num)));
        }

        // Check for boolean literals
        match token.as_str() {
            "true" => return Ok(Expression::Value(Value::Bool(true))),
            "false" => return Ok(Expression::Value(Value::Bool(false))),
            "null" => return Ok(Expression::Value(Value::Null)),
            "new" => {
                // Parse new ClassName or new ClassName()
                let class_name = self.advance().ok_or("Expected class name after 'new'")?;
                // Check for optional empty parentheses
                if self.peek() == Some(&"(".to_string()) {
                    self.advance(); // consume '('
                    self.expect(")")?; // expect ')'
                }
                return Ok(Expression::NewInstance { class_name });
            }
            _ => {}
        }

        // Check if it's a string literal (has quotes)
        if token.starts_with('"') && token.ends_with('"') {
            let string_content = &token[1..token.len()-1];
            // Check if string contains interpolation {var}
            if string_content.contains('{') {
                return Ok(Expression::InterpolatedString(string_content.to_string()));
            }
            return Ok(Expression::Value(Value::String(string_content.to_string())));
        }

        // Check if this looks like a command (contains hyphens and next token is pipe or end)
        // This handles cases like: let x = list-directory | collect-count;
        if token.contains('-') && !token.starts_with('-') {
            let next = self.peek().map(|s| s.as_str());
            if next == Some("|") || next == Some(";") {
                // This is a standalone command, wrap it as CommandOutput
                return Ok(Expression::CommandOutput(token));
            }
        }

        // Otherwise it's a variable
        Ok(Expression::Variable(token))
    }

    fn parse_args(&mut self) -> Result<Vec<Expression>, String> {
        self.expect("(")?;
        let mut args = Vec::new();

        while self.peek().map(|s| s.as_str()) != Some(")") {
            args.push(self.parse_expression()?);
            if self.peek().map(|s| s.as_str()) == Some(",") {
                self.advance();
            }
        }

        self.expect(")")?;
        Ok(args)
    }

    fn expect(&mut self, expected: &str) -> Result<(), String> {
        let token = self.advance().ok_or(format!("Expected '{}', got end of input", expected))?;
        if token == expected {
            Ok(())
        } else {
            Err(format!("Expected '{}', got '{}'", expected, token))
        }
    }
}
