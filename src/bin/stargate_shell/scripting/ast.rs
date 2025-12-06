use super::value::Value;

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
