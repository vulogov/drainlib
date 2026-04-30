# drainlib

A Rust implementation of the **DRAIN** log parsing algorithm — an online algorithm that clusters free-text log lines into templates by maintaining a fixed-depth prefix tree.

DRAIN processes each log line in a single pass and immediately assigns it to a cluster (or creates a new one), making it suitable for streaming log pipelines and real-time analysis.

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
drainlib = { path = "." }
```

```rust
use drainlib::DrainParser;

let mut parser = DrainParser::new(4, 0.5, 100);

let c = parser.parse("user alice logged in from 192.168.1.10");
println!("template: {}", c.template.join(" "));
// template: user alice logged in from <*>

let c = parser.parse("user bob logged in from 10.0.0.5");
println!("template: {}", c.template.join(" "));
// template: user <*> logged in from <*>

for cluster in parser.clusters() {
    println!("[{}] size={} {}", cluster.id, cluster.size, cluster.template.join(" "));
}
```

Run the bundled demo:

```
cargo run --example demo
```

Sample output:

```
--- 4 unique templates discovered ---

  [0] size=  5  <*> INFO <*> <*> <*> in <*> <*>
  [1] size=  2  <*> ERROR request to <*> failed after <*> retries
  [2] size=  3  <*> WARN disk usage at <*> percent on <*>
  [3] size=  2  <*> INFO connection <*> closed
```

## API

### `DrainParser`

```rust
pub fn new(depth: usize, sim_threshold: f64, max_children: usize) -> DrainParser
```

Creates a new parser. See [Tuning parameters](#tuning-parameters) for guidance on values.

```rust
pub fn parse(&mut self, line: &str) -> &LogCluster
```

Processes one log line. Returns a reference to the cluster it was merged into (existing or newly created). The returned reference borrows the parser; drop it before calling `parse` again.

```rust
pub fn clusters(&self) -> &[LogCluster]
```

Returns all discovered clusters in creation order.

---

### `LogCluster`

```rust
pub struct LogCluster {
    pub id: usize,              // monotone creation counter
    pub size: usize,            // number of log lines merged into this cluster
    pub template: Vec<String>,  // token list; variable positions hold "<*>"
}
```

`template` is updated in place as new lines are merged: any position where the new line differs from the current template is replaced with `"<*>"`.

## Tuning parameters

| Parameter | Type | Effect |
|---|---|---|
| `depth` | `usize` | Prefix-tree depth. Higher values route more selectively before cluster search, reducing false merges at the cost of more clusters. Must be ≥ 3. |
| `sim_threshold` | `f64` (0–1) | Minimum fraction of matching tokens required to merge a line into an existing cluster. Lower values merge more aggressively. Typical range: `0.4`–`0.7`. |
| `max_children` | `usize` | Maximum distinct token keys per internal node. Once reached, new tokens are routed under a shared `"<*>"` wildcard child, capping tree fan-out. |

### Recommended starting point

```rust
DrainParser::new(4, 0.5, 100)
```

Increase `sim_threshold` (e.g. `0.7`) if unrelated log patterns are being merged. Decrease `depth` (e.g. `3`) if lines with the same structure but different leading tokens are ending up in separate clusters.

## How it works

Each call to `parse` runs four steps:

1. **Preprocess** — split on whitespace; replace any token containing digits or a hex prefix (`0x…`) with `"<*>"`.
2. **Tree traversal** — descend the prefix tree: level 1 branches on token count, levels 2…`depth` branch on the first tokens (collapsing to `"<*>"` once `max_children` is reached at a node).
3. **Cluster search** — at the leaf, compute token-level similarity against each candidate cluster; select the best if its score meets `sim_threshold`.
4. **Update or create** — merge the line into the matched cluster (positions that differ become `"<*>"`) or append a new `LogCluster`.

The cluster list is append-only; leaf nodes in the tree hold indices into it.

## Development

```bash
cargo build       # compile
cargo test        # run all tests
cargo clippy      # lint
cargo fmt         # format
```
