use std::path::PathBuf;

use thiserror::Error;

/// CLI errors
#[derive(Debug, Error)]
pub enum CliError {
    #[error("Package error: {0}")]
    Package(#[from] Box<rowl::package::PackageError>),

    #[error("Code generation error: {0}")]
    Codegen(#[from] irukame::TurtleError),

    #[error("IO error: {0}")]
    Io(PathBuf, #[source] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] Box<rowl::ParseError>),

    #[error("Not a Dolfin package (no package.dlf found): {0}")]
    NotAPackage(PathBuf),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("{kind} '{name}' already exists in {file}")]
    DuplicateDeclaration {
        kind: String,
        name: String,
        file: PathBuf,
    },

    #[error("{kind} '{name}' not found in {file}")]
    DeclarationNotFound {
        kind: String,
        name: String,
        file: PathBuf,
    },

    #[error("{kind} '{name}' is used in {usage_count} location(s)")]
    SymbolInUse {
        kind: String,
        name: String,
        usage_count: usize,
    },

    #[error("Linting errors found")]
    LintError,

    #[error("Invalid rule IDs or categories: {0}")]
    InvalidRuleIds(String),

    #[error("Format check failed: files need formatting")]
    FormatCheckFailed,

    #[error("Formatting errors occurred")]
    FormatError,

    #[error("{0}")]
    Other(String),
}
