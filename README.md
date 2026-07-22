# Raft - Dolfin Ontology CLI

Raft is the command-line interface for working with Dolfin ontology files. It provides tools for linting, formatting, building, and managing Dolfin packages (`.dlf` files).

## Installation

### From Source

```bash
cd rust
cargo install --path raft
```

## Shell Completions

Raft can generate shell completions for bash, zsh, fish, elvish, and powershell.

### Bash

```bash
# System-wide installation (requires sudo)
raft completions bash | sudo tee /etc/bash_completion.d/raft

# User installation (recommended)
mkdir -p ~/.local/share/bash-completion/completions
raft completions bash > ~/.local/share/bash-completion/completions/raft

# Source completions (add to ~/.bashrc)
echo 'source ~/.local/share/bash-completion/completions/raft' >> ~/.bashrc

# Reload shell or source immediately
source ~/.bashrc
```

### Zsh

```bash
# Create completions directory
mkdir -p ~/.zsh/completions

# Generate completions
raft completions zsh > ~/.zsh/completions/_raft

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
raft completions fish > ~/.config/fish/completions/raft.fish

# Completions are automatically loaded - no reload needed
```

### Elvish

```bash
# Create completions directory
mkdir -p ~/.elvish/lib

# Generate completions
raft completions elvish > ~/.elvish/lib/raft.elv

# Add to ~/.elvish/rc.elv
echo 'use ./lib/raft' >> ~/.elvish/rc.elv

# Reload
elvish
```

### PowerShell

```powershell
# Generate to profile location
raft completions powershell > $PROFILE

# Or register in current session only
raft completions powershell | Out-String | Invoke-Expression
```

## Completion Features

The shell completions provide:

- **Command completion**: `raft <TAB>` shows all available commands
- **Option completion**: `raft check --<TAB>` shows all check options
- **Rule completion**: `raft check --disable-rule <TAB>` shows all available lint rules
- **Category completion**: `raft check --disable-category <TAB>` shows all rule categories

## Interactive Mode

Running a command with missing required arguments (or `raft` with no subcommand)
drops into an interactive prompt that collects the missing values instead of
failing outright.

## Usage Examples

### Initialize a new package

```bash
# Minimal
raft init --name "http://example.org/my-ontology" --description "My ontology"

# With authors (repeatable)
raft init --name "http://example.org/my-ontology" --author "Alice" --author "Bob"

# Seed the package from existing RDF/Turtle (reversed to Dolfin via mekarui).
# --name is optional here; it is derived from the RDF.
raft init --with-turtle base.ttl,extra.ttl
```

### Check for linting issues

```bash
# Check all files in package
raft check .

# Check with verbose output
raft check -v .

# Disable specific rules (with tab completion!)
raft check --disable-rule naming/concept-pascal-case .

# Disable entire categories
raft check --disable-category style --disable-category unused .

# Auto-fix issues where possible
raft check --fix .

# List all available rules
raft check --list-rules
```

### Format files

```bash
# Format all files in package
raft fmt .

# Format single file
raft fmt path/to/file.dlf

# Check if files need formatting (for CI)
raft fmt --check .

# Verbose output
raft fmt -v .

# Quiet mode (errors only)
raft fmt -q .

# Point at an explicit manifest
raft fmt --manifest-path path/to/package.dlf
```

### Build to Turtle

```bash
# Build package to stdout
raft build .

# Build to file
raft build --output ontology.ttl .

# With custom base IRI
raft build --base-iri http://example.org/ontology .

# Exclude comments from the Turtle output
raft build --no-comments .

# Write rules as comments instead of emitting them into the Turtle
raft build --no-rules .

# Emit only the N3 rules
raft build --emit n3-rules .
```

`--emit` accepts `turtle` (default) or `n3-rules`.

### Manage declarations

```bash
# Add a concept
raft add concept general.specific.MyConcept --sub Thing --has "name:string"

# Add a property
raft add property general.specific.hasOwner --domain Person --range Person \
    --domain-cardinality one --range-cardinality optional

# Modify a concept: add "age:int", remove the "name" has, drop parent "Pet"
raft modify concept general.specific.MyConcept --has age:int --has ~name --sub ~Pet

# Modify a property: change range and cardinality
raft modify property general.specific.hasOwner --range Organization --range-cardinality any

# Remove a concept (dry run shows what would be removed)
raft remove concept general.specific.MyConcept --dry-run

# Remove with force (even if used elsewhere)
raft remove property general.specific.hasOwner --force
```

Cardinalities accept `one`, `optional`, `any`, `some`, or numeric forms
(`N`, `N..M`, `N..*`). In `modify`, prefix a value with `~` to remove it
(`~name` removes a has, `~Parent` removes a sub, `~*` removes all subs).

### Inspect package metadata

```bash
# Print namespace, files, concepts, and properties as JSON.
# Intended for plugins to consume instead of re-loading the package.
raft metadata .
```

### Parse a single file (debugging)

```bash
# Show the AST
raft parse path/to/file.dlf

# Show tokens instead
raft parse --tokens path/to/file.dlf

# Emit JSON
raft parse --json path/to/file.dlf
```

## Available Commands

- `init` - Initialize a new Dolfin package (optionally seeded from Turtle)
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

Any command raft does not know about is forwarded, cargo-style, to an external
`raft-<command>` executable found on `PATH`. For example, `raft foo bar` runs
`raft-foo foo bar`, and `raft help foo` runs `raft-foo foo --help`. Discovered
plugins are listed under a `Plugins:` section in `raft --help`. Plugins can
read the current package via `raft metadata`.

## Lint Rule Categories

Raft's linter includes rules in the following categories:

- **naming**: Naming conventions (PascalCase, snake_case, etc.)
- **ordering**: Declaration and member ordering
- **unused**: Unused elements (prefixes, variables)
- **semantic**: Semantic correctness (unresolved types, circular inheritance)
- **style**: Code formatting style (trailing whitespace, blank lines)
- **package**: Package-level checks (missing version)

Run `raft check --list-rules` to see all available rules.

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
