# Tests

Run the default suite (native tools are simulated):

```sh
cargo test --locked --all-targets
```

Run every test, including actual tool integrations:

```sh
cargo test --locked --all-targets -- --include-ignored
```

The full suite requires Git, Rust/Cargo with rustfmt and Clippy, Ruff, Go/gofmt, and the native Biome executable on PATH. CI installs these tools on Windows and Linux. `FMT_SYNC_SMOKE_TOOL_PATH` can add tool directories for local runs.

| Coverage | What is verified |
| --- | --- |
| `cli.rs`, `execution.rs` | CLI validation, initialization, tool arguments, scope, errors and colors |
| `project_cases.rs` | Real Python, Go, Rust, TypeScript and mixed projects: init, broken check, format, lint diagnostics, corrected check, idempotence and unchanged excluded files |
| `smoke.rs` | Actual tool behavior, native configuration, import sorting, project boundaries and cache preservation |
| `update.rs` | Simulated release tags and installer outcomes, version ordering, colors, missing tools, custom install roots and Windows rollback/cleanup |
| `update_real.rs` | Actual Git and Cargo: successful upgrade, already-current version and compilation failure restoring the previous executable |

The real update tests create local version tags and redirect the official Git URL to their temporary repository using isolated Git configuration. Only the file Git protocol is allowed. Compilation, installation metadata and the installed executable are real; Cargo home, build output and installation paths are temporary. The user's installed fmt-sync and global configuration are not modified.
