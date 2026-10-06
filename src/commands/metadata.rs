//! Emit package metadata as JSON — the `cargo metadata` equivalent.
//!
//! Plugins should prefer `$RADEAU metadata` over re-implementing package
//! loading (package.dlf resolution, namespace layout, etc.) themselves.

use crate::error::CliError;
use rowl::package;
use serde_json::json;
use std::io::{self, Write};
use std::path::PathBuf;

pub fn run(path: PathBuf) -> Result<(), CliError> {
    let pkg = package::load_package(&path)?;

    let files: Vec<_> = pkg
        .iter_ontologies()
        .map(|(ns, onto)| {
            json!({
                "namespace": ns.full(),
                "path": onto.relative_path,
            })
        })
        .collect();

    let concepts: Vec<_> = pkg
        .all_concepts()
        .into_iter()
        .map(|(ns, c)| {
            json!({
                "namespace": ns.full(),
                "name": c.name.get(),
                "is_enum": c.one_of.is_some(),
            })
        })
        .collect();

    let properties: Vec<_> = pkg
        .all_properties()
        .into_iter()
        .map(|(ns, p)| {
            json!({
                "namespace": ns.full(),
                "name": p.name.get(),
            })
        })
        .collect();

    let metadata = json!({
        "namespace": pkg.namespace().full(),
        "version": pkg.version(),
        "dolfin_version": pkg.dolfin_version(),
        "root": pkg.root,
        "files": files,
        "concepts": concepts,
        "properties": properties,
    });

    io::stdout()
        .write_all(serde_json::to_string_pretty(&metadata).unwrap().as_bytes())
        .map_err(|e| CliError::Io(PathBuf::new(), e))?;
    println!();

    Ok(())
}
