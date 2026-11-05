# Real project test results — 2026-11-04

Five persistent source projects under tests/projects/ were exercised with actual Ruff, Go, Cargo/rustfmt/Clippy and Biome on Windows and Linux. Native executables were not mocked.

| Project | Deliberate lint issue | Windows | Linux |
| --- | --- | --- | --- |
| [python](projects/python/) | F401 unused import; F821 undefined name | PASS: [lint failure](.runs/windows-17280-1793799302595580800/python/_logs/04-check-formatted.log), [fixed success](.runs/windows-17280-1793799302595580800/python/_logs/06-check-fixed.log) | PASS: [lint failure](.runs/linux-2011-1793799322119432064/python/_logs/04-check-formatted.log), [fixed success](.runs/linux-2011-1793799322119432064/python/_logs/06-check-fixed.log) |
| [go](projects/go/) | Printf %d with a string argument | PASS: [lint failure](.runs/windows-17280-1793799302595580800/go/_logs/04-check-formatted.log), [fixed success](.runs/windows-17280-1793799302595580800/go/_logs/06-check-fixed.log) | PASS: [lint failure](.runs/linux-2011-1793799322119432064/go/_logs/04-check-formatted.log), [fixed success](.runs/linux-2011-1793799322119432064/go/_logs/06-check-fixed.log) |
| [rust](projects/rust/) | clippy::iter_count on a slice | PASS: [lint failure](.runs/windows-17280-1793799302595580800/rust/_logs/04-check-formatted.log), [fixed success](.runs/windows-17280-1793799302595580800/rust/_logs/06-check-fixed.log) | PASS: [lint failure](.runs/linux-2011-1793799322119432064/rust/_logs/04-check-formatted.log), [fixed success](.runs/linux-2011-1793799322119432064/rust/_logs/06-check-fixed.log) |
| [typescript](projects/typescript/) | lint/suspicious/noDoubleEquals | PASS: [lint failure](.runs/windows-17280-1793799302595580800/typescript/_logs/04-check-formatted.log), [fixed success](.runs/windows-17280-1793799302595580800/typescript/_logs/06-check-fixed.log) | PASS: [lint failure](.runs/linux-2011-1793799322119432064/typescript/_logs/04-check-formatted.log), [fixed success](.runs/linux-2011-1793799322119432064/typescript/_logs/06-check-fixed.log) |
| [backend](projects/backend/) | Python F401 and TypeScript noDoubleEquals | PASS: [lint failure](.runs/windows-17280-1793799302595580800/backend/_logs/04-check-formatted.log), [fixed success](.runs/windows-17280-1793799302595580800/backend/_logs/06-check-fixed.log) | PASS: [lint failure](.runs/linux-2011-1793799322119432064/backend/_logs/04-check-formatted.log), [fixed success](.runs/linux-2011-1793799322119432064/backend/_logs/06-check-fixed.log) |

## Verified sequence for every project

1. init: exit 0; generate actual native defaults.
2. check before formatting: exit 1; deliberate source problems detected.
3. format: exit 0; each primary source file actually changes.
4. check after formatting: exit 1; actual expected lint diagnostics remain.
5. Copy corrected source from tests/project_fixes/ and format: exit 0.
6. check corrected code: exit 0.
7. format again: exit 0; source stays byte-for-byte unchanged.

The excluded legacy/ project, original fixture sources and fmt-sync.toml remained unchanged. Freshly generated tool configuration produced no deprecation notices.

Original fixtures remain deliberately broken and unformatted. Working copies, corrected sources, full stdout/stderr logs and per-project result.json records remain under the linked run directories.

## Runtime issue discovered

Biome 2.6.4 reported a deprecation notice for the generated linter.rules.recommended field. A real-project assertion reproduced it before the change. The built-in configuration now omits this redundant field and uses the native recommended default, retaining the same lint detections without the deprecation notice.

## Reproduce

With the actual native tools available on PATH:

```sh
cargo test --locked --test project_cases -- --ignored --nocapture
```

For this checkout, the validation tools are also available under .scratch/. Windows PowerShell can use:

```powershell
$env:FMT_SYNC_SMOKE_TOOL_PATH = "$PWD/.scratch/venv/Scripts;$PWD/.scratch/biome/node_modules/@biomejs/cli-win32-x64"
cargo test --locked --test project_cases -- --ignored --nocapture
```

The project cases are included in the Windows/Linux CI workflow. Hosted CI has not been executed locally.

The existing ordinary suite, six actual-tool smoke tests, Clippy and release builds also passed on both locally tested platforms after the native-default adjustment.

## Full suite and CI — 2026-11-05

The current full suite passed locally on Windows (68 tests) and Ubuntu under WSL (69 tests, including one Unix-only symlink test), with no ignored tests:

```sh
cargo test --locked --all-targets -- --include-ignored
```

Both platforms used actual Ruff, Go/gofmt/go vet, Rust/Cargo/rustfmt/Clippy, and Biome for all five project fixtures and all 13 native-tool smoke tests. These cover initialization, failing checks, actual formatting changes, expected remaining lint diagnostics, successful corrected checks, repeated formatting, project exclusions and build/cache handling.

Three additional `update_real.rs` cases passed on both platforms: real Cargo installation of a newer local Git tag, an already-current tag without compilation, and a compilation error that preserves the old executable. The Git repository, URL rewrite configuration, Cargo home, installation root and build output are isolated in temporary directories; only file-protocol Git access is permitted.

The CI workflow now installs the native tools and includes all previously ignored integration cases. GitHub-hosted runner results will be available after the workflow is pushed to main.
