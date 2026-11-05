# Development and language adapters

fmt-sync keeps project boundaries and process execution shared, while each language owns its tool and configuration rules. Python, Go, Rust and TypeScript are working examples of the extension contract. Java is not implemented yet; the checklist below describes how to add it.

## Architecture

| Location | Responsibility |
| --- | --- |
| `src/cli.rs` | Parse commands and render command-level errors. |
| `src/config.rs` | Load and validate `fmt-sync.toml`; obtain accepted version keys from registered tools. |
| `src/language.rs` | Stable language names, aliases and the exhaustive language-to-adapter registry. |
| `src/orchestrator.rs` | Discover one project scope and execute configured languages in order. |
| `src/adapters/mod.rs` | `LanguageAdapter` contract and the narrow `RunContext`. |
| `src/adapters/<language>.rs` | Language commands, target selection, tool descriptors, native defaults and validation. |
| `src/adapters/rust/project.rs` | Cargo package discovery and Rust-specific formatting scope proof. |
| `src/scope.rs`, `src/paths.rs` | Project boundaries, file scans, ignore normalization and link handling. |
| `src/native.rs` | Shared native configuration/preset file policy and JSON/JSONC parsing. |
| `src/tools/mod.rs` | Executable lookup, process invocation and exact version checking. |
| `src/tools/hints.rs` | Existing installation advice, reusable by tool descriptors. |
| `src/runner.rs` | Single/batched stages, command-length limits, output collection and result classification. |
| `src/execution.rs` | `RunMode`, `StageKind`, `Status` and `StageResult`; no terminal output. |
| `src/report.rs` | Terminal rendering and the aggregate exit code. |
| `src/update.rs` | Independent Cargo-based self-update workflow. |

Runtime flow: CLI → config → orchestrator → scope → selected adapter → runner → report.

Initialization flow: CLI → init → selected adapter's native configuration/conflict rules → shared preflight and writes.

The runner, native file policy, scope and tools do not import adapters or match on a language. Add language-specific behavior inside an adapter; move a helper into shared infrastructure only when its contract is independent of a particular tool.

## Adapter contract

```rust
pub trait LanguageAdapter: Sync {
    fn native_config(&self) -> Option<NativeConfigSpec> { None }
    fn tools(&self) -> &'static [ToolSpec];
    fn check_init_conflicts(&self, root: &Path) -> Result<()> { Ok(()) }
    fn execute(&self, context: &RunContext<'_>, mode: RunMode)
        -> Result<Vec<StageResult>>;
}
```

Use a stateless adapter, as in `Typescript` or `Python`. `RunContext` exposes the canonical project root, scope and tool version requirements; it does not pass the complete application configuration into a tool.

- `native_config()` describes candidate filenames, preset filenames, an optional default and a content validator. Return `None` if the language needs no native settings.
- Validators receive `ConfigSource::Project` or `ConfigSource::Preset`, and return whether the file actually contains relevant settings. For example, a `pyproject.toml` without `[tool.ruff]` is valid but irrelevant.
- Project configurations take precedence over presets and built-in defaults. Validate every existing candidate, even a lower-priority one. Presets must reject external configuration inheritance before copying; keep native configuration and preset rules in the adapter.
- `check_init_conflicts()` handles overlapping tooling. The shared initializer preserves files, checks all selected languages before writing, and retains TOML comments.
- `tools()` lists every tool descriptor used by the adapter, including separately versioned components. Config automatically accepts their `VersionSpec.key` values in `[tool-versions]`; do not maintain a second whitelist.
- `execute()` returns setup failures through `Err` only when the language cannot proceed at all. Turn a stage-specific failure into `StageResult::error` and continue independent stages. Other languages still execute.

## Tools and stages

`ToolSpec` defines executable name, optional `VersionSpec`, installation advice and optional local resolver. A local resolver returns `Ok(None)` to fall back to PATH, `Ok(Some(tool))` to select a project tool, or `Err` for an unusable local installation. Use `tools::local_executable` to preserve executable/shim paths without following their target names.

Use `context.tool(&SPEC)` to resolve a tool and validate its optional exact version requirement. For components of the same executable, use `context.versioned(existing_tool.clone(), &COMPONENT_SPEC)`; Rust demonstrates Cargo, rustfmt and Clippy. A descriptor without version metadata does not accept a separate version key; Go reports gofmt with its Go toolchain version.

Invoke processes with argument arrays. Never construct shell command strings or use a fallback that downloads tools. Local JavaScript entry points can use `Tool.prefix` to run through Node; TypeScript demonstrates this without npx.

Each invocation supplies a named command and outcome policy:

```rust
let command = StageCommand {
    language,
    stage: StageKind::Lint,
    tool: &tool,
    args: &args,
    policy: OutcomePolicy::findings(&[1]),
};
let result = runner::run_batched(context.root(), &command, &paths);
```

Use `runner::run` for a project/package command and `run_batched` for explicit file/package lists. The latter prefixes relative paths with `.` and limits argument batches for Windows. An empty target list should produce `StageResult::skipped` in the adapter rather than invoking a tool on the whole working directory.

Choose finding exit codes from the actual tool's contract. Unlisted nonzero codes and process launch failures are errors. For tools like `gofmt -l`, set `StdoutPolicy::Findings` explicitly: successful nonempty stdout then counts as a finding. Preserve native diagnostics. Cargo's exit 101 cannot precisely distinguish denied lints from compilation errors; the Rust adapter preserves that limitation.

Exit 0 means passed/skipped, 1 means findings, and 2 means setup/environment/execution errors. Errors take precedence across batches, stages and languages; later successes must not erase earlier failures.

## Project scope rules

- Work only within the canonical `context.root()` and the discovered scope. Treat scoped file paths as relative `PathBuf` values, including on Windows.
- `scope.files_with_extensions(...)` filters the boundary-controlled file list. `scope.scan(ScanPolicy::RespectGitignore)` additionally applies gitignore rules while retaining fmt-sync's boundaries.
- Ignore entire configured subproject directories and directories with their own `fmt-sync.toml`. Do not traverse links/reparse points or built-in excluded directories.
- Do not pass an unrestricted root, `./...`, `--all` or `--workspace` to a recursively scanning tool and assume explicit-file boundaries still apply. Prove the tool's targets are within scope, or reject unsupported layouts before a write.
- Build-tool dependencies may still compile. fmt-sync does not rewrite dependency relationships or filter diagnostics to hide excluded dependencies.
- Go formatting respects gitignore, while vet selects complete packages without filtering individual source files by gitignore. Rust discovers package metadata separately from the formatting-only module proof. Preserve these differences.

## Adding a language, for example Java

1. Decide the native tooling and supported project unit first: formatter/linter, required runtime, project-local resolution, supported build layout and configuration inheritance. Choosing Maven, Gradle or a standalone formatter is part of implementing Java support, not a shared-runner decision.
2. Create `src/adapters/java.rs` with a stateless `Java` adapter implementing the contract above. Keep Java-specific tools, config validation, defaults, conflict checks and target discovery there; add submodules if a parser or resolver grows large.
3. Declare the module in `src/adapters/mod.rs`. Add `Java` to `Language` in `src/language.rs`, its display name, any serde/clap aliases, and its arm in `Language::adapter()`. `Language::all()` comes from the derived enum values, so no separate list needs updating.
4. List Java's descriptors in `tools()` and implement both modes. Use typed stages and explicit outcome policies. A source formatter may accept scoped `.java` files; a Maven/Gradle command needs a separate proof of its recursive project boundaries. Format-only validation must not block an otherwise independent lint stage.
5. Add controlled external-tool behavior to `tests/support/tool.rs` and behavioral tests to `tests/cli.rs` / `tests/execution.rs`: initialization preservation, config precedence, paths with spaces/Unicode, project-local tools, ignored/independent projects, missing tools, versions, findings/errors and stage isolation. Cover each Java-specific boundary rule.
6. Add real projects under `tests/projects/java/`, corrected files under `tests/project_fixes/java/`, and a case in `tests/project_cases.rs`. Verify broken check → format → lint diagnostics → corrected check → idempotence, with excluded files unchanged.
7. Add any additional real-tool regression in `tests/smoke.rs`, install the tools in `.github/workflows/ci.yml`, and update README support/install/default tables and `tests/README.md`.

Normal additions should not require Java-specific branches in `config`, `init`, `native`, `scope`, `tools`, `runner`, `report` or the orchestrator. Extend a generic contract only if the new tool demonstrates a shared capability that cannot be expressed by the existing adapter hooks. Document that contract and test its observable behavior.

## Verification

```sh
cargo fmt --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

Run actual integrations with the required tools available:

```sh
cargo test --locked --all-targets -- --include-ignored
```

`FMT_SYNC_SMOKE_TOOL_PATH` can supply additional Ruff/Biome/tool directories. See `tests/README.md` for prerequisites. Real self-update tests use temporary repositories and install roots; they must never update the developer's installed executable.

For behavior changes, first add a regression that fails for the expected reason, then implement the fix. For mechanical moves, preserve the existing integration contract. Test public behavior rather than private module structure or documentation text.
