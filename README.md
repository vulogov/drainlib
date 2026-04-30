# drainlib

A Rust implementation of the **DRAIN** log parsing algorithm — an online algorithm that clusters free-text log lines into templates by maintaining a fixed-depth prefix tree.

DRAIN processes each log line in a single pass and immediately assigns it to a cluster (or creates a new one), making it suitable for streaming log pipelines and real-time analysis.

## Features

- Online clustering — no pre-training, handles unbounded log streams
- Configurable masking — replace noisy tokens (numbers, IPs, UUIDs, …) before clustering
- Change detection — know whether a parse event created a new cluster, refined an existing template, or matched without change
- JSON persistence — save and restore the full parser state (clusters + prefix tree + mask config)

## Quick start

```toml
[dependencies]
drainlib = { path = "." }
```

```rust
use drainlib::{ChangeType, DrainParser};

let mut parser = DrainParser::new(4, 0.5, 100);

let r = parser.parse("user alice logged in");
println!("{:?} — {}", r.change_type, r.template.join(" "));
// New — user alice logged in

let r = parser.parse("user bob logged in");
println!("{:?} — {}", r.change_type, r.template.join(" "));
// Updated — user <*> logged in

let r = parser.parse("user carol logged in");
println!("{:?} — {}", r.change_type, r.template.join(" "));
// None — user <*> logged in
```

Run the bundled examples:

```
cargo run --example demo     # streaming parse with change-type labels
cargo run --example persist  # save state to JSON, reload, continue parsing
```

## API

### `DrainParser`

```rust
pub fn new(depth: usize, sim_threshold: f64, max_children: usize) -> DrainParser
```

Creates a parser with the default mask set (digit tokens and hex addresses).  
Use [`DrainParserBuilder`](#drainparserbuilder) for custom masking.

```rust
pub fn parse(&mut self, line: &str) -> ParseResult<'_>
```

Processes one log line. Returns a [`ParseResult`] that borrows the parser; drop it before the next `parse` call.

```rust
pub fn clusters(&self) -> &[LogCluster]
```

All discovered clusters in creation order.

```rust
pub fn to_json(&self)        -> serde_json::Result<String>
pub fn to_json_pretty(&self) -> serde_json::Result<String>
pub fn from_json(s: &str)   -> Result<DrainParser, Box<dyn Error>>
pub fn save(path)            -> Result<(), Box<dyn Error>>
pub fn load(path)            -> Result<DrainParser, Box<dyn Error>>
```

Serialise and restore the complete parser state — clusters, prefix tree, tuning parameters, and mask patterns — as JSON.

---

### `ParseResult<'a>`

```rust
pub struct ParseResult<'a> {
    pub cluster: &'a LogCluster,
    pub change_type: ChangeType,
}
```

Implements `Deref<Target = LogCluster>`, so `r.id`, `r.size`, and `r.template` are directly accessible.

---

### `ChangeType`

```rust
pub enum ChangeType {
    New,      // a fresh cluster was created
    Updated,  // merged into existing cluster; template gained ≥1 new "<*>"
    None,     // merged into existing cluster; template unchanged
}
```

---

### `LogCluster`

```rust
pub struct LogCluster {
    pub id: usize,              // monotone creation counter
    pub size: usize,            // number of log lines merged into this cluster
    pub template: Vec<String>,  // token list; variable positions hold "<*>"
}
```

---

### `DrainParserBuilder`

Fluent builder for custom masking configurations.

```rust
let mut parser = DrainParserBuilder::new()
    .depth(4)
    .sim_threshold(0.5)
    .max_children(100)
    // append masks on top of the defaults (digits, hex)
    .add_mask(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}")  // IPv4
    .add_mask(r"[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}")  // UUID
    .build()?;
```

`build()` returns `Err(regex::Error)` if any pattern is invalid.  
Use `.mask_patterns(vec![…])` to replace the default set entirely.

## Persistence example

```rust
use drainlib::DrainParser;

// Train on some log lines.
let mut p = DrainParser::new(4, 0.5, 100);
p.parse("user alice logged in");
p.parse("user bob logged in");

// Persist.
p.save("/tmp/drain.json")?;

// Reload in another process / after restart.
let mut p2 = DrainParser::load("/tmp/drain.json")?;

// Parsing continues seamlessly — clusters and tree are preserved.
let r = p2.parse("user carol logged in");
assert_eq!(r.id, 0);   // same cluster as alice and bob
assert_eq!(r.size, 3);
```

## Tuning parameters

| Parameter | Type | Effect |
|---|---|---|
| `depth` | `usize` (≥ 3) | Prefix-tree depth. Higher = more selective routing before cluster search, fewer false merges, more clusters. |
| `sim_threshold` | `f64` (0–1) | Minimum fraction of matching tokens to merge into an existing cluster. Typical range: `0.4`–`0.7`. |
| `max_children` | `usize` | Max distinct token keys per internal node. Once reached, new tokens are collapsed under a `"<*>"` wildcard child. |

**Recommended starting point:** `DrainParser::new(4, 0.5, 100)`

Increase `sim_threshold` if unrelated patterns are merging. Decrease `depth` if lines with the same structure but different leading tokens land in separate clusters.

## How it works

Each call to `parse` runs four steps:

1. **Preprocess** — split on whitespace; replace any token that matches a mask pattern with `"<*>"`.
2. **Tree traversal** — descend the prefix tree: level 1 branches on token count, levels 2…`depth` branch on the first tokens (collapsing to `"<*>"` at nodes that have reached `max_children`).
3. **Cluster search** — at the leaf, compute token-level similarity against each candidate; select the best if its score meets `sim_threshold`.
4. **Update or create** — merge the line into the matched cluster (positions that differ become `"<*>"`) or append a new `LogCluster`.

The cluster list is append-only; leaf nodes in the tree hold indices into it. The entire state is captured in a JSON snapshot for persistence.

## Development

```bash
cargo build       # compile
cargo test        # run all tests
cargo clippy      # lint
cargo fmt         # format
```
