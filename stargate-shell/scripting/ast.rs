// Copyright (c) 2025 Dmitry Kalashnikov
// Dual Licensed: Open-Source (see LICENSE) / Commercial (proprietary use)
// Commercial use requires a Commercial License. See LICENSE file.

use super::value::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum AccessModifier {
    Public,
    Private,
    Protected,
}

impl Default for AccessModifier {
    fn default() -> Self {
        AccessModifier::Public
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
        value_name: Option<String>,
        iterable: Expression,
        body: Vec<Statement>,
    },
    While {
        condition: Expression,
        body: Vec<Statement>,
    },
    FunctionDef {
        name: String,
        params: Vec<String>,
        body: Vec<Statement>,
        annotations: Vec<String>,
        access: AccessModifier,
    },
    ClassDef {
        name: String,
        parent: Option<String>,
        fields: Vec<(AccessModifier, String, Expression)>,
        methods: Vec<(AccessModifier, String, Vec<String>, Vec<Statement>)>,
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
    SetLiteral(Vec<Expression>),
    Closure {
        params: Vec<String>,
        body: Box<Expression>,
    },
    This, // References the current instance in a method
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
