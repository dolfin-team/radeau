//! Modify command — surgically edits existing declarations using AST span information.
//!
//! All edits are performed via precise byte-offset splices derived from the spans
//! recorded by the parser.  No declaration is ever fully regenerated from the AST;
//! only the targeted tokens are inserted, replaced, or deleted.

use crate::{ModifyKind, error::CliError};
use rowl::ast::{Cardinality, ConceptDef, Declaration, OntologyFile, PropertyDef, TypeRef};
use rowl::error::Span;
use rowl::parser;
use std::fs;
use std::path::{Path, PathBuf};

use super::add::{HasDecl, ParsedName};

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

pub fn run(kind: ModifyKind) -> Result<(), CliError> {
    match kind {
        ModifyKind::Concept {
            name,
            subs,
            has,
            path,
        } => modify_concept(&path, &name, &subs, &has),
        ModifyKind::Property {
            name,
            domain,
            range,
            domain_cardinality,
            range_cardinality,
            path,
        } => modify_property(
            &path,
            &name,
            domain.as_deref(),
            range.as_deref(),
            domain_cardinality.as_deref(),
            range_cardinality.as_deref(),
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Operation types
// ─────────────────────────────────────────────────────────────────────────────

/// Operation on a named collection (sub parents, enum variants).
enum NameOp {
    Add(String),
    Remove(String),
    RemoveAll,
}

/// Operation on has declarations.
enum HasOp {
    Add(HasDecl),
    Remove(String),
}

/// Parse `--sub` / `--variant` arguments into operations.
///
/// Each arg may be comma-separated.  Prefix `~` means remove; `~*` means remove-all.
fn parse_name_ops(args: &[String]) -> Vec<NameOp> {
    args.iter()
        .flat_map(|arg| arg.split(','))
        .map(|s| {
            let s = s.trim();
            if s == "~*" {
                NameOp::RemoveAll
            } else if let Some(name) = s.strip_prefix('~') {
                NameOp::Remove(name.to_string())
            } else {
                NameOp::Add(s.to_string())
            }
        })
        .collect()
}

/// Parse `--has` arguments into operations.
///
/// Each arg may be comma-separated.  Prefix `~` followed by just the property
/// name means remove.  Otherwise the format is `name:type` or `name:card:type`.
fn parse_has_ops(args: &[String]) -> Result<Vec<HasOp>, CliError> {
    args.iter()
        .flat_map(|arg| arg.split(','))
        .map(|s| {
            let s = s.trim();
            if let Some(name) = s.strip_prefix('~') {
                Ok(HasOp::Remove(name.to_string()))
            } else {
                HasDecl::parse(s).map(HasOp::Add)
            }
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Span-based editing primitives
// ─────────────────────────────────────────────────────────────────────────────

/// Insert `text` at byte `offset` in `source`.
fn splice_at(source: &str, offset: usize, text: &str) -> String {
    format!("{}{}{}", &source[..offset], text, &source[offset..])
}

/// Replace `source[start..end]` with `text`.
fn replace_range(source: &str, start: usize, end: usize, text: &str) -> String {
    format!("{}{}{}", &source[..start], text, &source[end..])
}

/// Byte offset of the `\n` that ends the line containing `offset`,
/// or `source.len()` when the line has no terminating newline.
fn end_of_line(source: &str, offset: usize) -> usize {
    source[offset..]
        .find('\n')
        .map_or(source.len(), |p| offset + p)
}

/// Byte offset of the first character of the line containing `offset`
/// (right after the preceding `\n`, or 0 for the very first line).
fn start_of_line(source: &str, offset: usize) -> usize {
    source[..offset].rfind('\n').map_or(0, |p| p + 1)
}

fn typeref_span(tr: &TypeRef) -> Option<Span> {
    match tr {
        TypeRef::Named { span, .. } => *span,
        TypeRef::Primitive { span, .. } => *span,
        TypeRef::Union { span, .. } => *span,
    }
}

fn cardinality_span(card: &Cardinality) -> Option<Span> {
    match card {
        Cardinality::One { span } => *span,
        Cardinality::Any { span } => *span,
        Cardinality::Some { span } => *span,
        Cardinality::Optional { span } => *span,
        Cardinality::Exact { span, .. } => *span,
        Cardinality::Range { span, .. } => *span,
    }
}

fn require_span(what: &str, span: Option<Span>) -> Result<Span, CliError> {
    span.ok_or_else(|| {
        CliError::Other(format!(
            "Span information missing for {what} — was the file parsed with span tracking enabled?"
        ))
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// AST helpers
// ─────────────────────────────────────────────────────────────────────────────

fn require_parse(source: &str, file_path: &Path) -> Result<OntologyFile, CliError> {
    let result = parser::parse_ontology(source);
    match result.ontology {
        Some(ontology) if result.is_ok() => Ok(ontology),
        _ => {
            let msg = if result.diagnostics.is_empty() {
                "unknown parse error".to_string()
            } else {
                result
                    .diagnostics
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            Err(CliError::Other(format!(
                "Failed to parse {}: {}",
                file_path.display(),
                msg
            )))
        }
    }
}

fn find_concept<'a>(ontology: &'a OntologyFile, name: &str) -> Option<&'a ConceptDef> {
    ontology.declarations.iter().find_map(|d| match d {
        Declaration::Concept(c) if c.name.get().as_str() == name => Some(c),
        _ => None,
    })
}

fn find_property<'a>(ontology: &'a OntologyFile, name: &str) -> Option<&'a PropertyDef> {
    ontology.declarations.iter().find_map(|d| match d {
        Declaration::Property(p) if p.name.get() == name => Some(p),
        _ => None,
    })
}

/// Highest `span.end.offset` across all members (has_declarations + parents).
fn concept_last_member_end(concept: &ConceptDef) -> Option<usize> {
    let last_has = concept
        .has_declarations
        .last()
        .and_then(|h| h.span)
        .map(|s| s.end.offset);
    let last_parent = concept
        .parents
        .last()
        .and_then(typeref_span)
        .map(|s| s.end.offset);
    match (last_has, last_parent) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn not_found(kind: &str, name: &str, file: PathBuf) -> CliError {
    CliError::DeclarationNotFound {
        kind: kind.to_string(),
        name: name.to_string(),
        file,
    }
}

fn duplicate(kind: &str, name: &str, file: PathBuf) -> CliError {
    CliError::DuplicateDeclaration {
        kind: kind.to_string(),
        name: name.to_string(),
        file,
    }
}

fn load_content(package_path: &Path, parsed: &ParsedName) -> Result<(PathBuf, String), CliError> {
    let package_file = package_path.join("package.dlf");
    if !package_file.exists() {
        return Err(CliError::NotAPackage(package_path.to_path_buf()));
    }
    let file_path = package_path.join(parsed.file_path());
    let content = fs::read_to_string(&file_path).map_err(|e| CliError::Io(file_path.clone(), e))?;
    Ok((file_path, content))
}

// ─────────────────────────────────────────────────────────────────────────────
// Concept modification
// ─────────────────────────────────────────────────────────────────────────────

fn modify_concept(
    package_path: &Path,
    qualified_name: &str,
    sub_args: &[String],
    has_args: &[String],
) -> Result<(), CliError> {
    let parsed = ParsedName::parse(qualified_name)?;
    let (file_path, content) = load_content(package_path, &parsed)?;

    // Validate existence upfront before applying any ops.
    {
        let ast = require_parse(&content, &file_path)?;
        find_concept(&ast, &parsed.decl_name)
            .ok_or_else(|| not_found("concept", &parsed.decl_name, parsed.file_path()))?;
    }

    let has_ops = parse_has_ops(has_args)?;
    let sub_ops = parse_name_ops(sub_args);
    let mut src = content;

    for op in &has_ops {
        let ast = require_parse(&src, &file_path)?;
        let concept = find_concept(&ast, &parsed.decl_name)
            .ok_or_else(|| not_found("concept", &parsed.decl_name, parsed.file_path()))?;
        src = apply_has_op(&src, concept, op, parsed.file_path())?;
    }

    for op in &sub_ops {
        let ast = require_parse(&src, &file_path)?;
        let concept = find_concept(&ast, &parsed.decl_name)
            .ok_or_else(|| not_found("concept", &parsed.decl_name, parsed.file_path()))?;
        src = apply_sub_op(&src, concept, op, parsed.file_path())?;
    }

    fs::write(&file_path, src).map_err(|e| CliError::Io(file_path.clone(), e))?;

    println!(
        "✓ Modified concept '{}' in {}",
        parsed.decl_name,
        parsed.file_path().display()
    );
    Ok(())
}

fn apply_has_op(
    source: &str,
    concept: &ConceptDef,
    op: &HasOp,
    file: PathBuf,
) -> Result<String, CliError> {
    match op {
        HasOp::Add(decl) => {
            if concept.has_declarations.iter().any(|h| h.name == decl.name) {
                return Err(duplicate("has", &decl.name, file));
            }
            let new_line = format!("  {}\n", decl.to_dolfin());
            match concept_last_member_end(concept) {
                Some(last_end) => {
                    // Insert after the last member's line (past the \n).
                    let insert_pt = end_of_line(source, last_end) + 1;
                    Ok(splice_at(source, insert_pt, &new_line))
                }
                None => {
                    // Concept has no body yet: append ":\n  has ...\n" after the name.
                    let span = require_span("concept", concept.span)?;
                    Ok(splice_at(
                        source,
                        span.end.offset,
                        &format!(":\n{new_line}"),
                    ))
                }
            }
        }
        HasOp::Remove(name) => {
            let decl = concept
                .has_declarations
                .iter()
                .find(|h| &h.name == name)
                .ok_or_else(|| not_found("has", name, file))?;
            let span = require_span("has declaration", decl.span)?;
            let line_start = start_of_line(source, span.start.offset);
            let line_end = end_of_line(source, span.end.offset);
            // Include the trailing newline in the deletion.
            let delete_end = if line_end < source.len() {
                line_end + 1
            } else {
                line_end
            };
            Ok(replace_range(source, line_start, delete_end, ""))
        }
    }
}

fn apply_sub_op(
    source: &str,
    concept: &ConceptDef,
    op: &NameOp,
    file: PathBuf,
) -> Result<String, CliError> {
    match op {
        NameOp::Add(name) => {
            if concept.parents.iter().any(|p| format!("{p}") == *name) {
                return Err(duplicate("sub", name, file));
            }
            if !concept.parents.is_empty() {
                // Append ", Name" at end of existing sub line.
                let last = concept.parents.last().unwrap();
                let span = typeref_span(last)
                    .ok_or_else(|| CliError::Other("Missing span for parent type".into()))?;
                let line_end = end_of_line(source, span.end.offset);
                Ok(splice_at(source, line_end, &format!(", {name}")))
            } else if !concept.has_declarations.is_empty() {
                // Prepend a sub line before the first has declaration.
                let first_has = concept.has_declarations.first().unwrap();
                let span = require_span("has declaration", first_has.span)?;
                let line_start = start_of_line(source, span.start.offset);
                Ok(splice_at(source, line_start, &format!("  sub {name}\n")))
            } else {
                // Concept has no body yet.
                let span = require_span("concept", concept.span)?;
                Ok(splice_at(
                    source,
                    span.end.offset,
                    &format!(":\n  sub {name}"),
                ))
            }
        }

        NameOp::Remove(name) => {
            let idx = concept
                .parents
                .iter()
                .position(|p| format!("{p}") == *name)
                .ok_or_else(|| not_found("sub", name, file))?;

            if concept.parents.len() == 1 {
                // Remove the entire "sub ..." line.
                let span = typeref_span(&concept.parents[0])
                    .ok_or_else(|| CliError::Other("Missing span for parent type".into()))?;
                let line_start = start_of_line(source, span.start.offset);
                let line_end = end_of_line(source, span.end.offset);
                let delete_end = if line_end < source.len() {
                    line_end + 1
                } else {
                    line_end
                };
                Ok(replace_range(source, line_start, delete_end, ""))
            } else if idx == 0 {
                // Remove first parent: delete from its start up to the next parent's start.
                // This removes "FirstParent, " including the trailing separator.
                let span = typeref_span(&concept.parents[0])
                    .ok_or_else(|| CliError::Other("Missing span for parent type".into()))?;
                let next_span = typeref_span(&concept.parents[1])
                    .ok_or_else(|| CliError::Other("Missing span for next parent".into()))?;
                Ok(replace_range(
                    source,
                    span.start.offset,
                    next_span.start.offset,
                    "",
                ))
            } else {
                // Remove a non-first parent: delete from the previous parent's end up to
                // this parent's end.  This removes ", ParentToRemove".
                let prev_span = typeref_span(&concept.parents[idx - 1])
                    .ok_or_else(|| CliError::Other("Missing span for previous parent".into()))?;
                let span = typeref_span(&concept.parents[idx])
                    .ok_or_else(|| CliError::Other("Missing span for parent type".into()))?;
                Ok(replace_range(
                    source,
                    prev_span.end.offset,
                    span.end.offset,
                    "",
                ))
            }
        }

        NameOp::RemoveAll => {
            if concept.parents.is_empty() {
                return Ok(source.to_string());
            }
            let first_span = typeref_span(&concept.parents[0])
                .ok_or_else(|| CliError::Other("Missing span for parent type".into()))?;
            let last_span = typeref_span(concept.parents.last().unwrap())
                .ok_or_else(|| CliError::Other("Missing span for parent type".into()))?;
            let line_start = start_of_line(source, first_span.start.offset);
            let line_end = end_of_line(source, last_span.end.offset);
            let delete_end = if line_end < source.len() {
                line_end + 1
            } else {
                line_end
            };
            Ok(replace_range(source, line_start, delete_end, ""))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Property modification
// ─────────────────────────────────────────────────────────────────────────────

fn modify_property(
    package_path: &Path,
    qualified_name: &str,
    domain: Option<&str>,
    range: Option<&str>,
    domain_cardinality: Option<&str>,
    range_cardinality: Option<&str>,
) -> Result<(), CliError> {
    let parsed = ParsedName::parse(qualified_name)?;
    let (file_path, content) = load_content(package_path, &parsed)?;

    // Verify existence upfront.
    {
        let ast = require_parse(&content, &file_path)?;
        find_property(&ast, &parsed.decl_name)
            .ok_or_else(|| not_found("property", &parsed.decl_name, parsed.file_path()))?;
    }

    let mut src = content;

    // Apply changes in a sequence that keeps earlier spans intact.
    // Order: domain type → domain cardinality → range type → range cardinality.
    // Because each change re-parses, subsequent offsets are always fresh.

    if let Some(new_domain) = domain {
        let ast = require_parse(&src, &file_path)?;
        let prop = find_property(&ast, &parsed.decl_name)
            .ok_or_else(|| not_found("property", &parsed.decl_name, parsed.file_path()))?;
        let span = typeref_span(&prop.domain)
            .ok_or_else(|| CliError::Other("Missing span for domain type".into()))?;
        src = replace_range(&src, span.start.offset, span.end.offset, new_domain);
    }

    if let Some(new_card) = domain_cardinality {
        let ast = require_parse(&src, &file_path)?;
        let prop = find_property(&ast, &parsed.decl_name)
            .ok_or_else(|| not_found("property", &parsed.decl_name, parsed.file_path()))?;
        let domain_span = typeref_span(&prop.domain)
            .ok_or_else(|| CliError::Other("Missing span for domain type".into()))?;
        src = apply_cardinality_change(
            &src,
            prop.domain_cardinality.as_ref(),
            domain_span,
            new_card,
        )?;
    }

    if let Some(new_range) = range {
        let ast = require_parse(&src, &file_path)?;
        let prop = find_property(&ast, &parsed.decl_name)
            .ok_or_else(|| not_found("property", &parsed.decl_name, parsed.file_path()))?;
        let span = typeref_span(&prop.range)
            .ok_or_else(|| CliError::Other("Missing span for range type".into()))?;
        src = replace_range(&src, span.start.offset, span.end.offset, new_range);
    }

    if let Some(new_card) = range_cardinality {
        let ast = require_parse(&src, &file_path)?;
        let prop = find_property(&ast, &parsed.decl_name)
            .ok_or_else(|| not_found("property", &parsed.decl_name, parsed.file_path()))?;
        let range_span = typeref_span(&prop.range)
            .ok_or_else(|| CliError::Other("Missing span for range type".into()))?;
        src =
            apply_cardinality_change(&src, prop.range_cardinality.as_ref(), range_span, new_card)?;
    }

    fs::write(&file_path, src).map_err(|e| CliError::Io(file_path.clone(), e))?;

    println!(
        "✓ Modified property '{}' in {}",
        parsed.decl_name,
        parsed.file_path().display()
    );
    Ok(())
}

/// Surgically insert, replace, or remove a cardinality keyword relative to `type_span`.
///
/// `"one"` (or empty string) is treated as "no cardinality keyword" (the Dolfin default).
fn apply_cardinality_change(
    source: &str,
    existing: Option<&Cardinality>,
    type_span: Span,
    new_card: &str,
) -> Result<String, CliError> {
    let is_default = matches!(new_card.trim(), "one" | "");

    match (existing, is_default) {
        (Some(card), true) => {
            // User wants the default (no keyword) — remove the existing keyword.
            let card_span = cardinality_span(card)
                .ok_or_else(|| CliError::Other("Missing span for cardinality".into()))?;
            // Delete from cardinality start to type start: removes e.g. "some ".
            Ok(replace_range(
                source,
                card_span.start.offset,
                type_span.start.offset,
                "",
            ))
        }
        (None, true) => {
            // Already the default — nothing to do.
            Ok(source.to_string())
        }
        (Some(card), false) => {
            // Replace the existing cardinality keyword in-place.
            let card_span = cardinality_span(card)
                .ok_or_else(|| CliError::Other("Missing span for cardinality".into()))?;
            Ok(replace_range(
                source,
                card_span.start.offset,
                card_span.end.offset,
                new_card,
            ))
        }
        (None, false) => {
            // Insert the new keyword before the type.
            Ok(splice_at(
                source,
                type_span.start.offset,
                &format!("{new_card} "),
            ))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_name_ops_add() {
        let args = vec!["Animal".to_string(), "Pet,Mammal".to_string()];
        let ops = parse_name_ops(&args);
        assert_eq!(ops.len(), 3);
        assert!(matches!(&ops[0], NameOp::Add(n) if n == "Animal"));
        assert!(matches!(&ops[1], NameOp::Add(n) if n == "Pet"));
        assert!(matches!(&ops[2], NameOp::Add(n) if n == "Mammal"));
    }

    #[test]
    fn test_parse_name_ops_remove() {
        let args = vec!["~Animal".to_string(), "~*".to_string()];
        let ops = parse_name_ops(&args);
        assert!(matches!(&ops[0], NameOp::Remove(n) if n == "Animal"));
        assert!(matches!(&ops[1], NameOp::RemoveAll));
    }

    #[test]
    fn test_parse_has_ops() {
        let args = vec![
            "age:int".to_string(),
            "~name".to_string(),
            "skills:some:Skill".to_string(),
        ];
        let ops = parse_has_ops(&args).unwrap();
        assert_eq!(ops.len(), 3);
        assert!(matches!(&ops[0], HasOp::Add(d) if d.name == "age"));
        assert!(matches!(&ops[1], HasOp::Remove(n) if n == "name"));
        assert!(matches!(&ops[2], HasOp::Add(d) if d.name == "skills"));
    }

    #[test]
    fn test_splice_at() {
        assert_eq!(
            splice_at("hello world", 5, " beautiful"),
            "hello beautiful world"
        );
    }

    #[test]
    fn test_replace_range() {
        assert_eq!(replace_range("hello world", 6, 11, "Rust"), "hello Rust");
    }

    #[test]
    fn test_end_of_line() {
        let s = "line1\nline2\nline3";
        assert_eq!(end_of_line(s, 0), 5); // points at the \n
        assert_eq!(end_of_line(s, 6), 11);
        assert_eq!(end_of_line(s, 12), 17); // no trailing \n → len
    }

    #[test]
    fn test_start_of_line() {
        let s = "line1\nline2\nline3";
        assert_eq!(start_of_line(s, 0), 0);
        assert_eq!(start_of_line(s, 8), 6); // inside "line2"
        assert_eq!(start_of_line(s, 14), 12); // inside "line3"
    }
}
