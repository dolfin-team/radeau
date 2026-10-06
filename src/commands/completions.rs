use clap::CommandFactory;
use clap_complete::{Shell, generate};

use crate::{Cli, plugin};

/// Plugins are discovered now, so the generated script is a snapshot:
/// re-run `radeau completions` after installing or removing a plugin.
pub fn run(shell: Shell) {
    let mut cmd = Cli::command();
    for name in plugin::discover_plugins() {
        // Full flag completion if the plugin describes itself, else name only.
        let sub = plugin::plugin_completion_command(&name).unwrap_or_else(|| {
            let about = plugin::plugin_about(&name).unwrap_or_default();
            clap::Command::new(name.clone()).about(about)
        });
        let about = sub.get_about().map(|a| a.to_string()).unwrap_or_default();
        cmd = cmd.subcommand(sub.about(format!("[plugin] {about}")));
    }

    for format in plugin::discover_init_plugins() {
        let sub = plugin::init_plugin_completion_command(&format)
            .unwrap_or_else(|| clap::Command::new(format.clone()));
        cmd = cmd.mut_subcommand("init", |init| init.subcommand(sub));
    }

    generate(shell, &mut cmd, "radeau", &mut std::io::stdout());
}
