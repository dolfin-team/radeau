//! Remove command implementation - removes declarations from Dolfin files.

use crate::CliError;
use std::fs;
use std::path::{Path, PathBuf};

/// The type of declaration being removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Concept,
    Property,
}

impl DeclKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeclKind::Concept => "concept",
            DeclKind::Property => "property",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "concept" => Some(DeclKind::Concept),
            "property" => Some(DeclKind::Property),
            _ => None,
        }
    }
}

/// A location where a symbol is used.
#[derive(Debug, Clone)]
pub struct UsageLocation {
    /// File path relative to package root
    pub file: PathBuf,
    /// Line number (1-indexed)
    pub line: usize,
    /// The line content (trimmed)
    pub context: String,
    /// What kind of usage this is
    pub usage_kind: UsageKind,
}

impl std::fmt::Display for UsageLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "  {}:{} - {} in: {}",
            self.file.display(),
            self.line,
            self.usage_kind,
            self.context
        )
    }
}

/// The kind of usage of a symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageKind {
    /// Used in a `sub` declaration
    SubDeclaration,
    /// Used as a type in a `has` declaration
    HasType,
    /// Used as domain in a property
    PropertyDomain,
    /// Used as range in a property
    PropertyRange,
    /// Used in a rule pattern
    RulePattern,
    /// Used in a rule assertion
    RuleAssertion,
}

impl std::fmt::Display for UsageKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsageKind::SubDeclaration => write!(f, "parent type"),
            UsageKind::HasType => write!(f, "has type"),
            UsageKind::PropertyDomain => write!(f, "property domain"),
            UsageKind::PropertyRange => write!(f, "property range"),
            UsageKind::RulePattern => write!(f, "rule pattern"),
            UsageKind::RuleAssertion => write!(f, "rule assertion"),
        }
    }
}

/// Result of checking for usages of a symbol.
#[derive(Debug)]
pub struct UsageCheckResult {
    /// Where the symbol is defined
    pub definition_file: PathBuf,
    /// Line number of definition
    pub definition_line: usize,
    /// All places where the symbol is used
    pub usages: Vec<UsageLocation>,
}

impl UsageCheckResult {
    pub fn has_usages(&self) -> bool {
        !self.usages.is_empty()
    }
}

/// Options for the remove command.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct RemoveOptions {
    pub force: bool,
    pub dry_run: bool,
}


/// Run the remove command.
pub fn run(
    package_path: &Path,
    qualified_name: &str,
    kind: DeclKind,
    options: RemoveOptions,
) -> Result<(), CliError> {
    // Verify we're in a package
    let package_file = package_path.join("package.dlf");
    if !package_file.exists() {
        return Err(CliError::NotAPackage(package_path.to_path_buf()));
    }

    // Parse the qualified name to find the file
    let parsed = super::add::ParsedName::parse(qualified_name)?;
    let file_path = package_path.join(parsed.file_path());

    // Check if the file exists
    if !file_path.exists() {
        return Err(CliError::DeclarationNotFound {
            kind: kind.as_str().to_string(),
            name: parsed.decl_name.clone(),
            file: parsed.file_path(),
        });
    }

    // Find the declaration and check usages
    let usage_result = find_usages(package_path, &parsed.decl_name, kind)?;

    // Check if declaration exists
    if usage_result.is_none() {
        return Err(CliError::DeclarationNotFound {
            kind: kind.as_str().to_string(),
            name: parsed.decl_name.clone(),
            file: parsed.file_path(),
        });
    }

    let usage_result = usage_result.unwrap();

    // If there are usages and not forcing, show them and abort
    if usage_result.has_usages() && !options.force {
        eprintln!(
            "Cannot remove {} '{}': it is used in {} location(s):\n",
            kind.as_str(),
            parsed.decl_name,
            usage_result.usages.len()
        );

        for usage in &usage_result.usages {
            eprintln!("{}", usage);
        }

        eprintln!();
        eprintln!("Use --force to remove anyway (may break your package)");

        return Err(CliError::SymbolInUse {
            kind: kind.as_str().to_string(),
            name: parsed.decl_name.clone(),
            usage_count: usage_result.usages.len(),
        });
    }

    // Warn if forcing removal with usages
    if usage_result.has_usages() && options.force {
        eprintln!(
            "⚠ Warning: removing {} '{}' which is used in {} location(s)",
            kind.as_str(),
            parsed.decl_name,
            usage_result.usages.len()
        );
    }

    // Dry run - just show what would happen
    if options.dry_run {
        println!(
            "Would remove {} '{}' from {}",
            kind.as_str(),
            parsed.decl_name,
            parsed.file_path().display()
        );
        return Ok(());
    }

    // Actually remove the declaration
    remove_declaration_from_file(&file_path, &parsed.decl_name, kind)?;

    println!(
        "✓ Removed {} '{}' from {}",
        kind.as_str(),
        parsed.decl_name,
        parsed.file_path().display()
    );

    Ok(())
}

/// Find all usages of a symbol across the package.
fn find_usages(
    package_path: &Path,
    name: &str,
    kind: DeclKind,
) -> Result<Option<UsageCheckResult>, CliError> {
    let mut definition_file: Option<PathBuf> = None;
    let mut definition_line: usize = 0;
    let mut usages = Vec::new();

    // Collect all .dlf files in the package
    let dlf_files = collect_dlf_files(package_path)?;

    for file_path in dlf_files {
        let relative_path = file_path
            .strip_prefix(package_path)
            .unwrap_or(&file_path)
            .to_path_buf();

        let content =
            fs::read_to_string(&file_path).map_err(|e| CliError::Io(file_path.clone(), e))?;

        // Check each line
        for (line_num, line) in content.lines().enumerate() {
            let line_num = line_num + 1; // 1-indexed
            let trimmed = line.trim();

            // Check if this is the definition
            if is_definition(trimmed, name, kind) {
                definition_file = Some(relative_path.clone());
                definition_line = line_num;
                continue;
            }

            // Check for usages
            if let Some(usage_kind) = find_usage_in_line(trimmed, name, kind) {
                usages.push(UsageLocation {
                    file: relative_path.clone(),
                    line: line_num,
                    context: trimmed.to_string(),
                    usage_kind,
                });
            }
        }
    }

    // If we didn't find the definition, return None
    match definition_file {
        Some(file) => Ok(Some(UsageCheckResult {
            definition_file: file,
            definition_line,
            usages,
        })),
        None => Ok(None),
    }
}

/// Check if a line is the definition of the symbol.
fn is_definition(line: &str, name: &str, kind: DeclKind) -> bool {
    let line = line.trim();
    let keyword = kind.as_str();
    let pattern = format!("{} {}", keyword, name);

    line == pattern
        || line.starts_with(&format!("{}:", pattern))
        || line.starts_with(&format!("{} ", pattern))
}

/// Find if a line contains a usage of the symbol.
fn find_usage_in_line(line: &str, name: &str, decl_kind: DeclKind) -> Option<UsageKind> {
    // Skip empty lines and comments
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Don't match the definition itself
    if is_definition(line, name, decl_kind) {
        return None;
    }

    // Check for sub declarations: "sub Parent" or "sub Parent, Other"
    if let Some(types_part) = line.strip_prefix("sub ")
        && contains_identifier(types_part, name) {
            return Some(UsageKind::SubDeclaration);
        }

    // Check for has declarations: "has prop: Type" or "has prop: optional Type"
    if line.starts_with("has ")
        && let Some(colon_pos) = line.find(':') {
            let type_part = &line[colon_pos + 1..];
            if contains_identifier(type_part, name) {
                return Some(UsageKind::HasType);
            }
        }

    // Check for property declarations: "property name: Domain -> Range"
    if line.starts_with("property ")
        && let Some(colon_pos) = line.find(':') {
            let rest = &line[colon_pos + 1..];
            if let Some(arrow_pos) = rest.find("->") {
                let domain_part = &rest[..arrow_pos];
                let range_part = &rest[arrow_pos + 2..];

                if contains_identifier(domain_part, name) {
                    return Some(UsageKind::PropertyDomain);
                }
                if contains_identifier(range_part, name) {
                    return Some(UsageKind::PropertyRange);
                }
            }
        }

    // Check for rule patterns: "?x is Type" or "?x has prop ?y"
    if line.contains(" is ") && contains_identifier(line, name) {
        return Some(UsageKind::RulePattern);
    }

    // Check for rule assertions in then blocks
    if line.contains(" a ") && contains_identifier(line, name) {
        return Some(UsageKind::RuleAssertion);
    }

    None
}

/// Check if a string contains an identifier (not as part of another word).
fn contains_identifier(text: &str, name: &str) -> bool {
    // Simple tokenization: split on common delimiters
    let delimiters = [' ', ',', ':', '|', '(', ')', '[', ']', '<', '>', '\t'];

    for token in text.split(|c| delimiters.contains(&c)) {
        let token = token.trim();
        if token == name {
            return true;
        }
    }

    false
}

/// Collect all .dlf files in a directory recursively.
fn collect_dlf_files(dir: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut files = Vec::new();
    collect_dlf_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn collect_dlf_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), CliError> {
    if !dir.is_dir() {
        return Ok(());
    }

    let entries = fs::read_dir(dir).map_err(|e| CliError::Io(dir.to_path_buf(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| CliError::Io(dir.to_path_buf(), e))?;

        let path = entry.path();

        if path.is_dir() {
            collect_dlf_files_recursive(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "dlf") {
            // Skip package.dlf
            if path.file_name().is_some_and(|n| n != "package.dlf") {
                files.push(path);
            }
        }
    }

    Ok(())
}

/// Remove a declaration from a file.
fn remove_declaration_from_file(
    file_path: &Path,
    name: &str,
    kind: DeclKind,
) -> Result<(), CliError> {
    let content =
        fs::read_to_string(file_path).map_err(|e| CliError::Io(file_path.to_path_buf(), e))?;

    let new_content = remove_declaration_from_content(&content, name, kind);

    // If the file is now empty (or just whitespace), delete it
    if new_content.trim().is_empty() {
        fs::remove_file(file_path).map_err(|e| CliError::Io(file_path.to_path_buf(), e))?;
        println!("  Deleted empty file: {}", file_path.display());
    } else {
        fs::write(file_path, new_content).map_err(|e| CliError::Io(file_path.to_path_buf(), e))?;
    }

    Ok(())
}

/// Remove a declaration from file content.
/// Handles multi-line declarations with indented blocks.
fn remove_declaration_from_content(content: &str, name: &str, kind: DeclKind) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut skip_until_dedent = false;
    let mut declaration_indent: Option<usize> = None;

    for line in lines {
        let trimmed = line.trim();
        let current_indent = line.len() - line.trim_start().len();

        // If we're skipping an indented block
        if skip_until_dedent
            && let Some(decl_indent) = declaration_indent {
                // Continue skipping if line is more indented or empty
                if line.trim().is_empty() || current_indent > decl_indent {
                    continue;
                }
                // We've dedented, stop skipping
                skip_until_dedent = false;
                declaration_indent = None;
            }

        // Check if this is the declaration to remove
        if is_definition(trimmed, name, kind) {
            // Check if it has an indented block (ends with :)
            if trimmed.ends_with(':') {
                skip_until_dedent = true;
                declaration_indent = Some(current_indent);
            }
            // Skip this line (the declaration itself)
            continue;
        }

        result.push(line);
    }

    // Clean up multiple consecutive empty lines
    let mut cleaned = Vec::new();
    let mut prev_empty = false;

    for line in result {
        let is_empty = line.trim().is_empty();
        if is_empty && prev_empty {
            continue;
        }
        cleaned.push(line);
        prev_empty = is_empty;
    }

    // Remove leading/trailing empty lines
    while cleaned.first().is_some_and(|l| l.trim().is_empty()) {
        cleaned.remove(0);
    }
    while cleaned.last().is_some_and(|l| l.trim().is_empty()) {
        cleaned.pop();
    }

    if cleaned.is_empty() {
        String::new()
    } else {
        cleaned.join("\n") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_identifier() {
        assert!(contains_identifier("has name: string", "string"));
        assert!(contains_identifier("has name: string", "name"));
        assert!(contains_identifier("sub Animal, Mammal", "Animal"));
        assert!(contains_identifier("sub Animal, Mammal", "Mammal"));
        assert!(!contains_identifier("has name: string", "str"));
        assert!(!contains_identifier("has name: LongString", "String"));
    }

    #[test]
    fn test_is_definition() {
        assert!(is_definition("concept Dog", "Dog", DeclKind::Concept));
        assert!(is_definition("concept Dog:", "Dog", DeclKind::Concept));
        assert!(is_definition(
            "property owns:\n  Person -> Pet",
            "owns",
            DeclKind::Property
        ));
        assert!(!is_definition("concept Dog", "Cat", DeclKind::Concept));
        assert!(!is_definition("sub Dog", "Dog", DeclKind::Concept));
    }

    #[test]
    fn test_find_usage_in_line() {
        // Sub usage
        assert_eq!(
            find_usage_in_line("sub Animal", "Animal", DeclKind::Concept),
            Some(UsageKind::SubDeclaration)
        );
        assert_eq!(
            find_usage_in_line("sub Animal, Pet", "Pet", DeclKind::Concept),
            Some(UsageKind::SubDeclaration)
        );

        // Has type usage
        assert_eq!(
            find_usage_in_line("has owner: Person", "Person", DeclKind::Concept),
            Some(UsageKind::HasType)
        );
        // Property domain/range
        assert_eq!(
            find_usage_in_line("property owns: Person -> Pet", "Person", DeclKind::Concept),
            Some(UsageKind::PropertyDomain)
        );
        assert_eq!(
            find_usage_in_line("property owns: Person -> Pet", "Pet", DeclKind::Concept),
            Some(UsageKind::PropertyRange)
        );

        // No usage
        assert_eq!(
            find_usage_in_line("has name: string", "Person", DeclKind::Concept),
            None
        );
    }

    #[test]
    fn test_remove_simple_declaration() {
        let content = r#"concept Dog

concept Cat:
  has name: string
"#;
        let result = remove_declaration_from_content(content, "Dog", DeclKind::Concept);
        assert_eq!(result, "concept Cat:\n  has name: string\n");
    }

    #[test]
    fn test_remove_declaration_with_block() {
        let content = r#"concept Dog:
  sub Animal
  has name: string

concept Cat:
  has name: string
"#;
        let result = remove_declaration_from_content(content, "Dog", DeclKind::Concept);
        assert_eq!(result, "concept Cat:\n  has name: string\n");
    }

    #[test]
    fn test_remove_middle_declaration() {
        let content = r#"concept Animal

concept Dog:
  sub Animal

concept Cat:
  sub Animal
"#;
        let result = remove_declaration_from_content(content, "Dog", DeclKind::Concept);
        assert_eq!(result, "concept Animal\n\nconcept Cat:\n  sub Animal\n");
    }

    #[test]
    fn test_remove_only_declaration() {
        let content = "concept Dog\n";
        let result = remove_declaration_from_content(content, "Dog", DeclKind::Concept);
        assert_eq!(result, "");
    }
}
