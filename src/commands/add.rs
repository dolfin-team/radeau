//! Add command implementation - adds declarations to Doflin files.

use crate::{AddKind, error::CliError};
use rowl::ast::Declaration::Concept;
use rowl::ast::Declaration::Property;
use rowl::parser;
use std::fs;
use std::path::{Path, PathBuf};

/// Parsed "has" declaration from CLI argument.
#[derive(Debug)]
pub(crate) struct HasDecl {
    pub(crate) name: String,
    pub(crate) cardinality: Option<String>,
    pub(crate) type_ref: String,
}

impl HasDecl {
    /// Parse a has declaration from string.
    /// Formats: "name:type" or "name:cardinality:type"
    pub(crate) fn parse(s: &str) -> Result<Self, CliError> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.len() {
            2 => Ok(HasDecl {
                name: parts[0].trim().to_string(),
                cardinality: None,
                type_ref: parts[1].trim().to_string(),
            }),
            3 => Ok(HasDecl {
                name: parts[0].trim().to_string(),
                cardinality: Some(parts[1].trim().to_string()),
                type_ref: parts[2].trim().to_string(),
            }),
            _ => Err(CliError::InvalidArgument(format!(
                "Invalid has declaration '{}'. Expected format: 'name:type' or 'name:cardinality:type'",
                s
            ))),
        }
    }

    /// Convert to Dolfin syntax.
    pub(crate) fn to_dolfin(&self) -> String {
        match &self.cardinality {
            Some(card) => format!("has {}: {} {}", self.name, card, self.type_ref),
            None => format!("has {}: {}", self.name, self.type_ref),
        }
    }
}

/// Parsed qualified name into path components and declaration name.
#[derive(Debug)]
pub(crate) struct ParsedName {
    /// Directory path components (e.g., ["general"] for "general.specific.MyConcept")
    pub(crate) directories: Vec<String>,
    /// File name without extension (e.g., "specific" for "general.specific.MyConcept")
    file_stem: String,
    /// Declaration name (e.g., "MyConcept" for "general.specific.MyConcept")
    pub(crate) decl_name: String,
}

impl ParsedName {
    /// Parse a qualified name like "general.specific.MyConcept"
    pub(crate) fn parse(qualified_name: &str) -> Result<Self, CliError> {
        let parts: Vec<&str> = qualified_name.split('.').collect();

        if parts.is_empty() {
            return Err(CliError::InvalidArgument(
                "Empty qualified name".to_string(),
            ));
        }

        if parts.len() == 1 {
            // Just "MyConcept" -> main.dlf in root
            Ok(ParsedName {
                directories: vec![],
                file_stem: "main".to_string(),
                decl_name: parts[0].to_string(),
            })
        } else if parts.len() == 2 {
            // "specific.MyConcept" -> specific.dlf in root
            Ok(ParsedName {
                directories: vec![],
                file_stem: parts[0].to_string(),
                decl_name: parts[1].to_string(),
            })
        } else {
            // "general.specific.MyConcept" -> general/specific.dlf
            let directories = parts[..parts.len() - 2]
                .iter()
                .map(|s| s.to_string())
                .collect();
            let file_stem = parts[parts.len() - 2].to_string();
            let decl_name = parts[parts.len() - 1].to_string();

            Ok(ParsedName {
                directories,
                file_stem,
                decl_name,
            })
        }
    }

    /// Get the full file path relative to the package root.
    pub(crate) fn file_path(&self) -> PathBuf {
        let mut path = PathBuf::new();
        for dir in &self.directories {
            path.push(dir);
        }
        path.push(format!("{}.dlf", self.file_stem));
        path
    }

    /// Get the directory path relative to the package root.
    fn dir_path(&self) -> PathBuf {
        let mut path = PathBuf::new();
        for dir in &self.directories {
            path.push(dir);
        }
        path
    }
}

/// The type of declaration being added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclKind {
    Concept,
    Property,
}

impl DeclKind {
    fn as_str(&self) -> &'static str {
        match self {
            DeclKind::Concept => "concept",
            DeclKind::Property => "property",
        }
    }
}

/// Run the add command.
pub fn run(kind: AddKind) -> Result<(), CliError> {
    match kind {
        AddKind::Concept {
            name,
            subs,
            has,
            path,
        } => add_concept(&path, &name, &subs, &has),
        AddKind::Property {
            name,
            domain,
            range,
            domain_cardinality,
            range_cardinality,
            path,
        } => add_property(
            &path,
            &name,
            &domain,
            &range,
            &domain_cardinality,
            &range_cardinality,
        ),
    }
}

/// Add a concept to the appropriate file.
fn add_concept(
    package_path: &Path,
    qualified_name: &str,
    subs: &[String],
    has_args: &[String],
) -> Result<(), CliError> {
    let parsed = ParsedName::parse(qualified_name)?;

    // Parse has declarations
    let has_decls: Vec<HasDecl> = has_args
        .iter()
        .map(|s| HasDecl::parse(s))
        .collect::<Result<Vec<_>, _>>()?;

    // Generate the concept definition
    let concept_code = generate_concept(&parsed.decl_name, subs, &has_decls);

    // Add to file
    add_to_file(package_path, &parsed, &concept_code, DeclKind::Concept)?;

    println!(
        "✓ Added concept '{}' to {}",
        parsed.decl_name,
        parsed.file_path().display()
    );

    Ok(())
}

/// Add a property to the appropriate file.
fn add_property(
    package_path: &Path,
    qualified_name: &str,
    domain: &str,
    range: &str,
    domain_cardinality: &str,
    range_cardinality: &str,
) -> Result<(), CliError> {
    let parsed = ParsedName::parse(qualified_name)?;

    // Generate the property definition
    let property_code = generate_property(
        &parsed.decl_name,
        domain,
        range,
        domain_cardinality,
        range_cardinality,
    );

    // Add to file
    add_to_file(package_path, &parsed, &property_code, DeclKind::Property)?;

    println!(
        "✓ Added property '{}' to {}",
        parsed.decl_name,
        parsed.file_path().display()
    );

    Ok(())
}

/// Generate Dolfin code for a concept.
fn generate_concept(name: &str, subs: &[String], has_decls: &[HasDecl]) -> String {
    let mut lines = vec![format!("concept {}:", name)];

    // Add sub declarations
    if !subs.is_empty() {
        lines.push(format!("  sub {}", subs.join(", ")));
    }

    // Add has declarations
    for has in has_decls {
        lines.push(format!("  {}", has.to_dolfin()));
    }

    // If no body, we still need the colon but can leave it empty
    // Actually, let's add a pass-like empty body or just leave the colon
    if subs.is_empty() && has_decls.is_empty() {
        // Concept with no body - remove the colon
        lines[0] = format!("concept {}", name);
    }

    lines.join("\n")
}

/// Generate Dolfin code for a property.
fn generate_property(
    name: &str,
    domain: &str,
    range: &str,
    domain_cardinality: &str,
    range_cardinality: &str,
) -> String {
    let domain_part = format_cardinality_type(domain_cardinality, domain);
    let range_part = format_cardinality_type(range_cardinality, range);

    format!("property {}: {} -> {}", name, domain_part, range_part)
}

/// Format a cardinality and type together.
fn format_cardinality_type(cardinality: &str, type_name: &str) -> String {
    match cardinality {
        "one" | "" => type_name.to_string(),
        card => format!("{} {}", card, type_name),
    }
}

/// Check if a declaration with the given name and kind already exists in the file content.
fn declaration_exists(content: &str, name: &str, kind: DeclKind) -> Result<bool, CliError> {
    // If file is empty, nothing exists
    if content.trim().is_empty() {
        return Ok(false);
    }

    let parsed = parser::parse_ontology(content);
    // Try to parse the file
    match parsed.ontology {
        Some(ontology_file) if parsed.is_ok() => {
            // Search through all statements
            for declaration in &ontology_file.declarations {
                let found = match declaration {
                    Concept(concept) if kind == DeclKind::Concept => {
                        concept.name.get().as_str() == name
                    }
                    Property(_property) if kind == DeclKind::Property => todo!(),
                    _ => false,
                };
                if found {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => {
            // If parsing fails, fall back to simple text-based detection
            // This handles files that might have syntax errors or use features
            // not yet fully supported by the parser
            Ok(declaration_exists_text_fallback(content, name, kind))
        }
    }
}

/// Fallback text-based check for declaration existence.
/// Used when parsing fails.
fn declaration_exists_text_fallback(content: &str, name: &str, kind: DeclKind) -> bool {
    let keyword = kind.as_str();

    // Build patterns to match
    // e.g., "concept MyConcept" or "concept MyConcept:"
    let pattern_no_colon = format!("{} {}", keyword, name);
    let pattern_with_colon = format!("{} {}:", keyword, name);

    for line in content.lines() {
        let trimmed = line.trim();
        // Check exact match or match with colon
        if trimmed == pattern_no_colon
            || trimmed == pattern_with_colon
            || trimmed.starts_with(&format!("{} ", pattern_no_colon))
            || trimmed.starts_with(&format!("{} ", pattern_with_colon))
        {
            return true;
        }
    }

    false
}

/// Add generated code to a file, creating directories and file if needed.
fn add_to_file(
    package_path: &Path,
    parsed: &ParsedName,
    code: &str,
    kind: DeclKind,
) -> Result<(), CliError> {
    // Verify we're in a package (package.dlf exists)
    let package_file = package_path.join("package.dlf");
    if !package_file.exists() {
        return Err(CliError::NotAPackage(package_path.to_path_buf()));
    }

    // Create directories if needed
    let dir_path = package_path.join(parsed.dir_path());
    if !dir_path.exists() && !parsed.directories.is_empty() {
        fs::create_dir_all(&dir_path).map_err(|e| CliError::Io(dir_path.clone(), e))?;
        println!("  Created directory: {}", parsed.dir_path().display());
    }

    // Get full file path
    let file_path = package_path.join(parsed.file_path());

    // Read existing content or start fresh
    let existing_content = if file_path.exists() {
        fs::read_to_string(&file_path).map_err(|e| CliError::Io(file_path.clone(), e))?
    } else {
        String::new()
    };

    // Check for duplicate declaration
    if declaration_exists(&existing_content, &parsed.decl_name, kind)? {
        return Err(CliError::DuplicateDeclaration {
            kind: kind.as_str().to_string(),
            name: parsed.decl_name.clone(),
            file: parsed.file_path(),
        });
    }

    // Append the new code
    let new_content = if existing_content.is_empty() {
        format!("{}\n", code)
    } else if existing_content.ends_with('\n') {
        format!("{}\n{}\n", existing_content.trim_end(), code)
    } else {
        format!("{}\n\n{}\n", existing_content, code)
    };

    // Write the file
    fs::write(&file_path, new_content).map_err(|e| CliError::Io(file_path.clone(), e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_has_decl_simple() {
        let decl = HasDecl::parse("name:string").unwrap();
        assert_eq!(decl.name, "name");
        assert_eq!(decl.cardinality, None);
        assert_eq!(decl.type_ref, "string");
        assert_eq!(decl.to_dolfin(), "has name: string");
    }

    #[test]
    fn test_parse_has_decl_with_cardinality() {
        let decl = HasDecl::parse("skills:some:Skill").unwrap();
        assert_eq!(decl.name, "skills");
        assert_eq!(decl.cardinality, Some("some".to_string()));
        assert_eq!(decl.type_ref, "Skill");
        assert_eq!(decl.to_dolfin(), "has skills: some Skill");
    }

    #[test]
    fn test_parse_name_single() {
        let parsed = ParsedName::parse("MyConcept").unwrap();
        assert!(parsed.directories.is_empty());
        assert_eq!(parsed.file_stem, "main");
        assert_eq!(parsed.decl_name, "MyConcept");
        assert_eq!(parsed.file_path(), PathBuf::from("main.dlf"));
    }

    #[test]
    fn test_parse_name_two_parts() {
        let parsed = ParsedName::parse("animals.Dog").unwrap();
        assert!(parsed.directories.is_empty());
        assert_eq!(parsed.file_stem, "animals");
        assert_eq!(parsed.decl_name, "Dog");
        assert_eq!(parsed.file_path(), PathBuf::from("animals.dlf"));
    }

    #[test]
    fn test_parse_name_three_parts() {
        let parsed = ParsedName::parse("animals.mammals.Dog").unwrap();
        assert_eq!(parsed.directories, vec!["animals"]);
        assert_eq!(parsed.file_stem, "mammals");
        assert_eq!(parsed.decl_name, "Dog");
        assert_eq!(parsed.file_path(), PathBuf::from("animals/mammals.dlf"));
    }

    #[test]
    fn test_parse_name_four_parts() {
        let parsed = ParsedName::parse("science.biology.animals.Cat").unwrap();
        assert_eq!(parsed.directories, vec!["science", "biology"]);
        assert_eq!(parsed.file_stem, "animals");
        assert_eq!(parsed.decl_name, "Cat");
        assert_eq!(
            parsed.file_path(),
            PathBuf::from("science/biology/animals.dlf")
        );
    }

    #[test]
    fn test_generate_concept_empty() {
        let code = generate_concept("Empty", &[], &[]);
        assert_eq!(code, "concept Empty");
    }

    #[test]
    fn test_generate_concept_with_sub() {
        let code = generate_concept("Dog", &["Animal".to_string(), "Pet".to_string()], &[]);
        assert_eq!(code, "concept Dog:\n  sub Animal, Pet");
    }

    #[test]
    fn test_generate_concept_with_has() {
        let has = vec![
            HasDecl {
                name: "name".to_string(),
                cardinality: None,
                type_ref: "string".to_string(),
            },
            HasDecl {
                name: "age".to_string(),
                cardinality: Some("optional".to_string()),
                type_ref: "int".to_string(),
            },
        ];
        let code = generate_concept("Person", &[], &has);
        assert_eq!(
            code,
            "concept Person:\n  has name: string\n  has age: optional int"
        );
    }

    #[test]
    fn test_declaration_exists_text_fallback() {
        let content = r#"
concept Dog:
  has name: string

concept Cat

property owns: Person -> Pet
"#;

        assert!(declaration_exists_text_fallback(
            content,
            "Dog",
            DeclKind::Concept
        ));
        assert!(declaration_exists_text_fallback(
            content,
            "Cat",
            DeclKind::Concept
        ));
        assert!(!declaration_exists_text_fallback(
            content,
            "Bird",
            DeclKind::Concept
        ));

        assert!(declaration_exists_text_fallback(
            content,
            "owns",
            DeclKind::Property
        ));
        assert!(!declaration_exists_text_fallback(
            content,
            "hasName",
            DeclKind::Property
        ));
    }
}
