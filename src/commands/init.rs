//! Initialize a new Dolfin package.

use crate::CliError;
use std::fs;
use std::path::PathBuf;

pub fn run(
    name: Option<String>,
    path: PathBuf,
    description: Option<String>,
    authors: Option<Vec<String>>,
    with_turtle: Vec<PathBuf>,
) -> Result<(), CliError> {
    // Create directory if it doesn't exist
    if !path.exists() {
        let path_for_err = path.clone();
        fs::create_dir_all(&path).map_err(|e| CliError::Io(path_for_err, e))?;
        println!("Created directory: {}", path.display());
    }

    // Check if package.dlf already exists
    let manifest_path = path.join("package.dlf");
    if manifest_path.exists() {
        return Err(CliError::Other(format!(
            "Package already exists: {}",
            manifest_path.display()
        )));
    }

    // Turtle-seeded init: reverse the RDF into Dolfin with mekarui.
    if !with_turtle.is_empty() {
        return run_from_turtle(&path, &with_turtle);
    }

    let name = name.ok_or_else(|| {
        CliError::Other("--name is required (or pass --with-turtle to derive it)".to_string())
    })?;

    // Generate package.dlf content
    let mut content = format!(
        r#"package {}:
  dolfin_version "1"
  version "0.1.0"
"#,
        name
    );

    if let Some(authors) = authors {
        for author in authors {
            content.push_str(&format!(
                "  author \"{}\"\n",
                escape_string(author.as_str())
            ));
        }
    }

    if let Some(description) = description {
        content.push_str(&format!(
            "  description \"{}\"\n",
            escape_string(&description)
        ));
    }

    // Write package.dlf
    fs::write(&manifest_path, &content).map_err(|e| CliError::Io(manifest_path.clone(), e))?;
    println!("Created {}", manifest_path.display());

    // Create a sample ontology file
    let sample_path = path.join("main.dlf");
    if !sample_path.exists() {
        let sample_content = r#"# Main ontology file
# Add your concepts, properties, enums, and rules here

concept Thing:
  has name: string
"#;
        fs::write(&sample_path, sample_content)
            .map_err(|e| CliError::Io(sample_path.clone(), e))?;
        println!("Created {}", sample_path.display());
    }

    println!();
    println!(
        "Initialized Dolfin package '{}' in {}",
        name,
        path.display()
    );
    println!();
    println!("Next steps:");
    println!(
        "  1. Edit {} to add your ontology definitions",
        sample_path.display()
    );
    println!("  2. Run 'dolfin check {}' to validate", path.display());
    println!(
        "  3. Run 'dolfin build {}' to generate Turtle output",
        path.display()
    );

    Ok(())
}

/// Seed a package by reversing one or more RDF/Turtle files into Dolfin with
/// mekarui. The files are loaded into a single graph, so declarations may span
/// several inputs.
fn run_from_turtle(path: &PathBuf, turtle_files: &[PathBuf]) -> Result<(), CliError> {
    // Concatenate the documents into one Turtle blob (oxigraph tolerates
    // repeated @prefix declarations across the inputs).
    let mut blob = String::new();
    for file in turtle_files {
        let text = fs::read_to_string(file).map_err(|e| CliError::Io(file.clone(), e))?;
        blob.push_str(&text);
        blob.push('\n');
    }

    let written = mekarui::turtle_to_dir(blob.as_bytes(), &mekarui::MekaruiOptions::default(), path)
        .map_err(|e| CliError::Other(format!("mekarui: {e}")))?;

    let sources: Vec<String> = turtle_files.iter().map(|p| p.display().to_string()).collect();
    println!(
        "Reversed {} into a Dolfin package at {}",
        sources.join(", "),
        path.display()
    );
    let mut written = written;
    written.sort();
    for f in &written {
        println!("  Created {}", f.display());
    }
    println!();
    println!("Next steps:");
    println!("  1. Run 'raft check {}' to validate", path.display());
    println!("  2. Run 'raft build {}' to regenerate Turtle", path.display());

    Ok(())
}

/// Escape a string for Dolfin string literals.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
