//! Build a Dolfin package into a target format.

use crate::{CliError, EmitFormat};
use irukame::{TurtleGenerator, TurtleOptions};
use rowl::package;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

pub fn run(
    path: PathBuf,
    output: Option<PathBuf>,
    base_iri: String,
    no_comments: bool,
    no_rules: bool,
    emit: EmitFormat,
) -> Result<(), CliError> {
    eprintln!("Loading package from {}...", path.display());

    // Load the package
    let package = package::load_package(&path)?;

    eprintln!(
        "Building {} ({} files)...",
        package.namespace().full(),
        package.ontologies.len()
    );

    let mut generator = TurtleGenerator::new(TurtleOptions {
        base_iri,
        include_comments: !no_comments,
        include_rules_as_comments: no_rules,
        include_queries_as_comments: true,
    });
    let content = match emit {
        EmitFormat::Turtle => generator.generate(&package)?,
        EmitFormat::N3Rules => generator.generate_n3_rules(&package)?,
    };
    for (rule, reason) in generator.skipped_rules() {
        eprintln!("warning: rule `{rule}` written as a comment: {reason}");
    }

    // Write output
    match output {
        Some(output_path) => {
            fs::write(&output_path, &content).map_err(|e| CliError::Io(output_path.clone(), e))?;
            eprintln!("✓ Written to {}", output_path.display());
        }
        None => {
            io::stdout()
                .write_all(content.as_bytes())
                .map_err(|e| CliError::Io(PathBuf::new(), e))?;
        }
    }

    Ok(())
}
