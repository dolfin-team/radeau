//! Interactive TUI fallback for missing CLI arguments.
//!
//! When required arguments are missing and stdin is a terminal (not piped),
//! this module prompts the user interactively using cliclack instead of
//! showing a raw clap error.

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::ValueEnum;
use cliclack::{input, intro, outro_cancel, select};

use crate::plugin;
use crate::{AddKind, Cli, Command, ModifyKind, RemoveKind};

/// Returns true if stdin is an interactive terminal (not a pipe or redirect).
pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal()
}

/// Outcome of the interactive TUI: either a `Cli` to run normally, or a
/// plugin that has already been executed (its exit code is final).
pub enum InteractiveOutcome {
    Run(Cli),
    Handled(ExitCode),
}

/// Try to interactively collect missing arguments via TUI.
///
/// Returns `None` if not in a terminal or the user cancelled — caller should
/// fall back to the raw clap error.
pub fn try_collect_interactive() -> Option<InteractiveOutcome> {
    if !is_interactive() {
        return None;
    }

    let args: Vec<String> = std::env::args().collect();

    intro(" raft ").ok()?;

    let cmd = args.get(1).map(String::as_str);

    let result = match cmd {
        // No command given: let the user pick
        None => pick_command(),
        Some("init") => collect_init(&args).map(InteractiveOutcome::Run),
        Some("add") => collect_add(&args).map(InteractiveOutcome::Run),
        Some("modify") => collect_modify(&args).map(InteractiveOutcome::Run),
        Some("remove") => collect_remove(&args).map(InteractiveOutcome::Run),
        Some("parse") => collect_parse().map(InteractiveOutcome::Run),
        Some("completions") => collect_completions().map(InteractiveOutcome::Run),
        // check / fmt / build / metadata have no required args — fall back to clap error
        _ => return None,
    };

    match result {
        Ok(outcome) => Some(outcome),
        Err(_) => {
            let _ = outro_cancel("Cancelled.");
            None
        }
    }
}

type TuiResult = Result<Cli, Box<dyn std::error::Error>>;
type TuiOutcomeResult = Result<InteractiveOutcome, Box<dyn std::error::Error>>;

fn pick_command() -> TuiOutcomeResult {
    let plugins = plugin::discover_plugins();

    let mut menu = select("What would you like to do?")
        .item(
            "init".to_string(),
            "init",
            "Initialize a new Dolfin package",
        )
        .item("add".to_string(), "add", "Add a new declaration")
        .item("modify".to_string(), "modify", "Modify an existing declaration")
        .item("remove".to_string(), "remove", "Remove a declaration")
        .item("check".to_string(), "check", "Check package for errors")
        .item("fmt".to_string(), "fmt", "Format Dolfin source files")
        .item(
            "build".to_string(),
            "build",
            "Build package into Turtle format",
        )
        .item("parse".to_string(), "parse", "Parse a single file (debug)")
        .item(
            "completions".to_string(),
            "completions",
            "Generate shell completions",
        );

    for name in &plugins {
        let about = plugin::plugin_about(name).unwrap_or_else(|| "(plugin)".to_string());
        menu = menu.item(format!("plugin:{name}"), name.clone(), about);
    }

    let command: String = menu.interact()?;

    if let Some(name) = command.strip_prefix("plugin:") {
        let code = plugin::dispatch(name, &[]).unwrap_or(ExitCode::FAILURE);
        return Ok(InteractiveOutcome::Handled(code));
    }

    let cli = match command.as_str() {
        "init" => collect_init(&[]),
        "add" => collect_add(&[]),
        "modify" => collect_modify(&[]),
        "remove" => collect_remove(&[]),
        "check" => Ok(Cli {
            command: Command::Check {
                path: PathBuf::from("."),
                verbose: false,
                fix: false,
                disable_rules: vec![],
                disable_categories: vec![],
                list_rules: false,
            },
        }),
        "fmt" => Ok(Cli {
            command: Command::Fmt {
                path: PathBuf::from("."),
                check: false,
                quiet: false,
                verbose: false,
                version: false,
                manifest_path: None,
            },
        }),
        "build" => Ok(Cli {
            command: Command::Build {
                path: PathBuf::from("."),
                output: None,
                base_iri: None,
                no_comments: false,
                no_rules: false,
                emit: crate::EmitFormat::Turtle,
            },
        }),
        "parse" => collect_parse(),
        "completions" => collect_completions(),
        _ => unreachable!(),
    }?;

    Ok(InteractiveOutcome::Run(cli))
}

fn collect_init(args: &[String]) -> TuiResult {
    let name: String = input("Package namespace")
        .placeholder("http://example.com/my-ontology")
        .validate(|v: &String| {
            if v.is_empty() {
                Err("Namespace cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact()?;

    // Re-use a path positional if the user already provided one
    let path = args
        .iter()
        .skip(2) // skip "raft", "init"
        .find(|a| !a.starts_with('-'))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(Cli {
        command: Command::Init {
            name: Some(name),
            path,
            description: None,
            author: None,
            with_turtle: Vec::new(),
        },
    })
}

fn collect_add(args: &[String]) -> TuiResult {
    // argv layout: ["raft", "add", "<kind?>", "<name?>", ...]
    let sub = args.get(2).map(String::as_str);

    let kind_str: String = match sub {
        Some("concept") | Some("property") => sub.unwrap().to_string(),
        _ => select("What kind of declaration?")
            .item("concept".to_string(), "concept", "A class or concept")
            .item("property".to_string(), "property", "A property or relation")
            .interact()?,
    };

    let kind = match kind_str.as_str() {
        "concept" => {
            let name: String = input("Fully qualified concept name")
                .placeholder("general.specific.MyConcept")
                .validate(|v: &String| {
                    if v.is_empty() {
                        Err("Name cannot be empty")
                    } else {
                        Ok(())
                    }
                })
                .interact()?;
            AddKind::Concept {
                name,
                subs: vec![],
                has: vec![],
                path: PathBuf::from("."),
            }
        }
        "property" => {
            let name: String = input("Fully qualified property name")
                .placeholder("general.specific.myProperty")
                .validate(|v: &String| {
                    if v.is_empty() {
                        Err("Name cannot be empty")
                    } else {
                        Ok(())
                    }
                })
                .interact()?;
            let domain: String = input("Domain type")
                .placeholder("general.MyConcept")
                .validate(|v: &String| {
                    if v.is_empty() {
                        Err("Domain cannot be empty")
                    } else {
                        Ok(())
                    }
                })
                .interact()?;
            let range: String = input("Range type")
                .placeholder("general.OtherConcept")
                .validate(|v: &String| {
                    if v.is_empty() {
                        Err("Range cannot be empty")
                    } else {
                        Ok(())
                    }
                })
                .interact()?;
            AddKind::Property {
                name,
                domain,
                range,
                domain_cardinality: "one".to_string(),
                range_cardinality: "one".to_string(),
                path: PathBuf::from("."),
            }
        }
        _ => unreachable!(),
    };

    Ok(Cli {
        command: Command::Add { kind },
    })
}

fn collect_modify(args: &[String]) -> TuiResult {
    // argv layout: ["raft", "modify", "<kind?>", "<name?>", ...]
    let sub = args.get(2).map(String::as_str);

    let kind_str: String = match sub {
        Some("concept") | Some("property") => sub.unwrap().to_string(),
        _ => select("What kind of declaration to modify?")
            .item("concept".to_string(), "concept", "A class or concept")
            .item("property".to_string(), "property", "A property or relation")
            .interact()?,
    };

    let kind = match kind_str.as_str() {
        "concept" => {
            let name: String = input("Fully qualified concept name")
                .placeholder("general.specific.MyConcept")
                .validate(|v: &String| {
                    if v.is_empty() {
                        Err("Name cannot be empty")
                    } else {
                        Ok(())
                    }
                })
                .interact()?;
            let has: String = input("Has declarations to add/remove (comma-separated, ~name to remove)")
                .placeholder("age:int, ~oldProp")
                .default_input("")
                .interact()?;
            let subs: String = input("Sub parents to add/remove (comma-separated, ~Parent to remove)")
                .placeholder("Animal, ~Pet")
                .default_input("")
                .interact()?;
            ModifyKind::Concept {
                name,
                has: split_nonempty(&has),
                subs: split_nonempty(&subs),
                path: PathBuf::from("."),
            }
        }
        "property" => {
            let name: String = input("Fully qualified property name")
                .placeholder("general.specific.myProperty")
                .validate(|v: &String| {
                    if v.is_empty() {
                        Err("Name cannot be empty")
                    } else {
                        Ok(())
                    }
                })
                .interact()?;
            let domain: String = input("New domain type (leave empty to keep unchanged)")
                .default_input("")
                .interact()?;
            let range: String = input("New range type (leave empty to keep unchanged)")
                .default_input("")
                .interact()?;
            ModifyKind::Property {
                name,
                domain: non_empty(domain),
                range: non_empty(range),
                domain_cardinality: None,
                range_cardinality: None,
                path: PathBuf::from("."),
            }
        }
        _ => unreachable!(),
    };

    Ok(Cli {
        command: Command::Modify { kind },
    })
}

fn split_nonempty(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
        .collect()
}

fn non_empty(s: String) -> Option<String> {
    if s.trim().is_empty() { None } else { Some(s) }
}

fn collect_remove(args: &[String]) -> TuiResult {
    let sub = args.get(2).map(String::as_str);

    let kind_str: String = match sub {
        Some("concept") | Some("property") => sub.unwrap().to_string(),
        _ => select("What kind of declaration to remove?")
            .item("concept".to_string(), "concept", "A class or concept")
            .item("property".to_string(), "property", "A property or relation")
            .interact()?,
    };

    let name: String = input(format!("Fully qualified {kind_str} name to remove"))
        .validate(|v: &String| {
            if v.is_empty() {
                Err("Name cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let kind = match kind_str.as_str() {
        "concept" => RemoveKind::Concept {
            name,
            force: false,
            dry_run: false,
            path: PathBuf::from("."),
        },
        "property" => RemoveKind::Property {
            name,
            force: false,
            dry_run: false,
            path: PathBuf::from("."),
        },
        _ => unreachable!(),
    };

    Ok(Cli {
        command: Command::Remove { kind },
    })
}

fn collect_parse() -> TuiResult {
    let path: String = input("File to parse")
        .placeholder("./path/to/file.dlf")
        .validate(|v: &String| {
            if v.is_empty() {
                Err("Path cannot be empty")
            } else {
                Ok(())
            }
        })
        .interact()?;

    Ok(Cli {
        command: Command::Parse {
            path: PathBuf::from(path),
            tokens: false,
            json: false,
        },
    })
}

fn collect_completions() -> TuiResult {
    let shell_str: String = select("Shell to generate completions for")
        .item("bash".to_string(), "bash", "Bash")
        .item("zsh".to_string(), "zsh", "Zsh")
        .item("fish".to_string(), "fish", "Fish")
        .item("powershell".to_string(), "powershell", "PowerShell")
        .item("elvish".to_string(), "elvish", "Elvish")
        .interact()?;

    let shell = clap_complete::Shell::from_str(&shell_str, true)
        .map_err(|e| format!("Invalid shell: {e}"))?;

    Ok(Cli {
        command: Command::Completions { shell },
    })
}
