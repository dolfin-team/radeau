# Radeau - Dolfin Ontology CLI

Radeau is the command-line interface for working with Dolfin ontology files. It provides tools for linting, formatting, building, and managing Dolfin packages (`.dlf` files).

## Installation

### From Source

```bash
cd rust
cargo install --path radeau
```

## Shell Completions

Radeau can generate shell completions for bash, zsh, fish, elvish, and powershell.

### Bash

```bash
# System-wide installation (requires sudo)
radeau completions bash | sudo tee /etc/bash_completion.d/radeau

# User installation (recommended)
mkdir -p ~/.local/share/bash-completion/completions
radeau completions bash > ~/.local/share/bash-completion/completions/radeau

# Source completions (add to ~/.bashrc)
echo 'source ~/.local/share/bash-completion/completions/radeau' >> ~/.bashrc

# Reload shell or source immediately
source ~/.bashrc
```

### Zsh

```bash
# Create completions directory
mkdir -p ~/.zsh/completions

# Generate completions
radeau completions zsh > ~/.zsh/completions/_radeau

# Add to ~/.zshrc (if not already present)
echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc

# Reload shell
exec $SHELL
```

### Fish

```bash
# Create completions directory
mkdir -p ~/.config/fish/completions

# Generate completions
radeau completions fish > ~/.config/fish/completions/radeau.fish

# Completions are automatically loaded - no reload needed
```

### Elvish

```bash
# Create completions directory
mkdir -p ~/.elvish/lib

# Generate completions
radeau completions elvish > ~/.elvish/lib/radeau.elv

# Add to ~/.elvish/rc.elv
echo 'use ./lib/radeau' >> ~/.elvish/rc.elv

# Reload
elvish
```

### PowerShell

```powershell
# Generate to profile location
radeau completions powershell > $PROFILE

# Or register in current session only
radeau completions powershell | Out-String | Invoke-Expression
```

## Completion Features

The shell completions provide:

- **Command completion**: `radeau <TAB>` shows all available commands
- **Option completion**: `radeau check --<TAB>` shows all check options
- **Rule completion**: `radeau check --disable-rule <TAB>` shows all available lint rules
- **Category completion**: `radeau check --disable-category <TAB>` shows all rule categories

## Interactive Mode

Running a command with missing required arguments (or `radeau` with no subcommand)
drops into an interactive prompt that collects the missing values instead of
failing outright. Installed plugins appear in its menus (see
[Interactive mode and plugins](#interactive-mode-and-plugins)).

## Usage Examples

### Initialize a new package

```bash
# Minimal
radeau init --name "http://example.org/my-ontology" --description "My ontology"

# With authors (repeatable)
radeau init --name "http://example.org/my-ontology" --author "Alice" --author "Bob"

# Seed the package from an existing ontology, through an importer plugin
# (see "Init importers" below). The importer defines its own arguments.
radeau init <format> base.ext extra.ext --path my-ontology
```

### Check for linting issues

```bash
# Check all files in package
radeau check .

# Check with verbose output
radeau check -v .

# Disable specific rules (with tab completion!)
radeau check --disable-rule naming/concept-pascal-case .

# Disable entire categories
radeau check --disable-category style --disable-category unused .

# Auto-fix issues where possible
radeau check --fix .

# List all available rules
radeau check --list-rules
```

### Format files

```bash
# Format all files in package
radeau fmt .

# Format single file
radeau fmt path/to/file.dlf

# Check if files need formatting (for CI)
radeau fmt --check .

# Verbose output
radeau fmt -v .

# Quiet mode (errors only)
radeau fmt -q .

# Point at an explicit manifest
radeau fmt --manifest-path path/to/package.dlf
```

### Build to Turtle

```bash
# Build package to stdout
radeau build .

# Build to file
radeau build --output ontology.ttl .

# With custom base IRI
radeau build --base-iri http://example.org/ontology .

# Exclude comments from the Turtle output
radeau build --no-comments .

# Write rules as comments instead of emitting them into the Turtle
radeau build --no-rules .

# Emit only the N3 rules
radeau build --emit n3-rules .
```

`--emit` accepts `turtle` (default) or `n3-rules`.

### Manage declarations

```bash
# Add a concept
radeau add concept general.specific.MyConcept --sub Thing --has "name:string"

# Add a property
radeau add property general.specific.hasOwner --domain Person --range Person \
    --domain-cardinality one --range-cardinality optional

# Modify a concept: add "age:int", remove the "name" has, drop parent "Pet"
radeau modify concept general.specific.MyConcept --has age:int --has ~name --sub ~Pet

# Modify a property: change range and cardinality
radeau modify property general.specific.hasOwner --range Organization --range-cardinality any

# Remove a concept (dry run shows what would be removed)
radeau remove concept general.specific.MyConcept --dry-run

# Remove with force (even if used elsewhere)
radeau remove property general.specific.hasOwner --force
```

Cardinalities accept `one`, `optional`, `any`, `some`, or numeric forms
(`N`, `N..M`, `N..*`). In `modify`, prefix a value with `~` to remove it
(`~name` removes a has, `~Parent` removes a sub, `~*` removes all subs).

### Inspect package metadata

```bash
# Print namespace, files, concepts, and properties as JSON.
# Intended for plugins to consume instead of re-loading the package.
radeau metadata .
```

### Parse a single file (debugging)

```bash
# Show the AST
radeau parse path/to/file.dlf

# Show tokens instead
radeau parse --tokens path/to/file.dlf

# Emit JSON
radeau parse --json path/to/file.dlf
```

## Available Commands

- `init` - Initialize a new Dolfin package (or import one with `radeau init <format>`, see [Init importers](#init-importers))
- `check` - Check a package for linting errors
- `fmt` - Format Dolfin source files
- `build` - Build a package into Turtle or N3-rules format
- `add` - Add a new declaration (concept or property) to the package
- `modify` - Modify an existing declaration (concept or property)
- `remove` - Remove a declaration (concept or property) from the package
- `parse` - Parse a single file and show the AST or tokens (for debugging)
- `metadata` - Print package metadata as JSON
- `completions` - Generate shell completions

## Plugins

Radeau can be extended without rebuilding it. A plugin is a separate executable
named `radeau-<name>`; radeau finds it at runtime and forwards to it, the way
`cargo` handles `cargo-<name>`.

Some plugins are maintained alongside radeau as `radeau-*` crates in this
workspace. Run `radeau --help` to see which ones are installed on your machine:
they are listed under `Plugins:` and `Init importers:`.

### How radeau finds a plugin

Radeau looks for `radeau-<name>` first in the directory of the running `radeau`
executable, then in each `PATH` directory. The first executable file found
wins. Installing a plugin next to `radeau` (e.g. both with `cargo install`) is
enough.

A plugin can never replace a built-in command: `radeau-check` would be ignored.

### Command plugins

Any command radeau does not know is forwarded:

| You type | Radeau runs |
|---|---|
| `radeau <name> <args>...` | `radeau-<name> <name> <args>...` |
| `radeau help <name>` | `radeau-<name> <name> --help` |

The plugin receives its own name as first argument (it should drop it before
parsing) and a `RADEAU` environment variable holding the path of the running
`radeau` binary, so it can call back into radeau instead of linking it. For
example `$RADEAU metadata <path>` prints the package as JSON, so a plugin does
not need to load the package itself.

### Init importers

`radeau init` has a second plugin level, for creating a package from an existing
ontology in some other format:

| You type | Radeau runs |
|---|---|
| `radeau init <format> <args>...` | `radeau-init-<format> <format> <args>...` |
| `radeau help init <format>` | `radeau-init-<format> <format> --help` |

Each importer defines its own arguments (input files, target directory, ...);
`radeau init <format> --help` shows them. Supporting a new input format means
writing a new `radeau-init-<format>` executable; radeau itself does not change.

If an importer named `<format>` is installed, `radeau init <format>` runs it
even when a directory of that name exists. Write `radeau init ./<format>` to
initialize that directory instead.

Importers are not listed as top-level plugins; `radeau --help` shows them in
their own `Init importers:` section.

### Interactive mode and plugins

In a terminal, an incomplete command opens the interactive prompt:

- `radeau` alone shows a menu of built-in commands followed by the installed
  command plugins. Picking a plugin runs it with no arguments.
- `radeau init` without `--name` first asks how to initialize: plain `init`, or
  one of the installed importers. Picking an importer runs it with no
  arguments.

From there, a plugin built with `radeau_plugin::run` (below) asks for its own
missing arguments in the same style.

### Shell completions

`radeau completions <shell>` includes installed plugins: command plugins as
top-level commands, importers as subcommands of `init`. Plugins that answer
the completion protocol (below) get full flag completion, others complete by
name only. The generated script is a snapshot: run `radeau completions` again
after installing or removing a plugin.

### Writing a plugin

Any executable named `radeau-<name>` works. For a Rust plugin, the
[`radeau-plugin`](../radeau-plugin) crate does the protocol work in one call:

```rust
use clap::Parser;
use std::process::ExitCode;

/// One-line description, shown in `radeau --help`
#[derive(Parser)]
#[command(name = "radeau-hello", version, about)]
struct Cli {
    /// Path to package root
    #[arg(default_value = ".")]
    path: std::path::PathBuf,
}

fn main() -> ExitCode {
    radeau_plugin::run("hello", |cli: Cli| -> Result<(), std::io::Error> {
        println!("hello from {}", cli.path.display());
        Ok(())
    })
}
```

`radeau_plugin::run(echo, body)`:

1. drops the `echo` argument radeau prepends (pass `<name>` for a command
   plugin, `<format>` for an init importer); running the binary directly
   still works;
2. answers `--radeau-completion-spec`, radeau's request for the plugin's flags as
   JSON, used by `radeau completions`;
3. in a terminal, prompts for missing required arguments, then offers the
   optional ones;
4. parses the arguments and calls `body`;
5. on error, prints it with its cause chain and exits with status 1.

`body` may return any error type that converts to `Box<dyn Error>`, a
`String` included.

The first non-empty line of `radeau-<name> <name> --help` (clap's `about`) is
the description radeau shows in `radeau --help` and in the interactive menu.

## Lint Rule Categories

Radeau's linter includes rules in the following categories:

- **naming**: Naming conventions (PascalCase, camelCase, etc.)
- **ordering**: Declaration and member ordering
- **unused**: Unused elements (prefixes, variables)
- **semantic**: Semantic correctness (unresolved types, circular inheritance)
- **style**: Code formatting style (trailing whitespace, blank lines)
- **package**: Package-level checks (missing version)

Run `radeau check --list-rules` to see all available rules.

## Configuration

Create a `.dolfin-lint.toml` file in your package root to configure the linter:

```toml
version = "1"

[lint]
# Disable specific rules
disabled_rules = [
    "style/trailing-whitespace",
    "ordering/alphabetical-members"
]

# Disable entire categories
disabled_categories = ["unused"]
```

The configuration cascades from parent directories, allowing project-wide defaults with per-directory overrides.

## Development

### Building from source

```bash
cd rust
cargo build --release
```

### Running tests

```bash
cargo test
```

### Running linter on itself

```bash
cargo clippy
cargo fmt --check
```

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Contributing

Contributions welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for workflow,
branch naming, coding standards, and how to report bugs or suggest features.
</content>
</invoke>
