//! Interactive TUI fallback for missing CLI arguments.
//!
//! The prompting itself lives in `radeau_plugin::tui`, shared with plugins.
//! radeau only adds its installed plugins to the top-level menu, and its
//! `radeau-init-<format>` importers to the `init` step. Picking one runs it
//! with no arguments: any menu from there on is the plugin's own.

use std::process::ExitCode;

use clap::{CommandFactory, FromArgMatches};
use radeau_plugin::tui::{self, Outcome};

use crate::Cli;
use crate::plugin;

pub use radeau_plugin::tui::is_interactive;

/// Outcome of the interactive TUI: either a `Cli` to run normally, or an
/// exit code that is already final (plugin ran, user cancelled, clap error
/// already printed).
pub enum InteractiveOutcome {
    Run(Cli),
    Handled(ExitCode),
}

/// Try to interactively complete the command line that produced `err`.
///
/// Returns `None` if not in a terminal or `err` is not about something
/// missing (typos, bad values): caller should show the clap error.
pub fn try_collect_interactive(err: &clap::Error) -> Option<InteractiveOutcome> {
    if !tui::is_missing(err) || !is_interactive() {
        return None;
    }

    let plugins = plugin::discover_plugins().into_iter().map(|name| {
        let about = plugin::plugin_about(&name).unwrap_or_default();
        let choice = (
            format!("plugin:{name}"),
            format!("{name} [plugin]"),
            format!("external radeau-{name} · {about}"),
        );
        ("radeau".to_string(), choice)
    });
    let importers = plugin::discover_init_plugins().into_iter().map(|format| {
        let about = plugin::init_plugin_about(&format).unwrap_or_default();
        let choice = (
            format!("init:{format}"),
            format!("init {format} [importer]"),
            format!("external radeau-init-{format} · {about}"),
        );
        ("init".to_string(), choice)
    });
    let extras: Vec<tui::ScopedChoice> = plugins.chain(importers).collect();

    cliclack::intro(" radeau ").ok()?;
    let cmd = Cli::command();
    // On Esc / Ctrl-C cliclack has already printed "Operation cancelled".
    let outcome = match tui::complete(&cmd, std::env::args().collect(), "radeau", &extras) {
        Ok(Outcome::Args(argv)) => cmd
            .try_get_matches_from(argv)
            .and_then(|m| Cli::from_arg_matches(&m))
            .map(InteractiveOutcome::Run)
            .unwrap_or_else(|e| e.exit()),
        Ok(Outcome::Extra(choice)) => {
            let code = if let Some(format) = choice.strip_prefix("init:") {
                let _ = cliclack::outro(format!("Running importer: radeau init {format}"));
                plugin::dispatch_init(format, &[])
            } else {
                let name = choice.trim_start_matches("plugin:");
                let _ = cliclack::outro(format!("Running plugin: radeau {name}"));
                plugin::dispatch(name, &[])
            };
            InteractiveOutcome::Handled(code.unwrap_or(ExitCode::FAILURE))
        }
        Ok(Outcome::Failed(_)) | Err(_) => InteractiveOutcome::Handled(ExitCode::FAILURE),
    };
    Some(outcome)
}
