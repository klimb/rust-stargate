use clap::{Arg, ArgAction, Command};
use std::io::Write;
use syntax_tree::{AstNode, is_truthy};
use thiserror::Error;
use sgcore::os_string_to_vec;
use sgcore::translate;
use sgcore::{
    display::Quotable,
    error::{SGError, SGResult},
    format_usage,
};

mod locale_aware;
mod syntax_tree;

mod options {
    pub const VERSION: &str = "version";
    pub const HELP: &str = "help";
    pub const EXPRESSION: &str = "expression";
}

pub type ExprResult<T> = Result<T, ExprError>;

#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum ExprError {
    #[error("{}", translate!("expr-error-unexpected-argument", "arg" => _0.quote()))]
    UnexpectedArgument(String),
    #[error("{}", translate!("expr-error-missing-argument", "arg" => _0.quote()))]
    MissingArgument(String),
    #[error("{}", translate!("expr-error-non-integer-argument"))]
    NonIntegerArgument,
    #[error("{}", translate!("expr-error-missing-operand"))]
    MissingOperand,
    #[error("{}", translate!("expr-error-division-by-zero"))]
    DivisionByZero,
    #[error("{}", translate!("expr-error-invalid-regex-expression"))]
    InvalidRegexExpression,
    #[error("{}", translate!("expr-error-expected-closing-brace-after", "arg" => _0.quote()))]
    ExpectedClosingBraceAfter(String),
    #[error("{}", translate!("expr-error-expected-closing-brace-instead-of", "arg" => _0.quote()))]
    ExpectedClosingBraceInsteadOf(String),
    #[error("{}", translate!("expr-error-unmatched-opening-parenthesis"))]
    UnmatchedOpeningParenthesis,
    #[error("{}", translate!("expr-error-unmatched-closing-parenthesis"))]
    UnmatchedClosingParenthesis,
    #[error("{}", translate!("expr-error-unmatched-opening-brace"))]
    UnmatchedOpeningBrace,
    #[error("{}", translate!("expr-error-invalid-bracket-content"))]
    InvalidBracketContent,
    #[error("{}", translate!("expr-error-trailing-backslash"))]
    TrailingBackslash,
    #[error("{}", translate!("expr-error-too-big-range-quantifier-index"))]
    TooBigRangeQuantifierIndex,
    #[error("{}", translate!("expr-error-match-utf8", "arg" => _0.quote()))]
    UnsupportedNonUtf8Match(String),
}

impl SGError for ExprError {
    fn code(&self) -> i32 {
        2
    }

    fn usage(&self) -> bool {
        *self == Self::MissingOperand
    }
}

pub fn sg_app() -> Command {
    Command::new(sgcore::util_name())
        .version(sgcore::crate_version!())
        .help_template(sgcore::localized_help_template(sgcore::util_name()))
        .about(translate!("expr-about"))
        .override_usage(format_usage(&translate!("expr-usage")))
        .after_help(translate!("expr-after-help"))
        .infer_long_args(true)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(
            Arg::new(options::VERSION)
                .long(options::VERSION)
                .help(translate!("expr-help-version"))
                .action(ArgAction::Version)
        )
        .arg(
            Arg::new(options::HELP)
                .long(options::HELP)
                .help(translate!("expr-help-help"))
                .action(ArgAction::Help)
        )
        .arg(
            Arg::new(options::EXPRESSION)
                .action(ArgAction::Append)
                .allow_hyphen_values(true)
        )
}

#[sgcore::main]
pub fn sgmain(args: impl sgcore::Args) -> SGResult<()> {
    let args = args
        .skip(1)
        .map(os_string_to_vec)
        .collect::<Result<Vec<_>, _>>()?;
    sgcore::pledge::apply_pledge(&["stdio"])?;

    if args.len() == 1 && args[0] == b"--help" {
        let _ = sg_app().print_help();
    } else if args.len() == 1 && args[0] == b"--version" {
        println!("{} {}", sgcore::util_name(), sgcore::crate_version!());
    } else {
        let args = if !args.is_empty() && args[0] == b"--" {
            &args[1..]
        } else {
            &args
        };

        let res = AstNode::parse(args)?.eval()?.eval_as_string();
        let _ = std::io::stdout().write_all(&res);
        let _ = std::io::stdout().write_all(b"\n");

        if !is_truthy(&res.into()) {
            return Err(1.into());
        }
    }

    Ok(())
}

