# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build          # compile
cargo test           # run all tests
cargo test <name>    # run a single test by name substring
cargo clippy         # lint
cargo fmt            # format
```

## Architecture

`drainlib` is a Rust implementation of the **DRAIN** log parsing algorithm — an online algorithm that clusters free-text log lines into templates by maintaining a fixed-depth prefix tree.

### Core types (`src/lib.rs`)

- **`DrainParser`** — the stateful parser. Holds the prefix tree (`root: Node`), the flat cluster list (`clusters: Vec<LogCluster>`), and tuning parameters.
- **`LogCluster`** — a discovered template: `template` is a token list where variable positions are replaced with `"<*>"`, `id` is a monotone counter, `size` is the number of log lines merged into it.
- **`Node`** — recursive tree enum: `Internal(HashMap<String, Node>)` for branch nodes, `Leaf(Vec<usize>)` for cluster-index lists at the leaves.

### Parsing pipeline (`DrainParser::parse`)

1. **Preprocess** — split on whitespace; replace any token matching a header filter regex (digits, hex) with `"<*>"`.
2. **Tree traversal** — descend `depth` levels: level 1 keys on log length, levels 2…depth key on the first tokens (wildcarding to `"<*>"` once `max_children` is reached).
3. **Cluster search** — at the leaf, compute token-level similarity against each candidate cluster; pick the best if ≥ `sim_threshold`.
4. **Update or create** — merge the new line into the matched cluster (positions that differ become `"<*>"`) or create a new `LogCluster`.

### Key invariant

`clusters` is append-only and indexed by position; `Leaf` nodes hold indices into this `Vec`. Cluster retrieval after `parse` returns a reference into `self.clusters`, so the borrow checker enforces that no mutation happens while the reference is live.

### Tuning parameters

| Parameter | Effect |
|---|---|
| `depth` | Tree depth; higher = more selective routing before cluster search |
| `sim_threshold` | Fraction of matching tokens required to merge into an existing cluster |
| `max_children` | Max distinct token keys per internal node before wildcarding |
