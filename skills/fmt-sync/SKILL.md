---
name: fmt-sync
description: >-
  Use when the user explicitly requests fmt-sync, formatting/lint checks, or formatting or initialization of a fmt-sync project. Do not use for ordinary code edits, general code reviews, commit messages, or automatic post-edit verification.
---

# fmt-sync

Use fmt-sync for the action and project the user requested. It coordinates Python/Ruff, Go/gofmt and go vet, Rust/rustfmt and Clippy, and TypeScript/Biome.

## Execution gate

**Run fmt-sync only when the active user task explicitly requests the corresponding operation.**

- A formatting/lint check request authorizes `check`, not source changes.
- Run `format` or `init` only when the user requests formatting or initialization.
- A direct `$fmt-sync` (Codex) or `/fmt-sync` (Claude Code) invocation defaults to `check` when no other action is specified.
- A usage question or mention of fmt-sync is not a request to execute it.
- Code edits, refactors, completion rituals, generic reviews and commit preparation do not authorize checks. An earlier completed check is not standing permission to check later edits.

When the user requests fixes and rechecks, perform those actions within that task. Otherwise report findings without automatically formatting, fixing or rerunning checks. Do not add hooks, watchers or post-edit automation.

## Choose the project

Use the requested project's directory as the working directory. `format` and `check` require `fmt-sync.toml` there; they do not search parent directories or dispatch to child projects. If it is missing, report the missing setup rather than running `init` or checking a different project. Do not walk a monorepo to check unrelated projects.

Read that TOML for languages and ignored subprojects. One project can contain multiple languages. Ignore entries are relative subproject directories, not individual files. For Go or Rust, use the project containing `go.mod` or `Cargo.toml`.

## Commands

| User request | Command |
| --- | --- |
| Check formatting and lint | `fmt-sync check` |
| Format source, including enabled Python import sorting | `fmt-sync format` |
| Initialize configuration | `fmt-sync init --lang python,typescript` (choose the project's actual languages) |

Only `init` accepts `--lang`. Use `--preset <directory>` only with `init` when requested. Preserve existing native configurations and let the CLI resolve project-local tools before PATH.

If fmt-sync is missing, report it and point to [installation instructions](https://github.com/dongqifong/fmt-sync#installation). If a native tool is missing, relay the CLI's installation guidance. Do not install tools or dependencies automatically.

## Report the result

Report the project, command, actual exit code and relevant diagnostics:

- `0`: all required stages passed or were skipped; mention skipped stages, especially if no source was checked.
- `1`: formatting or lint findings; summarize them without silently fixing source.
- `2`: configuration, missing tool, version or environment error; report the blocker, not a successful check.

Use exit codes rather than terminal colors. Leave source unchanged for check-only requests, and do not delete caches or Rust `target/` artifacts as part of checking.
