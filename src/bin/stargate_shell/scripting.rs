// Scripting language parser for stargate-shell

use serde_json;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Null,
    Object(serde_json::Value), // JSON object/array
}

impl Value {
    pub fn to_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Object(obj) => obj.to_string(),
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Null => false,
            Value::Object(_) => true,
        }
    }

    pub fn to_number(&self) -> f64 {
        match self {
            Value::Number(n) => *n,
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
            Value::String(s) => s.parse().unwrap_or(0.0),
            Value::Null => 0.0,
            Value::Object(_) => 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    VarDecl(String, Expression),
    Assignment(String, Expression),
    If {
        condition: Expression,
        then_block: Vec<Statement>,
        else_block: Option<Vec<Statement>>,
    },
    FunctionDef {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
    },
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
    Command(String),
    Return(Expression),
    Print(Expression),
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
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
    CommandOutput(String),
    InterpolatedString(String), // String with {var} placeholders
    PropertyAccess {
        object: Box<Expression>,
        property: String,
    },
    IndexAccess {
        object: Box<Expression>,
        index: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
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
                    '(' | ')' | '{' | '}' | ';' | ',' | '=' | '+' | '*' | '/' | '<' | '>' | '!' | '[' | ']' | '.' => {
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
        let token = self.peek().ok_or("Unexpected end of input")?.clone();

        match token.as_str() {
            "let" => self.parse_var_decl(),
            "if" => self.parse_if(),
            "fn" => self.parse_function_def(),
            "return" => self.parse_return(),
            "print" => self.parse_print(),
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
                
                // Check if it's an assignment or function call
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
        
        // Check if the right-hand side is a pipeline or command
        let expr = if self.contains_pipe_before_semicolon() || self.looks_like_command() {
            self.parse_pipeline_expr()?
        } else {
            self.parse_expression()?
        };
        
        self.expect(";")?;
        Ok(Statement::VarDecl(name, expr))
    }

    fn parse_assignment(&mut self) -> Result<Statement, String> {
        let name = self.advance().ok_or("Expected variable name")?;
        self.expect("=")?;
        
        // Check if the right-hand side is a pipeline or command
        let expr = if self.contains_pipe_before_semicolon() || self.looks_like_command() {
            self.parse_pipeline_expr()?
        } else {
            self.parse_expression()?
        };
        
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

    fn parse_function_def(&mut self) -> Result<Statement, String> {
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

        Ok(Statement::FunctionDef { name, params, body })
    }

    fn parse_return(&mut self) -> Result<Statement, String> {
        self.expect("return")?;
        let expr = self.parse_expression()?;
        self.expect(";")?;
        Ok(Statement::Return(expr))
    }

    fn parse_print(&mut self) -> Result<Statement, String> {
        self.expect("print")?;
        let expr = self.parse_expression()?;
        self.expect(";")?;
        Ok(Statement::Print(expr))
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
        
        self.expect(";")?;
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

    fn parse_expression(&mut self) -> Result<Expression, String> {
        self.parse_or()
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
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek().map(|s| s.as_str()) {
                Some(".") => {
                    self.advance(); // consume '.'
                    let property = self.advance().ok_or("Expected property name after '.'")?;
                    expr = Expression::PropertyAccess {
                        object: Box::new(expr),
                        property,
                    };
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
