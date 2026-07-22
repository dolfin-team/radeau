//! Build a Dolfin package into Turtle format.

use crate::CliError;
use rowl::package;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use irukame::{TurtleGenerator, TurtleOptions};

pub fn run(
    path: PathBuf,
    output: Option<PathBuf>,
    base_iri: String,
    no_comments: bool,
) -> Result<(), CliError> {
    eprintln!("Loading package from {}...", path.display());

    // Load the package
    let package = package::load_package(&path)?;

    eprintln!(
        "Building {} ({} files)...",
        package.namespace().full(),
        package.ontologies.len()
    );

    // Configure the generator
    let options = TurtleOptions {
        base_iri,
        include_comments: !no_comments,
        include_rules_as_comments: true,
    };

    let generator = TurtleGenerator::new(options);

    // Generate Turtle output
    let turtle = generator.generate(&package)?;

    // Write output
    match output {
        Some(output_path) => {
            fs::write(&output_path, &turtle)?;
            eprintln!("✓ Written to {}", output_path.display());
        }
        None => {
            io::stdout().write_all(turtle.as_bytes())?;
        }
    }

    Ok(())
}
