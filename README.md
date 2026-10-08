<h2 align="center">
doco
</h2>

<p align="center">
<a href="readme/zh_CN.md">简体中文</a>
</p>

`doco` is an agent-friendly documentation CLI. Inspired by Fission-AI's [OpenSpec](https://github.com/Fission-AI/openspec).

## Philosophy

Doco divides documentation into two categories:

- **Current-state documentation** describes the project as it is today, including its architecture, technical decisions, and specifications.
  - Doco treats code as useful current-state documentation. It guides agents to create a code index and write only the supporting documentation that is needed.
- **Change documentation** describes what a particular change will do, including its goals, specifications (optional), design (optional), and tasks.

Doco is flexible: most documents for a change are optional. You can ask an agent to write detailed documentation to support limited context windows or handoffs between agents. Or you can keep it simple and get the task done quickly, without spending unnecessary time and tokens on documentation.

In most cases, the doco skill helps the agent choose between a detailed change package and a lightweight one, so you do not need to make that decision yourself.

Doco also cares about CLI speed. See [Command performance](#command-performance) for benchmarks. Maintaining documentation should not slow an agent down.

## Quick start

Install with Cargo:

```
cargo install doco-cli
```

Then run the following command in your workspace:

```
doco init
```

### Update

```
cargo install doco-cli
```

Then run `doco update` in your workspace to update the skill.

### Build from source

You can also clone this project and build from source. Rust 1.85 or later is required:

```sh
cargo install --path . --locked
```

## Example interaction

```
You: I want to speed up the CLI's new/complete/archive/list commands. I think adding a hot cache would help. Give me a design.
AI:  [Design]
You: OK, create a doco change.
AI:  doco new optimize-cli-commands
    - proposal.md
    - work/
      - implement.md
      - tasks.md
      - specs/          # Optional target contracts, authored as needed
        - cache.md
> Switch to a cheaper model
You: Start implementing.
```

To see the current list of changes, run `doco list`.

## Nested libraries

Run `doco init` in a subdirectory of an initialized project to create a local
`doco/` library. Child libraries do not install skills or create `AGENTS.md` or
`CLAUDE.md`; they reuse the top-level workflow, leaving existing instructions intact.

Ordinary commands use the nearest ancestor library. Use `--root <directory>`
to select an exact library; `init` always targets the current or specified directory.
Change IDs, writes, caches and locks remain local to each library.

`doco list` shows active changes across the whole project family, including
siblings: current library first, then the remaining libraries in parent-before-child
order. Multi-library output adds a `PROJECT` column. `-c` includes completed changes;
`-a` includes all states. Nested Git roots and linked directories are excluded.
Run `doco update` at the top-level library to update the shared workflow.

## More about changes

Doco separates a change into `proposal.md`, which is retained after archiving, and a `work/` directory, which is kept only while working on the change. The `work/` directory contains optional `implement.md`, `specs/`, and `tasks.md` materials to help keep work consistent across sessions and models.

- `implement.md` records key implementation details, such as struct designs and complex algorithms.
- `tasks.md` is a trackable task list with detailed execution steps.
- `specs/` records concrete design specifications and constraints beyond the code, such as interaction behavior.

Most changes do not need the full set of `work/` files. Small fixes often need only `proposal.md` and `tasks.md`, or no doco change at all. The doco skill therefore guides agents not to create a spec for every task: small tasks can be handled directly.

## Command performance

The following benchmarks cover 0–10,000 changes on an Intel i7-10700. All times are in seconds.

| Command | 0 changes | 100 | 1,000 | 10,000 |
|---|---:|---:|---:|---:|
| `doco new` | 0.082 | 0.083 | 0.080 | 0.081 |
| `doco check` | 0.058 | 0.054 | 0.062 | 0.059 |
| `doco complete` | 0.100 | 0.097 | 0.123 | 0.100 |
| `doco list` | 0.035 | 0.033 | 0.053 | 0.053 |
| `doco list --archived` | 0.032 | 0.215 | 1.874 | 17.351 |
| `doco archive` | 0.117 | 0.830 | 8.404 | 69.865 |
| `doco fix` (rebuild cache) | 0.046 | 0.127 | 0.779 | 6.489 |

You can also run the benchmark yourself:

`cargo test --release --test scale -- --ignored --nocapture`.

## Documentation index

- [Documentation guide](doco/README.md)
- [Architecture and failure recovery](doco/architecture.md)
- [CLI output and interaction contract](doco/specs/cli.md)
- [Machine-readable document format](doco/specs/document-format.md)
- [Local change index cache](doco/specs/change-index-cache.md)
