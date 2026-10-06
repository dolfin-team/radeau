//! Dolfin CLI - Command line interface for the Dolfin ontology language.

use clap::Parser;
use radeau::{Cli, interactive, plugin, run};
use std::error::Error;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // `radeau init <format> <rest>` / `radeau help init <format>`: forward to the
    // `radeau-init-<format>` importer if one is installed. Otherwise the word
    // falls through to clap as init's PATH positional.
    if args.len() >= 3 && args[1] == "init" && !args[2].starts_with('-') {
        if let Some(code) = plugin::dispatch_init(&args[2], &args[3..]) {
            return code;
        }
    }
    if args.len() >= 4 && args[1] == "help" && args[2] == "init" {
        if let Some(code) = plugin::dispatch_init(&args[3], &["--help".to_string()]) {
            return code;
        }
    }

    // `radeau help ${command}` for a command radeau doesn't know about:
    // forward to `radeau-${command} ${command} --help`.
    if args.len() >= 3 && args[1] == "help" && !plugin::STATIC_COMMANDS.contains(&args[2].as_str())
    {
        if let Some(code) = plugin::dispatch_help(&args[2]) {
            return code;
        }
    }

    // `radeau ${command}` for a command radeau doesn't know about: forward to
    // `radeau-${command} ${command} <rest>`, cargo-external-subcommand style.
    if args.len() >= 2 && !plugin::STATIC_COMMANDS.contains(&args[1].as_str()) {
        if let Some(code) = plugin::dispatch(&args[1], &args[2..]) {
            return code;
        }
    }

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            use clap::error::ErrorKind;
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) {
                if err.kind() == ErrorKind::DisplayHelp && args.len() <= 2 {
                    exit_with_top_level_help(&err);
                }
                err.exit();
            }
            match interactive::try_collect_interactive(&err) {
                Some(interactive::InteractiveOutcome::Run(cli)) => cli,
                Some(interactive::InteractiveOutcome::Handled(code)) => return code,
                // Bare `radeau` outside a terminal: help, plugins included.
                None if args.len() == 1 => exit_with_top_level_help(&err),
                None => err.exit(),
            }
        }
    };

    run_cli(cli)
}

/// Print clap's top-level help followed by a `Plugins:` section listing the
/// external `radeau-*` commands found at runtime, then exit.
fn exit_with_top_level_help(err: &clap::Error) -> ! {
    let _ = err.print();
    let plugins = plugin::discover_plugins();
    if !plugins.is_empty() {
        println!("\nPlugins (external radeau-<name> executables):");
        let width = plugins.iter().map(String::len).max().unwrap_or(0);
        for name in &plugins {
            match plugin::plugin_about(name) {
                Some(about) => println!("  {name:width$}  [plugin] {about}"),
                None => println!("  {name:width$}  [plugin] (radeau-{name})"),
            }
        }
    }
    let importers = plugin::discover_init_plugins();
    if !importers.is_empty() {
        println!("\nInit importers (radeau init <format>, external radeau-init-<format>):");
        let width = importers.iter().map(String::len).max().unwrap_or(0);
        for format in &importers {
            let about = plugin::init_plugin_about(format).unwrap_or_default();
            println!("  {format:width$}  {about}");
        }
    }
    std::process::exit(err.exit_code());
}

fn run_cli(cli: Cli) -> ExitCode {
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
