//! Parse a single file for debugging.

use crate::CliError;
use rowl::lexer::Lexer;
use rowl::parser;
use std::fs;
use std::path::PathBuf;

pub fn run(path: PathBuf, tokens: bool, _json: bool) -> Result<(), CliError> {
    let source = fs::read_to_string(&path)?;

    if tokens {
        // Show tokens
        println!("Tokens for {}:", path.display());
        println!("{:-<60}", "");

        let lexer = Lexer::new(&source);
        for result in lexer {
            match result {
                Ok((start, token, end)) => {
                    println!(
                        "{:4}:{:<3} - {:4}:{:<3}  {:?}",
                        start.line, start.column, end.line, end.column, token
                    );
                }
                Err(e) => {
                    println!("ERROR: {:?}", e);
                }
            }
        }
    } else {
        // Show AST
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let ast = if file_name == "package.dlf" {
            println!("Package manifest: {}", path.display());
            println!("{:-<60}", "");
            let pkg = parser::parse_package(&source)?;
            format!("{:#?}", pkg)
        } else {
            println!("Ontology file: {}", path.display());
            println!("{:-<60}", "");
            let onto = parser::parse_ontology(&source).map_err(|e| CliError::Parse(e))?;
            format!("{:#?}", onto)
        };

        println!("{}", ast);
    }

    Ok(())
}
