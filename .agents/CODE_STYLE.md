# Code style and quality bar

This document is given to every coding agent alongside the spec. It defines
how the code should read, not just what it should do.

The bar: code that an expert Rust programmer would find **pleasant to read**.
Not just correct — composed, rhythmic, and visually clear.

---

## 1. File shape

A file should have a visible architecture when you glance at it. A reader
should be able to scroll through and understand the structure without reading
every line.

### Module sizing

- **Hard cap: 400 lines** (including tests). If a file approaches this, split.
- **Sweet spot: 150-300 lines.** A module should do one thing well.
- The right split is by concern, not by line count.
- Tests go in a separate file (`foo_tests.rs`), not inline, unless the module
  is under 100 lines.

### Visual layout of a file

A well-structured file reads top-down like a newspaper: the most important
things first, details later.

```rust
//! Hybrid BM25 + vector retrieval with reciprocal rank fusion.

// --- Imports (grouped, explicit) ---

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use tantivy::query::QueryParser;

use crate::index::store::FactStore;
use crate::index::embeddings::EmbeddingModel;
use crate::types::{AtomicFact, SearchMode, SearchResult};

// --- Public types ---

/// Orchestrates hybrid search across Tantivy and vector indexes.
pub struct HybridRetriever { ... }

// --- Public API ---

impl HybridRetriever {
    /// Create a retriever backed by the given stores.
    pub fn new(store: FactStore, index: TantivyIndex, embedder: EmbeddingModel) -> Self { ... }

    /// Search for facts matching a query, using the specified mode.
    pub fn search(&self, query: &str, mode: SearchMode, limit: usize) -> Result<Vec<SearchResult>> { ... }
}

// --- Private helpers ---

fn rrf_fuse(rankings: &[Vec<(i64, f32)>], k: f32) -> Vec<(i64, f32)> {
    ...
}
```

Notice:
- Section comments (`// --- ... ---`) create landmarks you can see while scrolling.
- Public items come before private items.
- Types before impls before free functions.

---

## 2. Imports

Imports are the first thing a reader sees. They signal whether the codebase
is curated or accumulated.

### Rules

- **No glob imports.** Never `use foo::*`. Never `use super::*`. Every symbol
  has an explicit origin.
- **Group imports** with blank lines between groups:
  1. `std`
  2. External crates
  3. Internal modules (`crate::`)
- **Sort alphabetically** within each group.
- **Import the type, not the module**, when you use the type directly:
  `use crate::types::AtomicFact;` not `use crate::types;` then `types::AtomicFact`.
- **Exception:** when a module is used as a namespace (many items),
  `use crate::index;` then `index::store::insert_fact(...)` is fine.

---

## 3. Functions

### Sizing

- **Hard cap: 60 lines.** If longer, extract helpers.
- **Sweet spot: 15-30 lines.**
- A function should be readable without scrolling.

### The outline pattern

Good functions read like an outline. The top level tells the story; details
are in well-named helpers.

**Good:**

```rust
pub fn ingest_window(
    store: &FactStore,
    backend: &dyn CompressorBackend,
    embedder: &EmbeddingModel,
    turns: &[ConversationTurn],
) -> Result<IngestSummary> {
    let candidates = backend.compress(turns)?;
    let mut summary = IngestSummary::default();

    for candidate in &candidates {
        let embedding = embedder.embed(&candidate.content)?;
        let similar = store.find_similar(&embedding, 5)?;
        let operation = backend.classify_crud(candidate, &similar)?;
        summary += apply_operation(store, embedder, candidate, operation)?;
    }

    Ok(summary)
}
```

You can read this in 5 seconds and understand the entire flow.

**Bad:**

```rust
pub fn ingest_window(...) -> Result<...> {
    // 40 lines of prompt formatting inline
    // 30 lines of JSON parsing inline
    // 50 lines of similarity search inline
    // 40 lines of CRUD classification inline
    // 60 lines of store update inline
}
```

### Naming

- Functions should be verbs: `compress_turns`, `find_similar`, `apply_operation`.
- Functions that return bool should read as questions: `is_processed`, `has_embedding`.
- Don't encode the return type in the name: `get_fact_or_none` → `find_fact`.

---

## 4. Types

### Struct sizing

- If a struct has more than 7 fields, consider whether it's doing too much.
- Group related fields into sub-structs.

### Use the type system

- Prefer enums over string constants. `Status::Active` not `"active"`.
- Prefer newtypes when semantics differ: `UserId(i64)` vs raw `i64`.
- Operations with distinct variants are enums, not strings.

### Error types

- The binary crate may use `anyhow` for top-level orchestration.
- Internal modules that could be extracted as libraries should use typed errors.
- Error variants should be specific enough to match on:

**Good:**

```rust
pub enum ConnectorError {
    SessionNotFound(PathBuf),
    MalformedEntry { path: PathBuf, line: usize, detail: String },
    Io(std::io::Error),
}
```

**Bad:**

```rust
anyhow::bail!("something went wrong: {}", detail)
// Every error is a string. Callers can't match on anything.
```

---

## 5. Error handling style

Error handling should be visually lightweight. The happy path should be
prominent; error paths should be concise.

**Good:**

```rust
let config = load_config(&path)
    .context("load application config")?;
```

**Bad:**

```rust
let config = match load_config(&path) {
    Ok(v) => v,
    Err(err) => {
        return Err(anyhow::anyhow!(
            "failed to load application config: {}",
            err
        ));
    }
};
```

Same meaning, 2 lines vs 8.

---

## 6. Documentation

### Module docs

Every module gets a `//!` doc comment. One sentence: what this module is for.

```rust
//! Configuration loading and validation from TOML files.
```

### Public API docs

Every public type, function, and method gets a `///` doc comment.
One sentence is usually enough. Don't restate the signature.

**Good:**

```rust
/// Find the top-k items most similar to the given embedding vector.
pub fn find_similar(&self, embedding: &[f32], k: usize) -> Result<Vec<ScoredItem>> {
```

**Bad:**

```rust
/// This function takes an embedding vector and a k value and returns
/// a Vec of ScoredItem representing the top k similar items.
pub fn find_similar(&self, embedding: &[f32], k: usize) -> Result<Vec<ScoredItem>> {
```

### When to add inline comments

- At decision points that aren't obvious from the code.
- At domain-specific logic (e.g., scoring formulas, threshold constants).
- NOT on obvious code. `// insert item` above `store.insert(item)` is noise.

---

## 7. Repetition

If you're writing the same pattern 3+ times, stop and abstract.

- 3 similar implementations → shared helpers or a macro.
- 3 similar output formatting blocks → a helper function.
- 3 similar match arms → a helper that takes the varying parts as arguments.

But: don't abstract prematurely. Two similar things are fine. Three is a
pattern.

---

## 8. Tests

### Placement

Tests go in `tests/foo_tests.rs`, not inline `#[cfg(test)] mod tests`.
Exception: modules under 100 lines may have inline tests.

### Style

```rust
#[test]
fn fusion_ranks_by_combined_score() {
    let first_results = vec![(1, 0.9), (2, 0.7), (3, 0.5)];
    let second_results = vec![(2, 0.95), (3, 0.8), (1, 0.3)];
    let fused = fuse(&[first_results, second_results], 60.0);
    assert_eq!(fused[0].0, 2, "item 2 should rank first");
}
```

- Test names describe what's being verified, not what's being called.
- Use `.expect("reason")` over bare `.unwrap()`.
- Extract test setup into helpers. A test should be: setup → act → assert.

---

## 9. Async

- Use native `async fn` in traits (Rust 1.75+). Do NOT use `async_trait`.
- Use `tokio::sync` locks in async code, not `std::sync`.
- Use `spawn_blocking` for filesystem I/O in async contexts.
- Keep critical sections short. Never hold a lock across an `.await`.

---

## 10. CLI and terminal output

### Colors

All colored terminal output goes through helpers in a dedicated module
(e.g., `colors.rs`). Never write raw ANSI escape codes in command handlers.
The color palette is defined once as constants, and helpers like `pill()`,
`dim()`, `ok()`, `warn()` are the only way to emit colored text.

```rust
// Good: uses color helpers
println!("  {} {}", pill("decision"), bold(&title));
println!("    {} item #{} {}", warn("UPDATE"), id, dim(&reason));

// Bad: raw escapes in application code
println!("  \x1b[48;2;30;58;95m decision \x1b[0m ...");
```

### JSON output

When a `--json` flag is present:
- A single JSON object or array on stdout.
- Zero ANSI codes, zero decoration, zero progress text on stdout.
- Progress/status text goes to stderr.
- Parseable by `jq` without cleanup.

---

## Summary: the feel

When you open a source file, it should feel like opening a well-organized
book chapter:

- You see the title (module doc comment).
- You see the cast of characters (types, imports).
- You read the story (public API, top-down).
- Details are in footnotes (private helpers, at the bottom).
- There's whitespace between paragraphs (section comments, blank lines
  between logical groups).

If a file feels like a wall of undifferentiated text, it's wrong — even if
every line is correct.
