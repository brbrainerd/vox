# P0 — `vox-vcs` Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a new L3 crate `vox-vcs` holding a `VcsBackend` trait plus a fully-working
self-contained in-memory `CasFallback` implementation, wire it into the workspace/arch-check, and
bump `jj-lib` to `0.42` — all with **zero behavior change** to existing callers.

**Architecture:** `vox-vcs` is the single future home of all `jj_lib::` calls (D4 in the spec). In P0
it ships only the trait + a new in-memory `CasFallback` (NOT a reuse of `vox-orchestrator`'s
`SnapshotStore` — that would create a dependency cycle, since `vox-orchestrator` will depend on
`vox-vcs`). The real `JjBackend` (jj-lib calls) is deferred to **P2**, where each method gets its own
research+TDD task against the jj-lib 0.42 API. P0 only proves jj-lib links and reserves the seams.

**Tech Stack:** Rust (edition via workspace), `jj-lib = 0.42`, `vox-arch-check`, cargo test.

**Source spec:** [`docs/superpowers/specs/2026-06-05-jj-first-class-vcs-design.md`](../specs/2026-06-05-jj-first-class-vcs-design.md)

---

## File Structure

| File | Responsibility |
|---|---|
| Create `crates/vox-vcs/Cargo.toml` | Crate manifest (mirrors `vox-git`, L3) |
| Create `crates/vox-vcs/src/lib.rs` | Crate root + module wiring + `//!` docstring |
| Create `crates/vox-vcs/src/types.rs` | Vox-native VCS types (`ChangeId`, `Change`, `Diff`, `Conflict`, `ResolveStrategy`, …) |
| Create `crates/vox-vcs/src/backend.rs` | `VcsBackend` trait + `VcsBackendKind` + `detect()` |
| Create `crates/vox-vcs/src/cas_fallback.rs` | `CasFallback` — self-contained in-memory impl |
| Modify root `Cargo.toml` | Add `vox-vcs` workspace member; bump `jj-lib` `=0.27.0`→`0.42`; add to `[workspace.dependencies]` |
| Modify `docs/src/architecture/layers.toml` | Add `vox-vcs = { layer = 3, max_loc = 20_000 }`; add `[[forbidden_pattern]]` for `jj_lib::`; fix stale exempt row |
| Modify `docs/src/architecture/where-things-live.md` | Add L3 row for `vox-vcs` |

---

### Task 1: Scaffold the `vox-vcs` crate

**Files:**
- Create: `crates/vox-vcs/Cargo.toml`
- Create: `crates/vox-vcs/src/lib.rs`
- Modify: `Cargo.toml` (root — `members` list + `[workspace.dependencies]`)

- [ ] **Step 1: Create the crate manifest**

`crates/vox-vcs/Cargo.toml` (mirrors `vox-git`'s structure):

```toml
[package]
name = "vox-vcs"
description = "VCS backend abstraction for Vox — VcsBackend trait, in-memory CasFallback, and (P2) the jj-lib 0.42 engine. The single home for all jj_lib:: calls."
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
workspace-hack = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 2: Create the crate root with a docstring and a smoke test**

`crates/vox-vcs/src/lib.rs`:

```rust
//! `vox-vcs` — the VCS backend abstraction for Vox.
//!
//! All `jj_lib::` calls are confined to this crate (see the arch-check
//! `jj-lib-confined` forbidden pattern). In P0 this crate ships the
//! [`backend::VcsBackend`] trait and a self-contained in-memory
//! [`cas_fallback::CasFallback`]. The real `JjBackend` (jj-lib 0.42) lands in P2.

pub mod backend;
pub mod cas_fallback;
pub mod types;

pub use backend::{VcsBackend, VcsBackendKind, detect};
pub use cas_fallback::CasFallback;
pub use types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};

#[cfg(test)]
mod tests {
    #[test]
    fn crate_links() {
        // Smoke test: the crate compiles and its public surface is reachable.
        assert_eq!(super::ChangeId(1).0, 1);
    }
}
```

(Note: `ChangeId` is defined in Task 2; this test will fail to compile until then — that is the
intended TDD red. If executing strictly task-by-task, write Task 2's `types.rs` before running.)

- [ ] **Step 3: Register the crate in the workspace**

In root `Cargo.toml`, add `"crates/vox-vcs"` to the `members` array (keep the array sorted), and add
to `[workspace.dependencies]` next to the other `vox-*` entries:

```toml
vox-vcs            = { path = "crates/vox-vcs" }
```

- [ ] **Step 4: Verify the crate builds**

Run: `cargo build -p vox-vcs`
Expected: compiles (after Task 2 lands `types.rs`).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-vcs/Cargo.toml crates/vox-vcs/src/lib.rs Cargo.toml
git commit -m "feat(vox-vcs): scaffold L3 crate skeleton"
```

---

### Task 2: Vox-native VCS types

**Files:**
- Create: `crates/vox-vcs/src/types.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-vcs/src/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_id_displays_with_prefix() {
        assert_eq!(format!("{}", ChangeId(42)), "chg-000042");
    }

    #[test]
    fn diff_default_is_empty() {
        let d = Diff::default();
        assert!(d.changed_paths.is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-vcs types::tests -- --nocapture`
Expected: FAIL — `ChangeId`/`Diff` not defined.

- [ ] **Step 3: Write the types**

Prepend to `crates/vox-vcs/src/types.rs`:

```rust
//! Vox-native VCS value types. jj-lib types never leak across this boundary.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Stable identifier for a change (jj "change id" analogue).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangeId(pub u64);

impl fmt::Display for ChangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "chg-{:06}", self.0)
    }
}

/// A recorded change / snapshot in the operation log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub id: ChangeId,
    pub label: Option<String>,
    pub changed_paths: Vec<PathBuf>,
}

/// A diff between two changes (or working copy and a change).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diff {
    pub changed_paths: Vec<PathBuf>,
}

/// A first-class conflict (jj "conflict as data").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub path: PathBuf,
    pub sides: Vec<String>,
}

/// Strategy for resolving a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolveStrategy {
    TakeLeft,
    TakeRight,
    Manual,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-vcs types::tests`
Expected: PASS (and the `lib.rs` `crate_links` smoke test now compiles).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-vcs/src/types.rs
git commit -m "feat(vox-vcs): Vox-native VCS value types"
```

---

### Task 3: `VcsBackend` trait + `CasFallback` in-memory impl

**Files:**
- Create: `crates/vox-vcs/src/backend.rs`
- Create: `crates/vox-vcs/src/cas_fallback.rs`

- [ ] **Step 1: Write the failing test**

`crates/vox-vcs/src/cas_fallback.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::VcsBackend;
    use std::path::PathBuf;

    #[test]
    fn snapshot_then_changes_roundtrips() {
        let mut b = CasFallback::new();
        let id = b.snapshot(Some("first"), vec![PathBuf::from("a.rs")]).unwrap();
        assert_eq!(id.0, 1);
        let changes = b.changes().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].label.as_deref(), Some("first"));
    }

    #[test]
    fn undo_drops_the_last_change() {
        let mut b = CasFallback::new();
        b.snapshot(None, vec![]).unwrap();
        b.snapshot(None, vec![]).unwrap();
        b.undo().unwrap();
        assert_eq!(b.changes().unwrap().len(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-vcs cas_fallback`
Expected: FAIL — `VcsBackend`/`CasFallback` not defined.

- [ ] **Step 3: Define the trait**

`crates/vox-vcs/src/backend.rs`:

```rust
//! The `VcsBackend` trait and runtime backend selection.

use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use std::path::{Path, PathBuf};

/// Errors a backend can surface. Kept deliberately small in P0.
#[derive(Debug, thiserror::Error)]
pub enum VcsError {
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("backend unavailable: {0}")]
    Unavailable(String),
}

/// A version-control backend. All methods are object-safe so callers can hold
/// a `Box<dyn VcsBackend>` (the orchestrator/CLI inject the concrete impl).
pub trait VcsBackend: Send + Sync {
    /// Capture the current working state as a new change. Returns its id.
    fn snapshot(&mut self, label: Option<&str>, paths: Vec<PathBuf>) -> Result<ChangeId, VcsError>;
    /// The operation log, oldest first.
    fn changes(&self) -> Result<Vec<Change>, VcsError>;
    /// Diff between two changes (or working copy when `None`).
    fn diff(&self, a: Option<ChangeId>, b: Option<ChangeId>) -> Result<Diff, VcsError>;
    /// Undo the most recent change.
    fn undo(&mut self) -> Result<ChangeId, VcsError>;
    /// First-class conflicts.
    fn conflicts(&self) -> Result<Vec<Conflict>, VcsError>;
    /// Resolve a conflict on `path` with `strategy`.
    fn resolve(&mut self, path: &Path, strategy: ResolveStrategy) -> Result<(), VcsError>;
}

/// Which backend is active at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcsBackendKind {
    /// jj-lib in-process engine (built in P2).
    Jj,
    /// In-memory fallback (always available).
    Cas,
}

/// Choose a backend for `repo_root`. In P0 the jj engine does not exist yet,
/// so this always returns [`VcsBackendKind::Cas`]; P2 makes it prefer `Jj`
/// when a colocated jj/git repo is present and initable.
pub fn detect(_repo_root: &Path) -> VcsBackendKind {
    VcsBackendKind::Cas
}
```

- [ ] **Step 4: Implement `CasFallback`**

Prepend to `crates/vox-vcs/src/cas_fallback.rs`:

```rust
//! Self-contained in-memory backend. Independent of `vox-orchestrator`'s
//! `SnapshotStore` to avoid a dependency cycle (vox-orchestrator → vox-vcs).

use crate::backend::{VcsBackend, VcsError};
use crate::types::{Change, ChangeId, Conflict, Diff, ResolveStrategy};
use std::path::{Path, PathBuf};

/// In-memory operation log. Cheap, always-available degraded mode.
#[derive(Debug, Default)]
pub struct CasFallback {
    changes: Vec<Change>,
    next_id: u64,
}

impl CasFallback {
    pub fn new() -> Self {
        Self { changes: Vec::new(), next_id: 0 }
    }
}

impl VcsBackend for CasFallback {
    fn snapshot(&mut self, label: Option<&str>, paths: Vec<PathBuf>) -> Result<ChangeId, VcsError> {
        self.next_id += 1;
        let id = ChangeId(self.next_id);
        self.changes.push(Change { id, label: label.map(str::to_owned), changed_paths: paths });
        Ok(id)
    }
    fn changes(&self) -> Result<Vec<Change>, VcsError> {
        Ok(self.changes.clone())
    }
    fn diff(&self, _a: Option<ChangeId>, _b: Option<ChangeId>) -> Result<Diff, VcsError> {
        Ok(Diff { changed_paths: self.changes.last().map(|c| c.changed_paths.clone()).unwrap_or_default() })
    }
    fn undo(&mut self) -> Result<ChangeId, VcsError> {
        self.changes.pop().map(|c| c.id).ok_or(VcsError::NothingToUndo)
    }
    fn conflicts(&self) -> Result<Vec<Conflict>, VcsError> {
        Ok(Vec::new())
    }
    fn resolve(&mut self, _path: &Path, _strategy: ResolveStrategy) -> Result<(), VcsError> {
        Ok(())
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vox-vcs`
Expected: PASS (types + cas_fallback + smoke).

- [ ] **Step 6: Format and commit**

```bash
cargo fmt -p vox-vcs
git add crates/vox-vcs/src/backend.rs crates/vox-vcs/src/cas_fallback.rs
git commit -m "feat(vox-vcs): VcsBackend trait + in-memory CasFallback"
```

---

### Task 4: Bump jj-lib to 0.42, arch-check rows, stale-row cleanup

**Files:**
- Modify: `Cargo.toml` (root — `jj-lib` version)
- Modify: `docs/src/architecture/layers.toml`
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Bump jj-lib (safe — no jj-lib APIs are called yet)**

In root `Cargo.toml` change:

```toml
jj-lib             = { version = "=0.27.0", default-features = false }
```
to:
```toml
jj-lib             = { version = "=0.42.0", default-features = false }
```

(Verified safe: `jj_backend.rs` calls zero jj-lib APIs; the `jj-backend` feature on
`vox-orchestrator`/`vox-git` stays untouched — it is removed in P2 alongside the `JjBridge`
replacement.)

- [ ] **Step 2: Add the layer row + forbidden pattern + fix the stale exempt path**

In `docs/src/architecture/layers.toml`, in the L3 crate block (near `vox-git = { layer = 3 }`), add:

```toml
vox-vcs                 = { layer = 3, max_loc = 20_000 }
```

Append a new forbidden pattern after the `raw-git-exec` block:

```toml
[[forbidden_pattern]]
name             = "jj-lib-confined"
pattern          = 'jj_lib::'
file_glob        = "crates/**/*.rs"
exempt_files     = [
    "crates/vox-vcs/src/jj_backend.rs",
]
allow_annotation = "// vox-arch-check: allow jj-lib"
reason           = "All jj-lib calls must live in vox-vcs so the unstable jj-lib API has one auditable blast radius."
```

In the existing `raw-git-exec` `exempt_files` list, replace the stale non-existent path
`"crates/vox-vcs-git/src/git_exec.rs",` with the real one:

```toml
    "crates/vox-orchestrator-mcp/src/git_exec.rs",
```

(That real path is already present later in the list — so this is a straight deletion of the stale
`vox-vcs-git` line; do not duplicate.)

- [ ] **Step 3: Add the where-things-live row**

In `docs/src/architecture/where-things-live.md`, in the L3 table, add (alongside the `vox-git` row):

```markdown
| [`vox-vcs`](../../../crates/vox-vcs/) | VCS backend abstraction: `VcsBackend` trait + in-memory `CasFallback`; the single home for all `jj_lib::` calls (jj-lib 0.42 `JjBackend` lands in P2). Injected as a trait object into `vox-compiler` to avoid L3 coupling. |
```

- [ ] **Step 4: Run arch-check**

Run: `cargo run -p vox-arch-check`
Expected: exit 0 (clean). The new crate is mapped (layers.toml + WTL + disk parity), the
forbidden-pattern compiles, and the stale-row error is gone.

- [ ] **Step 5: Build the whole workspace to confirm the jj-lib bump resolves**

Run: `cargo build -p vox-orchestrator -p vox-git -p vox-vcs`
Expected: compiles. (If the resolver complains about a transitive `gix` pin, capture the error —
this is the known gix-version interaction noted in the spec risks; resolve by letting jj-lib pull its
own `gix` and removing any stale `=` pin on `gix` in the root manifest.)

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "build(vox-vcs): jj-lib 0.42 + arch-check rows + stale-exempt cleanup"
```

---

## Self-Review

- **Spec coverage (P0 row):** crate + trait + `CasFallback` ✓ (T1–T3); jj-lib `0.42` non-optional in
  `vox-vcs` ✓ (T1/T4); layers.toml + WTL + forbidden-pattern ✓ (T4); stale exempt row fixed ✓ (T4);
  **zero behavior change** ✓ (nothing existing is rewired; `JjBackend`/feature-removal deferred to P2).
- **Refinement vs spec:** `CasFallback` is a NEW in-memory impl, not a reuse of `vox-orchestrator`'s
  `SnapshotStore` (cycle avoidance). `JjBackend` moves from P0 to P2. Both noted for a spec edit.
- **Placeholders:** none — every step shows real code/commands.
- **Type consistency:** `ChangeId`, `Change`, `Diff`, `Conflict`, `ResolveStrategy`, `VcsBackend`,
  `VcsError`, `CasFallback`, `VcsBackendKind`, `detect` are used consistently across tasks.
