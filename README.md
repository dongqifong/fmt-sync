# fmt-sync

Format and lint Python, Go, Rust, and TypeScript projects with the same commands. Supports Windows and Linux.

## Installation

Install [Rust](https://rust-lang.org/tools/install/), then run from this repository:

```sh
cargo install --path . --locked
```

Install the tools required for your project's languages:

| Language | Installation | Location |
| --- | --- | --- |
| Python | `uv add --dev ruff` (project)<br>or `uv tool install ruff` (shared) | Project `.venv` or user tool environment |
| Go | [Install Go](https://go.dev/doc/install) and add `go` and `gofmt` to PATH | Shared Go toolchain, including gofmt and go vet |
| Rust | Run `rustup component add rustfmt clippy` in the project directory | Active Rust toolchain, shared by projects using it |
| TypeScript | `npm install --save-dev --save-exact @biomejs/biome` (requires Node.js/npm) | Project `node_modules` |

Missing tools trigger installation instructions; fmt-sync does not install them automatically.
Ruff and Biome are resolved from the project first, then PATH. Cargo/rustup selects the Rust toolchain.

### AI agent skill

Install the skill with the [skills CLI](https://github.com/vercel-labs/skills) (Node.js 22.20+):

```sh
# Codex
npx skills add dongqifong/fmt-sync --skill fmt-sync --agent codex

# Claude Code
npx skills add dongqifong/fmt-sync --skill fmt-sync --agent claude-code
```

Add `-g` to install for all projects. The skill teaches the agent to run checks only when requested, not after every code edit. It does not install fmt-sync or its native tools.

Ask the agent to check your project, or invoke `$fmt-sync` in Codex / `/fmt-sync` in Claude Code.

## Usage

Run from the project directory:

```sh
fmt-sync init --lang python,typescript
fmt-sync format
fmt-sync check
```

- `init`: Create configuration, preserving existing native settings.
- `format`: Format and update files using the project's settings.
- `check`: Check formatting and lint without modifying source files.

Only `init` takes languages. Both `format` and `check` read `fmt-sync.toml` from the current directory.

### Project configuration

```toml
schema-version = 1
languages = ["python", "typescript"]
ignore = ["legacy/", "experimental/"]
```

Each project has its own TOML file, with no parent configuration inheritance. `ignore` lists subproject directories relative to the TOML file. Entire directories are skipped; individual files are not supported.

For Go and Rust, run from the project directory containing `go.mod` or `Cargo.toml`, respectively.

### Custom presets

Place native configuration files in a preset directory, then reference it:

```sh
fmt-sync init --lang python,typescript --preset ../my-preset
```

For example, a preset can contain `ruff.toml`, `rustfmt.toml`, and `biome.json`. Without a preset, or when it lacks a language's configuration, the defaults below apply. After initialization, edit the project's native configuration files directly.

## Defaults

### Python — Ruff

- **Configuration:** `ruff.toml`.
- **Formatting:** 4 spaces, line width 88; organize imports before formatting.
- **Lint:** `E4`, `E7`, `E9`, `F`, `I` (basic errors, unused variables/imports, and import sorting).
- **Existing projects:** Add `"I"` to `extend-select` under `[lint]` to enable sorting. `check` does not modify files.
- **Cache:** Disabled; no `.ruff_cache` is created.

### Go — gofmt + go vet

- **Configuration:** None required.
- **Formatting:** Standard gofmt style with tabs.
- **Lint:** Default go vet checks.
- **Cache:** Uses Go's shared system cache by default.

### Rust — rustfmt + Clippy

- **Configuration:** `rustfmt.toml`.
- **Formatting:** 4 spaces, line width 100.
- **Build output:** Retains Cargo artifacts (default: `target/`) for reuse. Custom Cargo directories are respected.
- **Lint:** Default Clippy checks; warnings cause failure.

### TypeScript — Biome

- **Configuration:** `biome.json`.
- **Formatting:** Tabs, line width 80, double quotes.
- **Lint:** Biome's recommended rules.
- **Cache:** Normal runs do not create a project cache.
