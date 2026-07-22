//! Initialize a new Dolfin package.

use crate::CliError;
use std::fs;
use std::path::PathBuf;

pub fn run(
    name: String,
    path: PathBuf,
    description: Option<String>,
    authors: Option<Vec<String>>,
) -> Result<(), CliError> {
    // Create directory if it doesn't exist
    if !path.exists() {
        fs::create_dir_all(&path)?;
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
    fs::write(&manifest_path, &content)?;
    println!("Created {}", manifest_path.display());

    // Create a sample ontology file
    let sample_path = path.join("main.dlf");
    if !sample_path.exists() {
        let sample_content = r#"# Main ontology file
# Add your concepts, properties, enums, and rules here

concept Thing:
  has name: string
"#;
        fs::write(&sample_path, sample_content)?;
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

/// Escape a string for Dolfin string literals.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
