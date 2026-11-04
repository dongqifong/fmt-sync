# Real project fixtures

These projects deliberately contain formatting and lint errors. All source files and test output are in English.

| Project | Actual tool | Deliberate lint issue |
| --- | --- | --- |
| `python/` | Ruff | `F401`: unused import; `F821`: undefined name |
| `go/` | go vet | `Printf` uses `%d` with a string argument |
| `rust/` | Clippy | `clippy::iter_count`: use slice `.len()` instead |
| `typescript/` | Biome | `noDoubleEquals`: `==` instead of `===` |
| `backend/` | Ruff and Biome | Unused Python import and TypeScript `==` |

## Run the actual tools

Install the native tools, then run from the repository root:

```sh
cargo test --locked --test project_cases -- --ignored --nocapture
```

The tests copy these real projects into `tests/.runs/<platform>-<run-id>/`, execute the built fmt-sync CLI, and keep every command's stdout, stderr and exit code in each project's `_logs/` directory. No native executables are mocked.

Each project must complete this sequence:

| Step | Expected exit |
| --- | --- |
| Initialize native defaults | 0 |
| Check deliberately broken source | 1 |
| Format | 0 |
| Check formatted source; original lint issues remain | 1 |
| Apply `tests/project_fixes/<project>/`, then format | 0 |
| Check corrected source | 0 |
| Format again; source must remain unchanged | 0 |

The excluded `legacy/` project must remain byte-for-byte unchanged. The original fixtures stay broken so you can inspect or rerun them.

The actual tool output must also contain no configuration deprecation notice for freshly generated defaults.

`FMT_SYNC_SMOKE_TOOL_PATH` can provide extra native tool directories to the test runner, using the platform PATH separator. Otherwise the runner uses your normal PATH and project-local executables.

## Try a fixture manually

From the repository root, install the CLI:

```sh
cargo install --path . --locked
```

Then, for example:

```sh
cd tests/projects/python
fmt-sync init --lang python
fmt-sync check
fmt-sync format
fmt-sync check
```

The last check should still report `F401` and `F821`: formatting does not fix lint errors. Manual formatting edits this fixture; use a copy if you want to preserve its initial formatting.
