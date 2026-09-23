//! Dolfin CLI - Command line interface for the Dolfin ontology language.

use clap::Parser;
use raft::{Cli, interactive, plugin, run};
use std::error::Error;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // `raft help ${command}` for a command raft doesn't know about:
    // forward to `raft-${command} ${command} --help`.
    if args.len() >= 3 && args[1] == "help" && !plugin::STATIC_COMMANDS.contains(&args[2].as_str())
    {
        if let Some(code) = plugin::dispatch_help(&args[2]) {
            return code;
        }
    }

    // `raft ${command}` for a command raft doesn't know about: forward to
    // `raft-${command} ${command} <rest>`, cargo-external-subcommand style.
    if args.len() >= 2 && !plugin::STATIC_COMMANDS.contains(&args[1].as_str()) {
        if let Some(code) = plugin::dispatch(&args[1], &args[2..]) {
            return code;
        }
    }

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            use clap::error::ErrorKind;
            if err.kind() == ErrorKind::DisplayHelp && args.len() <= 2 {
                let _ = err.print();
                let plugins = plugin::discover_plugins();
                if !plugins.is_empty() {
                    println!("\nPlugins:");
                    let width = plugins.iter().map(String::len).max().unwrap_or(0);
                    for name in &plugins {
                        match plugin::plugin_about(name) {
                            Some(about) => println!("  {name:width$}  {about}"),
                            None => println!("  {name:width$}  (raft-{name})"),
                        }
                    }
                }
                std::process::exit(err.exit_code());
            }
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                err.exit();
            }
            match interactive::try_collect_interactive() {
                Some(interactive::InteractiveOutcome::Run(cli)) => cli,
                Some(interactive::InteractiveOutcome::Handled(code)) => return code,
                None => err.exit(),
            }
        }
    };

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
