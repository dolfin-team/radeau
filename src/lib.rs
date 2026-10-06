//! Dolfin CLI library.

pub mod commands;
pub mod error;
pub mod interactive;
pub mod plugin;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Output format for `radeau build --emit`.
#[derive(ValueEnum, Debug, Clone, Default)]
pub enum EmitFormat {
    /// Turtle/OWL (default)
    #[default]
    Turtle,
    /// N3 rules only — suitable for retox::load_n3_rules_from_str
    N3Rules,
}

use crate::error::CliError;

fn get_available_rules() -> Vec<&'static str> {
    let rules = dolfin_lint::LintEngine::list_all_rules();
    rules
        .iter()
        .map(|r| Box::leak(r.id.clone().into_boxed_str()) as &'static str)
        .collect()
}

fn get_available_categories() -> Vec<&'static str> {
    let rules = dolfin_lint::LintEngine::list_all_rules();
    let mut categories: Vec<String> = rules.iter().map(|r| r.category.clone()).collect();
    categories.sort();
    categories.dedup();
    categories
        .into_iter()
        .map(|c| Box::leak(c.into_boxed_str()) as &'static str)
        .collect()
}

/// Dolfin - Ontology definition language toolchain
#[derive(Parser, Debug)]
#[command(name = "radeau")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialize a new Dolfin package
    #[command(
        after_help = "Import an existing ontology instead with `radeau init <format> <FILE>...`,\n\
        served by an external radeau-init-<format> importer (e.g. `radeau init turtle a.ttl`).\n\
        Installed importers are listed by `radeau --help`."
    )]
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

        /// Automatically fix linting issues where possible
        #[arg(long)]
        fix: bool,

        /// Disable specific lint rules (can be repeated)
        /// Example: --disable-rule naming/concept-pascal-case
        #[arg(
            long = "disable-rule",
            value_name = "RULE_ID",
            value_parser = get_available_rules()
        )]
        disable_rules: Vec<String>,

        /// Disable entire rule categories (can be repeated)
        /// Available categories: naming, ordering, unused, semantic, style, package
        #[arg(
            long = "disable-category",
            value_name = "CATEGORY",
            value_parser = get_available_categories()
        )]
        disable_categories: Vec<String>,

        /// List all available lint rules
        #[arg(long)]
        list_rules: bool,
    },

    /// Format Dolfin source files
    Fmt {
        /// Path to package root or a single file
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Run in check mode (exit with error if files need formatting)
        #[arg(long)]
        check: bool,

        /// Suppress output except for errors
        #[arg(short, long)]
        quiet: bool,

        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Print version information
        #[arg(long)]
        version: bool,

        /// Path to package.dlf manifest
        #[arg(long)]
        manifest_path: Option<PathBuf>,
    },

    /// Build a package into a target format
    Build {
        /// Path to package root (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Base IRI for the ontology
        #[arg(long)]
        base_iri: Option<String>,

        /// Exclude comments from output (turtle only)
        #[arg(long)]
        no_comments: bool,

        /// Exclude rules from Turtle output (write them as comments instead)
        #[arg(long)]
        no_rules: bool,

        /// Output format: turtle (default) or n3-rules
        #[arg(long, value_enum, default_value_t = EmitFormat::Turtle)]
        emit: EmitFormat,
    },

    /// Add a new declaration to the package
    Add {
        #[command(subcommand)]
        kind: AddKind,
    },

    /// Modify an existing declaration in the package
    Modify {
        #[command(subcommand)]
        kind: ModifyKind,
    },

    /// Remove a declaration from the package
    Remove {
        #[command(subcommand)]
        kind: RemoveKind,
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

    /// Print package metadata as JSON (namespace, files, concepts,
    /// properties). Intended for plugins to consume via `$RADEAU metadata`
    /// instead of re-implementing package loading.
    Metadata {
        /// Path to package root (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Generate shell completions for the radeau CLI
    Completions {
        /// The shell to generate completions for
        /// Supported shells: bash, zsh, fish, elvish, powershell
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum AddKind {
    /// Add a new concept
    #[command(name = "concept")]
    Concept {
        /// Fully qualified concept name (e.g., general.specific.MyConcept)
        name: String,

        /// Parent concepts to inherit from (can be repeated)
        #[arg(long = "sub", short = 's')]
        subs: Vec<String>,

        /// Property declarations in format "name:type" or "name:cardinality:type" (can be repeated)
        #[arg(long = "has", short = 'H')]
        has: Vec<String>,

        /// Path to the package directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },

    /// Add a new property
    #[command(name = "property")]
    Property {
        /// Fully qualified property name (e.g., general.specific.myProperty)
        name: String,

        /// Domain type
        #[arg(long, short = 'd')]
        domain: String,

        /// Range type
        #[arg(long, short = 'r')]
        range: String,

        /// Domain cardinality (one, optional, any, some)
        #[arg(long, default_value = "one")]
        domain_cardinality: String,

        /// Range cardinality (one, optional, any, some)
        #[arg(long, default_value = "one")]
        range_cardinality: String,

        /// Path to the package directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum ModifyKind {
    /// Modify an existing concept (add/remove has declarations and sub parents)
    #[command(name = "concept")]
    Concept {
        /// Fully qualified concept name (e.g., general.specific.MyConcept)
        name: String,

        /// Has declarations to add ("name:type", "name:card:type") or remove ("~name").
        /// Comma-separated or repeated. Examples: --has age:int, --has ~name
        #[arg(long = "has", short = 'H')]
        has: Vec<String>,

        /// Sub parents to add or remove. Use "~Parent" to remove, "~*" to remove all.
        /// Comma-separated or repeated. Examples: --sub Animal,~Pet,~*
        #[arg(long = "sub", short = 's')]
        subs: Vec<String>,

        /// Path to the package directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },

    /// Modify an existing property (change domain/range types or cardinalities)
    #[command(name = "property")]
    Property {
        /// Fully qualified property name (e.g., general.specific.myProperty)
        name: String,

        /// New domain type
        #[arg(long, short = 'd')]
        domain: Option<String>,

        /// New range type
        #[arg(long, short = 'r')]
        range: Option<String>,

        /// New domain cardinality (one, any, some, optional, N, N..M, N..*).
        /// Use "one" to remove an existing cardinality.
        #[arg(long)]
        domain_cardinality: Option<String>,

        /// New range cardinality (one, any, some, optional, N, N..M, N..*).
        /// Use "one" to remove an existing cardinality.
        #[arg(long)]
        range_cardinality: Option<String>,

        /// Path to the package directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum RemoveKind {
    /// Remove a concept
    #[command(name = "concept")]
    Concept {
        /// Fully qualified concept name (e.g., general.specific.MyConcept)
        name: String,

        /// Force removal even if the concept is used elsewhere
        #[arg(long, short = 'f')]
        force: bool,

        /// Show what would be removed without actually removing
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Path to the package directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },

    /// Remove a property
    #[command(name = "property")]
    Property {
        /// Fully qualified property name (e.g., general.specific.myProperty)
        name: String,

        /// Force removal even if the property is used elsewhere
        #[arg(long, short = 'f')]
        force: bool,

        /// Show what would be removed without actually removing
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Path to the package directory
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
    },
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

        Command::Check {
            path,
            verbose,
            fix,
            disable_rules,
            disable_categories,
            list_rules,
        } => commands::check::run(
            path,
            verbose,
            fix,
            disable_rules,
            disable_categories,
            list_rules,
        ),

        Command::Build {
            path,
            output,
            base_iri,
            no_comments,
            no_rules,
            emit,
        } => commands::build::run(
            path,
            output,
            base_iri.unwrap_or("".to_string()),
            no_comments,
            no_rules,
            emit,
        ),
        Command::Add { kind } => commands::add::run(kind),
        Command::Modify { kind } => commands::modify::run(kind),
        Command::Remove { kind } => {
            use commands::remove::{DeclKind, RemoveOptions};

            let (path, name, decl_kind, options) = match kind {
                RemoveKind::Concept {
                    name,
                    force,
                    dry_run,
                    path,
                } => (
                    path,
                    name,
                    DeclKind::Concept,
                    RemoveOptions { force, dry_run },
                ),
                RemoveKind::Property {
                    name,
                    force,
                    dry_run,
                    path,
                } => (
                    path,
                    name,
                    DeclKind::Property,
                    RemoveOptions { force, dry_run },
                ),
            };

            commands::remove::run(&path, &name, decl_kind, options)
        }
        Command::Parse { path, tokens, json } => commands::parse::run(path, tokens, json),
        Command::Fmt {
            path,
            check,
            quiet,
            verbose,
            version,
            manifest_path,
        } => commands::fmt::run(path, check, quiet, verbose, version, manifest_path),
        Command::Metadata { path } => commands::metadata::run(path),

        Command::Completions { shell } => {
            commands::completions::run(shell);
            Ok(())
        }
    }
}
