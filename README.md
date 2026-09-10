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

# Create and inspect a change package.
doco new bounded-event-queue
doco context bounded-event-queue
doco check bounded-event-queue

# Complete the accepted work, then preview and archive it.
doco complete bounded-event-queue
doco archive bounded-event-queue --dry-run
doco archive bounded-event-queue
```

`doco new` creates only the package skeleton. A developer or agent must fill in the proposal, implementation plan, and task list before doing the work.

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
