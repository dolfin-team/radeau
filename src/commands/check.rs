use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::CliError;
use dolfin_diagnostic::{Severity, format_diagnostic};
use dolfin_lint::PackageKnowledge;
use dolfin_lint::config::{CliOverrides, LintConfig};
use dolfin_lint::engine::{LintDiagnostic, LintEngine, RuleInfo};
use rowl::ast::Declaration;
use rowl::package;

pub fn run(
    path: PathBuf,
    verbose: bool,
    fix: bool,
    disable_rules: Vec<String>,
    disable_categories: Vec<String>,
    list_rules: bool,
) -> Result<(), CliError> {
    if list_rules {
        list_all_rules();
        return Ok(());
    }

    println!("Checking package at {}...", path.display());
    println!();

    let package = package::load_package(&path)?;

    println!("Package: {}", package.namespace().full());
    println!("Version: {}", package.version());
    println!("Dolfin Version: {}", package.dolfin_version());
    println!();

    let mut concept_count = 0;
    let mut property_count = 0;
    let mut rule_count = 0;

    for (namespace, ontology) in package.iter_ontologies() {
        if verbose {
            println!(
                "  {} ({})",
                namespace.full(),
                ontology.relative_path.display()
            );
            if let Some(iri_name) = &ontology.iri_name {
                println!("    IRI name: {}", iri_name.as_str());
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
                        println!("    property {}", p.name.get());
                    }
                }
                Declaration::Rule(r) => {
                    rule_count += 1;
                    if verbose {
                        println!("    rule {}", r.name);
                    }
                }
                Declaration::Fact(_) => {}
                Declaration::Query(_) => {}
                Declaration::Unit(_) => {}
            }
        }
    }

    let warnings = package::check_package(&path)?;

    println!("Files: {}", package.ontologies.len());
    println!("Concepts: {}", concept_count);
    println!("Properties: {}", property_count);
    println!("Rules: {}", rule_count);
    println!();

    if !warnings.is_empty() {
        println!("Package warnings:");
        for warning in &warnings {
            println!("  ⚠ {}", warning);
        }
        println!();
    }

    println!("Running linter...");
    println!();

    let cli_overrides = CliOverrides {
        disable_rules,
        disable_categories,
    };

    let mut knowledge = PackageKnowledge::from_package(&package);

    // Pre-load all ASTs for cross-file rename fixes.
    for (_, ontology) in package.iter_ontologies() {
        let file_path = path.join(&ontology.relative_path);
        knowledge.all_asts.insert(
            file_path.to_string_lossy().into_owned(),
            ontology.ast.clone(),
        );
    }

    let mut has_errors = false;

    // Phase 1: collect all diagnostics and print them.
    // Stored as (file_path, namespace, source, diagnostics).
    let mut all_collected: Vec<(PathBuf, String, String, Vec<LintDiagnostic>)> = Vec::new();
    let mut any_engine: Option<LintEngine> = None;

    for (namespace, ontology) in package.iter_ontologies() {
        let file_path = path.join(&ontology.relative_path);

        let merged_config = LintConfig::load_cascading(&file_path, &path);

        let engine = LintEngine::with_merged_config(&merged_config, &cli_overrides)
            .map_err(|invalid_ids| CliError::InvalidRuleIds(invalid_ids.join(", ")))?;

        let source =
            fs::read_to_string(&file_path).map_err(|e| CliError::Io(file_path.clone(), e))?;

        let file_str = file_path.to_string_lossy();
        let diagnostics =
            engine.check_with_knowledge(&ontology.ast, &source, &file_str, &knowledge);

        if diagnostics.is_empty() {
            if verbose {
                println!("✓ {} - No issues", namespace.full());
            }
        } else {
            let (errors, others): (Vec<_>, Vec<_>) = diagnostics
                .iter()
                .partition(|d| d.severity == Severity::Error);

            if !errors.is_empty() {
                println!("{}:", namespace.full());
                has_errors = true;
                for diag in &errors {
                    eprintln!(
                        "{}",
                        format_diagnostic(diag, Some(&source), Some(&file_str))
                    );
                    eprintln!();
                }
            }

            if !others.is_empty() {
                println!("{}:", namespace.full());
                for diag in &others {
                    eprintln!(
                        "{}",
                        format_diagnostic(diag, Some(&source), Some(&file_str))
                    );
                    eprintln!();
                }
            }
        }

        if any_engine.is_none() {
            any_engine = Some(engine);
        }
        all_collected.push((file_path, namespace.full(), source, diagnostics));
    }

    println!();

    // Phase 2: apply all fixes at once so cross-file renames are consistent.
    let mut files_with_fixes: Vec<String> = Vec::new();
    if fix
        && let Some(engine) = any_engine {
            let all_sources: HashMap<String, String> = all_collected
                .iter()
                .map(|(p, _, s, _)| (p.to_string_lossy().into_owned(), s.clone()))
                .collect();

            let all_diags: Vec<LintDiagnostic> = all_collected
                .iter()
                .flat_map(|(_, _, _, d)| d.iter().cloned())
                .collect();

            let patched = engine.apply_fixes_multifile(&all_sources, "", &all_diags);

            for (file, new_source) in &patched {
                let file_pb = PathBuf::from(file);
                fs::write(&file_pb, new_source).map_err(|e| CliError::Io(file_pb.clone(), e))?;
                if let Some((_, ns, _, _)) = all_collected
                    .iter()
                    .find(|(p, _, _, _)| p.to_string_lossy() == file.as_str())
                {
                    files_with_fixes.push(ns.clone());
                }
            }
        }

    if has_errors {
        println!("❌ Errors found");
        return Err(CliError::LintError);
    } else if !files_with_fixes.is_empty() {
        println!("✓ Fixed {} file(s):", files_with_fixes.len());
        for file in &files_with_fixes {
            println!("  - {}", file);
        }
    } else {
        println!("✓ No linting issues found");
    }

    Ok(())
}

fn list_all_rules() {
    let rules = LintEngine::list_all_rules();
    println!("Available lint rules:\n");

    let mut by_category: HashMap<String, Vec<RuleInfo>> = HashMap::new();
    for rule in rules {
        by_category
            .entry(rule.category.clone())
            .or_default()
            .push(rule);
    }

    let mut categories: Vec<_> = by_category.keys().collect();
    categories.sort();

    for category in categories {
        let mut rules = by_category.get(category).unwrap().clone();
        rules.sort_by(|a, b| a.id.cmp(&b.id));

        println!("{}:", category);
        for rule in rules {
            println!("  {} [{}]", rule.id, rule.severity.as_str());
        }
        println!();
    }
}
