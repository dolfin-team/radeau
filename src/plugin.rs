//! Cargo-style external subcommand plugins.
//!
//! An invocation of `radeau ${command}` for an `${command}` radeau doesn't know
//! about is translated into `radeau-${command} ${command} <rest of argv>`,
//! looked up on `$PATH` (own exe dir first, mirroring cargo's
//! `$CARGO_HOME/bin` > `$PATH` precedence). `radeau help ${command}` forwards
//! as `radeau-${command} ${command} --help`. Plugins get a `RADEAU` env var
//! pointing back at the running `radeau` binary so they can shell out to it
//! instead of linking radeau as a library.
//!
//! `radeau init <format> <rest>` works the same way one level down: it runs
//! `radeau-init-<format> <format> <rest>` (e.g. `radeau-init-turtle`), so
//! package importers for new RDF syntaxes ship as separate binaries.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Top-level command words radeau handles itself. A plugin can never shadow
/// one of these.
pub const STATIC_COMMANDS: &[&str] = &[
    "init",
    "check",
    "fmt",
    "build",
    "add",
    "modify",
    "remove",
    "parse",
    "completions",
    "metadata",
    "help",
];

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Directories to search, in priority order: next to radeau's own exe, then `$PATH`.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
        }
    if let Some(path_var) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path_var));
    }
    dirs
}

fn plugin_bin_name(command: &str) -> String {
    format!("radeau-{command}")
}

/// Locate the binary for `radeau-${command}`, honoring exe-dir-then-PATH priority.
pub fn find_plugin_bin(command: &str) -> Option<PathBuf> {
    let bin_name = plugin_bin_name(command);
    search_dirs().into_iter().find_map(|dir| {
        let candidate = dir.join(&bin_name);
        is_executable(&candidate).then_some(candidate)
    })
}

/// Bin-name prefix of `radeau init <format>` importer plugins
/// (`radeau-init-<format>`). Kept out of the top-level plugin list.
pub const INIT_PREFIX: &str = "init-";

/// Discover all installed `radeau-*` plugins, deduped, sorted, excluding
/// anything that collides with a static command name and init importers.
pub fn discover_plugins() -> Vec<String> {
    discover_with_prefix("radeau-")
        .into_iter()
        .filter(|name| !name.starts_with(INIT_PREFIX) && !STATIC_COMMANDS.contains(&name.as_str()))
        .collect()
}

/// Discover installed `radeau-init-<format>` importers, returning the formats.
pub fn discover_init_plugins() -> Vec<String> {
    discover_with_prefix(&format!("radeau-{INIT_PREFIX}"))
}

fn discover_with_prefix(prefix: &str) -> Vec<String> {
    let mut found = std::collections::BTreeSet::new();
    for dir in search_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
                continue;
            };
            #[cfg(windows)]
            let file_name = file_name.strip_suffix(".exe").unwrap_or(file_name);
            let Some(name) = file_name.strip_prefix(prefix) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            if is_executable(&path) {
                found.insert(name.to_string());
            }
        }
    }
    found.into_iter().collect()
}

fn run_plugin(bin: &Path, command: &str, args: &[String]) -> ExitCode {
    let radeau_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("radeau"));

    let status = std::process::Command::new(bin)
        .arg(command)
        .args(args)
        .env("RADEAU", radeau_exe)
        .status();

    match status {
        Ok(status) => {
            if status.success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(status.code().unwrap_or(1) as u8)
            }
        }
        Err(e) => {
            eprintln!("Error: failed to run plugin '{}': {e}", bin.display());
            ExitCode::FAILURE
        }
    }
}

/// Dispatch `command` with `rest` to `radeau-${command}` if it's installed.
/// Returns `None` if no such plugin exists (caller should fall back to
/// radeau's own clap error / interactive flow).
pub fn dispatch(command: &str, rest: &[String]) -> Option<ExitCode> {
    let bin = find_plugin_bin(command)?;
    Some(run_plugin(&bin, command, rest))
}

/// Dispatch `radeau init <format> <rest>` to `radeau-init-<format> <format> <rest>`
/// if that importer is installed.
pub fn dispatch_init(format: &str, rest: &[String]) -> Option<ExitCode> {
    let bin = find_plugin_bin(&format!("{INIT_PREFIX}{format}"))?;
    Some(run_plugin(&bin, format, rest))
}

/// Handle `radeau help ${command}` for a plugin command: forwards as
/// `radeau-${command} ${command} --help`.
pub fn dispatch_help(command: &str) -> Option<ExitCode> {
    let bin = find_plugin_bin(command)?;
    Some(run_plugin(&bin, command, &["--help".to_string()]))
}

/// Ask a plugin to advertise itself: run `radeau-${command} ${command} --help`
/// with output captured (not inherited) and take clap's first line — the
/// derived `about` text — as the one-line description shown in `radeau --help`.
pub fn plugin_about(command: &str) -> Option<String> {
    about_of(command, command)
}

/// `plugin_about` for the `radeau init <format>` importer.
pub fn init_plugin_about(format: &str) -> Option<String> {
    about_of(&format!("{INIT_PREFIX}{format}"), format)
}

fn about_of(bin_command: &str, command: &str) -> Option<String> {
    let bin = find_plugin_bin(bin_command)?;
    let radeau_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("radeau"));

    let output = std::process::Command::new(&bin)
        .arg(command)
        .arg("--help")
        .env("RADEAU", radeau_exe)
        .output()
        .ok()?;

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
}

/// Ask a plugin for its completion spec (`radeau-${command} ${command}
/// --radeau-completion-spec`) and rebuild it as a clap command named
/// `${command}`. `None` if the plugin doesn't speak the protocol.
pub fn plugin_completion_command(command: &str) -> Option<clap::Command> {
    completion_command_of(command, command)
}

/// `plugin_completion_command` for the `radeau init <format>` importer.
pub fn init_plugin_completion_command(format: &str) -> Option<clap::Command> {
    completion_command_of(&format!("{INIT_PREFIX}{format}"), format)
}

fn completion_command_of(bin_command: &str, command: &str) -> Option<clap::Command> {
    let bin = find_plugin_bin(bin_command)?;
    let output = std::process::Command::new(&bin)
        .arg(command)
        .arg(radeau_plugin::SPEC_FLAG)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let spec: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    Some(radeau_plugin::from_spec(command, &spec))
}
