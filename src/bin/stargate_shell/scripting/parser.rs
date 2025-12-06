use super::ast::*;
use super::value::Value;
use crate::stargate_shell::commands;

pub struct Parser {
    tokens: Vec<String>,
    pos: usize,
    is_interactive: bool,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        let tokens = Self::tokenize(input);
        Parser { tokens, pos: 0, is_interactive: false }
    }

    pub fn new_interactive(input: &str) -> Self {
        let tokens = Self::tokenize(input);
        Parser { tokens, pos: 0, is_interactive: true }
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
                    '(' | ')' | '{' | '}' | ';' | ',' | '=' | '+' | '*' | '/' | '%' | '<' | '>' | '!' | '[' | ']' | '.' | '|' | '&' | ':' => {
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
        
        // Check if there's a comma for dictionary iteration: for k, v in dict
        let value_name = if self.peek().map(|s| s.as_str()) == Some(",") {
            self.advance(); // consume ","
            Some(self.advance().ok_or("Expected value variable name after ','")?)
        } else {
            None
        };
        
        self.expect("in")?;
        let iterable = self.parse_expression()?;
        self.expect("{")?;
        let body = self.parse_block()?;

        Ok(Statement::For {
            var_name,
            value_name,
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
                                args.push(self.parse_arg_or_closure()?);
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
            let mut first_token = None;
            
            while lookahead < self.tokens.len() && paren_depth > 0 {
                let t = &self.tokens[lookahead];
                if t == "(" {
                    paren_depth += 1;
                } else if t == ")" {
                    paren_depth -= 1;
                } else if t == "|" || (t.contains('-') && !t.starts_with('-') && t.len() > 1) {
                    // Contains pipe or has hyphens (but not negative numbers)
                    looks_like_command = true;
                } else if first_token.is_none() && t != ")" {
                    // Capture the first token to check if it's a known command
                    first_token = Some(t.clone());
                }
                lookahead += 1;
            }
            
            // Also check if the first token is a known stargate command
            if let Some(cmd_name) = &first_token {
                if commands::is_stargate_command(cmd_name) {
                    looks_like_command = true;
                }
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

        // Check for set() or set{} syntax for sets
        if token == "set" {
            let next = self.peek().map(|s| s.as_str());
            
            if next == Some("(") {
                // set() syntax
                self.advance(); // consume '('
                
                let mut elements = Vec::new();
                
                // Check if it's empty
                if self.peek().map(|s| s.as_str()) == Some(")") {
                    self.advance(); // consume ')'
                    return Ok(Expression::SetLiteral(elements));
                }
                
                // Parse comma-separated set elements
                loop {
                    elements.push(self.parse_expression()?);
                    
                    if self.peek().map(|s| s.as_str()) == Some(",") {
                        self.advance(); // consume ','
                        
                        // Check for trailing comma
                        if self.peek().map(|s| s.as_str()) == Some(")") {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                
                self.expect(")")?;
                return Ok(Expression::SetLiteral(elements));
            } else if next == Some("{") {
                // set{} syntax (also supported)
                self.advance(); // consume '{'
                
                let mut elements = Vec::new();
                
                // Check if it's empty
                if self.peek().map(|s| s.as_str()) == Some("}") {
                    self.advance(); // consume '}'
                    return Ok(Expression::SetLiteral(elements));
                }
                
                // Parse comma-separated set elements
                loop {
                    elements.push(self.parse_expression()?);
                    
                    if self.peek().map(|s| s.as_str()) == Some(",") {
                        self.advance(); // consume ','
                        
                        // Check for trailing comma
                        if self.peek().map(|s| s.as_str()) == Some("}") {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                
                self.expect("}")?;
                return Ok(Expression::SetLiteral(elements));
            }
        }

        if token == "{" {
            // Parse dictionary or set literal
            // Dict: {key: value, key: value, ...}
            // Set: {value, value, ...}
            
            // Handle empty dict/set - default to dict for backwards compatibility
            if self.peek().map(|s| s.as_str()) == Some("}") {
                self.advance(); // consume '}'
                return Ok(Expression::DictLiteral(Vec::new()));
            }
            
            // Parse first element to determine if it's a dict or set
            let first_expr = self.parse_expression()?;
            
            // Check if this is a dict (has colon) or set (no colon)
            if self.peek().map(|s| s.as_str()) == Some(":") {
                // It's a dict
                self.advance(); // consume ':'
                let first_value = self.parse_expression()?;
                let mut pairs = vec![(first_expr, first_value)];
                
                // Parse remaining key: value pairs
                while self.peek().map(|s| s.as_str()) == Some(",") {
                    self.advance(); // consume ','
                    
                    // Check for trailing comma
                    if self.peek().map(|s| s.as_str()) == Some("}") {
                        break;
                    }
                    
                    let key = self.parse_expression()?;
                    self.expect(":")?;
                    let value = self.parse_expression()?;
                    pairs.push((key, value));
                }
                
                self.expect("}")?;
                return Ok(Expression::DictLiteral(pairs));
            } else {
                // It's a set
                let mut elements = vec![first_expr];
                
                // Parse remaining elements
                while self.peek().map(|s| s.as_str()) == Some(",") {
                    self.advance(); // consume ','
                    
                    // Check for trailing comma
                    if self.peek().map(|s| s.as_str()) == Some("}") {
                        break;
                    }
                    
                    elements.push(self.parse_expression()?);
                }
                
                self.expect("}")?;
                return Ok(Expression::SetLiteral(elements));
            }
        }

        if token == "|" {
            // Parse closure: |param1, param2| expression
            let mut params = Vec::new();
            
            // Parse parameters
            if self.peek().map(|s| s.as_str()) != Some("|") {
                loop {
                    let param = self.advance().ok_or("Expected parameter name in closure")?;
                    params.push(param);
                    
                    if self.peek().map(|s| s.as_str()) == Some(",") {
                        self.advance(); // consume ','
                    } else {
                        break;
                    }
                }
            }
            
            self.expect("|")?; // closing pipe
            
            // Parse the body expression
            let body = self.parse_expression()?;
            
            return Ok(Expression::Closure {
                params,
                body: Box::new(body),
            });
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
            // Check if it's an integer in i32 range
            if num.fract() == 0.0 && num >= i32::MIN as f64 && num <= i32::MAX as f64 {
                return Ok(Expression::Value(Value::SmallInt(num as i32)));
            } else {
                return Ok(Expression::Value(Value::Number(num)));
            }
        }

        // Check for boolean literals
        match token.as_str() {
            "true" => return Ok(Expression::Value(Value::Bool(true))),
            "false" => return Ok(Expression::Value(Value::Bool(false))),
            "none" => return Ok(Expression::Value(Value::None)),
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

        // Check for 'this' keyword
        if token == "this" {
            return Ok(Expression::This);
        }

        // Otherwise it's a variable
        Ok(Expression::Variable(token))
    }

    fn parse_args(&mut self) -> Result<Vec<Expression>, String> {
        self.expect("(")?;
        let mut args = Vec::new();

        while self.peek().map(|s| s.as_str()) != Some(")") {
            // Check if this is a closure with new syntax: param: expr or param1, param2: expr
            let arg = self.parse_arg_or_closure()?;
            args.push(arg);
            if self.peek().map(|s| s.as_str()) == Some(",") {
                self.advance();
            }
        }

        self.expect(")")?;
        Ok(args)
    }
    
    fn parse_arg_or_closure(&mut self) -> Result<Expression, String> {
        // Try to detect closure pattern: param: expr or param1, param2: expr
        // We need to look ahead carefully to distinguish between:
        //   - Closure: x: x * 2  or  acc, x: acc + x
        //   - Regular expression: foo.bar
        
        let start_pos = self.pos;
        
        // Look ahead to check for closure pattern
        let mut lookahead_pos = self.pos;
        let mut found_colon = false;
        let mut identifiers_seen = 0;
        
        // Scan ahead looking for pattern: identifier [, identifier]* :
        loop {
            if lookahead_pos >= self.tokens.len() {
                break;
            }
            
            let tok = &self.tokens[lookahead_pos];
            
            if tok == ":" {
                // Found colon - if we've seen at least one identifier, this is a closure
                if identifiers_seen > 0 {
                    found_colon = true;
                }
                break;
            } else if tok == "," {
                // Comma - could be between params
                lookahead_pos += 1;
            } else if tok.chars().all(|c| c.is_alphanumeric() || c == '_') {
                // Looks like an identifier
                if tok.chars().next().map(|c| c.is_numeric()).unwrap_or(false) {
                    // Starts with number - not an identifier for params
                    break;
                }
                identifiers_seen += 1;
                lookahead_pos += 1;
            } else {
                // Something else - not a closure pattern
                break;
            }
        }
        
        if found_colon {
            // This is a closure! Parse parameters
            let mut params = Vec::new();
            
            loop {
                let param = self.advance().ok_or("Expected parameter name in closure")?;
                
                // Validate identifier
                if param.is_empty() || !param.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    return Err(format!("Invalid parameter name: {}", param));
                }
                if param.chars().next().unwrap().is_numeric() {
                    return Err(format!("Parameter name cannot start with digit: {}", param));
                }
                
                params.push(param);
                
                let next = self.peek().map(|s| s.as_str());
                if next == Some(",") {
                    self.advance(); // consume ','
                } else if next == Some(":") {
                    self.advance(); // consume ':'
                    break;
                } else {
                    return Err(format!("Expected ',' or ':' in closure, got {:?}", next));
                }
            }
            
            // Parse the body expression
            let body = self.parse_or()?;
            
            return Ok(Expression::Closure {
                params,
                body: Box::new(body),
            });
        }
        
        // Not a closure, parse as normal expression
        self.pos = start_pos;
        self.parse_expression()
    }
    
    fn parse_closure_body(&mut self) -> Result<Expression, String> {
        // Parse expression but stop at ',' or ')' at the same nesting level
        self.parse_or()
    }

    fn expect(&mut self, expected: &str) -> Result<(), String> {
        // In interactive mode, make semicolons optional
        if self.is_interactive && expected == ";" {
            if self.peek() == Some(&";".to_string()) {
                self.advance();
            }
            return Ok(());
        }
        
        let token = self.advance().ok_or(format!("Expected '{}', got end of input", expected))?;
        if token == expected {
            Ok(())
        } else {
            Err(format!("Expected '{}', got '{}'", expected, token))
        }
    }
}
