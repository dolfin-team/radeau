// dolfin-cli/src/commands/check.rs (continued)
use crate::CliError;
use rowl::ast::Declaration;
use rowl::package;
use std::path::PathBuf;

pub fn run(path: PathBuf, verbose: bool) -> Result<(), CliError> {
    println!("Checking package at {}...", path.display());
    println!();

    // Load the package (this performs parsing and resolution)
    let package = package::load_package(&path)?;

    // Print package info
    println!("Package: {}", package.namespace().full());
    println!("Version: {}", package.version());
    println!("Dolfin Version: {}", package.dolfin_version());
    println!();

    // Count declarations
    let mut concept_count = 0;
    let mut property_count = 0;
    let mut enum_count = 0;
    let mut rule_count = 0;

    for (namespace, ontology) in package.iter_ontologies() {
        if verbose {
            println!(
                "  {} ({})",
                namespace.full(),
                ontology.relative_path.display()
            );
            if let Some(iri_name) = &ontology.iri_name {
                println!("    IRI name: {}", iri_name);
            }
        }

        for decl in &ontology.ast.declarations {
            match decl {
                Declaration::Concept(c) => {
                    concept_count += 1;
                    if verbose {
                        println!("    concept {}", c.name);
                    }
                }
                Declaration::Property(p) => {
                    property_count += 1;
                    if verbose {
                        println!("    property {}", p.name);
                    }
                }
                Declaration::Enum(e) => {
                    enum_count += 1;
                    if verbose {
                        println!("    enum {} ({} variants)", e.name, e.variants.len());
                    }
                }
                Declaration::Rule(r) => {
                    rule_count += 1;
                    if verbose {
                        println!("    rule {}", r.name);
                    }
                }
            }
        }
    }

    // Run additional checks
    let warnings = package::check_package(&path)?;

    println!("Files: {}", package.ontologies.len());
    println!("Concepts: {}", concept_count);
    println!("Properties: {}", property_count);
    println!("Enums: {}", enum_count);
    println!("Rules: {}", rule_count);
    println!();

    if warnings.is_empty() {
        println!("✓ No issues found.");
    } else {
        println!("Warnings:");
        for warning in &warnings {
            println!("  ⚠ {}", warning);
        }
    }

    Ok(())
}
