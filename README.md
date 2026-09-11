# doco

`doco` is a file-backed Rust CLI for managing current project documentation and one-off change packages for coding agents. It does not run models or commit, release, or roll back code.

## Install

Requires Rust 1.85 or later.

```sh
cargo install --path . --locked
```

## Quick start

Run `doco` from the target project root:

```sh
# Initialize integrations for one or more agents.
doco init --agent codex --agent pi

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

A doco change is optional for implementation and is not an implementation-history log: archive retains only the proposal. Routine behavior fixes and implementation-detail edits may proceed without one unless tracking is explicitly requested. Use Git, pull requests, or release notes for implementation history, and still update current architecture/specs whenever their documented facts or contracts change.

Use `--root <path>` to target another project. Use `doco --help` or `doco <command> --help` for the complete command reference. In automation, pass explicit arguments and `--no-interactive`.

## Development

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
```

## Documentation

- [Documentation guide](doco/README.md)
- [Architecture and failure recovery](doco/architecture.md)
- [CLI output and interaction contract](doco/specs/cli.md)
- [Machine-readable document format](doco/specs/document-format.md)
