use std::fs;
use std::path::{Path, PathBuf};

use crate::CliError;

pub fn run(
    path: PathBuf,
    check: bool,
    quiet: bool,
    verbose: bool,
    version: bool,
    manifest_path: Option<PathBuf>,
) -> Result<(), CliError> {
    if version {
        println!("dolfin-fmt version {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let package_root = if let Some(manifest) = &manifest_path {
        manifest.parent().unwrap_or(path.as_path()).to_path_buf()
    } else {
        path.clone()
    };

    // Try to load as a package
    let package_result = rowl::package::load_package(&package_root);

    let files: Vec<PathBuf> = if let Ok(package) = package_result {
        // Format all ontology files in the package
        package
            .iter_ontologies()
            .map(|(_, ontology)| package_root.join(&ontology.relative_path))
            .collect()
    } else {
        // Try as a single file
        if path.is_file() {
            vec![path]
        } else {
            // Find all .dolfin files in directory
            find_dolfin_files(&path)?
        }
    };

    if files.is_empty() {
        if !quiet {
            println!("No Dolfin files found to format");
        }
        return Ok(());
    }

    let formatter = dolfin_lint::formatter::Formatter::new();
    let mut needs_formatting = Vec::new();
    let mut formatted_count = 0;
    let mut error_count = 0;

    for file_path in &files {
        let source = match fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(e) => {
                if !quiet {
                    eprintln!("Error reading {}: {}", file_path.display(), e);
                }
                error_count += 1;
                continue;
            }
        };

        match formatter.format(&source) {
            Ok(formatted) => {
                if source == formatted {
                    if verbose {
                        println!("✓ {} (already formatted)", file_path.display());
                    }
                } else if check {
                    if !quiet {
                        println!("Would reformat: {}", file_path.display());
                    }
                    needs_formatting.push(file_path.clone());
                } else {
                    match fs::write(file_path, &formatted) {
                        Ok(()) => {
                            formatted_count += 1;
                            if !quiet {
                                println!("Formatted: {}", file_path.display());
                            }
                        }
                        Err(e) => {
                            if !quiet {
                                eprintln!("Error writing {}: {}", file_path.display(), e);
                            }
                            error_count += 1;
                        }
                    }
                }
            }
            Err(e) => {
                if !quiet {
                    eprintln!("Error formatting {}: {}", file_path.display(), e);
                }
                error_count += 1;
            }
        }
    }

    if !check && !quiet {
        if formatted_count > 0 {
            println!();
            println!("Formatted {} file(s)", formatted_count);
        }
        if error_count > 0 {
            println!();
            eprintln!("Failed to format {} file(s)", error_count);
        }
    }

    if check && !needs_formatting.is_empty() {
        println!();
        eprintln!("{} file(s) need formatting", needs_formatting.len());
        return Err(CliError::FormatCheckFailed);
    }

    if error_count > 0 {
        return Err(CliError::FormatError);
    }

    Ok(())
}

fn find_dolfin_files(dir: &Path) -> Result<Vec<PathBuf>, CliError> {
    let mut files = Vec::new();

    fn visit_dir(dir: &std::path::Path, files: &mut Vec<PathBuf>) -> Result<(), CliError> {
        for entry in fs::read_dir(dir).map_err(|e| CliError::Io(dir.to_path_buf(), e))? {
            let entry = entry.map_err(|e| CliError::Io(dir.to_path_buf(), e))?;
            let path = entry.path();

            if path.is_dir() {
                visit_dir(&path, files)?;
            } else if let Some(ext) = path.extension()
                && (ext == "dolfin" || ext == "dlf")
            {
                files.push(path);
            }
        }
        Ok(())
    }

    visit_dir(dir, &mut files)?;
    Ok(files)
}
