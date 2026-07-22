//! Dolfin CLI - Command line interface for the Dolfin ontology language.

use clap::Parser;
use raft::{Cli, run};
use std::error::Error;

use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);

            // Print cause chain
            let mut source = e.source();
            while let Some(cause) = source {
                eprintln!("  Caused by: {}", cause);
                source = cause.source();
            }

            ExitCode::FAILURE
        }
    }
}
