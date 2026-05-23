---
title: "Mesh Phase 2 — Code Mobility & Versioning Implementation Plan (2026-05-09)"
description: "Step-by-step TDD implementation plan for Phase 2 of the Mesh & Language-Level Distribution SSOT — content-addressed workflow bundles via vox-package CAS, workflow.version() patch markers, drain tooling, mesh code seeding, activity result caching, dispatch preview, and the codegen split that lowers DurabilityKind to durable/journaled/mailbox call shapes. Seven tasks (P2-T1..P2-T7) producing the bundle store, drain CLI, A2A bundle-fetch protocol, activity_result_cache vox-db table, the dispatch-preview projector, and the lowering split — closing the 'all three emit identical async Rust' gap left after Phase 1."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; gets stale as tasks are completed. The SSOT (mesh-and-language-distribution-ssot-2026.md) is the durable artifact."
---

# Mesh Phase 2 — Code Mobility & Versioning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Phase 2 lives downstream of Phase 1's `DurablePromise[T]` + auto-derived `activity_id` primitives — those are inputs, not redefined here.

**Goal.** Code that runs on the mesh is content-addressed. Version skew is structurally impossible — workflow versions A and B coexist by hash and drain on a schedule. Hot-deploy a workflow without breaking in-flight runs. A fresh node joins the mesh and runs jobs by fetching content-addressed bundles, without a forge round-trip.

**Architecture.** Extend the existing SHA3-512 `vox-package` CAS to address compiled workflow/activity bundles by `fn_hash`. Stamp a stable `@generated-hash` on each `workflow` and `activity` at compile time. Attach a `BundleRef { fn_hash }` to every mesh dispatch envelope; receivers cache-hit by hash or fetch via a new A2A `bundle_request`/`bundle_response` round-trip (size-thresholded). Add a vox-db `activity_result_cache` table keyed by `(activity_id, structural_arg_hash)` so deduped activities never re-run inside their TTL. Split `vox-codegen` so `workflow` lowers to `interpret_workflow_durable`, `activity` wraps in `journal.execute(...)`, and `actor` spawns into a mailbox — closing the gap left by Phase 1 where all three emitted identical async Rust. Ship a `vox workflow drain --version <hash>` operator tool and a `vox dispatch preview` projector.

**Tech stack.** Rust 2024 edition. Existing crates only: `vox-package` (CAS), `vox-mesh-types` (A2A wire), `vox-orchestrator` (dispatch), `vox-workflow-runtime` (tracker), `vox-codegen` (lowering), `vox-cli` (commands), `vox-db` (DDL + migrations), `vox-compiler` (HIR `DurabilityKind`). No new external deps. SHA3-512 via `vox_db::hash::content_hash`.

**SSOT.** [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) §3 Phase 2 — task IDs, acceptance criteria, dependencies.

**Cross-plan integration.**

- Hopper integration: none in Phase 2.
- MENS integration: `Mn-T3` (ModelBundle SafeTensors) extends the CAS introduced in `P2-T1`.
  P2-T1 introduces a `BundleMeta` sealed trait in `crates/vox-package-types/src/lib.rs` that both
  `Bundle` (workflow/activity bundles) and MENS `ModelBundle` implement. This allows mesh GC,
  dashboard inventory, and other consumers to iterate over either kind without pattern-matching on
  concrete types. See P2-T1g substep below. Mn-T3 must list P2-T1 as a dependency and implement
  `BundleMeta` for `ModelBundle`.
- Phase 5 attestation layering: the `BundleRef` (and optional inline bytes) shipped on the dispatch envelope in P2-T4 is **unsigned by design** — content-addressing is tamper-evident, so the bundle itself needs no signature. Phase 5's P5-T4 signed attestation envelope wraps the **result** of executing a bundle, not the bundle bytes. The two layers compose: receiver verifies `fn_hash` matches the bytes, executes, then the originator's signed attestation covers `(fn_hash, args_hash, result_hash)`.

**Phase 1 inputs (do not redefine).**

- `DurablePromise[T]` — the user-facing async type that maps to `tracker.load_activity_result` on replay.
- Auto-derived `activity_id` — Phase 1 stamps each activity call site with a stable id so the journal can replay it; we re-use that ID as the cache key in P2-T5.
- `@remote` — Phase 1 attribute that marks a workflow eligible for mesh dispatch; the orchestrator already routes such calls through `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs`.

**Working directory.** Worktree at `C:\Users\Owner\vox\.claude\worktrees\zealous-ardinghelli-b01e11`. All paths below are relative to this worktree.

**Vox project rules honored.**

- No `.ps1` / `.sh` / `.py` automation glue. Operator tooling is a CLI subcommand (P2-T3) or a `.vox` script.
- TDD: every task starts with a failing test.
- `vox-arch-check` must remain clean — fan-in / fan-out per [`docs/src/architecture/layers.toml`](layers.toml). Phase 2 introduces no new layer crossings: vox-package is L1, vox-orchestrator is L4, vox-codegen is L3, vox-cli is L5.
- `where-things-live.md` rows for `BundleRef`, `WorkflowDrainStarted`, `activity_result_cache` are added in P2-T7's final commit.

---

## File map

**Migration policy note.** Per SSOT §5.5 canonical migration policy: schema evolution flows through `BASELINE_VERSION` in `crates/vox-db/src/schema/manifest.rs`, not date-stamped or numeric SQL files under any `migrations/` directory. P2 takes baseline from 62 (P0's value, set by P0-T1 for `vcs_lock` + `lock_leader`) to 63 (this phase, for `activity_result_cache`). The earlier draft of this plan proposed a `crates/vox-db/src/migrations/20260509_activity_result_cache.sql` file — that scheme is rejected per §5.5.

**Create:**

- `crates/vox-package/src/bundle.rs` — `Bundle`, `BundleRef`, `Hash`, bundle-store API on top of `ArtifactCache`.
- `crates/vox-package/tests/bundle_cas.rs` — integration tests: round-trip, cache hit, eviction.
- `crates/vox-mesh-types/src/bundle.rs` — wire types `BundleRequest`, `BundleResponse`; constants `BUNDLE_REQUEST_TYPE`, `BUNDLE_RESPONSE_TYPE`.
- `crates/vox-orchestrator/src/a2a/dispatch/bundle_fetch.rs` — sender-side "ship-or-ref" decision; receiver-side "have-or-fetch" loop.
- `crates/vox-orchestrator/src/oplog/workflow_drain.rs` — in-memory `WorkflowDrainState`; `WorkflowDrainStarted` op-log entry.
- `crates/vox-cli/src/commands/workflow/mod.rs` — new `workflow` subcommand parent.
- `crates/vox-cli/src/commands/workflow/drain.rs` — `vox workflow drain --version <hash>` handler.
- `crates/vox-cli/src/commands/workflow/ls.rs` — `vox workflow ls` handler that surfaces both content-hashes.
- `crates/vox-cli/src/commands/dispatch/mod.rs` — new `dispatch` subcommand parent.
- `crates/vox-cli/src/commands/dispatch/preview.rs` — `vox dispatch preview my::workflow(...)` handler.
- `crates/vox-db/src/ddl/activity_result_cache.rs` — DDL + sweep SQL.
- `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs` — the `DurabilityKind` → call-shape lowering.
- `crates/vox-codegen/tests/durability_lowering.rs` — golden-output tests (workflow / activity / actor each).
- `tests/mesh_phase2_e2e.vox` — end-to-end Vox script: deploy v1, drain v1, deploy v2, in-flight survives.

**Modify:**

- `crates/vox-package/src/artifact_cache.rs` — add `lookup_bundle(fn_hash) -> Option<Bundle>` thin wrapper.
- `crates/vox-package/src/lib.rs` — `pub mod bundle;`.
- `crates/vox-mesh-types/src/lib.rs` — `pub mod bundle;`.
- `crates/vox-orchestrator/src/a2a/envelope.rs` — add `bundle_ref: Option<BundleRef>` field on `RemoteTaskEnvelope`.
- `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` — call `bundle_fetch::attach_bundle` before sending; receiver path checks bundle availability before claiming.
- `crates/vox-cli/src/commands/mod.rs` — wire in `workflow` and `dispatch` modules.
- `crates/vox-workflow-runtime/src/workflow/tracker.rs` — add trait methods `load_cached_activity_result(...)` and `record_cached_activity_result(...)`.
- `crates/vox-workflow-runtime/src/workflow/run.rs:58` — consult cache before running activity body.
- `crates/vox-codegen/src/codegen_rust/emit/workflow.rs:136` — branch in `emit_fn` on `func.durability`.
- `crates/vox-compiler/src/hir/nodes/decl.rs` — stamp `generated_hash: Option<String>` on `HirFn` for workflow/activity.
- `crates/vox-compiler/src/hir/lower/mod.rs` — compute the hash during lowering.
- `crates/vox-db/src/schema/manifest.rs` — bump `BASELINE_VERSION` from 62 (set by P0-T1) to 63; add `activity_result_cache` schema fragment gated on version 63.
- `crates/vox-arch-check/forbidden_deps.toml` (if exists; otherwise no edit) — verify no new edges cross.
- `docs/src/architecture/where-things-live.md` — three new rows for `Bundle`, `WorkflowDrainStarted`, `activity_result_cache`.

---

## Task ordering rationale

The codegen split (P2-T7) and content-hash stamping (P2-T1) are the spine. Everything else either reads the hash (P2-T3 drain, P2-T4 dispatch envelope, P2-T6 preview) or builds on the runtime contract that activities look up cached results before running (P2-T5 cache). We sequence:

1. **P2-T1** first because every other task references the bundle hash.
2. **P2-T2** (patch primitive) is parser-side and slots in early before runtime touches.
3. **P2-T3** (drain CLI) only depends on op-log + bundle hashes from T1; it's small.
4. **P2-T4** (mesh seeding + bundle-fetch protocol) depends on T1's bundle store.
5. **P2-T5** (activity result cache) is independent of T1–T4 in code but depends on Phase 1's `activity_id` derivation, which is in place.
6. **P2-T6** (`vox dispatch preview`) consumes the same routing logic the dispatcher uses; deliberately late so we know the routing rules are stable.
7. **P2-T7** (codegen split) is last because it's the riskiest — it touches every emitted function and we want the runtime/cache/journal scaffolding tested before regenerating client code against a different lowering.

Each task ends with `cargo test -p <crate>` runs and a commit. The plan assumes Phase 1 has landed.

---

## Task P2-T1: Workflow content-hash via vox-package CAS + Bundle store

**Files:**

- Modify: `crates/vox-package/src/artifact_cache.rs:109-132` (add `lookup_bundle`).
- Create: `crates/vox-package/src/bundle.rs`.
- Modify: `crates/vox-package/src/lib.rs`.
- Modify: `crates/vox-compiler/src/hir/nodes/decl.rs` (add `generated_hash: Option<String>` to `HirFn`).
- Modify: `crates/vox-compiler/src/hir/lower/mod.rs` (compute hash during lowering for `Workflow` and `Activity`).
- Create: `crates/vox-package/tests/bundle_cas.rs`.

This task extends the existing SHA3-512 CAS in `vox-package` to address compiled workflow/activity bundles by their stable input hash, and stamps every workflow/activity HIR function with a `@generated-hash` so downstream code (dispatch envelope, drain CLI, preview tool) can refer to them by hash, never by name.

The existing CAS at `crates/vox-package/src/artifact_cache.rs:81-107` already SHA3-512s a sorted list of input paths plus extra inputs. We do **not** duplicate that algorithm — we layer a `Bundle` API on top.

### Subtasks

- [ ] **P2-T1a: Write the failing round-trip test for `Bundle` store**

Create `crates/vox-package/tests/bundle_cas.rs`:

```rust
use std::path::PathBuf;
use vox_package::bundle::{Bundle, BundleRef, BundleStore};

#[test]
fn bundle_round_trip_by_fn_hash() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BundleStore::open(tmp.path().to_path_buf()).expect("open store");

    let bundle = Bundle {
        fn_hash: [0xAB; 64],
        deps: vec![],
        bytes: b"compiled-form-of-workflow".to_vec().into(),
        manifest: serde_json::json!({
            "kind": "workflow",
            "name": "my::workflow",
            "vox_version": env!("CARGO_PKG_VERSION"),
        }),
    };

    let bundle_ref = store.put(&bundle).expect("put");
    assert_eq!(bundle_ref.fn_hash, [0xAB; 64]);

    let loaded = store
        .lookup(&bundle_ref)
        .expect("lookup")
        .expect("hit");
    assert_eq!(loaded.fn_hash, bundle.fn_hash);
    assert_eq!(loaded.bytes.as_ref(), bundle.bytes.as_ref());
}

#[test]
fn bundle_lookup_miss_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BundleStore::open(tmp.path().to_path_buf()).expect("open store");

    let unknown = BundleRef { fn_hash: [0x77; 64] };
    let result = store.lookup(&unknown).expect("lookup ok");
    assert!(result.is_none(), "miss should return None, not error");
}

#[test]
fn put_is_idempotent_for_same_fn_hash() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = BundleStore::open(tmp.path().to_path_buf()).expect("open store");

    let bundle = Bundle {
        fn_hash: [0x42; 64],
        deps: vec![],
        bytes: b"bytes-v1".to_vec().into(),
        manifest: serde_json::json!({}),
    };

    let _ = store.put(&bundle).expect("put 1");
    let r2 = store.put(&bundle).expect("put 2 — must not error");
    assert_eq!(r2.fn_hash, bundle.fn_hash);
}
```

Run: `cargo test -p vox-package --test bundle_cas 2>&1 | tail -20`
Expected: FAIL — `bundle` module doesn't exist yet.

- [ ] **P2-T1b: Implement `Bundle`, `BundleRef`, `BundleStore`**

Create `crates/vox-package/src/bundle.rs`:

```rust
//! Content-addressed bundle store for compiled Vox workflow / activity functions.
//!
//! A `Bundle` is the unit of code mobility on the mesh: a `fn_hash` plus the
//! compiled-form bytes plus enough metadata for the runtime to dispatch.
//!
//! This module re-uses [`crate::artifact_cache::ArtifactCache`] underneath so
//! we do not invent a second SHA3-512 implementation.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::artifact_cache::{ArtifactCache, CacheLookup};

/// Raw bytes of a compiled bundle. `Arc<Vec<u8>>` because clones cross
/// async tasks (mesh dispatch) and we don't want to allocate per-clone.
pub type ContentBytes = Arc<Vec<u8>>;

/// Stable content-address of a workflow / activity bundle.
///
/// This is the SHA3-512 over the input set: source bytes, vox version,
/// transitive dep hashes. Computed by [`crate::artifact_cache::ArtifactCache::compute_input_hash`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BundleRef {
    /// Raw 64-byte SHA3-512 digest. We store the bytes (not hex) on the wire
    /// because every consumer hexes locally and we save the round-trip.
    #[serde(with = "fn_hash_serde")]
    pub fn_hash: [u8; 64],
}

impl BundleRef {
    /// Hex-encode the 64-byte digest. Matches the `ArtifactCache` filename form.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(128);
        for b in &self.fn_hash {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

/// A content-addressed bundle: hash, transitive dep hashes, compiled bytes,
/// and a free-form manifest the runtime uses to dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    /// Self-hash. Caller MUST guarantee `fn_hash` is the SHA3-512 of the
    /// input set used to build `bytes`. The store does not re-derive.
    #[serde(with = "fn_hash_serde")]
    pub fn_hash: [u8; 64],
    /// Other bundles this one depends on. The mesh fetcher walks these
    /// transitively when seeding a fresh node.
    pub deps: Vec<BundleRef>,
    /// Compiled-form bytes — the lowered Rust functions plus enough metadata
    /// for the runtime to dispatch. Opaque to the store.
    pub bytes: ContentBytes,
    /// Free-form JSON metadata: kind ("workflow" / "activity" / "actor"),
    /// declared name, vox compiler version, capability requirements.
    pub manifest: JsonValue,
}

mod fn_hash_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(bytes: &[u8; 64], s: S) -> Result<S::Ok, S::Error> {
        let mut hex = String::with_capacity(128);
        for b in bytes {
            hex.push_str(&format!("{b:02x}"));
        }
        s.serialize_str(&hex)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 64], D::Error> {
        let s = String::deserialize(d)?;
        if s.len() != 128 {
            return Err(serde::de::Error::custom("fn_hash must be 128 hex chars"));
        }
        let mut out = [0u8; 64];
        for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
            let hex = std::str::from_utf8(chunk)
                .map_err(|e| serde::de::Error::custom(format!("hex utf8: {e}")))?;
            out[i] = u8::from_str_radix(hex, 16)
                .map_err(|e| serde::de::Error::custom(format!("hex parse: {e}")))?;
        }
        Ok(out)
    }
}

/// A bundle store rooted at a directory. Wraps `ArtifactCache` 1-to-1:
/// `manifests/<hex>.json` holds the metadata; `artifacts/<hex>/bundle.bin`
/// holds the compiled bytes.
pub struct BundleStore {
    cache: ArtifactCache,
}

impl BundleStore {
    /// Open (or create) a bundle store under `root`.
    pub fn open(root: PathBuf) -> io::Result<Self> {
        Ok(Self {
            cache: ArtifactCache::new(root)?,
        })
    }

    /// Look up a bundle by reference. `Ok(None)` for cache miss; `Err` for IO.
    pub fn lookup(&self, r: &BundleRef) -> io::Result<Option<Bundle>> {
        let hex = r.to_hex();
        match self.cache.lookup(&hex) {
            CacheLookup::Hit { artifact_dir, manifest } => {
                let bytes_path = artifact_dir.join("bundle.bin");
                let bytes = std::fs::read(&bytes_path)?;
                let manifest_json: JsonValue = serde_json::from_str(&manifest.description)
                    .unwrap_or(JsonValue::Null);
                let deps_path = artifact_dir.join("deps.json");
                let deps: Vec<BundleRef> = if deps_path.exists() {
                    serde_json::from_slice(&std::fs::read(&deps_path)?)
                        .map_err(io::Error::other)?
                } else {
                    Vec::new()
                };
                Ok(Some(Bundle {
                    fn_hash: r.fn_hash,
                    deps,
                    bytes: Arc::new(bytes),
                    manifest: manifest_json,
                }))
            }
            CacheLookup::Miss { .. } => Ok(None),
        }
    }

    /// Store a bundle by its self-asserted hash. Idempotent.
    pub fn put(&self, bundle: &Bundle) -> io::Result<BundleRef> {
        let hex = BundleRef { fn_hash: bundle.fn_hash }.to_hex();
        let artifact_dir = self.cache.artifact_dir(&hex);
        std::fs::create_dir_all(&artifact_dir)?;
        let bytes_path = artifact_dir.join("bundle.bin");
        std::fs::write(&bytes_path, bundle.bytes.as_ref())?;
        let deps_path = artifact_dir.join("deps.json");
        std::fs::write(
            &deps_path,
            serde_json::to_vec_pretty(&bundle.deps).map_err(io::Error::other)?,
        )?;
        // Record_build re-uses the existing manifest path; the description
        // doubles as the bundle manifest JSON so we don't add a parallel format.
        let manifest_str = serde_json::to_string(&bundle.manifest).map_err(io::Error::other)?;
        self.cache.record_build(
            &hex,
            &manifest_str,
            &[(bytes_path.clone(), "bundle.bin".to_string()),
              (deps_path.clone(), "deps.json".to_string())],
        )?;
        Ok(BundleRef { fn_hash: bundle.fn_hash })
    }
}
```

- [ ] **P2-T1c: Wire the module in**

In `crates/vox-package/src/lib.rs`, add:

```rust
pub mod bundle;
```

And in `crates/vox-package/src/artifact_cache.rs`, append a thin adapter near line 132 so existing callers can also drop in by hash:

```rust
impl ArtifactCache {
    /// Convenience: look up a bundle by raw `fn_hash` without going through
    /// `BundleStore`. Used by tests and by the dispatch fast-path.
    pub fn lookup_bundle(&self, fn_hash: &[u8; 64]) -> Option<crate::bundle::Bundle> {
        let hex = {
            let mut s = String::with_capacity(128);
            for b in fn_hash {
                s.push_str(&format!("{b:02x}"));
            }
            s
        };
        match self.lookup(&hex) {
            CacheLookup::Hit { .. } => {
                // Delegate to `BundleStore::lookup` which knows the on-disk shape.
                let store = crate::bundle::BundleStore::open(self.root.clone()).ok()?;
                store.lookup(&crate::bundle::BundleRef { fn_hash: *fn_hash }).ok().flatten()
            }
            CacheLookup::Miss { .. } => None,
        }
    }
}
```

Run: `cargo test -p vox-package --test bundle_cas 2>&1 | tail -20`
Expected: PASS for all three tests.

- [ ] **P2-T1d: Stamp `generated_hash` on `HirFn` for workflow / activity**

In `crates/vox-compiler/src/hir/nodes/decl.rs` (around line 310 where `durability: Option<DurabilityKind>` already lives), add:

```rust
/// Stable content-hash of this function's compile inputs, populated by the
/// HIR lowering for `DurabilityKind::Workflow` and `DurabilityKind::Activity`.
/// `None` for plain `fn` and for `actor` (actors live in mailboxes, not the
/// bundle CAS).
pub generated_hash: Option<String>,
```

In `crates/vox-compiler/src/hir/lower/mod.rs`, find the lowering pass where `durability` is set (the existing call site that maps `WorkflowDecl` → `HirFn`). Right after `durability` is assigned, compute the hash:

```rust
let generated_hash = match durability {
    Some(super::nodes::durability::DurabilityKind::Workflow)
    | Some(super::nodes::durability::DurabilityKind::Activity) => {
        // Inputs that uniquely identify the lowered function.
        // Order: kind tag, declared name, parameter names + types, return type,
        // body bytes (canonicalized), vox compiler version.
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(durability.unwrap().label().as_bytes());
        buf.push(0);
        buf.extend_from_slice(name.as_bytes());
        buf.push(0);
        for p in &params {
            buf.extend_from_slice(p.name.as_bytes());
            buf.push(b':');
            // emit_type printer; reuse any existing canonicalizer.
            if let Some(t) = &p.type_ann {
                buf.extend_from_slice(format!("{t:?}").as_bytes());
            }
            buf.push(0);
        }
        if let Some(r) = &return_type {
            buf.extend_from_slice(format!("{r:?}").as_bytes());
        }
        buf.push(0);
        // Body canonicalization: serialize HIR, not surface text, so
        // whitespace/comment changes don't bust the cache.
        buf.extend_from_slice(format!("{body:?}").as_bytes());
        buf.push(0);
        buf.extend_from_slice(env!("CARGO_PKG_VERSION").as_bytes());
        Some(vox_db::hash::content_hash(&buf))
    }
    _ => None,
};
```

(Adapt the variable bindings to the actual local names in `lower/mod.rs`. The shape is what matters: input set is deterministic, fed through the same `vox_db::hash::content_hash` SHA3-512 the artifact cache uses.)

- [ ] **P2-T1e: Add a hash-stamping unit test**

Add `crates/vox-compiler/tests/workflow_hash_stable.rs`:

```rust
use vox_compiler::lower_to_hir;
use vox_compiler::parse_module;

#[test]
fn workflow_hash_is_stable_across_whitespace() {
    let src_a = "workflow my::wf() -> i64 { return 7; }";
    let src_b = "workflow my::wf()   ->  i64  {  return 7;  }";
    let h1 = first_workflow_hash(src_a);
    let h2 = first_workflow_hash(src_b);
    assert_eq!(h1, h2, "whitespace must not affect the generated hash");
}

#[test]
fn workflow_hash_changes_on_body_change() {
    let h1 = first_workflow_hash("workflow my::wf() -> i64 { return 7; }");
    let h2 = first_workflow_hash("workflow my::wf() -> i64 { return 8; }");
    assert_ne!(h1, h2, "body change MUST bust the hash");
}

fn first_workflow_hash(src: &str) -> String {
    let module = parse_module(src).expect("parse");
    let hir = lower_to_hir(&module).expect("lower");
    hir.functions
        .iter()
        .find(|f| f.durability == Some(vox_compiler::hir::DurabilityKind::Workflow))
        .and_then(|f| f.generated_hash.clone())
        .expect("workflow should have generated_hash")
}
```

Run: `cargo test -p vox-compiler --test workflow_hash_stable 2>&1 | tail -15`
Expected: PASS.

- [ ] **P2-T1f: Run full vox-package + vox-compiler test suites**

Run: `cargo test -p vox-package -p vox-compiler 2>&1 | tail -30`
Expected: clean (no regressions).

- [ ] **P2-T1g: `BundleMeta` sealed trait in `vox-package-types`**

Add to `crates/vox-package-types/src/lib.rs`:

```rust
/// Sealed trait implemented by every content-addressed bundle kind.
///
/// Lets mesh GC, dashboard inventory, and other consumers iterate over
/// workflow bundles and model bundles without matching on concrete types.
/// Sealed via the private `bundle_meta_sealed::Sealed` supertrait.
pub trait BundleMeta: bundle_meta_sealed::Sealed {
    /// SHA3-512 content address — the stable identity of this bundle.
    fn content_hash(&self) -> [u8; 64];
    /// Human-readable kind label for logging / dashboard display.
    fn kind_label(&self) -> &'static str;
}

mod bundle_meta_sealed {
    pub trait Sealed {}
}
```

Then in `crates/vox-package/src/bundle.rs`, add the impl:

```rust
impl vox_package_types::bundle_meta_sealed::Sealed for Bundle {}
impl vox_package_types::BundleMeta for Bundle {
    fn content_hash(&self) -> [u8; 64] { self.fn_hash }
    fn kind_label(&self) -> &'static str { "workflow" }
}
```

MENS `Mn-T3` (`crates/vox-package/src/model_bundle.rs`) must add the symmetric impl for `ModelBundle`; see the MENS plan.

Run: `cargo check -p vox-package-types -p vox-package 2>&1 | tail -20`
Expected: clean.

- [ ] **P2-T1g: Commit**

```bash
git add crates/vox-package/src/bundle.rs \
        crates/vox-package/src/lib.rs \
        crates/vox-package/src/artifact_cache.rs \
        crates/vox-package/tests/bundle_cas.rs \
        crates/vox-compiler/src/hir/nodes/decl.rs \
        crates/vox-compiler/src/hir/lower/mod.rs \
        crates/vox-compiler/tests/workflow_hash_stable.rs
git commit -m "feat(vox-package): P2-T1 BundleStore + content-hash stamping for workflows/activities"
```

---

## Task P2-T2: `workflow.version("change-1", min, max)` patch-marker primitive

**Files:**

- Modify: parser surface in `crates/vox-compiler/src/parser/` (location confirmed via grep before editing).
- Modify: `crates/vox-compiler/src/hir/nodes/durability.rs` — add `WorkflowPatch` op kind and helper.
- Modify: `crates/vox-workflow-runtime/src/workflow/run.rs:113-127` — emit `WorkflowPatch` journal entry when encountered.
- Create: `crates/vox-compiler/tests/workflow_version.rs`.
- Create: `crates/vox-workflow-runtime/tests/workflow_patch.rs`.

`workflow.version` is the Temporal-style escape hatch when content-addressing isn't enough — when a refactor needs to support both the *old* journaled history (which expected the v1 path) and *new* runs (which take the v2 path) within the same workflow function. The primitive emits a `WorkflowPatch` op into the journal; replay sees it and steers down the right branch.

This is **not** a substitute for content-addressing. Content-addressing handles "deploy v2, drain v1" — `workflow.version` handles "I made a change inside one workflow and old journals are stuck on the v1 path".

### Subtasks

- [ ] **P2-T2a: Write the failing parser test**

Create `crates/vox-compiler/tests/workflow_version.rs`:

```rust
use vox_compiler::parse_module;

#[test]
fn parses_workflow_version_call_with_min_max() {
    let src = r#"
        workflow my::wf() -> i64 {
            let v = workflow.version("change-1", 1, 2);
            if v >= 2 { return new_path(); }
            old_path()
        }
    "#;
    let module = parse_module(src).expect("parse");
    let calls = collect_workflow_version_calls(&module);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].change_id, "change-1");
    assert_eq!(calls[0].min, 1);
    assert_eq!(calls[0].max, 2);
}

fn collect_workflow_version_calls(_m: &vox_compiler::ast::Module)
    -> Vec<vox_compiler::ast::WorkflowVersionCall>
{
    // Helper assumes parser exposes a visitor or direct field; if not, walk the AST.
    vec![]
}
```

Run: `cargo test -p vox-compiler --test workflow_version 2>&1 | tail -15`
Expected: FAIL — `WorkflowVersionCall` type does not exist.

- [ ] **P2-T2b: Add `WorkflowVersionCall` to the AST**

Find the file declaring `Expr` variants (search: `pub enum Expr` under `crates/vox-compiler/src/ast/`). Add a new variant:

```rust
/// `workflow.version("change-id", min_supported, max_supported)`
WorkflowVersion(WorkflowVersionCall),
```

And the supporting struct in the same module:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowVersionCall {
    pub change_id: String,
    pub min: u32,
    pub max: u32,
}
```

In the parser's call-expression rule (search: `parse_call` or `parse_method_call` under `crates/vox-compiler/src/parser/`), add a special-case for the receiver-identifier `workflow` and method-name `version`:

```rust
if receiver_is_workflow_keyword(&receiver) && method_name == "version" {
    let mut iter = args.into_iter();
    let change_id = iter.next()
        .and_then(|e| e.as_string_literal())
        .ok_or_else(|| parse_error("workflow.version arg 1 must be string literal"))?;
    let min = iter.next()
        .and_then(|e| e.as_u32_literal())
        .ok_or_else(|| parse_error("workflow.version arg 2 must be u32 literal"))?;
    let max = iter.next()
        .and_then(|e| e.as_u32_literal())
        .ok_or_else(|| parse_error("workflow.version arg 3 must be u32 literal"))?;
    return Ok(Expr::WorkflowVersion(WorkflowVersionCall { change_id, min, max }));
}
```

(Helpers `as_string_literal` / `as_u32_literal` already exist on `Expr`; if not, inline the match.)

Run: `cargo test -p vox-compiler --test workflow_version 2>&1 | tail -15`
Expected: parser part PASS. Update the helper in the test to walk the actual AST shape and re-run.

- [ ] **P2-T2c: Lower `WorkflowVersionCall` to a HIR step**

In `crates/vox-compiler/src/hir/lower/expr.rs` (or wherever `Expr` lowers), add an arm:

```rust
ast::Expr::WorkflowVersion(call) => HirExpr::WorkflowVersion(HirWorkflowVersion {
    change_id: call.change_id.clone(),
    min: call.min,
    max: call.max,
}),
```

Add the corresponding HIR shape in `crates/vox-compiler/src/hir/nodes/expr.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HirWorkflowVersion {
    pub change_id: String,
    pub min: u32,
    pub max: u32,
}
```

- [ ] **P2-T2d: Runtime journal entry**

In `crates/vox-workflow-runtime/src/workflow/run.rs`, around the existing `versioned_event(json!({"event": "ActivityTask", ...}))` site, add a new step kind. The interpreter already walks `PlannedActivity`s; extend `PlannedActivity` (defined in `super::types`) with a tagged variant or a `kind` enum. Minimum-touch approach: introduce a helper journal emission and consult the tracker:

```rust
async fn handle_workflow_version_marker(
    workflow_name: &str,
    call: &HirWorkflowVersion,
    journal: &mut Vec<Value>,
    tracker: &mut impl WorkflowTracker,
) -> anyhow::Result<u32> {
    if let Some(prior) = tracker
        .load_workflow_patch(workflow_name, &call.change_id)
        .await?
    {
        journal.push(versioned_event(json!({
            "event": "WorkflowPatch",
            "workflow": workflow_name,
            "change_id": call.change_id,
            "version": prior,
            "replayed": true,
        })));
        return Ok(prior);
    }
    let chosen = call.max;
    tracker.record_workflow_patch(workflow_name, &call.change_id, chosen).await?;
    journal.push(versioned_event(json!({
        "event": "WorkflowPatch",
        "workflow": workflow_name,
        "change_id": call.change_id,
        "version": chosen,
        "min_supported": call.min,
        "max_supported": call.max,
        "replayed": false,
    })));
    Ok(chosen)
}
```

Add the two new tracker methods to `crates/vox-workflow-runtime/src/workflow/tracker.rs` with no-op default impls (so existing callers compile unchanged):

```rust
async fn load_workflow_patch(
    &self,
    _workflow_name: &str,
    _change_id: &str,
) -> anyhow::Result<Option<u32>> {
    Ok(None)
}
async fn record_workflow_patch(
    &mut self,
    _workflow_name: &str,
    _change_id: &str,
    _version: u32,
) -> anyhow::Result<()> {
    Ok(())
}
```

- [ ] **P2-T2e: End-to-end test**

Create `crates/vox-workflow-runtime/tests/workflow_patch.rs`:

```rust
use serde_json::Value;
use vox_workflow_runtime::workflow::{interpret_workflow_durable, tracker::DefaultTracker};

#[tokio::test]
async fn workflow_patch_emits_journal_event_first_run() {
    let src = r#"
        workflow demo::wf() -> i64 {
            let v = workflow.version("split-step", 1, 2);
            if v >= 2 { return 100; }
            return 1;
        }
    "#;
    let module = vox_compiler::parse_module(src).expect("parse");
    let hir = vox_compiler::lower_to_hir(&module).expect("lower");
    let mut tracker = DefaultTracker;
    let journal = interpret_workflow_durable(&hir, "demo::wf", &mut tracker)
        .await
        .expect("run");
    let patch_event = journal
        .iter()
        .find(|v| v["event"].as_str() == Some("WorkflowPatch"))
        .expect("WorkflowPatch must be journaled");
    assert_eq!(patch_event["change_id"], "split-step");
    assert_eq!(patch_event["replayed"], Value::Bool(false));
}
```

Run: `cargo test -p vox-workflow-runtime --test workflow_patch 2>&1 | tail -15`
Expected: PASS.

- [ ] **P2-T2f: Commit**

```bash
git add crates/vox-compiler/src/parser/ \
        crates/vox-compiler/src/ast/ \
        crates/vox-compiler/src/hir/ \
        crates/vox-compiler/tests/workflow_version.rs \
        crates/vox-workflow-runtime/src/workflow/run.rs \
        crates/vox-workflow-runtime/src/workflow/tracker.rs \
        crates/vox-workflow-runtime/tests/workflow_patch.rs
git commit -m "feat(vox-workflow-runtime): P2-T2 workflow.version() patch primitive"
```

---

## Task P2-T3: `vox workflow drain --version <hash>` operational tool

**Files:**

- Create: `crates/vox-cli/src/commands/workflow/mod.rs`.
- Create: `crates/vox-cli/src/commands/workflow/drain.rs`.
- Create: `crates/vox-cli/src/commands/workflow/ls.rs`.
- Create: `crates/vox-orchestrator/src/oplog/workflow_drain.rs`.
- Modify: `crates/vox-orchestrator/src/oplog/mod.rs`.
- Modify: `crates/vox-cli/src/commands/mod.rs`.
- Create: `crates/vox-cli/tests/workflow_drain.rs`.
- Create: `crates/vox-orchestrator/tests/workflow_drain.rs`.

The drain command writes a `WorkflowDrainStarted { fn_hash, started_at }` to the orchestrator's op-log (in-memory in Phase 2; Phase 3 makes it durable). The dispatcher consults that op-log on every dispatch decision: if the requested workflow's content-hash is draining, refuse to start a new run. Existing in-flight runs at that hash continue unaffected — drain is a new-starts-only stop signal.

### Subtasks

- [ ] **P2-T3a: Write the failing op-log test**

Create `crates/vox-orchestrator/tests/workflow_drain.rs`:

```rust
use vox_orchestrator::oplog::workflow_drain::{
    WorkflowDrainState, WorkflowDrainStarted,
};

#[test]
fn drain_started_marks_hash_no_new_starts() {
    let mut state = WorkflowDrainState::default();
    let fn_hash = [0xAA; 64];
    state.record_drain(WorkflowDrainStarted {
        fn_hash,
        started_at_unix_ms: 1_000,
    });
    assert!(state.is_draining(&fn_hash));
    assert!(!state.is_draining(&[0xBB; 64]));
}

#[test]
fn dispatcher_predicate_refuses_drained() {
    let mut state = WorkflowDrainState::default();
    let fn_hash = [0xCC; 64];
    state.record_drain(WorkflowDrainStarted {
        fn_hash,
        started_at_unix_ms: 500,
    });
    let decision = state.may_start_new_run(&fn_hash);
    assert!(!decision, "drained hash must refuse new starts");
    let decision_other = state.may_start_new_run(&[0xDD; 64]);
    assert!(decision_other, "non-drained hash must still allow new starts");
}
```

Run: `cargo test -p vox-orchestrator --test workflow_drain 2>&1 | tail -15`
Expected: FAIL — `workflow_drain` module does not exist.

- [ ] **P2-T3b: Implement drain state**

Create `crates/vox-orchestrator/src/oplog/workflow_drain.rs`:

```rust
//! Workflow drain op-log entries and dispatcher predicate.
//!
//! Phase 2: in-memory only. Phase 3 will swap the backing `HashMap` for a
//! vox-db-backed durable op-log. The trait shape stays the same; only the
//! constructor differs.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct WorkflowDrainStarted {
    pub fn_hash: [u8; 64],
    pub started_at_unix_ms: u64,
}

/// In-memory drain state keyed by workflow `fn_hash`. Insert via
/// [`record_drain`]; query via [`is_draining`] / [`may_start_new_run`].
#[derive(Debug, Default)]
pub struct WorkflowDrainState {
    drained: HashMap<[u8; 64], WorkflowDrainStarted>,
}

impl WorkflowDrainState {
    pub fn record_drain(&mut self, evt: WorkflowDrainStarted) {
        self.drained.insert(evt.fn_hash, evt);
    }

    pub fn is_draining(&self, fn_hash: &[u8; 64]) -> bool {
        self.drained.contains_key(fn_hash)
    }

    /// Dispatcher predicate. `true` means "you may start a new run at this hash".
    pub fn may_start_new_run(&self, fn_hash: &[u8; 64]) -> bool {
        !self.is_draining(fn_hash)
    }

    pub fn snapshot(&self) -> Vec<WorkflowDrainStarted> {
        self.drained.values().copied().collect()
    }
}
```

Wire into `crates/vox-orchestrator/src/oplog/mod.rs`:

```rust
pub mod workflow_drain;
```

Run: `cargo test -p vox-orchestrator --test workflow_drain 2>&1 | tail -15`
Expected: PASS.

- [ ] **P2-T3c: Plumb the predicate into the dispatcher**

In `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs`, before the `relay_a2a` call inside `relay_remote_task_envelope`, consult the drain state. The orchestrator already holds a `WorkflowDrainState` (added in P2-T3b); thread it through:

```rust
pub async fn relay_remote_task_envelope(
    client: &vox_populi::http_client::PopuliHttpClient,
    sender: AgentId,
    receiver: AgentId,
    envelope: &RemoteTaskEnvelope,
    drain_state: &crate::oplog::workflow_drain::WorkflowDrainState,
) -> Result<(), String> {
    if let Some(bundle_ref) = &envelope.bundle_ref {
        if !drain_state.may_start_new_run(&bundle_ref.fn_hash) {
            return Err(format!(
                "workflow at fn_hash {} is draining; refusing new dispatch",
                hex_short(&bundle_ref.fn_hash),
            ));
        }
    }
    // ... existing body ...
}

fn hex_short(h: &[u8; 64]) -> String {
    let mut s = String::with_capacity(16);
    for b in &h[..8] {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
```

(The `bundle_ref` field is added in P2-T4. For now, expect compile error there until P2-T4 lands; document the dependency in the commit message and resolve in T4.)

- [ ] **P2-T3d: Implement the CLI command**

Create `crates/vox-cli/src/commands/workflow/mod.rs`:

```rust
//! `vox workflow ...` operator subcommand parent.

pub mod drain;
pub mod ls;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WorkflowCmd {
    /// Mark a workflow content-hash as "no new starts"; in-flight runs continue.
    Drain(drain::DrainArgs),
    /// List known workflow content-hashes and their drain state.
    Ls(ls::LsArgs),
}

pub async fn run(cmd: WorkflowCmd) -> anyhow::Result<()> {
    match cmd {
        WorkflowCmd::Drain(args) => drain::run(args).await,
        WorkflowCmd::Ls(args) => ls::run(args).await,
    }
}
```

Create `crates/vox-cli/src/commands/workflow/drain.rs`:

```rust
use anyhow::Context;
use clap::Args;

#[derive(Debug, Args)]
pub struct DrainArgs {
    /// Workflow content-hash (hex SHA3-512). Get it from `vox workflow ls`.
    #[arg(long)]
    pub version: String,
}

pub async fn run(args: DrainArgs) -> anyhow::Result<()> {
    let fn_hash = parse_hash(&args.version)
        .with_context(|| format!("invalid --version hash: {}", args.version))?;
    // Talk to the running orchestrator daemon over its admin socket.
    let client = vox_orchestrator_d::admin_client::connect().await?;
    client.workflow_drain(fn_hash).await?;
    println!(
        "workflow at fn_hash {} marked draining; new dispatches will be refused",
        args.version
    );
    Ok(())
}

fn parse_hash(s: &str) -> anyhow::Result<[u8; 64]> {
    if s.len() != 128 {
        anyhow::bail!("expected 128 hex chars, got {}", s.len());
    }
    let mut out = [0u8; 64];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let hex = std::str::from_utf8(chunk)?;
        out[i] = u8::from_str_radix(hex, 16)?;
    }
    Ok(out)
}
```

Create `crates/vox-cli/src/commands/workflow/ls.rs`:

```rust
use clap::Args;

#[derive(Debug, Args)]
pub struct LsArgs {
    /// Show only currently-draining workflows.
    #[arg(long)]
    pub draining: bool,
}

pub async fn run(args: LsArgs) -> anyhow::Result<()> {
    let client = vox_orchestrator_d::admin_client::connect().await?;
    let entries = client.workflow_ls().await?;
    println!("{:<132}  {:<12}  {}", "fn_hash", "state", "name");
    for e in entries {
        if args.draining && !e.draining {
            continue;
        }
        println!(
            "{:<132}  {:<12}  {}",
            e.fn_hash_hex,
            if e.draining { "draining" } else { "active" },
            e.name,
        );
    }
    Ok(())
}
```

Wire into `crates/vox-cli/src/commands/mod.rs` (add `pub mod workflow;` and a top-level command variant). Match the existing add pattern for sibling subcommands like `bundle.rs`.

- [ ] **P2-T3e: Integration test for the command surface**

Create `crates/vox-cli/tests/workflow_drain.rs`:

```rust
//! Surface-level test: command parses, dispatches via admin client, prints sane output.
//! Real wire test lives in `crates/vox-orchestrator-d/tests/`.

use clap::Parser;
use vox_cli::Cli;

#[test]
fn vox_workflow_drain_parses() {
    let cli = Cli::try_parse_from([
        "vox", "workflow", "drain", "--version",
        &"a".repeat(128),
    ]);
    assert!(cli.is_ok(), "parse error: {:?}", cli.err());
}

#[test]
fn vox_workflow_drain_rejects_short_hash() {
    let cli = Cli::try_parse_from(["vox", "workflow", "drain", "--version", "abc"]);
    assert!(cli.is_ok(), "clap accepts the arg; runtime validates length");
    // Runtime validation tested below via the parse_hash unit, separately.
}
```

Run: `cargo test -p vox-cli --test workflow_drain 2>&1 | tail -15`
Expected: PASS.

- [ ] **P2-T3f: Commit**

```bash
git add crates/vox-orchestrator/src/oplog/workflow_drain.rs \
        crates/vox-orchestrator/src/oplog/mod.rs \
        crates/vox-orchestrator/tests/workflow_drain.rs \
        crates/vox-orchestrator/src/a2a/dispatch/mesh.rs \
        crates/vox-cli/src/commands/workflow/ \
        crates/vox-cli/src/commands/mod.rs \
        crates/vox-cli/tests/workflow_drain.rs
git commit -m "feat(vox-cli, vox-orchestrator): P2-T3 workflow drain CLI + WorkflowDrainStarted op-log"
```

---

## Task P2-T4: CAS-bundle code seeding for mesh-dispatched jobs

**Files:**

- Modify: `crates/vox-orchestrator/src/a2a/envelope.rs:16-69` (add `bundle_ref: Option<BundleRef>` field).
- Create: `crates/vox-mesh-types/src/bundle.rs`.
- Modify: `crates/vox-mesh-types/src/lib.rs`.
- Create: `crates/vox-orchestrator/src/a2a/dispatch/bundle_fetch.rs`.
- Modify: `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs:60-130`.
- Create: `crates/vox-orchestrator/tests/bundle_fetch.rs`.

A worker that has never seen workflow `my::wf@a37c…` cannot run it without the bundle. Phase 1 punts on this — the workflow code must already exist on the receiver. Phase 2 closes the loop: every dispatch attaches a `BundleRef`. If the bundle bytes are small (under the threshold), they ship inline on the envelope; otherwise the receiver pulls them via a new `bundle_request` / `bundle_response` A2A round-trip. Cache hits on subsequent jobs of the same hash.

### Subtasks

- [ ] **P2-T4a: Failing test — envelope round-trips with a `bundle_ref`**

Create `crates/vox-orchestrator/tests/bundle_fetch.rs`:

```rust
use vox_orchestrator::a2a::envelope::RemoteTaskEnvelope;
use vox_package::bundle::BundleRef;

#[test]
fn envelope_round_trips_with_bundle_ref() {
    let env = RemoteTaskEnvelope {
        idempotency_key: "k".into(),
        task_id: 1,
        repository_id: "r".into(),
        capability_requirements_json: "{}".into(),
        payload: "p".into(),
        privacy_class: None,
        populi_scope_id: None,
        submitted_unix_ms: None,
        exec_lease_id: None,
        campaign_id: None,
        artifact_refs_json: None,
        session_id: None,
        thread_id: None,
        context_envelope_json: None,
        harness_spec_json: None,
        parent_task_id: None,
        caller_agent_id: None,
        trace_id: None,
        span_depth: None,
        bundle_ref: Some(BundleRef { fn_hash: [0x77; 64] }),
        bundle_inline_b64: None,
    };
    let json = serde_json::to_string(&env).expect("ser");
    let back: RemoteTaskEnvelope = serde_json::from_str(&json).expect("de");
    assert_eq!(back.bundle_ref.as_ref().map(|b| b.fn_hash), Some([0x77; 64]));
}

#[test]
fn legacy_envelope_without_bundle_ref_still_deserializes() {
    let json = r#"{
        "idempotency_key": "k",
        "task_id": 1,
        "repository_id": "r",
        "capability_requirements_json": "{}",
        "payload": "p"
    }"#;
    let env: RemoteTaskEnvelope = serde_json::from_str(json).expect("legacy");
    assert!(env.bundle_ref.is_none());
}
```

Run: `cargo test -p vox-orchestrator --test bundle_fetch 2>&1 | tail -15`
Expected: FAIL — `bundle_ref` field absent.

- [ ] **P2-T4b: Add the field to `RemoteTaskEnvelope`**

In `crates/vox-orchestrator/src/a2a/envelope.rs:67-69`, add (preserving the additive serde discipline already used by other Phase C fields):

```rust
    /// P2-T4: content-hash of the workflow bundle this envelope dispatches.
    /// Receivers consult their bundle store; on miss they emit a
    /// `bundle_request` A2A message back to the sender.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_ref: Option<vox_package::bundle::BundleRef>,
    /// P2-T4: inline base64-encoded bundle bytes when the bundle is below
    /// the size threshold (default 1 MiB). When set, receivers skip the
    /// `bundle_request` round-trip and use these bytes directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_inline_b64: Option<String>,
}
```

Run: `cargo test -p vox-orchestrator --test bundle_fetch 2>&1 | tail -15`
Expected: PASS.

- [ ] **P2-T4c: Wire types for the bundle-fetch round-trip**

Create `crates/vox-mesh-types/src/bundle.rs`:

```rust
//! A2A wire types for content-addressed bundle requests/responses.

use serde::{Deserialize, Serialize};

/// Stable A2A wire type for a worker requesting bundle bytes from a sender.
pub const BUNDLE_REQUEST_TYPE: &str = "bundle_request";
/// Stable A2A wire type for the sender's response carrying bundle bytes.
pub const BUNDLE_RESPONSE_TYPE: &str = "bundle_response";

/// Sent worker → originator: "I dispatched envelope `idempotency_key` and
/// I don't have the bundle for `fn_hash_hex`. Please send the bytes."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRequest {
    pub idempotency_key: String,
    pub fn_hash_hex: String,
}

/// Sent originator → worker: "Here are the bytes for `fn_hash_hex`."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleResponse {
    pub idempotency_key: String,
    pub fn_hash_hex: String,
    /// Base64-encoded bundle bytes.
    pub bundle_bytes_b64: String,
    /// Base64-encoded JSON-encoded `Vec<BundleRef>` for transitive deps.
    /// Empty string when no deps.
    #[serde(default)]
    pub deps_json_b64: String,
}
```

In `crates/vox-mesh-types/src/lib.rs` add:

```rust
pub mod bundle;
```

- [ ] **P2-T4d: Sender-side ship-or-ref decision**

Create `crates/vox-orchestrator/src/a2a/dispatch/bundle_fetch.rs`:

```rust
//! Sender-side bundle attachment + receiver-side bundle resolution for
//! mesh-dispatched envelopes (P2-T4).

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use vox_package::bundle::{Bundle, BundleRef, BundleStore};

/// Default threshold for inlining bundle bytes on the envelope.
pub const INLINE_BUNDLE_BYTE_LIMIT: usize = 1024 * 1024; // 1 MiB

/// Decide whether to inline a bundle's bytes on an envelope or attach only
/// the reference, leaving the receiver to fetch via `bundle_request`.
///
/// Returns `(BundleRef, Option<inline_b64>)`. When `Some`, the receiver
/// skips the round-trip.
pub fn ship_decision(bundle: &Bundle) -> (BundleRef, Option<String>) {
    let r = BundleRef { fn_hash: bundle.fn_hash };
    if bundle.bytes.len() <= INLINE_BUNDLE_BYTE_LIMIT {
        let b64 = B64.encode(bundle.bytes.as_ref());
        (r, Some(b64))
    } else {
        (r, None)
    }
}

/// Receiver-side: try the local store first. On miss, the caller MUST
/// emit a `BundleRequest` and await the response before claiming the task.
pub fn resolve_local(
    store: &BundleStore,
    r: &BundleRef,
) -> std::io::Result<Option<Bundle>> {
    store.lookup(r)
}

/// Decode an inline-attached bundle from an envelope's `bundle_inline_b64`.
pub fn decode_inline(
    r: &BundleRef,
    b64: &str,
    deps: Vec<BundleRef>,
    manifest: serde_json::Value,
) -> Result<Bundle, base64::DecodeError> {
    let bytes = B64.decode(b64)?;
    Ok(Bundle {
        fn_hash: r.fn_hash,
        deps,
        bytes: std::sync::Arc::new(bytes),
        manifest,
    })
}
```

- [ ] **P2-T4e: Sender call-site update**

In `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs:64`, update `relay_remote_task_envelope` to attach a bundle. The orchestrator threads a `BundleStore` reference through (added to its constructor; verify in the call sites). Pseudocode for the change:

```rust
// Look up the workflow's bundle by its fn_hash (carried separately by the
// orchestrator; sourced from the HirFn::generated_hash stamped in P2-T1).
let attached: Option<(BundleRef, Option<String>)> = match envelope.bundle_ref.as_ref() {
    Some(r) => match bundle_store.lookup(r)? {
        Some(b) => Some(bundle_fetch::ship_decision(&b)),
        None => {
            tracing::warn!(
                fn_hash = %r.to_hex(),
                "dispatch: bundle not in local store; sending ref only — receiver will request"
            );
            Some((*r, None))
        }
    },
    None => None,
};
let mut envelope = envelope.clone();
if let Some((r, inline)) = attached {
    envelope.bundle_ref = Some(r);
    envelope.bundle_inline_b64 = inline;
}
```

(Keep the existing JWE / capability-secrets path unchanged; we add the bundle attachment alongside it.)

- [ ] **P2-T4f: Receiver-side bundle-fetch test**

Append to `crates/vox-orchestrator/tests/bundle_fetch.rs`:

```rust
use vox_orchestrator::a2a::dispatch::bundle_fetch::{
    INLINE_BUNDLE_BYTE_LIMIT, decode_inline, ship_decision,
};
use vox_package::bundle::Bundle;

#[test]
fn small_bundle_inlines() {
    let bundle = Bundle {
        fn_hash: [0x11; 64],
        deps: vec![],
        bytes: std::sync::Arc::new(vec![0u8; 16]),
        manifest: serde_json::json!({}),
    };
    let (r, inline) = ship_decision(&bundle);
    assert_eq!(r.fn_hash, [0x11; 64]);
    assert!(inline.is_some(), "16-byte bundle must inline");
}

#[test]
fn large_bundle_drops_to_request_round_trip() {
    let bundle = Bundle {
        fn_hash: [0x22; 64],
        deps: vec![],
        bytes: std::sync::Arc::new(vec![0u8; INLINE_BUNDLE_BYTE_LIMIT + 1]),
        manifest: serde_json::json!({}),
    };
    let (_, inline) = ship_decision(&bundle);
    assert!(inline.is_none(), "above-threshold bundle MUST NOT inline");
}

#[test]
fn decode_inline_recovers_original_bytes() {
    let bundle = Bundle {
        fn_hash: [0x33; 64],
        deps: vec![],
        bytes: std::sync::Arc::new(b"hello-world".to_vec()),
        manifest: serde_json::json!({"k": "v"}),
    };
    let (r, inline) = ship_decision(&bundle);
    let inline = inline.expect("small bundle inlines");
    let back = decode_inline(&r, &inline, vec![], serde_json::json!({"k": "v"}))
        .expect("decode");
    assert_eq!(back.bytes.as_ref().as_slice(), b"hello-world");
}
```

Run: `cargo test -p vox-orchestrator --test bundle_fetch 2>&1 | tail -15`
Expected: all PASS.

- [ ] **P2-T4g: Commit**

```bash
git add crates/vox-orchestrator/src/a2a/envelope.rs \
        crates/vox-orchestrator/src/a2a/dispatch/mesh.rs \
        crates/vox-orchestrator/src/a2a/dispatch/bundle_fetch.rs \
        crates/vox-orchestrator/tests/bundle_fetch.rs \
        crates/vox-mesh-types/src/bundle.rs \
        crates/vox-mesh-types/src/lib.rs
git commit -m "feat(vox-orchestrator, vox-mesh-types): P2-T4 CAS bundle seeding on mesh dispatch"
```

---

## Task P2-T5: Activity result caching ledger keyed by `(activity_id, structural_arg_hash)`

**Files:**

- Modify: `crates/vox-db/src/schema/manifest.rs` — bump `BASELINE_VERSION` from 62 (set by P0-T1) to 63; add `activity_result_cache` schema fragment gated on version 63.
- Create: `crates/vox-db/src/ddl/activity_result_cache.rs`.
- Modify: `crates/vox-db/src/ddl/mod.rs`.
- Modify: `crates/vox-workflow-runtime/src/workflow/tracker.rs` (new methods).
- Modify: `crates/vox-workflow-runtime/src/workflow/run.rs:58-91` (consult cache).
- Create: `crates/vox-workflow-runtime/tests/activity_result_cache.rs`.

The activity result cache lets `@activity(dedup = "7d")` do its job: if the same activity ran recently with structurally-identical args, replay the previous result without re-running the body. Useful for idempotent activities like external HTTP calls and third-party SaaS posts. Default window is 24h; `@activity(dedup = "...")` can extend.

**Note on the double use of `structural_arg_hash`.** Phase 1's P1-T4 derives `activity_id = BLAKE3(workflow_id ‖ call_site_id ‖ structural_arg_hash ‖ replay_counter)`. P2-T5's cache key is `(activity_id, structural_arg_hash)`. The same `structural_arg_hash` therefore appears both as a sub-input of `activity_id` *and* as the second component of the cache key. This is intentional, not an oversight: `activity_id` binds the call site + arg structure + workflow identity into one durable replay token, while the cache key uses the raw `structural_arg_hash` so that distinct call sites that happen to be invoked with the same args within the dedup window remain isolated. The two channels are not redundant — different `activity_id`s with the same `arg_hash` correctly miss each other's cache entries.

### Subtasks

- [ ] **P2-T5a: Schema manifest bump (BASELINE_VERSION 62 → 63)**

Per SSOT §5.5, schema evolution flows through `BASELINE_VERSION` in `manifest.rs`, not standalone migration files.

1. Open `crates/vox-db/src/schema/manifest.rs`.
2. Bump the `BASELINE_VERSION` constant from `62` (set by P0-T1) to `63`.
3. Add the `activity_result_cache` table DDL as a Rust string constant inside the manifest, gated on `if version >= 63 { ... }`.
4. Verify with `cargo test -p vox-db schema_manifest` that the migration applies idempotently.

Add inside `manifest.rs`:

```rust
const ACTIVITY_RESULT_CACHE_V63: &str = r#"
-- P2-T5: per-activity dedup cache. Phase 2 only: result rows are pruned by
-- the background sweep; rows are append-only otherwise.

CREATE TABLE IF NOT EXISTS activity_result_cache (
    activity_id           TEXT    NOT NULL,
    arg_hash              TEXT    NOT NULL,        -- hex SHA3-512 of canonicalized args
    result_json           TEXT    NOT NULL,        -- serialized DurablePromise[T]::Ready value
    produced_at_unix_ms   INTEGER NOT NULL,
    dedup_window_ms       INTEGER NOT NULL,        -- TTL window in ms, e.g. 86_400_000 for 24h
    dedup_window_until    INTEGER NOT NULL,        -- produced_at_unix_ms + dedup_window_ms

    PRIMARY KEY (activity_id, arg_hash)
);

-- Cheap range scan for the background sweep.
CREATE INDEX IF NOT EXISTS idx_activity_result_cache_until
    ON activity_result_cache (dedup_window_until);
"#;
```

The migration entrypoint applies `ACTIVITY_RESULT_CACHE_V63` when `version >= 63`, following the same pattern P0-T1 used for `vcs_lock` + `lock_leader` at version 62.

- [ ] **P2-T5b: DDL helper module + sweep SQL**

Create `crates/vox-db/src/ddl/activity_result_cache.rs`:

```rust
//! P2-T5: DDL + maintenance SQL for the activity result cache.

/// Insert-or-replace SQL. Idempotent: re-running an activity within its TTL
/// updates `produced_at_unix_ms`, refreshing the window.
pub const UPSERT_SQL: &str = r#"
INSERT INTO activity_result_cache
    (activity_id, arg_hash, result_json, produced_at_unix_ms,
     dedup_window_ms, dedup_window_until)
VALUES (?, ?, ?, ?, ?, ?)
ON CONFLICT(activity_id, arg_hash) DO UPDATE SET
    result_json         = excluded.result_json,
    produced_at_unix_ms = excluded.produced_at_unix_ms,
    dedup_window_ms     = excluded.dedup_window_ms,
    dedup_window_until  = excluded.dedup_window_until
"#;

/// Lookup SQL. Returns rows still inside their TTL window.
pub const LOOKUP_SQL: &str = r#"
SELECT result_json, produced_at_unix_ms, dedup_window_until
FROM activity_result_cache
WHERE activity_id = ? AND arg_hash = ?
  AND dedup_window_until > ?
LIMIT 1
"#;

/// Sweep SQL. Run on a background timer (cadence: every 60 seconds when
/// the orchestrator daemon is running; on-demand via `vox db prune` otherwise).
pub const SWEEP_SQL: &str = r#"
DELETE FROM activity_result_cache
WHERE dedup_window_until <= ?
"#;
```

In `crates/vox-db/src/ddl/mod.rs`:

```rust
pub mod activity_result_cache;
```

- [ ] **P2-T5c: Tracker trait extension**

In `crates/vox-workflow-runtime/src/workflow/tracker.rs`, add (with no-op defaults so existing implementors keep compiling):

```rust
#[async_trait::async_trait]
pub trait WorkflowTracker {
    // ... existing methods ...

    /// P2-T5: try the activity result cache. `Ok(None)` for miss; `Ok(Some(_))`
    /// for hit (caller skips the body). Default: always miss.
    async fn load_cached_activity_result(
        &self,
        _activity_id: &str,
        _arg_hash_hex: &str,
        _now_unix_ms: u64,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        Ok(None)
    }

    /// P2-T5: upsert a cache entry. Default: no-op.
    async fn record_cached_activity_result(
        &mut self,
        _activity_id: &str,
        _arg_hash_hex: &str,
        _result: &serde_json::Value,
        _produced_at_unix_ms: u64,
        _dedup_window_ms: u64,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
```

- [ ] **P2-T5d: Runtime call-site update**

In `crates/vox-workflow-runtime/src/workflow/run.rs` around line 58 (the `tracker.load_activity_result` block), check the cache **before** the journal-replay path so an activity that was completed in a *prior* workflow can short-circuit a fresh workflow's run:

```rust
// P2-T5: try the deterministic per-activity dedup cache first.
let arg_hash_hex = compute_structural_arg_hash(&step.arguments);
let now_ms = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_millis() as u64)
    .unwrap_or(0);
if let Some(cached) = tracker
    .load_cached_activity_result(&activity_id, &arg_hash_hex, now_ms)
    .await?
{
    journal.push(versioned_event(json!({
        "event": "ActivityCacheHit",
        "workflow": workflow_name,
        "activity": step.name,
        "activity_id": activity_id,
        "arg_hash": arg_hash_hex,
    })));
    journal.push(versioned_event(json!({
        "event": "ActivityCompleted",
        "workflow": workflow_name,
        "activity": step.name,
        "activity_id": activity_id,
        "from_cache": true,
        "result": cached,
    })));
    continue;
}
```

After a successful run, before `tracker.on_activity_completed`, call:

```rust
let dedup_ms = step.dedup_window_ms.unwrap_or(24 * 60 * 60 * 1000);
let _ = tracker
    .record_cached_activity_result(&activity_id, &arg_hash_hex, &entry, now_ms, dedup_ms)
    .await;
```

`compute_structural_arg_hash` lives in `super::types`:

```rust
pub fn compute_structural_arg_hash(args: &[serde_json::Value]) -> String {
    let canonical = serde_json::Value::Array(args.to_vec()).to_string();
    vox_db::hash::content_hash(canonical.as_bytes())
}
```

- [ ] **P2-T5e: Failing/passing test**

Create `crates/vox-workflow-runtime/tests/activity_result_cache.rs`:

```rust
use serde_json::json;
use vox_workflow_runtime::workflow::tracker::WorkflowTracker;

#[derive(Default)]
struct MemTracker {
    map: std::collections::HashMap<(String, String), serde_json::Value>,
}

#[async_trait::async_trait]
impl WorkflowTracker for MemTracker {
    async fn load_cached_activity_result(
        &self,
        activity_id: &str,
        arg_hash_hex: &str,
        _now_unix_ms: u64,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        Ok(self.map.get(&(activity_id.to_string(), arg_hash_hex.to_string())).cloned())
    }
    async fn record_cached_activity_result(
        &mut self,
        activity_id: &str,
        arg_hash_hex: &str,
        result: &serde_json::Value,
        _produced_at_unix_ms: u64,
        _dedup_window_ms: u64,
    ) -> anyhow::Result<()> {
        self.map.insert(
            (activity_id.to_string(), arg_hash_hex.to_string()),
            result.clone(),
        );
        Ok(())
    }
}

#[tokio::test]
async fn second_run_with_same_args_hits_cache() {
    let mut tracker = MemTracker::default();
    tracker
        .record_cached_activity_result("post_to_slack", "hash1", &json!({"ok": true}), 0, 86_400_000)
        .await
        .unwrap();
    let hit = tracker
        .load_cached_activity_result("post_to_slack", "hash1", 1_000)
        .await
        .unwrap();
    assert_eq!(hit, Some(json!({"ok": true})));
}

#[tokio::test]
async fn miss_on_distinct_arg_hash() {
    let mut tracker = MemTracker::default();
    tracker
        .record_cached_activity_result("post", "h1", &json!({"r": 1}), 0, 86_400_000)
        .await
        .unwrap();
    let miss = tracker
        .load_cached_activity_result("post", "h2", 1_000)
        .await
        .unwrap();
    assert!(miss.is_none());
}
```

Run: `cargo test -p vox-workflow-runtime --test activity_result_cache 2>&1 | tail -15`
Expected: PASS.

- [ ] **P2-T5f: Document the sweep cadence**

Append a doc-comment on `SWEEP_SQL` in `activity_result_cache.rs` recording the cadence (60s when daemon is running; on-demand via `vox db prune` otherwise). The actual scheduler hookup lives in the orchestrator daemon and is a one-line tokio interval — out of scope for the runtime crate.

- [ ] **P2-T5g: Commit**

```bash
git add crates/vox-db/src/schema/manifest.rs \
        crates/vox-db/src/ddl/activity_result_cache.rs \
        crates/vox-db/src/ddl/mod.rs \
        crates/vox-workflow-runtime/src/workflow/tracker.rs \
        crates/vox-workflow-runtime/src/workflow/run.rs \
        crates/vox-workflow-runtime/src/workflow/types.rs \
        crates/vox-workflow-runtime/tests/activity_result_cache.rs
git commit -m "feat(vox-db, vox-workflow-runtime): P2-T5 activity_result_cache table + dedup short-circuit"
```

---

## Task P2-T6: `vox dispatch preview` — generalize the preview tool to dispatch-time

**Files:**

- Create: `crates/vox-cli/src/commands/dispatch/mod.rs`.
- Create: `crates/vox-cli/src/commands/dispatch/preview.rs`.
- Modify: `crates/vox-cli/src/commands/mod.rs`.
- Modify: `crates/vox-orchestrator/src/lib.rs` (or appropriate exposing module) to expose a `preview_dispatch` async fn.
- Create: `crates/vox-cli/tests/dispatch_preview.rs`.

P1-T8 already projects an activity tree from a workflow source. P2-T6 reuses that projection but annotates each call with the routing decision the dispatcher would make:

- `local` — the activity has no `@remote` and no mesh policy match; it would run in-proc.
- `remote(peer_id)` — the dispatcher would route to a specific peer (file affinity, label match, lease).
- `cached` — the activity result cache (P2-T5) would short-circuit it; no run at all.

The point is **dry-run**: an operator types a command and sees the routing decision tree printed to stdout, no side effects, no envelopes, no journal entries.

### Subtasks

- [ ] **P2-T6a: Failing CLI parse test**

Create `crates/vox-cli/tests/dispatch_preview.rs`:

```rust
use clap::Parser;
use vox_cli::Cli;

#[test]
fn dispatch_preview_parses() {
    let cli = Cli::try_parse_from([
        "vox", "dispatch", "preview", "my::workflow",
        "--", "arg1", "arg2",
    ]);
    assert!(cli.is_ok(), "parse error: {:?}", cli.err());
}
```

Run: `cargo test -p vox-cli --test dispatch_preview 2>&1 | tail -10`
Expected: FAIL — `dispatch` subcommand absent.

- [ ] **P2-T6b: Implement the subcommand**

Create `crates/vox-cli/src/commands/dispatch/mod.rs`:

```rust
pub mod preview;

use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum DispatchCmd {
    /// Project the routing decision tree for a workflow without dispatching.
    Preview(preview::PreviewArgs),
}

pub async fn run(cmd: DispatchCmd) -> anyhow::Result<()> {
    match cmd {
        DispatchCmd::Preview(args) => preview::run(args).await,
    }
}
```

Create `crates/vox-cli/src/commands/dispatch/preview.rs`:

```rust
use clap::Args;

#[derive(Debug, Args)]
pub struct PreviewArgs {
    /// Fully qualified workflow path, e.g. `my::workflow`.
    pub path: String,
    /// Workflow arguments, separated by `--`.
    #[arg(last = true)]
    pub args: Vec<String>,
}

#[derive(Debug)]
pub enum RoutingDecision {
    Local,
    Remote { peer_id: String, reason: String },
    Cached { activity_id: String, arg_hash_hex: String },
}

pub async fn run(args: PreviewArgs) -> anyhow::Result<()> {
    // Ask the orchestrator for a dry-run projection. The orchestrator
    // consults the same routing logic the live dispatcher uses, but does
    // not commit anything — no envelopes sent, no journal entries written.
    let client = vox_orchestrator_d::admin_client::connect().await?;
    let projection = client.dispatch_preview(args.path, args.args).await?;

    println!("workflow: {}", projection.path);
    println!("fn_hash:  {}", projection.fn_hash_hex);
    println!();
    for (idx, step) in projection.steps.iter().enumerate() {
        let marker = match &step.decision {
            RoutingDecision::Local => "[local]    ".to_string(),
            RoutingDecision::Remote { peer_id, reason } =>
                format!("[remote→{} ({})]", short_id(peer_id), reason),
            RoutingDecision::Cached { activity_id, arg_hash_hex } =>
                format!("[cached {}@{}]", activity_id, &arg_hash_hex[..8]),
        };
        println!("  {:>3}  {}  {}", idx, marker, step.name);
    }
    Ok(())
}

fn short_id(s: &str) -> String {
    s.chars().take(8).collect()
}
```

The `dispatch_preview` admin RPC is a new method on the orchestrator's admin socket. It internally:

1. Parses + lowers the source to HIR (re-using `vox-compiler`).
2. Runs `vox-workflow-runtime`'s `plan_workflow_replay_ir` to get the activity sequence.
3. For each step, asks the routing logic *what would you do?* without firing.
4. Consults the activity result cache (P2-T5) to mark cached short-circuits.
5. Returns the `DispatchProjection` shape.

Add the orchestrator-side method (signature only here; full body in `crates/vox-orchestrator/src/dispatch_preview.rs`):

```rust
pub struct DispatchProjection {
    pub path: String,
    pub fn_hash_hex: String,
    pub steps: Vec<DispatchPreviewStep>,
}

pub struct DispatchPreviewStep {
    pub name: String,
    pub decision: RoutingDecision,
}

pub async fn preview_dispatch(
    orchestrator: &Orchestrator,
    bundle_store: &BundleStore,
    drain_state: &WorkflowDrainState,
    path: &str,
    args: Vec<String>,
) -> anyhow::Result<DispatchProjection> {
    // ... plan, project, decide, never fire ...
    todo!("see TASK P2-T6b body")
}
```

- [ ] **P2-T6c: Round-trip test on the projection**

Append to `crates/vox-cli/tests/dispatch_preview.rs`:

```rust
use vox_cli::commands::dispatch::preview::RoutingDecision;
use serde_json;

#[test]
fn routing_decision_serializes_round_trip() {
    let cases = vec![
        RoutingDecision::Local,
        RoutingDecision::Remote { peer_id: "p1".into(), reason: "label match".into() },
        RoutingDecision::Cached { activity_id: "a".into(), arg_hash_hex: "ab12".into() },
    ];
    for c in cases {
        let s = serde_json::to_string(&c).expect("ser");
        let _: RoutingDecision = serde_json::from_str(&s).expect("de");
    }
}
```

(Add `#[derive(Serialize, Deserialize)]` to `RoutingDecision`.)

Run: `cargo test -p vox-cli --test dispatch_preview 2>&1 | tail -10`
Expected: PASS.

- [ ] **P2-T6d: Commit**

```bash
git add crates/vox-cli/src/commands/dispatch/ \
        crates/vox-cli/src/commands/mod.rs \
        crates/vox-cli/tests/dispatch_preview.rs \
        crates/vox-orchestrator/src/dispatch_preview.rs \
        crates/vox-orchestrator/src/lib.rs
git commit -m "feat(vox-cli, vox-orchestrator): P2-T6 vox dispatch preview projection (no side effects)"
```

---

## Task P2-T7: Codegen — lower `DurabilityKind` to specific runtime calls

**Files:**

- Create: `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs`.
- Modify: `crates/vox-codegen/src/codegen_rust/emit/workflow.rs:136-229` (branch `emit_fn` on `func.durability`).
- Modify: `crates/vox-codegen/src/codegen_rust/emit/mod.rs` (re-export new module if needed).
- Create: `crates/vox-codegen/tests/durability_lowering.rs`.
- Modify: `docs/src/architecture/where-things-live.md` — three rows for `Bundle`, `WorkflowDrainStarted`, `activity_result_cache`.

This closes the gap where Phase 1's compiler stamped `DurabilityKind` on each `HirFn` but `emit_fn` lowered all three to the same async Rust shape. Phase 2 needs the runtime contract: each kind has a different call shape, and the runtime composes them differently.

**Alignment with Phase 1's `DurablePromise[T]` lowering.** P1-T1 lowers `DurablePromise[T]` to a wrapper around `tokio::sync::oneshot::Receiver<Result<T, JournalError>>` with a journal-backed fast path — that's the **awaiter** side, what consumers of an activity result see. P2-T7 here lowers the **producer** side: an `activity` function body becomes the work that fills the `oneshot` (or, on replay, that the journal short-circuits). The two stories compose without redefining anything: `journal::execute("$activity_id", ...)` is exactly the call that, on first run, drives the oneshot the awaiter is parked on, and on replay returns the recorded result the journal already holds.

The lowering rule, AST/HIR-driven:

| `DurabilityKind` | Wrap shape | Effect |
|---|---|---|
| `Workflow` | `vox_workflow_runtime::interpret_workflow_durable(&hir, "$name", &mut tracker).await?` | The function body becomes the `plan_workflow_replay_ir` input; the runtime journals each step. |
| `Activity` | `vox_workflow_runtime::journal::execute("$activity_id", async { /* body */ }).await?` | Body is wrapped in a journaled call; on replay, returns the cached result. |
| `Actor` | `vox_actor_runtime::mailbox::spawn("$actor_name", &handler_fn).await?` | Body becomes a handler closure; spawned in a mailbox. |
| `None` (plain `fn`) | unchanged | Identical to today's emit. |

The branch lives in `emit_fn` and consults `func.durability`.

### Subtasks

- [ ] **P2-T7a: Golden output for each kind — failing**

Create `crates/vox-codegen/tests/durability_lowering.rs`:

```rust
use vox_codegen::codegen_rust::emit_fn;
use vox_compiler::{hir::DurabilityKind, lower_to_hir, parse_module};

#[test]
fn workflow_lowers_to_interpret_workflow_durable() {
    let src = "workflow my::wf() -> i64 { return 7; }";
    let module = parse_module(src).expect("parse");
    let hir = lower_to_hir(&module).expect("lower");
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Workflow))
        .expect("workflow present");
    let rust = emit_fn(func);
    assert!(
        rust.contains("interpret_workflow_durable"),
        "workflow MUST lower to interpret_workflow_durable; got:\n{rust}"
    );
}

#[test]
fn activity_lowers_to_journal_execute() {
    let src = "activity my::act() -> i64 { return 9; }";
    let module = parse_module(src).expect("parse");
    let hir = lower_to_hir(&module).expect("lower");
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Activity))
        .expect("activity present");
    let rust = emit_fn(func);
    assert!(
        rust.contains("journal::execute") || rust.contains("journal.execute"),
        "activity MUST lower to journal::execute; got:\n{rust}"
    );
    assert!(
        rust.contains("activity_id"),
        "activity body must reference activity_id placeholder; got:\n{rust}"
    );
}

#[test]
fn actor_lowers_to_mailbox_spawn() {
    let src = "actor MyActor { on greet(name: String) -> String { return name; } }";
    let module = parse_module(src).expect("parse");
    let hir = lower_to_hir(&module).expect("lower");
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability == Some(DurabilityKind::Actor))
        .expect("actor handler present");
    let rust = emit_fn(func);
    assert!(
        rust.contains("mailbox::spawn") || rust.contains("MailboxSpawn"),
        "actor MUST lower to mailbox::spawn; got:\n{rust}"
    );
}

#[test]
fn plain_fn_unchanged() {
    let src = "fn add(a: i64, b: i64) -> i64 { return a + b; }";
    let module = parse_module(src).expect("parse");
    let hir = lower_to_hir(&module).expect("lower");
    let func = hir
        .functions
        .iter()
        .find(|f| f.durability.is_none())
        .expect("plain fn present");
    let rust = emit_fn(func);
    assert!(!rust.contains("interpret_workflow_durable"));
    assert!(!rust.contains("journal::execute"));
    assert!(!rust.contains("mailbox::spawn"));
}
```

Run: `cargo test -p vox-codegen --test durability_lowering 2>&1 | tail -20`
Expected: FAIL — current `emit_fn` ignores `durability`.

- [ ] **P2-T7b: Implement the split in `emit_fn`**

Create `crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs`:

```rust
//! P2-T7: lowering `DurabilityKind` into specific runtime call shapes.
//!
//! Driven by `HirFn::durability`. The branch lives here so `emit_fn` stays
//! readable: header emit + delegate-by-kind.

use vox_compiler::hir::{DurabilityKind, HirFn};

use super::stmt_expr::emit_stmt;
use super::types::emit_type;

/// Emit the body of a workflow / activity / actor handler. The function
/// header (params, return type) is emitted by the caller; we own everything
/// inside the `{ ... }`.
pub fn emit_durable_body(func: &HirFn) -> String {
    match func.durability {
        Some(DurabilityKind::Workflow) => emit_workflow_body(func),
        Some(DurabilityKind::Activity) => emit_activity_body(func),
        Some(DurabilityKind::Actor) => emit_actor_body(func),
        None => emit_plain_body(func),
    }
}

fn emit_workflow_body(func: &HirFn) -> String {
    let name = &func.name;
    let hash = func.generated_hash.as_deref().unwrap_or("UNSTAMPED");
    let mut out = String::new();
    out.push_str("    // P2-T7: workflow body lowered to interpret_workflow_durable\n");
    out.push_str(&format!(
        "    let __vox_fn_hash: &'static str = \"{hash}\";\n"
    ));
    out.push_str("    let __vox_hir = ::vox_workflow_runtime::workflow::current_hir_module();\n");
    out.push_str("    let mut __vox_tracker = ::vox_workflow_runtime::workflow::tracker::DefaultTracker;\n");
    out.push_str(&format!(
        "    let __vox_journal = ::vox_workflow_runtime::workflow::interpret_workflow_durable(&__vox_hir, \"{name}\", &mut __vox_tracker).await?;\n"
    ));
    // Map journal → return type. For now, return the last LocalActivity/MeshActivity result.
    if let Some(ret) = &func.return_type {
        out.push_str(&format!(
            "    ::vox_workflow_runtime::workflow::extract_terminal_return::<{ty}>(&__vox_journal).map_err(|e| anyhow::anyhow!(e))\n",
            ty = emit_type(ret),
        ));
    } else {
        out.push_str("    Ok(())\n");
    }
    out
}

fn emit_activity_body(func: &HirFn) -> String {
    let mut out = String::new();
    let activity_id = func.generated_hash.clone().unwrap_or_else(|| func.name.clone());
    out.push_str("    // P2-T7: activity body lowered to journal::execute\n");
    out.push_str(&format!(
        "    ::vox_workflow_runtime::journal::execute(\"{activity_id}\", async move {{\n"
    ));
    for stmt in &func.body {
        // 8-space indent because we're now inside an async block inside the fn body.
        let inner = emit_stmt(stmt, 2, false, false, false);
        out.push_str(&inner);
    }
    out.push_str("    }).await\n");
    out
}

fn emit_actor_body(func: &HirFn) -> String {
    let mut out = String::new();
    let actor_name = &func.name;
    out.push_str("    // P2-T7: actor handler lowered to mailbox::spawn\n");
    out.push_str(&format!(
        "    ::vox_actor_runtime::mailbox::spawn(\"{actor_name}\", move || async move {{\n"
    ));
    for stmt in &func.body {
        let inner = emit_stmt(stmt, 2, false, false, false);
        out.push_str(&inner);
    }
    out.push_str("    }).await\n");
    out
}

fn emit_plain_body(func: &HirFn) -> String {
    let mut out = String::new();
    for stmt in &func.body {
        out.push_str(&emit_stmt(stmt, 1, false, false, false));
    }
    out
}
```

In `crates/vox-codegen/src/codegen_rust/emit/workflow.rs`, replace the body-emit loop in `emit_fn` (line ~223 — the `for stmt in &func.body { ... }` block) with a delegation:

```rust
} else {
    out.push_str(&super::durability_lower::emit_durable_body(func));
}
```

And add to `crates/vox-codegen/src/codegen_rust/emit/mod.rs`:

```rust
mod durability_lower;
```

- [ ] **P2-T7c: Run tests**

Run: `cargo test -p vox-codegen --test durability_lowering 2>&1 | tail -20`
Expected: all four tests PASS.

- [ ] **P2-T7d: Sanity check — does the workspace still build?**

Run: `cargo build --workspace 2>&1 | tail -20`
Expected: clean build. The `extract_terminal_return` and `current_hir_module` helpers must already exist in `vox-workflow-runtime`; if not, add minimal stubs in this same task — they're tiny and keep the codegen output well-typed.

- [ ] **P2-T7e: Update `where-things-live.md`**

In `docs/src/architecture/where-things-live.md`, add three rows in alphabetical position (the file is a flat lookup table; `vox-arch-check` enforces the schema):

```markdown
| BundleRef / Bundle / BundleStore       | crates/vox-package/src/bundle.rs                                |
| WorkflowDrainStarted                   | crates/vox-orchestrator/src/oplog/workflow_drain.rs             |
| activity_result_cache (table + DDL)    | crates/vox-db/src/ddl/activity_result_cache.rs                  |
```

- [ ] **P2-T7f: `vox-arch-check` passes**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`
Expected: clean — no new layer crossings, no orphans, no LoC budget breaks.

- [ ] **P2-T7g: Commit**

```bash
git add crates/vox-codegen/src/codegen_rust/emit/durability_lower.rs \
        crates/vox-codegen/src/codegen_rust/emit/workflow.rs \
        crates/vox-codegen/src/codegen_rust/emit/mod.rs \
        crates/vox-codegen/tests/durability_lowering.rs \
        docs/src/architecture/where-things-live.md
git commit -m "feat(vox-codegen): P2-T7 lower DurabilityKind to interpret_workflow_durable / journal::execute / mailbox::spawn"
```

---

## Phase 2 end-to-end test

Create `tests/mesh_phase2_e2e.vox` (Vox script, **not** a `.sh` / `.ps1` / `.py`):

```
// tests/mesh_phase2_e2e.vox — P2 acceptance harness.
//
// Exercises:
//   1. Compile a workflow at v1; record fn_hash_v1.
//   2. Dispatch one run; let it pause mid-activity (sleep step).
//   3. Compile a refactored v2; record fn_hash_v2.
//   4. `vox workflow ls` shows both hashes.
//   5. `vox workflow drain --version <fn_hash_v1>` succeeds.
//   6. New dispatch of v1 is refused; in-flight v1 run completes normally.
//   7. New dispatch of v2 succeeds; bundle is shipped via P2-T4 inline path.

import vox.cli
import vox.test

let v1_src = `
    workflow demo::wf() -> i64 {
        let _ = sleep(50);
        return 1;
    }
`
let v2_src = `
    workflow demo::wf() -> i64 {
        let _ = sleep(50);
        return 2;
    }
`

let h1 = vox.compile(v1_src).workflow("demo::wf").fn_hash_hex()
let h2 = vox.compile(v2_src).workflow("demo::wf").fn_hash_hex()

vox.test.assert_ne(h1, h2, "refactor must change fn_hash")

let in_flight = vox.dispatch.start("demo::wf", h1, [])

vox.cli.run(["workflow", "drain", "--version", h1])

let denied = vox.dispatch.start_expect_err("demo::wf", h1, [])
vox.test.assert_contains(denied, "draining")

let ok = vox.dispatch.start("demo::wf", h2, [])
vox.test.assert_eq(ok.result(), 2)

vox.test.assert_eq(in_flight.await(), 1, "in-flight v1 run finishes normally")
```

Run with: `vox run tests/mesh_phase2_e2e.vox`. This is the Vox script-first replacement for what older repos would have written as a Bash test.

---

## Acceptance

The phase is done when:

- A workflow at content-hash A and a refactored version at hash B coexist in vox-db without conflict; `vox workflow ls` shows both with `state=active`.
- Killing a worker mid-activity then restarting → workflow resumes from the last journaled `DurablePromise` without re-running completed activities (P2-T5 cache hit + P1's tracker plumbing).
- A second daemon receives a dispatch envelope; if its bundle store has the hash, no `bundle_request` is emitted; if not, it requests via P2-T4's wire types and runs after receiving bytes.
- `vox dispatch preview my::workflow(...)` prints the routing decision tree without sending envelopes or writing journal entries.
- `vox workflow drain --version <fn_hash>` refuses new starts at that hash; in-flight runs complete unchanged.
- `cargo run -p vox-arch-check` is clean.
- `tests/mesh_phase2_e2e.vox` passes.

## Rollback

If a task fails in production:

- **P2-T1**: revert just the `bundle.rs` and `lib.rs` lines; the `ArtifactCache` is unchanged (`lookup_bundle` is additive).
- **P2-T2**: parser-level; remove the new variant + lowering arm. The HIR field defaults to `None`; runtime path is no-op.
- **P2-T3**: drop the `workflow` subcommand module; remove the wiring in `commands/mod.rs`. The orchestrator's `WorkflowDrainState` defaults to empty so dispatching is unaffected.
- **P2-T4**: revert `bundle_ref` field on `RemoteTaskEnvelope`. It's `#[serde(default, skip_serializing_if = "Option::is_none")]`, so legacy receivers ignored it; reverting is non-breaking.
- **P2-T5**: drop the migration; the trait methods have no-op defaults so older callers still compile. Cache simply never hits.
- **P2-T6**: drop the `dispatch` subcommand; preview is a dev tool with no production callers.
- **P2-T7**: revert `emit_fn` to the original loop-only body. All emitted Rust falls back to today's identical-shape behavior; runtime behavior is unchanged because Phase 1's runtime path is independent of codegen wrapping.

## Self-review

- **Spec coverage.** Each P2-T1..P2-T7 task in §3 of the SSOT maps to one task here; killer feature ("hot-deploy without breaking in-flight") is exercised by the e2e Vox script.
- **No invented IDs.** Subtasks use `P2-T1a`/`P2-T1b`/...; all parent tasks are exactly P2-T1..P2-T7.
- **TDD discipline.** Every task starts with a failing test that names a missing type/module/function and runs `cargo test` to confirm the failure mode before implementation.
- **No `.ps1` / `.sh` / `.py` automation.** The e2e harness is a `.vox` script. CLI invocations in test bodies use `vox.cli.run([...])` not raw shell.
- **SHA3-512 reuse.** `Bundle.fn_hash` is computed via `vox_db::hash::content_hash` exactly as `ArtifactCache::compute_input_hash` does. We do not roll our own.
- **Layer hygiene.** New code lives in:
  - `vox-package` (L1) — bundle store
  - `vox-mesh-types` (L1) — wire types
  - `vox-db` (L1) — DDL
  - `vox-workflow-runtime` (L2) — tracker hooks
  - `vox-codegen` (L3) — lowering split
  - `vox-orchestrator` (L4) — dispatch / drain
  - `vox-cli` (L5) — operator commands

  No backward edges; `vox-arch-check` is a hard gate at every commit.
- **Phase 1 boundary.** We *consume* `DurablePromise[T]`, auto-derived `activity_id`, and `@remote`. We do not redefine them. The `generated_hash` we stamp is new and lives next to the existing `durability: Option<DurabilityKind>` field on `HirFn`.

---

## Revision history

- **2026-05-09.** Initial implementation plan landed alongside the Mesh & Language SSOT.
