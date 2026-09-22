# doco

`doco` is a file-backed Rust CLI for managing current project documentation and one-off change packages for coding agents. It does not run models or commit, release, or roll back code.

## Install

Download a prebuilt archive from [GitHub Releases](https://github.com/gloridifice/doco/releases) for Linux x64 (glibc), Windows x64, or macOS Intel/Apple Silicon. Extract it and put `doco` (`doco.exe` on Windows) on your `PATH`. Each release includes `SHA256SUMS` for verifying the archives.

Building from source requires Rust 1.85 or later.

```sh
cargo install --path . --locked
```

## See it in action

```
You: I wanna optimize cli's new/complete/archive/list commands speed. I think its better to add a hot cache. Give me a desgin.
AI:  [Design]
You: ok, create doco change
AI:  doco new optimize-cli-commands
    - proposal.md
    - work/
      - implement.md
      - tasks.md
      - specs/          # Optional target contracts, authored as needed
        - cache.md
> Switch to cheaper model
You:  Start to implement
```

## Quick start

Run `doco` from the target project root:

```sh
# Initialize one or both integration targets.
doco init
```

## Commands

```
# Refresh installed skill bundles and managed instruction entries after upgrading doco.
doco update --dry-run
doco update

# Rebuild the local change index cache (never committed; see the note below).
doco fix --dry-run
doco fix

# Create and inspect a full change package.
doco new bounded-event-queue
doco context bounded-event-queue
doco check bounded-event-queue

# Or track a bounded change with proposal.md only.
doco new fix-empty-output --proposal-only

# Complete the accepted work, then preview and archive it.
doco complete bounded-event-queue
doco archive bounded-event-queue --dry-run
doco archive bounded-event-queue
```

`doco new` creates only a package skeleton. The default full package requires a proposal, implementation plan, and task list. `--proposal-only` creates just a marked `proposal.md` for a tracked change whose behavior and acceptance need no separate design or dependent task breakdown.

For more complex full changes, add optional `work/specs/<capability>.md` target contracts using the installed skill's `templates/spec.md`. Subdirectories are supported. Describe requirements and acceptance scenarios there, and link them from implementation plans and tasks instead of duplicating them. `new` does not create the directory or placeholder specs; small changes need neither.

`context` automatically discovers these specs, and `check` validates nonempty bodies, template residue, references and explicit blockers. Work specs describe intended changes, not current project facts: merge delivered contracts into `doco/specs/` before completion. Complete/reopen preserve work specs; archive/cancel delete them with the rest of `work/`. Proposal-only changes still cannot contain `work/`.

A doco change is optional for implementation and is not an implementation-history log: archive retains only the proposal. Routine behavior fixes and implementation-detail edits may proceed without one unless tracking is explicitly requested. Use Git, pull requests, or release notes for implementation history, and still update current architecture/specs whenever their documented facts or contracts change.

`--agent most` installs `.agents/skills/doco` with an `AGENTS.md` entry; `--agent claude` installs `.claude/skills/doco` with a `CLAUDE.md` entry. `doco update` refreshes only complete integrations already installed at those locations.

Change lookups use a disposable cache at `doco/tmp/changes-index.csv`. It is ignored through `doco/.gitignore`, never committed, and machine-local: after cloning a project that already uses doco, `doco init` or `doco fix` rebuilds it, and every other command works without it. Only `init` and `fix` write it outside a successful lifecycle command; read-only commands and dry runs never write it.

Use `--root <path>` to target another project. Use `doco --help` or `doco <command> --help` for the complete command reference. In automation, pass explicit arguments and `--no-interactive`.

## Development

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked

# Explicit scale check for the local change index cache; ignored by default.
cargo test --release --test scale -- --ignored --nocapture
```

### Releases

Set `[package].version` in `Cargo.toml`, refresh `Cargo.lock` with `cargo check`, and commit both before pushing a matching tag, for example:

```sh
git tag v0.1.0
git push origin v0.1.0
```

[Release CI](.github/workflows/release.yml) accepts stable `vX.Y.Z` tags only and rejects versions that differ from `Cargo.toml`. It runs formatting, Clippy, tests, and release builds for all four targets before publishing a GitHub Release with generated notes, binary archives, and SHA-256 checksums. Windows archives use `.zip`; Linux and macOS use `.tar.gz`. Each archive contains the executable and this README. The workflow does not publish to crates.io.

### Command performance

The local change index removes the cost that grows with the number of change
packages. Measured with the release binary on Windows 11 / Intel i7-10700, on a
temporary project holding N minimal archived packages plus one valid target
package, median of 2–3 runs in seconds. Every column except the last is a warm
run (valid cache); the last one is cold (cache deleted, i.e. the old behavior of
enumerating every change directory on each command):

| Command | 0 changes | 100 | 1,000 | 10,000 | 10,000, cold |
|---|---:|---:|---:|---:|---:|
| `doco new` | 0.082 | 0.083 | 0.080 | 0.081 | 6.92 (85×) |
| `doco check` | 0.058 | 0.054 | 0.062 | 0.059 | 6.29 (107×) |
| `doco complete` | 0.100 | 0.097 | 0.123 | 0.100 | 12.50 (125×) |
| `doco list` | 0.035 | 0.033 | 0.053 | 0.053 | 6.95 (131×) |
| `doco list --archived` | 0.032 | 0.215 | 1.874 | 17.351 | 22.91 (1.3×) |
| `doco archive` | 0.117 | 0.830 | 8.404 | 69.865 | 92.68 (1.3×) |
| `doco fix` (rebuild) | 0.046 | 0.127 | 0.779 | 6.489 | — |


`cargo test --release --test scale -- --ignored --nocapture`.

## Documentation

- [Documentation guide](doco/README.md)
- [Architecture and failure recovery](doco/architecture.md)
- [CLI output and interaction contract](doco/specs/cli.md)
- [Machine-readable document format](doco/specs/document-format.md)
- [Local change index cache](doco/specs/change-index-cache.md)
