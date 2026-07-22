//! Dolfin CLI library.

pub mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use thiserror::Error;

/// Dolfin - Ontology definition language toolchain
#[derive(Parser, Debug)]
#[command(name = "raft")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize a new Dolfin package
    Init {
        /// Package namespace (e.g., <http://example.com>)
        #[arg(short, long)]
        name: String,

        /// Directory to initialize (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Package description
        #[arg(short, long)]
        description: Option<String>,

        /// Package authors
        #[arg(short, long("author"))]
        author: Option<Vec<String>>,
    },

    /// Check a package for errors
    Check {
        /// Path to package root (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Build a package into Turtle format
    Build {
        /// Path to package root (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Base IRI for the ontology
        #[arg(long, default_value = "http://example.org/")]
        base_iri: String,

        /// Exclude comments from output
        #[arg(long)]
        no_comments: bool,
    },

    /// Parse a single file and show the AST (for debugging)
    Parse {
        /// File to parse
        path: PathBuf,

        /// Show tokens instead of AST
        #[arg(long)]
        tokens: bool,

        /// Show as JSON
        #[arg(long)]
        json: bool,
    },
}

/// CLI errors
#[derive(Debug, Error)]
pub enum CliError {
    #[error("Package error: {0}")]
    Package(#[from] rowl::package::PackageError),

    #[error("Code generation error: {0}")]
    Codegen(#[from] irukame::TurtleError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] rowl::ParseError),

    #[error("{0}")]
    Other(String),
}

/// Run the CLI with the given arguments.
pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Init {
            name,
            path,
            description,
            author,
        } => commands::init::run(name, path, description, author),

        Command::Check { path, verbose } => commands::check::run(path, verbose),

        Command::Build {
            path,
            output,
            base_iri,
            no_comments,
        } => commands::build::run(path, output, base_iri, no_comments),

        Command::Parse { path, tokens, json } => commands::parse::run(path, tokens, json),
    }
}
