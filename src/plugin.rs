//! Cargo-style external subcommand plugins.
//!
//! An invocation of `raft ${command}` for an `${command}` raft doesn't know
//! about is translated into `raft-${command} ${command} <rest of argv>`,
//! looked up on `$PATH` (own exe dir first, mirroring cargo's
//! `$CARGO_HOME/bin` > `$PATH` precedence). `raft help ${command}` forwards
//! as `raft-${command} ${command} --help`. Plugins get a `RAFT` env var
//! pointing back at the running `raft` binary so they can shell out to it
//! instead of linking raft as a library.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Top-level command words raft handles itself. A plugin can never shadow
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

/// Directories to search, in priority order: next to raft's own exe, then `$PATH`.
fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
        }
    }
    if let Some(path_var) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path_var));
    }
    dirs
}

fn plugin_bin_name(command: &str) -> String {
    format!("raft-{command}")
}

/// Locate the binary for `raft-${command}`, honoring exe-dir-then-PATH priority.
pub fn find_plugin_bin(command: &str) -> Option<PathBuf> {
    let bin_name = plugin_bin_name(command);
    search_dirs().into_iter().find_map(|dir| {
        let candidate = dir.join(&bin_name);
        is_executable(&candidate).then_some(candidate)
    })
}

/// Discover all installed `raft-*` plugins, deduped, sorted, excluding
/// anything that collides with a static command name.
pub fn discover_plugins() -> Vec<String> {
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
            let Some(name) = file_name.strip_prefix("raft-") else {
                continue;
            };
            if name.is_empty() || STATIC_COMMANDS.contains(&name) {
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
    let raft_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("raft"));

    let status = std::process::Command::new(bin)
        .arg(command)
        .args(args)
        .env("RAFT", raft_exe)
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

/// Dispatch `command` with `rest` to `raft-${command}` if it's installed.
/// Returns `None` if no such plugin exists (caller should fall back to
/// raft's own clap error / interactive flow).
pub fn dispatch(command: &str, rest: &[String]) -> Option<ExitCode> {
    let bin = find_plugin_bin(command)?;
    Some(run_plugin(&bin, command, rest))
}

/// Handle `raft help ${command}` for a plugin command: forwards as
/// `raft-${command} ${command} --help`.
pub fn dispatch_help(command: &str) -> Option<ExitCode> {
    let bin = find_plugin_bin(command)?;
    Some(run_plugin(&bin, command, &["--help".to_string()]))
}

/// Ask a plugin to advertise itself: run `raft-${command} ${command} --help`
/// with output captured (not inherited) and take clap's first line — the
/// derived `about` text — as the one-line description shown in `raft --help`.
pub fn plugin_about(command: &str) -> Option<String> {
    let bin = find_plugin_bin(command)?;
    let raft_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("raft"));

    let output = std::process::Command::new(&bin)
        .arg(command)
        .arg("--help")
        .env("RAFT", raft_exe)
        .output()
        .ok()?;

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
}
