---
title: "Local Skill/Code Discovery + Dedup Engine — Implementation Plan (Subsystem A wedge)"
description: "Antigravity/Gemini-3.5-Flash-executable, TDD, bite-sized plan to build vox-similarity (pure L2 simhash/minhash/LSH core) and vox-skill-discovery (L3 orchestrator: repeated .vox code-block mining, installed-skill dedup, MCP-SSOT-drift detection) plus a self-contained vox-discover binary. Wedge of the decentralized skill/code marketplace."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Local Skill/Code Discovery + Dedup Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a local, on-demand engine that mines repeated `.vox` code blocks, dedups skills/tools, and flags MCP↔skill SSOT drift, reporting advisory candidates — never installing, executing, or publishing.

**Architecture:** Two new crates. `vox-similarity` (L2, pure, no IO) implements blake3-derived simhash + minhash signatures, an LSH band index, clustering, and one-vs-many overlap. `vox-skill-discovery` (L3) adds source adapters (code-block miner, installed-catalog), a unified `Candidate` model, a `Reporter`, and a self-contained `vox-discover` binary. The similarity core is reused later by marketplace subsystems B (submission dedup) and C (content addressing).

**Tech Stack:** Rust, `blake3` (already in workspace, `pure` feature), `serde`/`serde_json`, `walkdir`, `clap` (bin), `vox-plugin-types` (`SkillManifest`), `vox-mcp-registry` (`TOOL_REGISTRY`). **No niche similarity/AST crates** — simhash/minhash are implemented in-crate to avoid mis-wiring on a fast model.

**Execution target:** Gemini 3.5 Flash in Google Antigravity. See `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`. **Hard rules for every task:** (1) end GREEN + committed (a mid-task kill must leave a compiling, tested tree); (2) verify-before-use — run the listed `rg`/read before referencing any external symbol; (3) self-contained — never rely on memory of earlier tasks; (4) two-strike circuit breaker — if a verification fails twice, STOP and write a handoff note, do not loop; (5) `cargo fmt -p <crate>` only (never `--all`); (6) VoxScript-only automation.

**Spec:** `docs/superpowers/specs/2026-06-18-skill-discovery-dedup-engine-design.md`.

**Scope of THIS plan (the wedge):** repeated `.vox` code-block mining + installed-skill dedup + MCP-SSOT-drift, via library crates and the `vox-discover` binary. **Deferred to their own follow-up plans** (each independently shippable): `PromptFlowMiner` (session-transcript mining), `RegistrySource` (external-skill surfacing), `vox-db` result cache, and integration into the monolithic `vox skill discover` CLI. Listed in the final "Deferred" section.

---

## Pre-flight (run once, before Task 1 — anti-hallucination baseline)

- [ ] **P0. Confirm the repo builds clean at baseline.**

Run: `cargo run -p vox-arch-check`
Expected: exits 0 (architecture parity OK). If it already fails, STOP and report — do not start on a red baseline.

- [ ] **P1. Confirm `blake3` is a workspace dependency with the `pure` feature.**

Run: `rg -n '^blake3' Cargo.toml`
Expected: `blake3 = { version = "1", features = ["pure"] }`

- [ ] **P2. Confirm the exact `SkillManifest` type and fields.**

Run: `rg -n 'pub struct SkillManifest' -A 30 crates/vox-plugin-types/src/skill_manifest.rs`
Expected fields used by this plan: `id: String`, `name: String`, `description: String`, `tools: Vec<String>`, `tags: Vec<String>`, `category: SkillCategory`. The type derives `Default`, `Clone`, `Serialize`, `Deserialize`.

- [ ] **P3. Confirm the MCP tool registry symbol and entry shape.**

Run: `rg -n 'TOOL_REGISTRY|pub struct McpToolRegistryEntry|SKILL_TOOLS|ORCHESTRATOR_TOOLS' crates/vox-mcp-registry/src/lib.rs`
Expected: `McpToolRegistryEntry { name: &'static str, ... }`; a generated `TOOL_REGISTRY` slice (included from `OUT_DIR`); `SKILL_TOOLS: &[&str]`; `ORCHESTRATOR_TOOLS: &[&str]`.
If `TOOL_REGISTRY` is not visible, run `rg -n 'TOOL_REGISTRY' $(find . -name tool_registry.rs)` to confirm the generated const name, and use that exact name in Task 12.

- [ ] **P4. Confirm workspace-dependency and layers registration formats.**

Run: `rg -n '^vox-search ' Cargo.toml` and `rg -n 'vox-search|vox-mcp-registry' docs/src/architecture/layers.toml`
Expected: workspace dep style `vox-search = { path = "crates/vox-search" }`; layers style `name = { layer = N }`. The `[workspace] members = ["crates/*"]` glob means a new `crates/<name>/` is auto-included — do NOT edit `members`.

---

## Phase 1 — `vox-similarity` (pure L2 core)

### Task 1: Scaffold the `vox-similarity` crate and register it [SEQUENTIAL]

**Files:**
- Create: `crates/vox-similarity/Cargo.toml`
- Create: `crates/vox-similarity/src/lib.rs`
- Modify: `Cargo.toml` (root — add to `[workspace.dependencies]`)
- Modify: `docs/src/architecture/layers.toml` (add layer entry)

- [ ] **Step 1: Create the crate manifest**

`crates/vox-similarity/Cargo.toml`:
```toml
[package]
name = "vox-similarity"
version = "0.1.0"
edition = "2021"
description = "Pure simhash/minhash/LSH near-duplicate similarity core for Vox discovery and marketplace dedup."

[dependencies]
blake3 = { workspace = true }
serde = { workspace = true }

[dev-dependencies]
```

- [ ] **Step 2: Create a placeholder lib so the crate compiles**

`crates/vox-similarity/src/lib.rs`:
```rust
//! Pure near-duplicate similarity core: simhash + minhash signatures, an LSH
//! band index, clustering, and one-vs-many overlap. No filesystem, DB, or network.

pub mod signature;
pub mod fragment;
pub mod index;

pub use fragment::{Fragment, FragmentKind};
pub use index::{Cluster, LshIndex, Match};
pub use signature::{hamming, jaccard_estimate, minhash, shingle, simhash64, tokenize, Signature};
```

- [ ] **Step 3: Create empty module files so the crate compiles before tests**

Create `crates/vox-similarity/src/signature.rs` with `// filled in Task 2`
Create `crates/vox-similarity/src/fragment.rs` with `// filled in Task 3`
Create `crates/vox-similarity/src/index.rs` with `// filled in Task 4`

> NOTE: lib.rs references items not yet defined; it will NOT compile until Task 4. To keep this task green, temporarily comment the `pub use` lines and the three `pub mod` lines, leaving only the doc comment. Uncomment them progressively as each module lands. (Self-contained rule: do this now.)

Make `crates/vox-similarity/src/lib.rs` exactly:
```rust
//! Pure near-duplicate similarity core: simhash + minhash signatures, an LSH
//! band index, clustering, and one-vs-many overlap. No filesystem, DB, or network.
```

- [ ] **Step 4: Register the workspace dependency**

In root `Cargo.toml`, in the `[workspace.dependencies]` block near the other `vox-*` path entries (next to `vox-search = { path = "crates/vox-search" }`), add:
```toml
vox-similarity = { path = "crates/vox-similarity" }
```

- [ ] **Step 5: Register the crate layer**

In `docs/src/architecture/layers.toml`, in the layer-2 group (near `vox-config = { layer = 2, ... }`), add:
```toml
vox-similarity        = { layer = 2 }
```

- [ ] **Step 6: Verify the crate compiles and arch-check passes**

Run: `cargo check -p vox-similarity`
Expected: compiles (warnings about unused files are fine).
Run: `cargo run -p vox-arch-check`
Expected: exits 0. If it complains about the new crate's layer/parity, follow its message (it names the file and the missing entry) and re-run.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-similarity Cargo.toml docs/src/architecture/layers.toml
git commit -m "feat(vox-similarity): scaffold pure similarity crate + register"
```

---

### Task 2: Signatures — tokenize, shingle, simhash, minhash [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-similarity/src/signature.rs`
- Test: inline `#[cfg(test)]` in the same file

- [ ] **Step 1: Write the failing tests**

Put this in `crates/vox-similarity/src/signature.rs`:
```rust
//! Token shingling and blake3-derived simhash / minhash signatures. Deterministic.

use serde::{Deserialize, Serialize};

/// Split text into lowercase alphanumeric/underscore tokens.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// k-token shingles (sliding windows). Falls back to a single joined token when
/// the token count is below `k`. Empty input yields an empty vec.
pub fn shingle(text: &str, k: usize) -> Vec<String> {
    let toks = tokenize(text);
    if toks.is_empty() {
        return Vec::new();
    }
    if toks.len() < k || k == 0 {
        return vec![toks.join(" ")];
    }
    toks.windows(k).map(|w| w.join(" ")).collect()
}

/// 64-bit SimHash over shingles (blake3 of each shingle → per-bit vote).
pub fn simhash64(shingles: &[String]) -> u64 {
    let mut acc = [0i32; 64];
    for s in shingles {
        let h = blake3::hash(s.as_bytes());
        let v = u64::from_le_bytes(h.as_bytes()[0..8].try_into().unwrap());
        for (i, slot) in acc.iter_mut().enumerate() {
            if (v >> i) & 1 == 1 {
                *slot += 1;
            } else {
                *slot -= 1;
            }
        }
    }
    let mut out = 0u64;
    for (i, &slot) in acc.iter().enumerate() {
        if slot > 0 {
            out |= 1u64 << i;
        }
    }
    out
}

/// MinHash with `num_hashes` independent blake3-seeded hash functions.
pub fn minhash(shingles: &[String], num_hashes: usize) -> Vec<u32> {
    let mut mins = vec![u32::MAX; num_hashes];
    for s in shingles {
        for (i, slot) in mins.iter_mut().enumerate() {
            let mut hasher = blake3::Hasher::new();
            hasher.update(&(i as u32).to_le_bytes());
            hasher.update(s.as_bytes());
            let h = hasher.finalize();
            let v = u32::from_le_bytes(h.as_bytes()[0..4].try_into().unwrap());
            if v < *slot {
                *slot = v;
            }
        }
    }
    mins
}

/// Hamming distance between two 64-bit simhashes.
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Estimated Jaccard similarity from two equal-length minhash vectors.
pub fn jaccard_estimate(a: &[u32], b: &[u32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let eq = a.iter().zip(b).filter(|(x, y)| x == y).count();
    eq as f32 / a.len() as f32
}

/// A deterministic similarity signature for a piece of text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Signature {
    pub simhash: u64,
    pub minhash: Vec<u32>,
}

impl Signature {
    pub fn from_text(text: &str, k: usize, num_hashes: usize) -> Self {
        let sh = shingle(text, k);
        Signature {
            simhash: simhash64(&sh),
            minhash: minhash(&sh, num_hashes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_splits_and_lowercases() {
        assert_eq!(tokenize("Foo.bar_baz 42"), vec!["foo", "bar_baz", "42"]);
    }

    #[test]
    fn shingle_makes_k_windows() {
        assert_eq!(shingle("a b c d", 2), vec!["a b", "b c", "c d"]);
    }

    #[test]
    fn identical_text_has_zero_hamming_and_full_jaccard() {
        let a = Signature::from_text("let x = compute(value) + 1", 3, 64);
        let b = Signature::from_text("let x = compute(value) + 1", 3, 64);
        assert_eq!(hamming(a.simhash, b.simhash), 0);
        assert_eq!(jaccard_estimate(&a.minhash, &b.minhash), 1.0);
    }

    #[test]
    fn dissimilar_text_has_low_jaccard() {
        let a = Signature::from_text("the quick brown fox jumps over", 3, 64);
        let b = Signature::from_text("completely unrelated tokens here now please", 3, 64);
        assert!(jaccard_estimate(&a.minhash, &b.minhash) < 0.3);
    }

    #[test]
    fn signatures_are_deterministic() {
        let a = Signature::from_text("repeat me", 2, 32);
        let b = Signature::from_text("repeat me", 2, 32);
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 2: Re-enable the module in lib.rs**

Set `crates/vox-similarity/src/lib.rs` to:
```rust
//! Pure near-duplicate similarity core: simhash + minhash signatures, an LSH
//! band index, clustering, and one-vs-many overlap. No filesystem, DB, or network.

pub mod signature;

pub use signature::{hamming, jaccard_estimate, minhash, shingle, simhash64, tokenize, Signature};
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p vox-similarity signature`
Expected: 5 tests PASS.

- [ ] **Step 4: Format and commit**

```bash
cargo fmt -p vox-similarity
git add crates/vox-similarity/src
git commit -m "feat(vox-similarity): tokenize/shingle/simhash/minhash signatures"
```

---

### Task 3: `Fragment` type [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-similarity/src/fragment.rs`
- Modify: `crates/vox-similarity/src/lib.rs`

- [ ] **Step 1: Write the failing test + implementation**

Put this in `crates/vox-similarity/src/fragment.rs`:
```rust
//! A `Fragment` is the universal comparable unit: text + a blake3 content hash +
//! a similarity `Signature` + provenance (`source_ref`).

use serde::{Deserialize, Serialize};

use crate::signature::Signature;

/// What kind of thing a fragment represents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FragmentKind {
    Code,
    Prompt,
    InstalledSkill,
    McpTool,
    ExternalSkill,
}

/// A normalized comparable unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fragment {
    pub id: String,
    pub kind: FragmentKind,
    /// blake3 hex of the raw text (exact-duplicate key).
    pub content_hash: String,
    pub signature: Signature,
    /// Provenance: "path:line", skill id, or registry id.
    pub source_ref: String,
    pub text: String,
}

impl Fragment {
    pub fn new(
        id: impl Into<String>,
        kind: FragmentKind,
        text: impl Into<String>,
        source_ref: impl Into<String>,
        shingle_k: usize,
        num_hashes: usize,
    ) -> Self {
        let text = text.into();
        let content_hash = blake3::hash(text.as_bytes()).to_hex().to_string();
        let signature = Signature::from_text(&text, shingle_k, num_hashes);
        Fragment {
            id: id.into(),
            kind,
            content_hash,
            signature,
            source_ref: source_ref.into(),
            text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_hashes_and_signs_text() {
        let f = Fragment::new("f1", FragmentKind::Code, "let y = 2", "a.vox:1", 2, 16);
        assert_eq!(f.content_hash.len(), 64); // blake3 hex
        assert_eq!(f.signature.minhash.len(), 16);
        assert_eq!(f.kind, FragmentKind::Code);
    }

    #[test]
    fn identical_text_same_content_hash() {
        let a = Fragment::new("a", FragmentKind::Code, "same body", "x:1", 2, 8);
        let b = Fragment::new("b", FragmentKind::Code, "same body", "y:9", 2, 8);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
```

- [ ] **Step 2: Re-enable the module in lib.rs**

Set `crates/vox-similarity/src/lib.rs` to:
```rust
//! Pure near-duplicate similarity core: simhash + minhash signatures, an LSH
//! band index, clustering, and one-vs-many overlap. No filesystem, DB, or network.

pub mod fragment;
pub mod signature;

pub use fragment::{Fragment, FragmentKind};
pub use signature::{hamming, jaccard_estimate, minhash, shingle, simhash64, tokenize, Signature};
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p vox-similarity fragment`
Expected: 2 tests PASS.

- [ ] **Step 4: Format and commit**

```bash
cargo fmt -p vox-similarity
git add crates/vox-similarity/src
git commit -m "feat(vox-similarity): Fragment type with content hash + signature"
```

---

### Task 4: `LshIndex` — insert, overlap, cluster [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-similarity/src/index.rs`
- Modify: `crates/vox-similarity/src/lib.rs`

- [ ] **Step 1: Write the implementation + failing tests**

Put this in `crates/vox-similarity/src/index.rs`:
```rust
//! LSH band index over minhash signatures: cheap near-neighbor candidate
//! generation, plus jaccard-confirmed clustering and one-vs-many overlap.

use std::collections::{BTreeSet, HashMap};

use crate::fragment::Fragment;
use crate::signature::{jaccard_estimate, Signature};

/// A confirmed near-duplicate match for a query fragment.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub index: usize,
    pub jaccard: f32,
}

/// A group of mutually-similar fragments.
#[derive(Debug, Clone, PartialEq)]
pub struct Cluster {
    pub members: Vec<usize>,
}

/// Banded LSH index. `bands * rows` must equal the minhash length used by inserted
/// fragments for full banding; shorter signatures are clamped.
pub struct LshIndex {
    fragments: Vec<Fragment>,
    bands: usize,
    rows: usize,
    buckets: HashMap<(usize, u64), Vec<usize>>,
}

impl LshIndex {
    pub fn new(bands: usize, rows: usize) -> Self {
        Self {
            fragments: Vec::new(),
            bands,
            rows,
            buckets: HashMap::new(),
        }
    }

    fn band_keys(&self, sig: &Signature) -> Vec<(usize, u64)> {
        let mut keys = Vec::new();
        for b in 0..self.bands {
            let start = b * self.rows;
            let end = (start + self.rows).min(sig.minhash.len());
            if start >= end {
                break;
            }
            let mut hasher = blake3::Hasher::new();
            for &v in &sig.minhash[start..end] {
                hasher.update(&v.to_le_bytes());
            }
            let h = hasher.finalize();
            let key = u64::from_le_bytes(h.as_bytes()[0..8].try_into().unwrap());
            keys.push((b, key));
        }
        keys
    }

    pub fn insert(&mut self, fragment: Fragment) -> usize {
        let idx = self.fragments.len();
        for key in self.band_keys(&fragment.signature) {
            self.buckets.entry(key).or_default().push(idx);
        }
        self.fragments.push(fragment);
        idx
    }

    pub fn len(&self) -> usize {
        self.fragments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fragments.is_empty()
    }

    pub fn fragment(&self, idx: usize) -> &Fragment {
        &self.fragments[idx]
    }

    fn near_indices(&self, sig: &Signature) -> Vec<usize> {
        let mut set = BTreeSet::new();
        for key in self.band_keys(sig) {
            if let Some(v) = self.buckets.get(&key) {
                for &i in v {
                    set.insert(i);
                }
            }
        }
        set.into_iter().collect()
    }

    /// Confirmed matches for an external query fragment. Excludes any indexed
    /// fragment that shares the query's content hash AND source_ref (self).
    pub fn overlap(&self, query: &Fragment, min_jaccard: f32) -> Vec<Match> {
        let mut out = Vec::new();
        for i in self.near_indices(&query.signature) {
            let f = &self.fragments[i];
            if f.content_hash == query.content_hash && f.source_ref == query.source_ref {
                continue;
            }
            let j = jaccard_estimate(&query.signature.minhash, &f.signature.minhash);
            if j >= min_jaccard {
                out.push(Match { index: i, jaccard: j });
            }
        }
        out.sort_by(|a, b| b.jaccard.partial_cmp(&a.jaccard).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    fn neighbors_of(&self, idx: usize, min_jaccard: f32) -> Vec<usize> {
        let q = &self.fragments[idx];
        let mut out = Vec::new();
        for i in self.near_indices(&q.signature) {
            if i == idx {
                continue;
            }
            let j = jaccard_estimate(&q.signature.minhash, &self.fragments[i].signature.minhash);
            if j >= min_jaccard {
                out.push(i);
            }
        }
        out
    }

    /// Group indexed fragments into clusters via union-find over confirmed
    /// neighbor pairs. Returns only clusters with `>= min_members`.
    pub fn cluster(&self, min_members: usize, min_jaccard: f32) -> Vec<Cluster> {
        let n = self.fragments.len();
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while parent[r] != r {
                r = parent[r];
            }
            let mut c = x;
            while parent[c] != c {
                let next = parent[c];
                parent[c] = r;
                c = next;
            }
            r
        }

        for i in 0..n {
            for m in self.neighbors_of(i, min_jaccard) {
                let a = find(&mut parent, i);
                let b = find(&mut parent, m);
                if a != b {
                    parent[a] = b;
                }
            }
        }

        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let r = find(&mut parent, i);
            groups.entry(r).or_default().push(i);
        }

        let mut clusters: Vec<Cluster> = groups
            .into_values()
            .filter(|g| g.len() >= min_members)
            .map(|mut members| {
                members.sort_unstable();
                Cluster { members }
            })
            .collect();
        clusters.sort_by(|a, b| a.members.cmp(&b.members));
        clusters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::{Fragment, FragmentKind};

    fn frag(id: &str, text: &str, src: &str) -> Fragment {
        Fragment::new(id, FragmentKind::Code, text, src, 3, 64)
    }

    #[test]
    fn cluster_groups_identical_fragments() {
        let mut idx = LshIndex::new(16, 4);
        idx.insert(frag("a", "let total = price * quantity + tax", "a.vox:1"));
        idx.insert(frag("b", "let total = price * quantity + tax", "b.vox:9"));
        idx.insert(frag("c", "print hello world unrelated entirely", "c.vox:3"));
        let clusters = idx.cluster(2, 0.7);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].members, vec![0, 1]);
    }

    #[test]
    fn overlap_finds_similar_query() {
        let mut idx = LshIndex::new(16, 4);
        idx.insert(frag("a", "let total = price * quantity + tax", "a.vox:1"));
        let q = frag("q", "let total = price * quantity + tax", "q.vox:1");
        let matches = idx.overlap(&q, 0.7);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].index, 0);
        assert!(matches[0].jaccard >= 0.9);
    }

    #[test]
    fn no_clusters_when_all_unique() {
        let mut idx = LshIndex::new(16, 4);
        idx.insert(frag("a", "alpha beta gamma delta epsilon", "a:1"));
        idx.insert(frag("b", "one two three four five six", "b:1"));
        assert!(idx.cluster(2, 0.7).is_empty());
    }
}
```

- [ ] **Step 2: Re-enable the module + full exports in lib.rs**

Set `crates/vox-similarity/src/lib.rs` to:
```rust
//! Pure near-duplicate similarity core: simhash + minhash signatures, an LSH
//! band index, clustering, and one-vs-many overlap. No filesystem, DB, or network.

pub mod fragment;
pub mod index;
pub mod signature;

pub use fragment::{Fragment, FragmentKind};
pub use index::{Cluster, LshIndex, Match};
pub use signature::{hamming, jaccard_estimate, minhash, shingle, simhash64, tokenize, Signature};
```

- [ ] **Step 3: Run the full crate test suite**

Run: `cargo test -p vox-similarity`
Expected: all tests PASS (signature 5 + fragment 2 + index 3 = 10).

- [ ] **Step 4: Clippy, format, commit**

Run: `cargo clippy -p vox-similarity -- -D warnings`
Expected: clean.
```bash
cargo fmt -p vox-similarity
git add crates/vox-similarity/src
git commit -m "feat(vox-similarity): LSH band index with overlap + clustering"
```

---

## Phase 2 — `vox-skill-discovery` (L3 orchestrator + binary)

### Task 5: Scaffold the `vox-skill-discovery` crate and register it [SEQUENTIAL]

**Files:**
- Create: `crates/vox-skill-discovery/Cargo.toml`
- Create: `crates/vox-skill-discovery/src/lib.rs`
- Modify: `Cargo.toml` (root `[workspace.dependencies]`)
- Modify: `docs/src/architecture/layers.toml`

- [ ] **Step 1: Create the crate manifest**

`crates/vox-skill-discovery/Cargo.toml`:
```toml
[package]
name = "vox-skill-discovery"
version = "0.1.0"
edition = "2021"
description = "Local on-demand discovery + dedup engine: repeated .vox blocks, installed-skill dedup, MCP SSOT drift. Advisory only."

[dependencies]
vox-similarity = { workspace = true }
vox-plugin-types = { workspace = true }
vox-mcp-registry = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
walkdir = { workspace = true }
anyhow = { workspace = true }

[[bin]]
name = "vox-discover"
path = "src/bin/vox_discover.rs"

[dependencies.clap]
workspace = true
```

> VERIFY-BEFORE-USE: confirm each dep is a workspace dependency. Run:
> `rg -n '^vox-plugin-types|^vox-mcp-registry|^serde |^serde_json|^walkdir|^anyhow|^clap ' Cargo.toml`
> All must be present (they are, per Pre-flight). `vox-similarity` was added in Task 1.

- [ ] **Step 2: Create the lib with module declarations (modules filled in later tasks)**

`crates/vox-skill-discovery/src/lib.rs`:
```rust
//! Local, on-demand discovery + dedup engine. Mines repeated `.vox` code blocks,
//! dedups installed skills, and flags MCP↔skill SSOT drift. Advisory only — it
//! never installs, executes, or publishes.

pub mod candidate;
pub mod options;
pub mod code_miner;
pub mod catalog;
pub mod report;

pub use candidate::{Candidate, CandidateKind, DraftFrontmatter};
pub use options::DiscoverOptions;
```

- [ ] **Step 3: Create stub module files so the crate compiles**

Create these files, each with only a doc-comment line for now:
- `crates/vox-skill-discovery/src/candidate.rs` → `//! filled in Task 6`
- `crates/vox-skill-discovery/src/options.rs` → `//! filled in Task 6`
- `crates/vox-skill-discovery/src/code_miner.rs` → `//! filled in Tasks 7-9`
- `crates/vox-skill-discovery/src/catalog.rs` → `//! filled in Tasks 10-12`
- `crates/vox-skill-discovery/src/report.rs` → `//! filled in Task 13`
- `crates/vox-skill-discovery/src/bin/vox_discover.rs` → `fn main() { println!("vox-discover: not yet wired"); }`

To keep THIS task green, temporarily reduce `lib.rs` to only the doc comment plus `pub mod candidate; pub mod options;` is NOT yet valid (empty files). Instead, set `lib.rs` to ONLY the doc comment for now:
```rust
//! Local, on-demand discovery + dedup engine. Mines repeated `.vox` code blocks,
//! dedups installed skills, and flags MCP↔skill SSOT drift. Advisory only — it
//! never installs, executes, or publishes.
```
(Each later task re-adds its `pub mod` + `pub use` lines as it lands.)

- [ ] **Step 4: Register workspace dep + layer**

Root `Cargo.toml` `[workspace.dependencies]` (near `vox-search`):
```toml
vox-skill-discovery = { path = "crates/vox-skill-discovery" }
```
`docs/src/architecture/layers.toml` (layer-3 group):
```toml
vox-skill-discovery   = { layer = 3 }
```

- [ ] **Step 5: Verify compile + arch-check**

Run: `cargo check -p vox-skill-discovery`
Expected: compiles.
Run: `cargo run -p vox-arch-check`
Expected: exits 0 (fix any parity message it prints for the new crate).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-skill-discovery Cargo.toml docs/src/architecture/layers.toml
git commit -m "feat(vox-skill-discovery): scaffold discovery crate + register"
```

---

### Task 6: `DiscoverOptions` + `Candidate` model [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-discovery/src/options.rs`
- Modify: `crates/vox-skill-discovery/src/candidate.rs`
- Modify: `crates/vox-skill-discovery/src/lib.rs`

- [ ] **Step 1: Write `options.rs`**

`crates/vox-skill-discovery/src/options.rs`:
```rust
//! Tunables for the discovery engine. Defaults mirror the design spec.

#[derive(Debug, Clone)]
pub struct DiscoverOptions {
    /// Minimum token count for a code block to be considered.
    pub min_tokens: usize,
    /// Minimum occurrences for a code cluster to be reported.
    pub min_occurrences: usize,
    /// Shingle window size (tokens).
    pub shingle_k: usize,
    /// LSH bands.
    pub bands: usize,
    /// LSH rows per band. `bands * rows` is the minhash length.
    pub rows: usize,
    /// Confirmed-jaccard threshold for clustering / overlap.
    pub min_jaccard: f32,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            min_tokens: 40,
            min_occurrences: 3,
            shingle_k: 5,
            bands: 32,
            rows: 4,
            min_jaccard: 0.7,
        }
    }
}

impl DiscoverOptions {
    /// Minhash length implied by the band configuration.
    pub fn num_hashes(&self) -> usize {
        self.bands * self.rows
    }
}
```

- [ ] **Step 2: Write `candidate.rs` with a test**

`crates/vox-skill-discovery/src/candidate.rs`:
```rust
//! The unified advisory output of the discovery engine.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CandidateKind {
    /// A recurring code block that could be extracted into a reusable skill/snippet.
    RepeatedCode,
    /// Two or more installed skills/tools that overlap heavily.
    DuplicatesInstalled,
    /// A skill declares an MCP tool that does not exist in the registry.
    SsotDrift,
}

/// Advisory draft frontmatter the user MAY accept (never auto-applied).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftFrontmatter {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub kind: CandidateKind,
    /// Provenance refs: "path:line", skill ids, or "skill_id->tool".
    pub members: Vec<String>,
    pub score: f32,
    pub suggested_action: String,
    pub draft_frontmatter: Option<DraftFrontmatter>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_serializes_to_json() {
        let c = Candidate {
            kind: CandidateKind::RepeatedCode,
            members: vec!["a.vox:1".into(), "b.vox:9".into()],
            score: 0.95,
            suggested_action: "Extract into a reusable Vox skill".into(),
            draft_frontmatter: None,
        };
        let j = serde_json::to_string(&c).unwrap();
        assert!(j.contains("RepeatedCode"));
        assert!(j.contains("a.vox:1"));
    }
}
```

- [ ] **Step 3: Update lib.rs**

Set `crates/vox-skill-discovery/src/lib.rs` to:
```rust
//! Local, on-demand discovery + dedup engine. Mines repeated `.vox` code blocks,
//! dedups installed skills, and flags MCP↔skill SSOT drift. Advisory only — it
//! never installs, executes, or publishes.

pub mod candidate;
pub mod options;

pub use candidate::{Candidate, CandidateKind, DraftFrontmatter};
pub use options::DiscoverOptions;
```

- [ ] **Step 4: Test, format, commit**

Run: `cargo test -p vox-skill-discovery candidate`
Expected: 1 test PASS.
```bash
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/src
git commit -m "feat(vox-skill-discovery): DiscoverOptions + Candidate model"
```

---

### Task 7: Code block extraction (blank-line paragraphs) [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-discovery/src/code_miner.rs`
- Modify: `crates/vox-skill-discovery/src/lib.rs`

- [ ] **Step 1: Write `extract_blocks` + test**

Put this in `crates/vox-skill-discovery/src/code_miner.rs`:
```rust
//! Mines repeated `.vox` code blocks. v1 splits files into blank-line-delimited
//! blocks (language-agnostic, no grammar dependency). AST-based blocking via
//! tree-sitter is a documented phase-2 refinement (see the design spec).

use std::path::Path;

use vox_similarity::{tokenize, Fragment, FragmentKind, LshIndex};

use crate::candidate::{Candidate, CandidateKind, DraftFrontmatter};
use crate::options::DiscoverOptions;

/// Split text into (start_line, block_text) on blank-line boundaries.
pub(crate) fn extract_blocks(text: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    let mut cur_start = 1usize;
    let mut line_no = 0usize;
    for line in text.lines() {
        line_no += 1;
        if line.trim().is_empty() {
            if !cur.trim().is_empty() {
                blocks.push((cur_start, std::mem::take(&mut cur)));
            }
            cur.clear();
            cur_start = line_no + 1;
        } else {
            if cur.is_empty() {
                cur_start = line_no;
            }
            cur.push_str(line);
            cur.push('\n');
        }
    }
    if !cur.trim().is_empty() {
        blocks.push((cur_start, cur));
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_blocks_splits_on_blank_lines() {
        let text = "line a\nline b\n\nline c\n";
        let blocks = extract_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, 1);
        assert!(blocks[0].1.contains("line a"));
        assert_eq!(blocks[1].0, 4);
        assert!(blocks[1].1.contains("line c"));
    }
}
```

> NOTE: the `use` lines reference items used by Task 8/9. They will trigger
> `unused_import` warnings now but compile. That is acceptable for this task
> (warnings, not errors). They are consumed in Task 8.

- [ ] **Step 2: Update lib.rs**

Add `pub mod code_miner;` under the existing `pub mod` lines in `crates/vox-skill-discovery/src/lib.rs` (keep the existing candidate/options lines).

- [ ] **Step 3: Test, format, commit**

Run: `cargo test -p vox-skill-discovery extract_blocks`
Expected: 1 test PASS.
```bash
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/src
git commit -m "feat(vox-skill-discovery): blank-line code block extraction"
```

---

### Task 8: `mine_repeated_code` over a directory [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-discovery/src/code_miner.rs`

- [ ] **Step 1: Append `mine_repeated_code` + a fixture-based test**

Add to `crates/vox-skill-discovery/src/code_miner.rs` (above the `#[cfg(test)]` module), then extend the test module:
```rust
use walkdir::WalkDir;

/// Mine repeated `.vox` code blocks under `root`. Returns `RepeatedCode` candidates,
/// one per cluster of `>= min_occurrences` similar blocks.
pub fn mine_repeated_code(root: &Path, opts: &DiscoverOptions) -> Vec<Candidate> {
    let mut index = LshIndex::new(opts.bands, opts.rows);
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(p) else {
            continue;
        };
        for (start, block) in extract_blocks(&text) {
            if tokenize(&block).len() < opts.min_tokens {
                continue;
            }
            let src = format!("{}:{}", p.display(), start);
            let frag = Fragment::new(
                src.clone(),
                FragmentKind::Code,
                block,
                src,
                opts.shingle_k,
                opts.num_hashes(),
            );
            index.insert(frag);
        }
    }

    let mut candidates = Vec::new();
    for cluster in index.cluster(opts.min_occurrences, opts.min_jaccard) {
        let members: Vec<String> = cluster
            .members
            .iter()
            .map(|&i| index.fragment(i).source_ref.clone())
            .collect();
        let score = if cluster.members.len() >= 2 {
            let a = &index.fragment(cluster.members[0]).signature.minhash;
            let b = &index.fragment(cluster.members[1]).signature.minhash;
            vox_similarity::jaccard_estimate(a, b)
        } else {
            1.0
        };
        let stem = stem_of(&members[0]);
        candidates.push(Candidate {
            kind: CandidateKind::RepeatedCode,
            members,
            score,
            suggested_action: "Extract this recurring block into a reusable Vox skill/snippet"
                .to_string(),
            draft_frontmatter: Some(DraftFrontmatter {
                name: format!("{stem}-block"),
                description: "Recurring code block detected across the repository.".to_string(),
                category: "refactor".to_string(),
                tags: vec!["auto-discovered".to_string(), "duplicate".to_string()],
            }),
        });
    }
    candidates
}

/// Best-effort file stem from a "path:line" source ref.
fn stem_of(source_ref: &str) -> String {
    let path = source_ref.rsplit_once(':').map(|(p, _)| p).unwrap_or(source_ref);
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("vox")
        .to_string()
}
```

Add to the `tests` module in the same file:
```rust
    #[test]
    fn mine_finds_duplicate_block_across_two_files() {
        let dir = std::env::temp_dir().join(format!("voxdisc_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let body = "let subtotal = unit_price * quantity\nlet tax = subtotal * tax_rate\nlet total = subtotal + tax\nreturn total\n";
        std::fs::write(dir.join("a.vox"), body).unwrap();
        std::fs::write(dir.join("b.vox"), body).unwrap();

        let opts = DiscoverOptions {
            min_tokens: 5,
            min_occurrences: 2,
            ..DiscoverOptions::default()
        };
        let cands = mine_repeated_code(&dir, &opts);
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].kind, CandidateKind::RepeatedCode);
        assert_eq!(cands[0].members.len(), 2);
        assert!(cands[0].score >= 0.9);
    }
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p vox-skill-discovery mine_finds_duplicate_block`
Expected: PASS.

- [ ] **Step 3: Clippy, format, commit**

Run: `cargo clippy -p vox-skill-discovery -- -D warnings`
Expected: clean (the earlier unused-import warnings are now resolved).
```bash
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/src
git commit -m "feat(vox-skill-discovery): mine_repeated_code over .vox tree"
```

---

### Task 9: Export the code miner [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-discovery/src/lib.rs`

- [ ] **Step 1: Re-export the public miner fn**

In `crates/vox-skill-discovery/src/lib.rs`, add to the `pub use` section:
```rust
pub use code_miner::mine_repeated_code;
```

- [ ] **Step 2: Verify + commit**

Run: `cargo test -p vox-skill-discovery`
Expected: all tests PASS.
```bash
git add crates/vox-skill-discovery/src/lib.rs
git commit -m "feat(vox-skill-discovery): export mine_repeated_code"
```

---

### Task 10: Installed-skill dedup (`dedup_skills`) [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-discovery/src/catalog.rs`
- Modify: `crates/vox-skill-discovery/src/lib.rs`

- [ ] **Step 1: Verify the `SkillManifest` import path**

Run: `rg -n 'pub use|pub struct SkillManifest' crates/vox-plugin-types/src/lib.rs crates/vox-plugin-types/src/skill_manifest.rs`
Expected: `SkillManifest` is reachable as `vox_plugin_types::skill_manifest::SkillManifest`. Confirm whether `vox_plugin_types::SkillManifest` is also re-exported; if so prefer the shorter path. Use whichever the grep confirms in the code below (this plan assumes `vox_plugin_types::skill_manifest::SkillManifest`).

- [ ] **Step 2: Write `dedup_skills` + test**

Put this in `crates/vox-skill-discovery/src/catalog.rs`:
```rust
//! Dedup installed skills and validate the MCP↔skill SSOT. Operates on a provided
//! slice of `SkillManifest` (caller supplies; v1 loads from a JSON file). Wiring to
//! a live `SkillRegistry` is a deferred follow-up.

use vox_plugin_types::skill_manifest::SkillManifest;
use vox_similarity::{Fragment, FragmentKind, LshIndex};

use crate::candidate::{Candidate, CandidateKind};
use crate::options::DiscoverOptions;

/// Build the comparable text for a skill manifest.
fn manifest_text(m: &SkillManifest) -> String {
    let mut parts = vec![m.name.clone(), m.description.clone()];
    parts.extend(m.tags.iter().cloned());
    parts.extend(m.tools.iter().cloned());
    parts.join(" ")
}

/// Find installed skills that overlap heavily (near-duplicate skills).
pub fn dedup_skills(manifests: &[SkillManifest], opts: &DiscoverOptions) -> Vec<Candidate> {
    let mut index = LshIndex::new(opts.bands, opts.rows);
    for m in manifests {
        let text = manifest_text(m);
        let frag = Fragment::new(
            m.id.clone(),
            FragmentKind::InstalledSkill,
            text,
            m.id.clone(),
            opts.shingle_k,
            opts.num_hashes(),
        );
        index.insert(frag);
    }

    let mut out = Vec::new();
    for cluster in index.cluster(2, opts.min_jaccard) {
        let members: Vec<String> = cluster
            .members
            .iter()
            .map(|&i| index.fragment(i).source_ref.clone())
            .collect();
        out.push(Candidate {
            kind: CandidateKind::DuplicatesInstalled,
            members,
            score: opts.min_jaccard,
            suggested_action: "These installed skills overlap — consider consolidating or reusing one"
                .to_string(),
            draft_frontmatter: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_plugin_types::skill_manifest::SkillCategory;

    fn manifest(id: &str, name: &str, desc: &str) -> SkillManifest {
        SkillManifest::new(id, name, "0.1.0", "test", desc, SkillCategory::Unknown)
    }

    #[test]
    fn dedup_flags_near_identical_skills() {
        let opts = DiscoverOptions {
            shingle_k: 2,
            ..DiscoverOptions::default()
        };
        let manifests = vec![
            manifest("a.fmt", "format vox", "Formats vox source files with the standard style"),
            manifest("b.fmt", "format vox", "Formats vox source files with the standard style"),
            manifest("c.git", "git status", "Shows the working tree status using git"),
        ];
        let cands = dedup_skills(&manifests, &opts);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].members.len(), 2);
    }
}
```

- [ ] **Step 3: Update lib.rs**

Add `pub mod catalog;` and `pub use catalog::dedup_skills;` to `crates/vox-skill-discovery/src/lib.rs`.

- [ ] **Step 4: Test, clippy, format, commit**

Run: `cargo test -p vox-skill-discovery dedup_flags`
Expected: PASS.
Run: `cargo clippy -p vox-skill-discovery -- -D warnings`
Expected: clean.
```bash
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/src
git commit -m "feat(vox-skill-discovery): installed-skill dedup"
```

---

### Task 11: MCP↔skill SSOT drift detection (`validate_ssot`) [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-discovery/src/catalog.rs`
- Modify: `crates/vox-skill-discovery/src/lib.rs`

- [ ] **Step 1: Confirm the known-tool sources**

Run: `rg -n 'pub const SKILL_TOOLS|pub const ORCHESTRATOR_TOOLS|TOOL_REGISTRY' crates/vox-mcp-registry/src/lib.rs`
Expected: `SKILL_TOOLS: &[&str]`, `ORCHESTRATOR_TOOLS: &[&str]`, and a `TOOL_REGISTRY` slice of `McpToolRegistryEntry` (each has `.name`). If `TOOL_REGISTRY` is not importable as `vox_mcp_registry::TOOL_REGISTRY`, find its exact name from the generated file (`rg -n 'TOOL_REGISTRY' $(find . -path '*tool_registry.rs')`) and substitute below.

- [ ] **Step 2: Append `validate_ssot` + test to catalog.rs**

Add to `crates/vox-skill-discovery/src/catalog.rs` (above the test module):
```rust
use std::collections::HashSet;

/// The set of all known MCP tool names (registry + skill + orchestrator tool lists).
fn known_tool_names() -> HashSet<String> {
    let mut set = HashSet::new();
    for entry in vox_mcp_registry::TOOL_REGISTRY.iter() {
        set.insert(entry.name.to_string());
    }
    for t in vox_mcp_registry::SKILL_TOOLS {
        set.insert((*t).to_string());
    }
    for t in vox_mcp_registry::ORCHESTRATOR_TOOLS {
        set.insert((*t).to_string());
    }
    set
}

/// Flag skills that declare a `tool` not present in the MCP registry (SSOT drift).
pub fn validate_ssot(manifests: &[SkillManifest]) -> Vec<Candidate> {
    let known = known_tool_names();
    let mut out = Vec::new();
    for m in manifests {
        for tool in &m.tools {
            if !known.contains(tool) {
                out.push(Candidate {
                    kind: CandidateKind::SsotDrift,
                    members: vec![format!("{}->{}", m.id, tool)],
                    score: 1.0,
                    suggested_action: format!(
                        "Skill '{}' declares tool '{}' which is not in the MCP registry — fix the manifest or register the tool",
                        m.id, tool
                    ),
                    draft_frontmatter: None,
                });
            }
        }
    }
    out
}
```

Add to the `tests` module:
```rust
    #[test]
    fn validate_ssot_flags_unknown_tool() {
        let mut m = manifest("x.bad", "bad skill", "declares a phantom tool");
        m.tools = vec!["vox_totally_made_up_tool".to_string()];
        let cands = validate_ssot(&[m]);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].kind, CandidateKind::SsotDrift);
        assert!(cands[0].members[0].contains("vox_totally_made_up_tool"));
    }

    #[test]
    fn validate_ssot_accepts_known_tool() {
        let mut m = manifest("x.good", "good skill", "declares a real tool");
        m.tools = vec!["vox_skill_list".to_string()];
        assert!(validate_ssot(&[m]).is_empty());
    }
```

- [ ] **Step 3: Export + test**

Add `pub use catalog::validate_ssot;` to `crates/vox-skill-discovery/src/lib.rs`.
Run: `cargo test -p vox-skill-discovery validate_ssot`
Expected: 2 tests PASS.

> If `validate_ssot_accepts_known_tool` fails because `vox_skill_list` is not in any list, pick a tool name the Step-1 grep actually showed (e.g. another `SKILL_TOOLS` entry) and use it in the test. Do not invent one.

- [ ] **Step 4: Clippy, format, commit**

Run: `cargo clippy -p vox-skill-discovery -- -D warnings`
Expected: clean.
```bash
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/src
git commit -m "feat(vox-skill-discovery): MCP-skill SSOT drift detection"
```

---

### Task 12: `Reporter` — terminal + JSON rendering [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-discovery/src/report.rs`
- Modify: `crates/vox-skill-discovery/src/lib.rs`

- [ ] **Step 1: Write `report.rs` + test**

Put this in `crates/vox-skill-discovery/src/report.rs`:
```rust
//! Render candidates for human or machine consumption. Advisory only.

use crate::candidate::Candidate;

/// Human-readable terminal report.
pub fn render_terminal(candidates: &[Candidate]) -> String {
    if candidates.is_empty() {
        return "No discovery candidates found.".to_string();
    }
    let mut out = format!("Found {} candidate(s):\n", candidates.len());
    for (i, c) in candidates.iter().enumerate() {
        out.push_str(&format!(
            "\n[{}] {:?} (score {:.2})\n    action: {}\n    members:\n",
            i + 1,
            c.kind,
            c.score,
            c.suggested_action
        ));
        for m in &c.members {
            out.push_str(&format!("      - {m}\n"));
        }
    }
    out
}

/// Machine-readable JSON report.
pub fn render_json(candidates: &[Candidate]) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(candidates)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate::CandidateKind;

    fn sample() -> Vec<Candidate> {
        vec![Candidate {
            kind: CandidateKind::RepeatedCode,
            members: vec!["a.vox:1".into(), "b.vox:9".into()],
            score: 0.95,
            suggested_action: "Extract block".into(),
            draft_frontmatter: None,
        }]
    }

    #[test]
    fn terminal_lists_members() {
        let r = render_terminal(&sample());
        assert!(r.contains("a.vox:1"));
        assert!(r.contains("RepeatedCode"));
    }

    #[test]
    fn empty_terminal_is_clean() {
        assert!(render_terminal(&[]).contains("No discovery candidates"));
    }

    #[test]
    fn json_round_trips() {
        let j = render_json(&sample()).unwrap();
        assert!(j.contains("RepeatedCode"));
    }
}
```

- [ ] **Step 2: Update lib.rs**

Add `pub mod report;` and `pub use report::{render_json, render_terminal};` to `crates/vox-skill-discovery/src/lib.rs`.

- [ ] **Step 3: Test, format, commit**

Run: `cargo test -p vox-skill-discovery report`
Expected: 3 tests PASS.
```bash
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/src
git commit -m "feat(vox-skill-discovery): terminal + JSON reporter"
```

---

### Task 13: `vox-discover` binary [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-skill-discovery/src/bin/vox_discover.rs`

- [ ] **Step 1: Write the CLI binary**

Set `crates/vox-skill-discovery/src/bin/vox_discover.rs` to:
```rust
//! `vox-discover` — on-demand local discovery + dedup. Advisory only; never
//! installs, executes, or publishes.

use std::path::PathBuf;

use clap::Parser;
use vox_plugin_types::skill_manifest::SkillManifest;
use vox_skill_discovery::{
    catalog::{dedup_skills, validate_ssot},
    code_miner::mine_repeated_code,
    render_json, render_terminal, Candidate, DiscoverOptions,
};

#[derive(Parser, Debug)]
#[command(name = "vox-discover", about = "Local skill/code discovery + dedup (advisory)")]
struct Args {
    /// Repository root to scan for repeated `.vox` code blocks.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Comma-separated sources: code,installed
    #[arg(long, default_value = "code")]
    source: String,

    /// Path to a JSON file containing `[SkillManifest, ...]` for installed-source checks.
    #[arg(long)]
    manifests: Option<PathBuf>,

    /// Output format: terminal | json
    #[arg(long, default_value = "terminal")]
    format: String,

    /// Minimum token count for a code block.
    #[arg(long, default_value_t = 40)]
    min_tokens: usize,

    /// Minimum occurrences for a code cluster.
    #[arg(long, default_value_t = 3)]
    min_occurrences: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let opts = DiscoverOptions {
        min_tokens: args.min_tokens,
        min_occurrences: args.min_occurrences,
        ..DiscoverOptions::default()
    };
    let sources: Vec<&str> = args.source.split(',').map(|s| s.trim()).collect();

    let mut candidates: Vec<Candidate> = Vec::new();

    if sources.contains(&"code") {
        candidates.extend(mine_repeated_code(&args.root, &opts));
    }

    if sources.contains(&"installed") {
        let manifests: Vec<SkillManifest> = match &args.manifests {
            Some(path) => {
                let raw = std::fs::read_to_string(path)?;
                serde_json::from_str(&raw)?
            }
            None => Vec::new(),
        };
        candidates.extend(dedup_skills(&manifests, &opts));
        candidates.extend(validate_ssot(&manifests));
    }

    let rendered = match args.format.as_str() {
        "json" => render_json(&candidates)?,
        _ => render_terminal(&candidates),
    };
    println!("{rendered}");
    Ok(())
}
```

> VERIFY-BEFORE-USE: this references `vox_skill_discovery::catalog::dedup_skills`,
> `::catalog::validate_ssot`, `::code_miner::mine_repeated_code`, `render_json`,
> `render_terminal`, `Candidate`, `DiscoverOptions`. All were defined/exported in
> Tasks 6–12. If any import fails, run `rg -n 'pub use|pub mod' crates/vox-skill-discovery/src/lib.rs`
> and fix the path — do NOT invent a new symbol.

- [ ] **Step 2: Build the binary**

Run: `cargo build -p vox-skill-discovery --bin vox-discover`
Expected: compiles.

- [ ] **Step 3: Smoke-test on this repo (the headline behavior)**

Run: `cargo run -p vox-skill-discovery --bin vox-discover -- --root crates/vox-similarity --min-tokens 20 --min-occurrences 2`
Expected: prints either "No discovery candidates found." or a list of `RepeatedCode` candidates with `path:line` members. Either is a PASS (it ran end-to-end without panicking).

- [ ] **Step 4: Clippy, format, commit**

Run: `cargo clippy -p vox-skill-discovery -- -D warnings`
Expected: clean.
```bash
cargo fmt -p vox-skill-discovery
git add crates/vox-skill-discovery/src/bin/vox_discover.rs
git commit -m "feat(vox-skill-discovery): vox-discover CLI binary"
```

---

### Task 14: Final verification + arch-check [SEQUENTIAL]

**Files:** none (verification only).

- [ ] **Step 1: Full test pass on both crates**

Run: `cargo test -p vox-similarity -p vox-skill-discovery`
Expected: all tests PASS (paste the counts).

- [ ] **Step 2: Clippy both crates**

Run: `cargo clippy -p vox-similarity -p vox-skill-discovery -- -D warnings`
Expected: clean.

- [ ] **Step 3: Architecture parity**

Run: `cargo run -p vox-arch-check`
Expected: exits 0.

- [ ] **Step 4: Stub check (no placeholders shipped)**

Run: `cargo run -p vox-cli -- stub-check` (or `vox stub-check` if installed)
Expected: no stubs reported in the two new crates. If `stub-check` is unavailable, run `rg -n 'todo!|unimplemented!|TODO|FIXME' crates/vox-similarity/src crates/vox-skill-discovery/src` and confirm zero hits.

- [ ] **Step 5: Commit any formatting drift**

```bash
git status --short
# if clean, nothing to do; otherwise:
cargo fmt -p vox-similarity -p vox-skill-discovery
git add -A && git commit -m "chore(discovery): final fmt"
```

---

## Deferred follow-up plans (each its own spec → plan → build)

These are intentionally **out of scope** for this wedge plan. Each ships independently:

1. **`PromptFlowMiner`** — mine recurring prompts/agent flows from `vox-db` session transcripts → `RepeatedPrompt` candidates. New source adapter; reuses `vox-similarity` unchanged. Privacy: opt-in, local-only, never auto-published.
2. **`RegistrySource`** — query `SkillsRegistryClient` (`crates/vox-skills/src/registry_api.rs`) to surface importable external skills → `ImportableExternal` candidates. Needs network + offline-tolerance tests.
3. **`vox-db` result cache** — content-hash-keyed cache (à la `visus_review`) so re-runs only re-review changed inputs.
4. **`vox skill discover` CLI integration** — thread the engine into the monolithic `vox-cli` `skill` subcommand enum (the wedge ships as the standalone `vox-discover` binary to avoid coupling to that large surface during the weak-model run).
5. **Live `SkillRegistry` source** — replace the `--manifests <json>` input with manifests pulled from a hydrated `SkillRegistry`, plus the `tool → skills` reverse index.

These feed marketplace subsystems **B** (agentic submission review), **C** (decentralized distribution + signing), and **D** (GUI), per `docs/src/architecture/skill-code-marketplace-research-and-audit-2026-06-18.md`.

---

## Self-review notes (author)

- **Spec coverage:** code-block mining (Tasks 7–9), installed dedup (Task 10), MCP↔SSOT drift byproduct (Task 11), `Candidate`/`DraftFrontmatter` model + advisory posture (Task 6, no install/exec/publish path anywhere), config tunables (Task 6 `DiscoverOptions`), layered crates L2/L3 (Tasks 1, 5), reporter (Task 12), CLI (Task 13). Prompts/registry/db-cache explicitly deferred (spec §1 phasing) — listed above.
- **Type consistency:** `Fragment::new(id, kind, text, source_ref, shingle_k, num_hashes)` used identically in Tasks 8 and 10; `DiscoverOptions::num_hashes()` = `bands*rows`; `Candidate { kind, members, score, suggested_action, draft_frontmatter }` consistent across Tasks 6/8/10/11/12.
- **Antigravity shaping:** every task ends with build/test + commit; external symbols (`SkillManifest`, `TOOL_REGISTRY`, `SKILL_TOOLS`, `ORCHESTRATOR_TOOLS`) gated behind verify-before-use `rg` steps; tasks are self-contained (full code inline, no "same as Task N"); `cargo fmt -p` only.
</content>
