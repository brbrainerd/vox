# Orchestrator Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce `vox-orchestrator` compile times and fan-in dependency count by (A) gating expensive optional deps behind feature flags and (B) extracting a new `vox-orchestrator-core` crate for the model registry, usage tracking, and budget subsystems.

**Architecture:** Two phases with no behavior changes. Phase A is purely additive Cargo.toml surgery — existing consumers are unaffected. Phase B moves files into a new crate and updates import paths; `vox-orchestrator` re-exports everything so downstream crates need zero changes.

**Tech Stack:** Rust, Cargo workspace, PowerShell (Windows), `cargo check`, `cargo test`

**Prerequisite:** None. This plan must complete before Plans 2, 3, and 4.

---

## File Map

### Phase A — feature-gate surgery (no new files)

| File | Change |
|---|---|
| `crates/vox-orchestrator/Cargo.toml` | Move 5 deps to optional / remove from default |
| `crates/vox-orchestrator/src/lib.rs` | Add `#[cfg(feature = "...")]` guards on affected modules |

### Phase B — vox-orchestrator-core extraction

| Action | Path |
|---|---|
| CREATE | `crates/vox-orchestrator-core/Cargo.toml` |
| CREATE | `crates/vox-orchestrator-core/build.rs` (copy + trim from orchestrator) |
| CREATE | `crates/vox-orchestrator-core/src/lib.rs` |
| MOVE   | `crates/vox-orchestrator/src/models/` → `crates/vox-orchestrator-core/src/models/` |
| MOVE   | `crates/vox-orchestrator/src/usage.rs` → `crates/vox-orchestrator-core/src/usage.rs` |
| MOVE   | `crates/vox-orchestrator/src/usage_policy.rs` → `crates/vox-orchestrator-core/src/usage_policy.rs` |
| MOVE   | `crates/vox-orchestrator/src/budget/` → `crates/vox-orchestrator-core/src/budget/` |
| MODIFY | `Cargo.toml` (workspace root) — add `vox-orchestrator-core` member |
| MODIFY | `crates/vox-orchestrator/Cargo.toml` — add `vox-orchestrator-core` dep, remove moved deps |
| MODIFY | `crates/vox-orchestrator/src/lib.rs` — replace moved module defs with re-exports |
| MODIFY | All `crate::models::` / `crate::usage::` / `crate::budget::` imports in moved files |

---

## Task 1: Baseline health check

**Files:** (read-only)

- [ ] **Step 1: Verify the workspace compiles clean before touching anything**

```powershell
cargo check -p vox-orchestrator 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected output: no lines starting with `error`. If errors exist, stop and fix them before proceeding.

- [ ] **Step 2: Record current incremental build time as a baseline**

```powershell
# Touch lib.rs to force incremental rebuild, time it
(Get-Item crates\vox-orchestrator\src\lib.rs).LastWriteTime = Get-Date
Measure-Command { cargo check -p vox-orchestrator 2>&1 | Out-Null } | Select-Object TotalSeconds
```

Note the result. Target after Phase A: at least 15% faster clean build for crates that only need the lean feature set.

- [ ] **Step 3: Record current test count**

```powershell
cargo test -p vox-orchestrator --no-run 2>&1 | Select-String "test\[" | Measure-Object | Select-Object Count
```

This count must match after every phase. If it drops, something was accidentally removed.

---

## Task 2: Phase A — gate `axum` behind `http-server` feature

The `axum` web framework (lines 110 in `crates/vox-orchestrator/Cargo.toml`) is a non-optional dep
inside the core library. Only the orchestrator's HTTP routes need it; MCP tools already live in
`vox-orchestrator-mcp`. Gating it lets crates that import `vox-orchestrator` without HTTP avoid
compiling axum and its tower/hyper chain.

**Files:**
- Modify: `crates/vox-orchestrator/Cargo.toml`
- Modify: `crates/vox-orchestrator/src/lib.rs`

- [ ] **Step 1: Find every file in vox-orchestrator/src that imports axum**

```powershell
rg "use axum" crates/vox-orchestrator/src/ --files-with-matches
```

Note the list. These files' parent modules need `#[cfg(feature = "http-server")]` guards.

- [ ] **Step 2: Move `axum` to optional in Cargo.toml**

In `crates/vox-orchestrator/Cargo.toml`, change line 110:

```toml
# Before:
axum = { workspace = true, features = ["tokio", "http1", "json"] }

# After:
axum = { workspace = true, features = ["tokio", "http1", "json"], optional = true }
```

Add to the `[features]` section (after line 36):

```toml
# HTTP server routes (axum). Enabled by vox-orchestrator-d and vox-cli
# routes that serve JSON over TCP. Not needed by library consumers.
http-server = ["dep:axum"]
```

- [ ] **Step 3: Verify the feature compiles**

```powershell
cargo check -p vox-orchestrator --features http-server 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: errors about `use axum` in specific files — these will be fixed in Step 4.

- [ ] **Step 4: Gate axum-using modules in lib.rs**

For each file identified in Step 1, find its parent module declaration in
`crates/vox-orchestrator/src/lib.rs` and wrap it with `#[cfg(feature = "http-server")]`.

Example — if `src/services/http_routes.rs` imports axum:

```rust
// In crates/vox-orchestrator/src/lib.rs
#[cfg(feature = "http-server")]
pub mod services; // or whichever sub-module contains axum usage
```

If only a sub-module uses axum, gate the sub-module in that module's `mod.rs` instead:

```rust
// In crates/vox-orchestrator/src/services/mod.rs
#[cfg(feature = "http-server")]
pub mod http_routes;
```

- [ ] **Step 5: Verify no-default-features compiles**

```powershell
cargo check -p vox-orchestrator --no-default-features 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors (or errors only about unresolved features that haven't been gated yet — fix those).

- [ ] **Step 6: Verify default features still compile**

```powershell
cargo check -p vox-orchestrator 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 7: Run all orchestrator tests**

```powershell
cargo test -p vox-orchestrator 2>&1 | tail -10
```

Expected: last line contains `test result: ok`.

- [ ] **Step 8: Commit**

```powershell
git add crates/vox-orchestrator/Cargo.toml crates/vox-orchestrator/src/lib.rs
git add crates/vox-orchestrator/src/services/  # or whichever modules were gated
git commit -m "refactor(orchestrator): gate axum behind http-server feature"
```

---

## Task 3: Phase A — gate `tiktoken-rs` behind `token-counting` feature

`tiktoken-rs` (Cargo.toml line 59) is used for estimating token counts before calls. It's expensive
to compile and only needed on code paths that pre-count tokens for cost estimation or context
window management. Gate it so crates that don't need pre-call counting avoid the compile cost.

**Files:**
- Modify: `crates/vox-orchestrator/Cargo.toml`
- Modify: files that `use tiktoken_rs`

- [ ] **Step 1: Find all tiktoken_rs usages**

```powershell
rg "tiktoken" crates/vox-orchestrator/src/ --files-with-matches
```

- [ ] **Step 2: Make tiktoken-rs optional**

In `crates/vox-orchestrator/Cargo.toml`, change line 59:

```toml
# Before:
tiktoken-rs = { workspace = true }

# After:
tiktoken-rs = { workspace = true, optional = true }
```

Add to `[features]`:

```toml
# Pre-call token counting via tiktoken-rs. Needed for context-window
# management and pre-flight cost estimates. Off by default in the library;
# enabled by the daemon binary and tests that exercise context management.
token-counting = ["dep:tiktoken-rs"]
```

- [ ] **Step 3: Gate tiktoken usages with cfg**

For each file using `tiktoken_rs`, wrap the import and the using function with `#[cfg(feature = "token-counting")]`.

Example — in `crates/vox-orchestrator/src/context_envelope.rs` (if it uses tiktoken):

```rust
#[cfg(feature = "token-counting")]
use tiktoken_rs::cl100k_base;

#[cfg(feature = "token-counting")]
pub fn count_tokens(text: &str) -> usize {
    let bpe = cl100k_base().unwrap();
    bpe.encode_with_special_tokens(text).len()
}

#[cfg(not(feature = "token-counting"))]
pub fn count_tokens(text: &str) -> usize {
    // Cheap word-count approximation when tiktoken is disabled.
    // Accuracy is sufficient for routing decisions but not billing.
    text.split_whitespace().count() * 4 / 3
}
```

The fallback (`not(feature)`) uses a word-count heuristic so call sites don't need cfg guards.

- [ ] **Step 4: Verify both configurations compile**

```powershell
cargo check -p vox-orchestrator --no-default-features 2>&1 | Select-String "^error" | Select-Object -First 20
cargo check -p vox-orchestrator --features token-counting 2>&1 | Select-String "^error" | Select-Object -First 20
```

Both should produce zero errors.

- [ ] **Step 5: Run all orchestrator tests**

```powershell
cargo test -p vox-orchestrator 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator/Cargo.toml
git add crates/vox-orchestrator/src/  # all cfg-annotated files
git commit -m "refactor(orchestrator): gate tiktoken-rs behind token-counting feature"
```

---

## Task 4: Phase A — remove `toestub-gate` from default features

`toestub-gate` pulls `vox-code-audit` + `vox-lsp` + `tower-lsp-server` into every consumer
of `vox-orchestrator`, even in test/CI builds that don't use the LSP or quality gate.
These are heavy crates. They should be opt-in.

**Files:**
- Modify: `crates/vox-orchestrator/Cargo.toml`

- [ ] **Step 1: Remove `toestub-gate` from the `default` array**

In `crates/vox-orchestrator/Cargo.toml`, change the `[features]` `default` line:

```toml
# Before (lines 9-14):
default = [
    "toestub-gate",
    "runtime",
    "json-schema",
    "jj",
]

# After:
default = [
    "runtime",
    "json-schema",
    "jj",
]
```

`toestub-gate` and `lsp` features remain available; they're just no longer on by default.
Consumers that need the quality gate (like `vox-orchestrator-d`) add the feature explicitly.

- [ ] **Step 2: Update `vox-orchestrator-d/Cargo.toml` to opt-in explicitly**

`vox-orchestrator-d` is the daemon that needs the full quality gate. Add it there:

```toml
# In crates/vox-orchestrator-d/Cargo.toml, change:
vox-orchestrator = { workspace = true }

# To:
vox-orchestrator = { workspace = true, features = ["toestub-gate"] }
```

- [ ] **Step 3: Find other crates that need toestub-gate and add it there too**

```powershell
rg "toestub|vox_lsp|vox_code_audit" crates/ --files-with-matches -l
```

For each crate that uses those symbols directly, add `features = ["toestub-gate"]` to its
`vox-orchestrator` dep in `Cargo.toml`.

- [ ] **Step 4: Verify default build compiles**

```powershell
cargo check -p vox-orchestrator 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 5: Verify daemon builds with the explicit feature**

```powershell
cargo check -p vox-orchestrator-d 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 6: Run orchestrator tests**

```powershell
cargo test -p vox-orchestrator 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 7: Commit**

```powershell
git add crates/vox-orchestrator/Cargo.toml crates/vox-orchestrator-d/Cargo.toml
git commit -m "refactor(orchestrator): remove toestub-gate from default features"
```

---

## Task 5: Phase A — gate `vox-corpus database` behind `corpus-db` feature

`vox-corpus` is always pulled in with its `database` feature (Cargo.toml line 86). This forces
the DB machinery into every orchestrator consumer. Gate it.

**Files:**
- Modify: `crates/vox-orchestrator/Cargo.toml`
- Modify: files in `src/` that use `vox_corpus` with DB calls

- [ ] **Step 1: Find all corpus DB usages**

```powershell
rg "vox_corpus" crates/vox-orchestrator/src/ --files-with-matches
```

- [ ] **Step 2: Make the database feature optional**

```toml
# Before (line 86):
vox-corpus = { workspace = true, features = ["database"] }

# After:
vox-corpus = { workspace = true }
vox-corpus-db = ["vox-corpus/database"]  # add to [features]
```

Add to `[features]`:

```toml
corpus-db = ["vox-corpus/database"]
```

And add `"corpus-db"` to the `default` array (it was always on before, keep it default for now;
only remove from default in a follow-up once call sites are fully audited):

```toml
default = [
    "runtime",
    "json-schema",
    "jj",
    "corpus-db",
]
```

This is a no-behavior-change step — we're just making the feature explicit so it can be turned off
in lean builds later.

- [ ] **Step 3: Verify compilation**

```powershell
cargo check -p vox-orchestrator 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 4: Run tests**

```powershell
cargo test -p vox-orchestrator 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 5: Commit**

```powershell
git add crates/vox-orchestrator/Cargo.toml
git commit -m "refactor(orchestrator): gate vox-corpus/database behind corpus-db feature"
```

---

## Task 6: Phase A — measure build improvement

- [ ] **Step 1: Clean and time a fresh build**

```powershell
cargo clean -p vox-orchestrator
Measure-Command { cargo build -p vox-orchestrator 2>&1 | Out-Null } | Select-Object TotalSeconds
```

Compare against the Task 1 baseline. Target: ≥15% improvement in clean build time for the library.

- [ ] **Step 2: Measure incremental build**

```powershell
(Get-Item crates\vox-orchestrator\src\lib.rs).LastWriteTime = Get-Date
Measure-Command { cargo check -p vox-orchestrator 2>&1 | Out-Null } | Select-Object TotalSeconds
```

Target: ≤3.5 seconds incremental.

- [ ] **Step 3: Commit measurement note**

Append to `docs/src/architecture/build-time-log.md`:

```markdown
## 2026-06-18 Phase A feature-gate surgery

| Scenario | Before | After | Delta |
|---|---|---|---|
| Orchestrator incremental | <baseline from Task 1> | <measured> | <delta> |
| Clean build | <baseline> | <measured> | <delta> |
```

```powershell
git add docs/src/architecture/build-time-log.md
git commit -m "docs: record build times after Phase A feature-gate surgery"
```

---

## Task 7: Phase B — create `vox-orchestrator-core` crate scaffold

**Files:**
- Create: `crates/vox-orchestrator-core/Cargo.toml`
- Create: `crates/vox-orchestrator-core/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create the directory**

```powershell
New-Item -ItemType Directory -Force crates/vox-orchestrator-core/src | Out-Null
Write-Host "created"
```

- [ ] **Step 2: Write `crates/vox-orchestrator-core/Cargo.toml`**

```toml
[package]
name = "vox-orchestrator-core"
description = "Model registry, selection, autonomic discovery, usage tracking, and budget management. The model-agnostic compile unit for vox-orchestrator. No HTTP, no LSP, no code-audit."
version.workspace = true
edition.workspace = true
license.workspace = true

[features]
default = []
# Pre-call token counting. Mirrors the flag in vox-orchestrator.
token-counting = ["dep:tiktoken-rs"]

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
async-trait = { workspace = true }
parking_lot = { workspace = true }
blake3 = { workspace = true }
uuid = { workspace = true, features = ["v4"] }
chrono = { workspace = true }
getrandom = "0.2"
hmac = { workspace = true }
sha2 = { workspace = true }
sha3 = { workspace = true }
ed25519-dalek = { workspace = true }
base64 = { workspace = true }
hex = { workspace = true }
rand = { workspace = true }
rand_distr = { workspace = true }
futures-util = { workspace = true }
reqwest = { workspace = true, features = ["json"] }
regex = { workspace = true }

tiktoken-rs = { workspace = true, optional = true }

vox-db = { workspace = true }
vox-config = { workspace = true }
vox-secrets = { workspace = true }
vox-foundation = { workspace = true }
vox-orchestrator-types = { workspace = true }
vox-telemetry = { workspace = true }
vox-mesh-types = { workspace = true }
vox-http-client = { workspace = true }
vox-research-events = { workspace = true }
workspace-hack = { workspace = true }

[build-dependencies]
serde_yaml = { workspace = true }
serde = { workspace = true, features = ["derive"] }

[lints]
workspace = true
```

- [ ] **Step 3: Write `crates/vox-orchestrator-core/src/lib.rs` (stub — modules filled in later)**

```rust
//! # vox-orchestrator-core
//!
//! Model-agnostic core for `vox-orchestrator`: model registry, selection,
//! autonomic discovery, usage tracking, and budget management.
//!
//! This crate has **no** `axum`, LSP, or code-audit dependencies so it
//! compiles quickly and independently of HTTP or IDE tooling.

pub mod budget;
pub mod models;
pub mod usage;
pub mod usage_policy;
```

- [ ] **Step 4: Add to workspace `Cargo.toml`**

Open the root `Cargo.toml` and find the `[workspace] members` array. Add:

```toml
"crates/vox-orchestrator-core",
```

Keep it alphabetically adjacent to `"crates/vox-orchestrator"`.

- [ ] **Step 5: Verify the new crate is recognized**

```powershell
cargo metadata --no-deps --format-version 1 2>&1 | Select-String "vox-orchestrator-core"
```

Expected: a line containing `vox-orchestrator-core`.

- [ ] **Step 6: Commit the scaffold**

```powershell
git add crates/vox-orchestrator-core/ Cargo.toml
git commit -m "feat(orchestrator-core): add empty crate scaffold"
```

---

## Task 8: Phase B — copy `build.rs` to `vox-orchestrator-core`

The build script reads `contracts/orchestration/model-routing.v1.yaml` and generates Rust enums.
It must move with the `models/` module since the generated types are consumed there.

**Files:**
- Create: `crates/vox-orchestrator-core/build.rs` (copy of `crates/vox-orchestrator/build.rs`)

- [ ] **Step 1: Copy the build script**

```powershell
Copy-Item crates/vox-orchestrator/build.rs crates/vox-orchestrator-core/build.rs
```

- [ ] **Step 2: Verify the new crate builds (just the build script)**

```powershell
cargo check -p vox-orchestrator-core 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: errors about missing modules `budget`, `models`, `usage`, `usage_policy` — these are filled
in the next tasks. Build-script errors would show as `error[E...]: ...build.rs...`; fix those first.

- [ ] **Step 3: Commit**

```powershell
git add crates/vox-orchestrator-core/build.rs
git commit -m "feat(orchestrator-core): copy build.rs codegen script"
```

---

## Task 9: Phase B — move `models/` to `vox-orchestrator-core`

**Files:**
- Move: `crates/vox-orchestrator/src/models/` → `crates/vox-orchestrator-core/src/models/`
- Modify: all `crate::` references inside the moved files
- Modify: `crates/vox-orchestrator/src/lib.rs` — replace `pub mod models` with re-export

- [ ] **Step 1: Copy the models directory to the new crate**

```powershell
Copy-Item -Recurse crates/vox-orchestrator/src/models crates/vox-orchestrator-core/src/models
```

(We copy first, fix imports, then delete the original in Step 5.)

- [ ] **Step 2: Fix `crate::` imports in the copied files**

The moved files use `crate::catalog`, `crate::config`, `crate::types`, `crate::usage` etc.
Find all `crate::` references that now need to become cross-crate:

```powershell
rg "crate::" crates/vox-orchestrator-core/src/models/ --files-with-matches
```

For each file, apply these substitutions:

| Old import | New import |
|---|---|
| `crate::types::AgentTask` | `vox_orchestrator_types::AgentTask` |
| `crate::types::TaskCategory` | `vox_orchestrator_types::TaskCategory` |
| `crate::usage::LlmUsageKey` | `crate::usage::LlmUsageKey` (stays — usage moves too) |
| `crate::config::CostPreference` | `vox_config::CostPreference` (check if it's in vox_config) |
| `crate::catalog::ModelCatalog` | keep as `crate::catalog::ModelCatalog` (catalog moves with models) |

Run the check again to find any remaining `crate::` references to non-moved modules:

```powershell
rg "crate::" crates/vox-orchestrator-core/src/models/ | Select-String -NotMatch "crate::(usage|models|catalog|budget)"
```

Each remaining hit needs to be resolved to either a cross-crate import or a type that should also move.

- [ ] **Step 3: Copy `catalog.rs` to `vox-orchestrator-core` (models depend on it)**

```powershell
Copy-Item crates/vox-orchestrator/src/catalog.rs crates/vox-orchestrator-core/src/catalog.rs
```

Add `pub mod catalog;` to `crates/vox-orchestrator-core/src/lib.rs`.

Fix any `crate::` references in `catalog.rs` the same way as Step 2.

- [ ] **Step 4: Verify the new crate compiles with models**

```powershell
cargo check -p vox-orchestrator-core 2>&1 | Select-String "^error" | Select-Object -First 30
```

Fix errors one by one. Common patterns:
- Missing deps: add them to `vox-orchestrator-core/Cargo.toml`
- Unresolved `crate::` refs: update to `vox_orchestrator_types::` or `vox_config::`
- Feature-gated code: ensure the same `cfg` guards are present in the moved files

- [ ] **Step 5: Replace models in `vox-orchestrator` with re-exports**

After Step 4 compiles, update `vox-orchestrator`:

In `crates/vox-orchestrator/Cargo.toml`, add:

```toml
vox-orchestrator-core = { path = "../vox-orchestrator-core" }
```

In `crates/vox-orchestrator/src/lib.rs`, replace:

```rust
pub mod models;
```

with:

```rust
pub use vox_orchestrator_core::models;
```

Delete the original models directory from vox-orchestrator:

```powershell
Remove-Item -Recurse crates/vox-orchestrator/src/models
```

- [ ] **Step 6: Verify vox-orchestrator still compiles**

```powershell
cargo check -p vox-orchestrator 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 7: Run all orchestrator tests**

```powershell
cargo test -p vox-orchestrator 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 8: Commit**

```powershell
git add crates/vox-orchestrator-core/ crates/vox-orchestrator/
git commit -m "refactor(orchestrator): move models/ to vox-orchestrator-core"
```

---

## Task 10: Phase B — move `usage.rs`, `usage_policy.rs`, `budget/`

**Files:**
- Move: `crates/vox-orchestrator/src/usage.rs` → `crates/vox-orchestrator-core/src/usage.rs`
- Move: `crates/vox-orchestrator/src/usage_policy.rs` → `crates/vox-orchestrator-core/src/usage_policy.rs`
- Move: `crates/vox-orchestrator/src/budget/` → `crates/vox-orchestrator-core/src/budget/`

- [ ] **Step 1: Copy files to new crate**

```powershell
Copy-Item crates/vox-orchestrator/src/usage.rs crates/vox-orchestrator-core/src/usage.rs
Copy-Item crates/vox-orchestrator/src/usage_policy.rs crates/vox-orchestrator-core/src/usage_policy.rs
Copy-Item -Recurse crates/vox-orchestrator/src/budget crates/vox-orchestrator-core/src/budget
```

- [ ] **Step 2: Fix `crate::` imports in copied files**

```powershell
rg "crate::" crates/vox-orchestrator-core/src/usage.rs
rg "crate::" crates/vox-orchestrator-core/src/usage_policy.rs
rg "crate::" crates/vox-orchestrator-core/src/budget/
```

`usage.rs` references `crate::usage_policy::resolve_provider_limits` — this is also moving, so keep as `crate::usage_policy`.

`budget/mod.rs` likely references `crate::types` — change to `vox_orchestrator_types`.

- [ ] **Step 3: Verify new crate compiles**

```powershell
cargo check -p vox-orchestrator-core 2>&1 | Select-String "^error" | Select-Object -First 30
```

- [ ] **Step 4: Re-export from vox-orchestrator**

In `crates/vox-orchestrator/src/lib.rs`, add:

```rust
pub use vox_orchestrator_core::budget;
pub use vox_orchestrator_core::usage;
pub use vox_orchestrator_core::usage_policy;
```

Replace (or remove) the existing `pub mod budget;`, `pub mod usage;`, `pub mod usage_policy;` lines.

Delete originals:

```powershell
Remove-Item crates/vox-orchestrator/src/usage.rs
Remove-Item crates/vox-orchestrator/src/usage_policy.rs
Remove-Item -Recurse crates/vox-orchestrator/src/budget
```

- [ ] **Step 5: Verify both crates compile**

```powershell
cargo check -p vox-orchestrator-core 2>&1 | Select-String "^error" | Select-Object -First 20
cargo check -p vox-orchestrator 2>&1 | Select-String "^error" | Select-Object -First 20
```

Both: zero errors.

- [ ] **Step 6: Run all tests**

```powershell
cargo test -p vox-orchestrator-core 2>&1 | tail -10
cargo test -p vox-orchestrator 2>&1 | tail -10
```

Both: `test result: ok`.

- [ ] **Step 7: Commit**

```powershell
git add crates/vox-orchestrator-core/ crates/vox-orchestrator/
git commit -m "refactor(orchestrator): move usage + budget to vox-orchestrator-core"
```

---

## Task 11: Phase B — final build-time measurement and cleanup

- [ ] **Step 1: Remove build.rs from vox-orchestrator once core has it**

`vox-orchestrator/build.rs` still exists but now the codegen lives in `vox-orchestrator-core/build.rs`.
The orchestrator's build.rs should be stripped to just a rerun marker (or removed entirely if it
produces no output of its own):

```rust
// crates/vox-orchestrator/build.rs — stripped to just the rerun marker
fn main() {
    // Codegen for model routing enums moved to vox-orchestrator-core.
    // This stub satisfies Cargo's build.rs detection.
    println!("cargo:rerun-if-changed=build.rs");
}
```

Or delete it entirely if no other codegen lives there:

```powershell
Remove-Item crates/vox-orchestrator/build.rs
```

Verify:

```powershell
cargo check -p vox-orchestrator 2>&1 | Select-String "^error" | Select-Object -First 10
```

- [ ] **Step 2: Measure final build times**

```powershell
cargo clean -p vox-orchestrator -p vox-orchestrator-core
Measure-Command { cargo build -p vox-orchestrator 2>&1 | Out-Null } | Select-Object TotalSeconds
```

```powershell
# Touch only the models registry — should NOT rebuild vox-orchestrator routing/session/A2A now
(Get-Item crates\vox-orchestrator-core\src\models\registry.rs).LastWriteTime = Get-Date
Measure-Command { cargo check -p vox-orchestrator 2>&1 | Out-Null } | Select-Object TotalSeconds
```

This last measurement is the key win: touching model registry should only rebuild
`vox-orchestrator-core`, not the full `vox-orchestrator`.

- [ ] **Step 3: Run the full workspace check**

```powershell
cargo check --workspace 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 4: Run all related test suites**

```powershell
cargo test -p vox-orchestrator-core -p vox-orchestrator -p vox-orchestrator-mcp 2>&1 | tail -20
```

Expected: all suites report `test result: ok`.

- [ ] **Step 5: Update build-time-log.md with Phase B measurements**

```markdown
## 2026-06-18 Phase B vox-orchestrator-core extraction

| Scenario | Before | After | Delta |
|---|---|---|---|
| Touch models/registry.rs → incremental | <baseline> | <measured> | <delta> |
| Clean build orchestrator | <Phase A> | <measured> | <delta> |
```

- [ ] **Step 6: Final commit**

```powershell
git add .
git commit -m "refactor(orchestrator): Phase B complete — vox-orchestrator-core extraction

- models/, catalog.rs, usage.rs, usage_policy.rs, budget/ moved to vox-orchestrator-core
- vox-orchestrator re-exports all moved public APIs (zero breaking changes)
- build.rs codegen now lives in vox-orchestrator-core
- touching model registry no longer forces full orchestrator rebuild"
```

---

## Verification Checklist

Before marking Plan 1 complete:

- [ ] `cargo check --workspace` passes with zero errors
- [ ] `cargo test -p vox-orchestrator-core` passes
- [ ] `cargo test -p vox-orchestrator` passes (same test count as Task 1 baseline)
- [ ] `cargo test -p vox-orchestrator-mcp` passes
- [ ] `cargo check -p vox-orchestrator --no-default-features` compiles
- [ ] `cargo check -p vox-orchestrator --features http-server,token-counting,toestub-gate` compiles
- [ ] Build times documented in `build-time-log.md`
- [ ] All commits have descriptive messages

Plans 2, 3, and 4 may proceed in parallel once this checklist is green.
