use thiserror::Error;
use sgcore::translate;

/// Represents an error encountered while parsing a test expression
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("{}", translate!("test-error-expected-value"))]
    ExpectedValue,
    #[error("{}", translate!("test-error-expected", "value" => .0))]
    Expected(String),
    #[error("{}", translate!("test-error-extra-argument", "argument" => .0))]
    ExtraArgument(String),
    #[error("{}", translate!("test-error-missing-argument", "argument" => .0))]
    MissingArgument(String),
    #[error("{}", translate!("test-error-unknown-operator", "operator" => .0))]
    UnknownOperator(String),
    #[error("{}", translate!("test-error-invalid-integer", "value" => .0))]
    InvalidInteger(String),
    #[error("{}", translate!("test-error-unary-operator-expected", "operator" => .0))]
    UnaryOperatorExpected(String),
}

/// A Result type for parsing test expressions
pub type ParseResult<T> = Result<T, ParseError>;

/// Implement `SGError` trait for `ParseError` to make it easier to return useful error codes from `main()`.
impl sgcore::error::SGError for ParseError {
    fn code(&self) -> i32 {
        2
    }
}

