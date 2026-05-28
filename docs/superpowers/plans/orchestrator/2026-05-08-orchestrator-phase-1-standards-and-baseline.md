# Phase 1 — Orchestrator Standards & Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Read the master plan first: [`2026-05-08-orchestrator-master-plan.md`](2026-05-08-orchestrator-master-plan.md).

**Goal:** Land instrumentation, fixtures, benchmarks, baselines, and contract scaffolds for the orchestrator policy program with **zero behavior change**. Every later phase reads from infrastructure landed here.

**Architecture:** One new crate (`vox-orchestrator-test-helpers`), one new directory (`crates/vox-orchestrator/benches/`), one schema migration (v59 → v60), four contract scaffolds, ~30 golden routing tests, one perf-baseline doc, one arch-check tightening. No production code changes. All tasks are TDD: failing test → implementation → passing test → commit.

**Tech Stack:** Rust + cargo workspace; `criterion` 0.5 for benchmarks; `proptest` 1.4 for property tests; `serde_yaml` 0.9 for contract loading; `jsonschema` 0.18 for schema validation; existing `vox-db`, `vox-orchestrator`, `vox-orchestrator-types`, `vox-arch-check` crates.

**Estimated tasks:** 22. Bite-sized, ≤5 min each.

---

## Pre-flight Checklist (do once before Task 1)

- [ ] **Verify worktree.** Confirm you are on a Vox worktree branch (not `main`). `git rev-parse --show-toplevel` should print a `.claude/worktrees/...` path.
- [ ] **Verify clean tree.** `git status` is clean.
- [ ] **Verify build green.** `cargo build --workspace` exits 0. If it doesn't, stop — fix the build first.
- [ ] **Verify arch-check green.** `cargo run -p vox-arch-check` exits 0.
- [ ] **Read these three sections:**
  - Master plan §3 (Five Quality Gates) — [`2026-05-08-orchestrator-master-plan.md`](2026-05-08-orchestrator-master-plan.md#3-the-five-quality-gates)
  - Master plan §8 (Cross-Cutting Standards)
  - Research doc Part 11 — [`docs/src/architecture/autonomous-orchestration-policy-research-2026.md`](../../../src/architecture/autonomous-orchestration-policy-research-2026.md#part-11--vox-mapping-reasonably-automatable-today)

---

## Task 1: Scaffold the `vox-orchestrator-test-helpers` crate

**Why this task exists.** No test-helper crate exists today (per pre-plan exploration). Every later phase needs a `MockModelRegistry`, a `MockBulletinBoard`, and a fixture loader; landing them once here eliminates duplication.

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/Cargo.toml`
- Create: `crates/vox-orchestrator-test-helpers/src/lib.rs`
- Modify: `Cargo.toml` (workspace root) — add to `members`
- Modify: `docs/src/architecture/layers.toml` — add `[crates.vox-orchestrator-test-helpers]` block
- Modify: `docs/src/architecture/where-things-live.md` — add row

- [ ] **Step 1.1: Read existing test-only crate to learn conventions.**

Find one example by reading `docs/src/architecture/layers.toml` and grepping for `kind = "test-only"`. Open that crate's `Cargo.toml` to confirm Rust edition, license, and feature shape.

```bash
cargo run -p vox-arch-check 2>&1 | head -5
# (sanity: should pass before any change)
```

- [ ] **Step 1.2: Create the crate's Cargo.toml.**

Create `crates/vox-orchestrator-test-helpers/Cargo.toml`:

```toml
[package]
name = "vox-orchestrator-test-helpers"
version = { workspace = true }
edition = { workspace = true }
license = { workspace = true }
publish = false
description = "Test fixtures and mocks for vox-orchestrator. Not for production code."

[lib]
path = "src/lib.rs"

[dependencies]
vox-orchestrator = { path = "../vox-orchestrator" }
vox-orchestrator-types = { path = "../vox-orchestrator-types" }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["sync"] }
```

If any of `version`/`edition`/`license` aren't workspace-managed in this repo, copy the literal values from `crates/vox-orchestrator/Cargo.toml`.

- [ ] **Step 1.3: Add the crate to the workspace.**

Open the root `Cargo.toml`. Find the `[workspace]` section's `members = [...]` array. Add `"crates/vox-orchestrator-test-helpers"` in alphabetical order with the other members.

- [ ] **Step 1.4: Add layer entry.**

Edit `docs/src/architecture/layers.toml`. Add this block in alphabetical order with other crate entries:

```toml
[crates.vox-orchestrator-test-helpers]
layer = 3
kind = "test-only"
max_dependents = 25
staleness_exempt = true   # test-only — not part of feature SLA
```

- [ ] **Step 1.5: Write a failing crate-import test.**

Create `crates/vox-orchestrator-test-helpers/src/lib.rs` with empty content first:

```rust
//! Test fixtures, mocks, and golden helpers for `vox-orchestrator`.
//!
//! **Decision axis:** infrastructure for all phases.
//!
//! This crate is `kind = "test-only"` per `layers.toml` — production code
//! must not depend on it.
```

- [ ] **Step 1.6: Verify the workspace compiles with the new crate.**

```bash
cargo build -p vox-orchestrator-test-helpers
```

Expected: builds clean.

- [ ] **Step 1.7: Verify arch-check still passes.**

```bash
cargo run -p vox-arch-check
```

Expected: exit 0. If it complains about `where-things-live.md` (P11 adds the rule), defer that to step 1.9.

- [ ] **Step 1.8: Add the where-things-live row.**

Open `docs/src/architecture/where-things-live.md`. Add the row in the appropriate alphabetical position:

```markdown
| test fixtures and mocks for orchestrator | `vox-orchestrator-test-helpers` | `MockModelRegistry`, `MockBulletinBoard`, `load_golden_fixture` |
```

- [ ] **Step 1.9: Commit.**

```bash
git add crates/vox-orchestrator-test-helpers Cargo.toml docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "$(cat <<'EOF'
chore(orchestrator): scaffold vox-orchestrator-test-helpers crate

Phase 1 of orchestrator-master-plan. Empty test-helpers crate so later
tasks can populate it with MockModelRegistry, MockBulletinBoard, and
golden-fixture loader. No production behavior change.

Refs: docs/superpowers/plans/orchestrator/2026-05-08-orchestrator-master-plan.md
EOF
)"
```

---

## Task 2: Implement `MockModelRegistry`

**Why this task exists.** Phases 2–10 need to instantiate a `ModelRegistry` in unit tests without hitting the OpenRouter catalog. The mock returns deterministic specs for a fixed set of model IDs.

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/src/mock_registry.rs`
- Modify: `crates/vox-orchestrator-test-helpers/src/lib.rs`
- Test: `crates/vox-orchestrator-test-helpers/tests/mock_registry_test.rs`

- [ ] **Step 2.1: Read the real ModelRegistry shape.**

Open `crates/vox-orchestrator/src/models/registry.rs` and `crates/vox-orchestrator/src/models/spec.rs`. Note the public constructors and methods. The mock must use only the public API — do not reach into private fields.

Specifically capture: how `ModelRegistry::new()` (or equivalent) is constructed today, what `ModelSpec` requires, and whether `best_for_task()` takes `&self` or `&mut self`.

- [ ] **Step 2.2: Write the failing test.**

Create `crates/vox-orchestrator-test-helpers/tests/mock_registry_test.rs`:

```rust
use vox_orchestrator_test_helpers::MockModelRegistry;
use vox_orchestrator::models::TaskCategory;

#[test]
fn mock_registry_returns_a_model_for_code_task() {
    let registry = MockModelRegistry::with_default_models().build();
    let pick = registry.best_for(TaskCategory::Code).expect("should have a default model for Code");
    assert!(!pick.id.is_empty());
}

#[test]
fn mock_registry_has_three_tiers() {
    let registry = MockModelRegistry::with_default_models().build();
    let cheap = registry.best_for(TaskCategory::Chat).expect("chat tier exists");
    let strong = registry.best_for(TaskCategory::Refactor).expect("refactor tier exists");
    assert_ne!(cheap.id, strong.id, "tiers should differentiate at least one task category");
}
```

If the actual public method names differ (per Step 2.1's findings), substitute them — but keep the test asserting the *behavior* described, not the name.

- [ ] **Step 2.3: Run test — verify it fails.**

```bash
cargo test -p vox-orchestrator-test-helpers --test mock_registry_test 2>&1 | tail -10
```

Expected: compile error — `MockModelRegistry` not found.

- [ ] **Step 2.4: Implement the mock.**

Create `crates/vox-orchestrator-test-helpers/src/mock_registry.rs`. The implementation depends on what Step 2.1 found about the real registry's public API; adapt the skeleton below:

```rust
//! Mock `ModelRegistry` builder for unit tests.
//!
//! **Decision axis:** infrastructure (used by all phases).

use vox_orchestrator::models::{ModelRegistry, ModelSpec};

/// Builder for a minimal `ModelRegistry` populated with deterministic specs.
///
/// Use [`MockModelRegistry::with_default_models`] to get a registry with
/// a Cheap, Mid, and Strong model already wired. Use the builder methods
/// to override before calling [`Self::build`].
pub struct MockModelRegistry {
    specs: Vec<ModelSpec>,
}

impl MockModelRegistry {
    /// Create a builder pre-populated with one Cheap, one Mid, one Strong model.
    pub fn with_default_models() -> Self {
        Self {
            specs: vec![
                Self::cheap_spec(),
                Self::mid_spec(),
                Self::strong_spec(),
            ],
        }
    }

    /// Replace all specs with a custom set.
    pub fn with_specs(mut self, specs: Vec<ModelSpec>) -> Self {
        self.specs = specs;
        self
    }

    /// Add a single spec to the registry.
    pub fn add_spec(mut self, spec: ModelSpec) -> Self {
        self.specs.push(spec);
        self
    }

    /// Materialize the `ModelRegistry`. Reads from no external source.
    pub fn build(self) -> ModelRegistry {
        // Use the real public constructor. If `ModelRegistry::new()` doesn't
        // accept a Vec<ModelSpec>, look at how OpenRouterCatalog populates the
        // registry today (crates/vox-orchestrator/src/catalog.rs) and mirror
        // that pattern — but read from `self.specs` instead of HTTP.
        ModelRegistry::from_specs(self.specs)
    }

    fn cheap_spec() -> ModelSpec {
        // Fill with the same fields as a real `ModelSpec`. Look at
        // crates/vox-orchestrator/src/models/spec.rs for the field list.
        // The cheap tier should map to StrengthTag::Light (or equivalent).
        // Use deterministic values: id = "mock-cheap", paid_input_per_million = 1.0, etc.
        unimplemented!("populate from real ModelSpec — see spec.rs for fields")
    }

    fn mid_spec() -> ModelSpec {
        unimplemented!("populate — id = mock-mid")
    }

    fn strong_spec() -> ModelSpec {
        unimplemented!("populate — id = mock-strong")
    }
}
```

The `unimplemented!()` calls in `*_spec()` are intentional — when running this task, **read the actual `ModelSpec` struct** in `crates/vox-orchestrator/src/models/spec.rs` and fill in concrete values. The plan can't enumerate fields the agent has not yet read; it directs the agent to read them.

If `ModelRegistry::from_specs` does not exist in the real registry, add it as a `pub` constructor in the real registry first (it's a 5-line wrapper around the existing init path) — that's the only production-code change in P1 and it's purely additive.

- [ ] **Step 2.5: Wire mock_registry into the lib.rs.**

Edit `crates/vox-orchestrator-test-helpers/src/lib.rs`:

```rust
//! Test fixtures, mocks, and golden helpers for `vox-orchestrator`.
//!
//! **Decision axis:** infrastructure for all phases.
//!
//! This crate is `kind = "test-only"` per `layers.toml` — production code
//! must not depend on it.

mod mock_registry;
pub use mock_registry::MockModelRegistry;
```

- [ ] **Step 2.6: Run the test — verify it passes.**

```bash
cargo test -p vox-orchestrator-test-helpers --test mock_registry_test
```

Expected: 2 tests pass.

- [ ] **Step 2.7: Commit.**

```bash
git add crates/vox-orchestrator-test-helpers crates/vox-orchestrator/src/models/registry.rs
git commit -m "$(cat <<'EOF'
test(orchestrator): add MockModelRegistry to test-helpers

Builder-pattern mock for ModelRegistry. Pre-populated with
deterministic Cheap/Mid/Strong specs. Used by phase 2-10 unit tests.

Refs: docs/superpowers/plans/orchestrator/2026-05-08-orchestrator-phase-1-standards-and-baseline.md
EOF
)"
```

---

## Task 3: Implement `MockBulletinBoard`

**Why this task exists.** Phase 6 (HITL interrupts) and Phase 2 (circuit breaker) both publish `EscalationEvent` and trip events to the bulletin. Tests need a way to assert "this event was published" without spinning up the full async runtime.

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/src/mock_bulletin.rs`
- Modify: `crates/vox-orchestrator-test-helpers/src/lib.rs`
- Test: `crates/vox-orchestrator-test-helpers/tests/mock_bulletin_test.rs`

- [ ] **Step 3.1: Read existing BulletinBoard shape.**

Open `crates/vox-orchestrator/src/bulletin.rs`. Note `BulletinBoard::publish`, `BulletinBoard::subscribe`, the `AgentMessage` enum, and the existing test patterns inside that file (lines 64–100 per pre-plan exploration).

- [ ] **Step 3.2: Write failing test.**

Create `crates/vox-orchestrator-test-helpers/tests/mock_bulletin_test.rs`:

```rust
use vox_orchestrator_test_helpers::MockBulletinBoard;
use vox_orchestrator::types::AgentMessage;

#[tokio::test]
async fn mock_bulletin_records_published_messages() {
    let bulletin = MockBulletinBoard::new();
    let task_id = "task-123".to_string();
    let agent_id = "agent-A".to_string();
    bulletin.publish(AgentMessage::TaskCompleted {
        task_id: task_id.clone(),
        agent_id: agent_id.clone(),
    }).await;

    let recorded = bulletin.recorded_messages();
    assert_eq!(recorded.len(), 1);
    match &recorded[0] {
        AgentMessage::TaskCompleted { task_id: t, agent_id: a } => {
            assert_eq!(t, &task_id);
            assert_eq!(a, &agent_id);
        }
        other => panic!("expected TaskCompleted, got {other:?}"),
    }
}

#[tokio::test]
async fn mock_bulletin_finds_message_by_predicate() {
    let bulletin = MockBulletinBoard::new();
    bulletin.publish(AgentMessage::TaskCompleted {
        task_id: "task-A".into(),
        agent_id: "agent-1".into(),
    }).await;
    bulletin.publish(AgentMessage::TaskCompleted {
        task_id: "task-B".into(),
        agent_id: "agent-2".into(),
    }).await;

    let found = bulletin.find_message(|m| matches!(m,
        AgentMessage::TaskCompleted { task_id, .. } if task_id == "task-B"
    ));
    assert!(found.is_some());
}
```

If the `AgentMessage` shape differs (e.g., variant names), substitute the correct variant per Step 3.1's findings.

- [ ] **Step 3.3: Run test — verify it fails.**

```bash
cargo test -p vox-orchestrator-test-helpers --test mock_bulletin_test
```

Expected: compile error — `MockBulletinBoard` not found.

- [ ] **Step 3.4: Implement the mock.**

Create `crates/vox-orchestrator-test-helpers/src/mock_bulletin.rs`:

```rust
//! Mock `BulletinBoard` for unit tests. Records published messages
//! into an in-memory `Vec` instead of fanning out via tokio broadcast.
//!
//! **Decision axis:** infrastructure (used by phases 2, 6, 9, 10).

use std::sync::Arc;
use tokio::sync::Mutex;
use vox_orchestrator::types::AgentMessage;

/// In-memory mock that records every published message.
#[derive(Clone, Default)]
pub struct MockBulletinBoard {
    messages: Arc<Mutex<Vec<AgentMessage>>>,
}

impl MockBulletinBoard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mirror `BulletinBoard::publish` — records the message.
    pub async fn publish(&self, msg: AgentMessage) {
        self.messages.lock().await.push(msg);
    }

    /// Snapshot of all messages published so far. Returns a clone — does
    /// not block the producer.
    pub fn recorded_messages(&self) -> Vec<AgentMessage> {
        // Try-lock is fine; tests are single-threaded by default.
        self.messages.try_lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Find the first message matching `predicate`. Convenience for tests
    /// that want to assert on a specific event without iterating.
    pub fn find_message<F>(&self, predicate: F) -> Option<AgentMessage>
    where
        F: Fn(&AgentMessage) -> bool,
    {
        self.recorded_messages().into_iter().find(|m| predicate(m))
    }

    /// Number of messages published.
    pub fn count(&self) -> usize {
        self.messages.try_lock().map(|g| g.len()).unwrap_or(0)
    }

    /// Clear recorded messages — useful for multi-phase tests.
    pub async fn clear(&self) {
        self.messages.lock().await.clear();
    }
}
```

If `AgentMessage` does not implement `Clone`, derive it on `AgentMessage` in the same commit (this is additive, not a behavior change). If it can't be `Clone` for legitimate reasons (e.g., owned `tokio::Sender`), wrap published items in `Arc<AgentMessage>` instead and adjust the test.

- [ ] **Step 3.5: Wire into lib.rs.**

Edit `crates/vox-orchestrator-test-helpers/src/lib.rs` to append:

```rust
mod mock_bulletin;
pub use mock_bulletin::MockBulletinBoard;
```

- [ ] **Step 3.6: Run test — verify it passes.**

```bash
cargo test -p vox-orchestrator-test-helpers --test mock_bulletin_test
```

Expected: 2 tests pass.

- [ ] **Step 3.7: Commit.**

```bash
git add crates/vox-orchestrator-test-helpers
git add crates/vox-orchestrator/src/types  # only if AgentMessage Clone derive was added
git commit -m "test(orchestrator): add MockBulletinBoard to test-helpers"
```

---

## Task 4: Golden-fixture loader

**Why this task exists.** Phase 1 (Task 18) and later phases need to load JSON fixtures of (input → expected output). A single loader avoids 30 ad-hoc `std::fs::read` calls.

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/src/golden.rs`
- Create: `crates/vox-orchestrator-test-helpers/fixtures/.gitkeep`
- Modify: `crates/vox-orchestrator-test-helpers/src/lib.rs`
- Test: `crates/vox-orchestrator-test-helpers/tests/golden_test.rs`

- [ ] **Step 4.1: Write failing test.**

Create `crates/vox-orchestrator-test-helpers/fixtures/example.json`:

```json
{ "input": { "task": "ping" }, "expected": { "response": "pong" } }
```

Create `crates/vox-orchestrator-test-helpers/tests/golden_test.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Debug, Deserialize, PartialEq)]
struct Example {
    input: ExampleInput,
    expected: ExampleOutput,
}

#[derive(Debug, Deserialize, PartialEq)]
struct ExampleInput { task: String }

#[derive(Debug, Deserialize, PartialEq)]
struct ExampleOutput { response: String }

#[test]
fn loads_a_json_fixture_by_relative_path() {
    let fixture: Example = load_golden_fixture("example.json")
        .expect("fixture should load");
    assert_eq!(fixture.input.task, "ping");
    assert_eq!(fixture.expected.response, "pong");
}

#[test]
fn missing_fixture_returns_clear_error() {
    let result: Result<Example, _> = load_golden_fixture("does-not-exist.json");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("does-not-exist.json"), "error should name the missing file");
}
```

- [ ] **Step 4.2: Run — verify fail.**

```bash
cargo test -p vox-orchestrator-test-helpers --test golden_test
```

Expected: compile error — `load_golden_fixture` not found.

- [ ] **Step 4.3: Implement loader.**

Create `crates/vox-orchestrator-test-helpers/src/golden.rs`:

```rust
//! Golden-fixture loader. Reads JSON files from the crate's `fixtures/`
//! directory and deserializes them into a caller-supplied type.
//!
//! **Decision axis:** infrastructure (used by P1 task 18 and P2-P10 golden tests).

use serde::de::DeserializeOwned;
use std::path::PathBuf;

/// Error type for fixture loading. Names the fixture path on every variant.
#[derive(Debug, thiserror::Error)]
pub enum FixtureError {
    #[error("fixture file not found: {0}")]
    NotFound(PathBuf),
    #[error("fixture {path} could not be read: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("fixture {path} could not be parsed: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// Load a JSON fixture from the test-helpers `fixtures/` directory.
///
/// `relative_path` is relative to `crates/vox-orchestrator-test-helpers/fixtures/`.
/// Use forward slashes for subdirectories on all platforms.
pub fn load_golden_fixture<T: DeserializeOwned>(
    relative_path: impl AsRef<std::path::Path>,
) -> Result<T, FixtureError> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = crate_dir.join("fixtures").join(relative_path.as_ref());
    if !path.exists() {
        return Err(FixtureError::NotFound(path));
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| FixtureError::Read { path: path.clone(), source: e })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| FixtureError::Parse { path, source: e })
}
```

Add `thiserror = { workspace = true }` and `serde_json = { workspace = true }` to the test-helpers `Cargo.toml` if not already present (Task 1.2 had `serde_json`; add `thiserror`).

- [ ] **Step 4.4: Wire into lib.rs.**

Append to `crates/vox-orchestrator-test-helpers/src/lib.rs`:

```rust
mod golden;
pub use golden::{FixtureError, load_golden_fixture};
```

- [ ] **Step 4.5: Run — verify pass.**

```bash
cargo test -p vox-orchestrator-test-helpers --test golden_test
```

Expected: 2 tests pass.

- [ ] **Step 4.6: Commit.**

```bash
git add crates/vox-orchestrator-test-helpers
git commit -m "test(orchestrator): add golden-fixture loader"
```

---

## Task 5: First benchmark — `route_decision`

**Why this task exists.** Phase 1 must establish a perf baseline. `best_for_task` is on the hot path of every tool call — we need to know its current p50/p99.

**Files:**
- Create: `crates/vox-orchestrator/benches/route_decision.rs`
- Modify: `crates/vox-orchestrator/Cargo.toml`

- [ ] **Step 5.1: Read existing bench setup.**

Open `crates/vox-compiler/benches/compiler_pipeline.rs` (per pre-plan exploration). Note the criterion setup, the `[[bench]]` block in that crate's Cargo.toml, and any helpers used. Mirror those conventions.

- [ ] **Step 5.2: Add criterion + the bench section to vox-orchestrator's Cargo.toml.**

Edit `crates/vox-orchestrator/Cargo.toml`. In `[dev-dependencies]`, add (if not already present):

```toml
criterion = { workspace = true }
vox-orchestrator-test-helpers = { path = "../vox-orchestrator-test-helpers" }
```

Add at the bottom of the file:

```toml
[[bench]]
name = "route_decision"
harness = false
```

- [ ] **Step 5.3: Write the bench file.**

Create `crates/vox-orchestrator/benches/route_decision.rs`:

```rust
//! Benchmark for `ModelRegistry::best_for_task` — the hot path on every routing decision.
//!
//! **Phase 1 baseline.** Captures p50/p99 for the decision used by every tool call.
//! Budget for any future change: ≤ baseline × 1.20.

use criterion::{criterion_group, criterion_main, Criterion};
use vox_orchestrator::models::TaskCategory;
use vox_orchestrator_test_helpers::MockModelRegistry;

fn bench_best_for_task(c: &mut Criterion) {
    let registry = MockModelRegistry::with_default_models().build();
    let categories = [
        TaskCategory::Code,
        TaskCategory::Chat,
        TaskCategory::Refactor,
    ];
    let mut idx = 0usize;
    c.bench_function("best_for_task", |b| {
        b.iter(|| {
            let cat = categories[idx % categories.len()];
            idx = idx.wrapping_add(1);
            // We use `best_for` here because the Mock doesn't simulate full
            // AgentTask shapes. If `best_for_task` is the canonical hot path,
            // build a minimal AgentTask and call it instead. See registry.rs:569.
            let _ = registry.best_for(cat);
        });
    });
}

criterion_group!(benches, bench_best_for_task);
criterion_main!(benches);
```

If `criterion` is not already in `[workspace.dependencies]`, add `criterion = "0.5"` to the workspace Cargo.toml's `[workspace.dependencies]` block.

- [ ] **Step 5.4: Run the bench.**

```bash
cargo bench -p vox-orchestrator --bench route_decision
```

Expected: criterion runs, prints a number per iteration. Capture the printed stat — you'll record it in Task 10.

- [ ] **Step 5.5: Verify arch-check still green.**

```bash
cargo run -p vox-arch-check
```

Expected: exit 0. The new `[[bench]]` should not change layer-classification.

- [ ] **Step 5.6: Commit.**

```bash
git add crates/vox-orchestrator/benches crates/vox-orchestrator/Cargo.toml Cargo.toml
git commit -m "$(cat <<'EOF'
perf(orchestrator): add route_decision criterion benchmark

Captures p50/p99 of best_for_task on the routing hot path. First entry
in the orchestrator benchmark suite. Baseline numbers will be recorded
in orchestrator-perf-baseline-2026.md (task 10).

Refs: docs/superpowers/plans/orchestrator/2026-05-08-orchestrator-phase-1-standards-and-baseline.md
EOF
)"
```

---

## Task 6: Benchmark — `socrates_gate`

**Files:** Create `crates/vox-orchestrator/benches/socrates_gate.rs`; modify `crates/vox-orchestrator/Cargo.toml`.

- [ ] **Step 6.1: Add `[[bench]]` entry.** Append to `crates/vox-orchestrator/Cargo.toml`:

```toml
[[bench]]
name = "socrates_gate"
harness = false
```

- [ ] **Step 6.2: Write the bench.**

```rust
//! Benchmark for ConfidencePolicy::evaluate_risk_decision (Socrates gate eval).
//!
//! Hot path for every claim emitted with stakes >= medium. Phase 1 baseline.

use criterion::{criterion_group, criterion_main, Criterion};
use vox_orchestrator_types::socrates_policy::{
    ConfidencePolicy, RiskBand, // adjust per the real exports in mod.rs
};

fn bench_evaluate_risk_decision(c: &mut Criterion) {
    let policy = ConfidencePolicy::default();
    let bands = [RiskBand::Low, RiskBand::Medium, RiskBand::High];
    let mut idx = 0usize;
    c.bench_function("confidence_policy::evaluate_risk_decision", |b| {
        b.iter(|| {
            let band = bands[idx % bands.len()];
            idx = idx.wrapping_add(1);
            // Confirm the actual method signature in confidence_policy.rs:99
            // and adjust the call shape if needed.
            let _ = policy.evaluate_risk_decision(band, /* confidence */ 0.72);
        });
    });
}

criterion_group!(benches, bench_evaluate_risk_decision);
criterion_main!(benches);
```

The exact signature of `evaluate_risk_decision` was named in pre-plan exploration but not fully captured. **Open `crates/vox-orchestrator-types/src/socrates_policy/confidence_policy.rs` and read the real signature** before finalizing this bench.

- [ ] **Step 6.3: Run.** `cargo bench -p vox-orchestrator --bench socrates_gate`. Expected: criterion runs.

- [ ] **Step 6.4: Commit.** `git commit -m "perf(orchestrator): add socrates_gate benchmark"`

---

## Task 7: Benchmark — `bulletin_throughput`

**Files:** Create `crates/vox-orchestrator/benches/bulletin_throughput.rs`; modify Cargo.toml.

- [ ] **Step 7.1: Add `[[bench]]` entry** named `bulletin_throughput`.

- [ ] **Step 7.2: Write the bench.** Measures `BulletinBoard::publish` + a single subscriber receive cycle. Pattern:

```rust
//! Benchmark for BulletinBoard publish/subscribe throughput.
//! Phase 1 baseline. Budget for new event types: must not regress p99.

use criterion::{criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;
use vox_orchestrator::bulletin::BulletinBoard;
use vox_orchestrator::types::AgentMessage;

fn bench_publish_subscribe_roundtrip(c: &mut Criterion) {
    let rt = Runtime::new().expect("tokio runtime");
    let bulletin = BulletinBoard::new();
    let mut sub = rt.block_on(async { bulletin.subscribe() });
    c.bench_function("bulletin::publish_then_recv", |b| {
        b.iter(|| {
            rt.block_on(async {
                bulletin.publish(AgentMessage::TaskCompleted {
                    task_id: "bench".into(),
                    agent_id: "bench-agent".into(),
                }).await;
                let _ = sub.recv().await;
            });
        });
    });
}

criterion_group!(benches, bench_publish_subscribe_roundtrip);
criterion_main!(benches);
```

If `BulletinBoard::new()` doesn't exist as a public constructor, use whatever the existing tests at lines 64–100 of `bulletin.rs` use to construct one.

- [ ] **Step 7.3: Run.** `cargo bench -p vox-orchestrator --bench bulletin_throughput`. Expected: criterion runs.

- [ ] **Step 7.4: Commit.** `git commit -m "perf(orchestrator): add bulletin_throughput benchmark"`

---

## Task 8: Benchmark — `compaction_pipeline`

**Files:** Create `crates/vox-orchestrator/benches/compaction_pipeline.rs`; modify Cargo.toml.

- [ ] **Step 8.1: Add `[[bench]]` entry.**

- [ ] **Step 8.2: Read the existing compaction surface.** Open `crates/vox-orchestrator/src/compaction.rs`. Identify the public entry point (likely `Compactor::compact()` or similar) and what it accepts.

- [ ] **Step 8.3: Write the bench.** Pattern (adjust to real API):

```rust
//! Benchmark for the compaction pipeline. Phase 1 baseline.
//! P8 will replace single-threshold compaction with the 5-layer pipeline;
//! that phase's perf budget is "no layer's p99 exceeds the current single-threshold p99".

use criterion::{criterion_group, criterion_main, Criterion};
// use vox_orchestrator::compaction::*;  // confirm the real path

fn bench_compact_full_context(c: &mut Criterion) {
    // Build a fake context of ~50KB of varied messages.
    // Use the real Compactor entry point.
    c.bench_function("compaction::compact_50kb_context", |b| {
        b.iter(|| {
            // let _ = compactor.compact(&context);
            unimplemented!("read crates/vox-orchestrator/src/compaction.rs for real API")
        });
    });
}

criterion_group!(benches, bench_compact_full_context);
criterion_main!(benches);
```

The `unimplemented!()` call is intentional in the plan — when executing, read the real compaction surface and replace it with the actual call.

- [ ] **Step 8.4: Run.** `cargo bench -p vox-orchestrator --bench compaction_pipeline`. Expected: numbers printed.

- [ ] **Step 8.5: Commit.** `git commit -m "perf(orchestrator): add compaction_pipeline benchmark"`

---

## Task 9: Benchmark — `plan_refinement`

**Files:** Create `crates/vox-orchestrator/benches/plan_refinement.rs`; modify Cargo.toml.

- [ ] **Step 9.1: Add `[[bench]]` entry.**

- [ ] **Step 9.2: Write the bench.** Targets the hash-fingerprint detection in `plan_loop.rs::hash_fingerprint`. This is a pure function and an easy first measurement.

```rust
//! Benchmark for plan-refinement structural fingerprinting.
//! Phase 1 baseline. P5 may replace the heuristic; budget: no regression.

use criterion::{criterion_group, criterion_main, Criterion};
// use vox_orchestrator_mcp::chat_tools::plan_loop::hash_fingerprint; // confirm real path

fn bench_hash_fingerprint(c: &mut Criterion) {
    let sample_plan_json = include_str!("fixtures/sample_plan.json");
    c.bench_function("plan_loop::hash_fingerprint", |b| {
        b.iter(|| {
            // let _ = hash_fingerprint(sample_plan_json);
            unimplemented!("read plan_loop.rs:33 for the real signature")
        });
    });
}

criterion_group!(benches, bench_hash_fingerprint);
criterion_main!(benches);
```

Create a fixture `crates/vox-orchestrator/benches/fixtures/sample_plan.json` with a representative plan structure (~1KB JSON).

- [ ] **Step 9.3: Run.** `cargo bench -p vox-orchestrator --bench plan_refinement`. Expected: numbers.

- [ ] **Step 9.4: Commit.** `git commit -m "perf(orchestrator): add plan_refinement benchmark"`

---

## Task 10: Capture the perf-baseline doc

**Why this task exists.** Master plan §3 Gate G3 requires every phase to assert p99 against a budget. The budgets for P2–P11 are derived from this baseline.

**Files:**
- Create: `docs/src/architecture/orchestrator-perf-baseline-2026.md`
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 10.1: Run all benchmarks and capture numbers.**

```bash
cargo bench -p vox-orchestrator 2>&1 | tee /tmp/orch-bench.log
```

Open `/tmp/orch-bench.log`. For each bench, extract:
- mean (ns/iter)
- p99 (criterion reports `slope` and `mean ± std`; for a quick p99 estimate, use mean + 2.33*std-dev or run with `--measurement-time 30` for tighter intervals).

Capture them in a table.

- [ ] **Step 10.2: Write the baseline doc.**

Create `docs/src/architecture/orchestrator-perf-baseline-2026.md`:

```markdown
---
title: "Orchestrator Performance Baseline (P1)"
description: "Baseline p50/p99 numbers for the orchestrator hot paths captured at the start of the autonomous-orchestration policy program. Every later phase must hold p99 ≤ 1.20 × baseline on the relevant path or document a justified deviation."
category: "architecture"
status: "current"
last_updated: "2026-05-08"
training_eligible: true
training_rationale: "Authoritative perf baseline for orchestrator program; consumed by phase plans P2–P11."
---

# Orchestrator Performance Baseline (Phase 1)

## How to read this

Each row records the p50 and p99 of one criterion benchmark at the point Phase 1 of [the orchestrator master plan](./2026-05-08-orchestrator-master-plan.md) merged. Subsequent phases append rows with their own measurements.

Quality gate G3 (master plan §3) is enforced as: any change touching a benchmarked path must keep p99 ≤ 1.20 × baseline on that path, or the phase plan must include an explicit perf-deviation justification.

## Baseline (P1 merge)

| Bench | p50 (ns) | p99 (ns) | Captured | Notes |
|---|---|---|---|---|
| `route_decision::best_for_task` | _fill_ | _fill_ | 2026-05-08 | MockModelRegistry; 3 categories rotated |
| `socrates_gate::evaluate_risk_decision` | _fill_ | _fill_ | 2026-05-08 | default ConfidencePolicy |
| `bulletin::publish_then_recv` | _fill_ | _fill_ | 2026-05-08 | single subscriber |
| `compaction::compact_50kb_context` | _fill_ | _fill_ | 2026-05-08 | synthetic 50KB context |
| `plan_loop::hash_fingerprint` | _fill_ | _fill_ | 2026-05-08 | 1KB sample plan JSON |

## Per-phase deltas

(Filled in as phases land. Empty until P2 merges.)

| Phase | Bench | p99 baseline | p99 post | Δ% | Status |
|---|---|---|---|---|---|

## Hardware & build

- Target: native (whatever `rustc -vV --target` reports on the build machine).
- Cargo profile: `release` (criterion default).
- Run command: `cargo bench -p vox-orchestrator`.
- CARGO_TARGET_DIR: shared per `.cargo/config.toml`.
```

Replace the `_fill_` placeholders with the numbers from Step 10.1.

- [ ] **Step 10.3: Add the where-things-live row.**

Edit `docs/src/architecture/where-things-live.md`:

```markdown
| orchestrator perf baseline | `docs/src/architecture/` | `orchestrator-perf-baseline-2026.md` |
```

- [ ] **Step 10.4: Commit.**

```bash
git add docs/src/architecture/orchestrator-perf-baseline-2026.md docs/src/architecture/where-things-live.md
git commit -m "docs(orchestrator): capture P1 perf baseline"
```

---

## Task 11: Schema migration v59 → v60 — add three new columns

**Why this task exists.** Research doc §4.4 requires `logprob_entropy`, `sep_estimate`, `self_consistency_score` columns on `llm_interactions` for the Phase 3 fusion function. Adding the columns now (NULLABLE) keeps migration purely additive.

**Files:**
- Modify: `crates/vox-db/src/research_metrics_contract.rs` (column constants & comments)
- Modify: existing migration runner / schema file (find via search)
- Create: a migration file under wherever migrations live (likely `crates/vox-db/migrations/` or similar)
- Test: `crates/vox-db/tests/migration_v60_test.rs`

- [ ] **Step 11.1: Find the current schema-version assertion.**

```bash
# Find all references to "v59" or schema version constants
grep -rn "v59\|SCHEMA_VERSION\|schema_version" crates/vox-db/src/ | head -30
```

The search should land you in `research_metrics_contract.rs` and any migration runner. Read both before continuing.

- [ ] **Step 11.2: Write the migration test.**

Create `crates/vox-db/tests/migration_v60_test.rs`:

```rust
//! Asserts that the v59→v60 migration adds three confidence-fusion columns
//! to llm_interactions. Per research doc §4.4 and master plan P3.

use vox_db::store::test_helpers::open_test_db; // adjust to real helper

#[test]
fn v60_migration_adds_logprob_entropy_column() {
    let db = open_test_db();
    let cols: Vec<String> = db.query_column_names("llm_interactions");
    assert!(cols.contains(&"logprob_entropy".to_string()),
        "v60 must add logprob_entropy");
}

#[test]
fn v60_migration_adds_sep_estimate_column() {
    let db = open_test_db();
    let cols: Vec<String> = db.query_column_names("llm_interactions");
    assert!(cols.contains(&"sep_estimate".to_string()),
        "v60 must add sep_estimate");
}

#[test]
fn v60_migration_adds_self_consistency_score_column() {
    let db = open_test_db();
    let cols: Vec<String> = db.query_column_names("llm_interactions");
    assert!(cols.contains(&"self_consistency_score".to_string()),
        "v60 must add self_consistency_score");
}

#[test]
fn v60_columns_are_nullable_for_back_compat() {
    let db = open_test_db();
    // Must not require the new columns on insert; existing INSERT statements
    // that don't mention them should still work.
    db.exec("INSERT INTO llm_interactions (session_id) VALUES ('t')")
        .expect("insert without new columns should succeed (NULLABLE)");
}
```

The `open_test_db` and `query_column_names` helpers may need to be adjusted to whatever the real test surface is. If `vox-db` doesn't have a test_helpers module, write the migration test inline against the real `Pool::connect` path.

- [ ] **Step 11.3: Run — verify fail.**

```bash
cargo test -p vox-db --test migration_v60_test
```

Expected: tests fail (columns don't exist yet).

- [ ] **Step 11.4: Write the migration.**

Find the migration directory or runner. Add the v60 migration. If migrations are SQL files, the migration is:

```sql
-- crates/vox-db/migrations/v60_confidence_fusion_columns.sql
ALTER TABLE llm_interactions ADD COLUMN logprob_entropy REAL;
ALTER TABLE llm_interactions ADD COLUMN sep_estimate REAL;
ALTER TABLE llm_interactions ADD COLUMN self_consistency_score REAL;
```

If migrations are Rust functions, write the equivalent. Either way, bump the `SCHEMA_VERSION` constant from 59 to 60.

- [ ] **Step 11.5: Update the contract.**

Edit `crates/vox-db/src/research_metrics_contract.rs`. Add doc-commented constants describing the new columns:

```rust
// --- Phase 1 (program: orchestrator-policy-2026) -------------------------
//
// Confidence-fusion columns on llm_interactions (research doc §4.4).
// All three are NULLABLE; populated by Phase 3 (D3 fusion). Phase 1 only
// adds the columns and bumps the schema version.
//
// OTel SemConv mapping: extends `gen_ai.usage.*` with internal-confidence
// dimensions; not yet standardized upstream.

/// Per-call token-logprob entropy (LogU / LogTokU, research doc §4.1).
/// Range: [0.0, ∞). NULL when the provider does not surface logprobs.
pub const COLUMN_LOGPROB_ENTROPY: &str = "logprob_entropy";

/// Single-generation Semantic Entropy Probe estimate (research doc §4.1).
/// Range: [0.0, 1.0]. NULL when SEP head not loaded.
pub const COLUMN_SEP_ESTIMATE: &str = "sep_estimate";

/// Per-claim self-consistency score (research doc §4.1).
/// Range: [0.0, 1.0]. NULL unless on-demand resampling fired.
pub const COLUMN_SELF_CONSISTENCY_SCORE: &str = "self_consistency_score";
```

Bump the schema version constant (whatever it's named — `SCHEMA_VERSION_V59` → `SCHEMA_VERSION_V60` and emit a `pub const SCHEMA_VERSION: u32 = 60;` if a single-source-of-truth constant exists).

- [ ] **Step 11.6: Run — verify pass.**

```bash
cargo test -p vox-db --test migration_v60_test
cargo test -p vox-db                   # full suite, in case migration broke an existing test
```

Both must pass.

- [ ] **Step 11.7: Commit.**

```bash
git add crates/vox-db
git commit -m "$(cat <<'EOF'
feat(vox-db): bump schema v59 → v60, add confidence-fusion columns

Adds logprob_entropy, sep_estimate, self_consistency_score (REAL,
NULLABLE) to llm_interactions. Populated by Phase 3 (D3 fusion).

Migration is purely additive — existing INSERT statements unaffected.

Refs: docs/superpowers/plans/orchestrator/2026-05-08-orchestrator-phase-1-standards-and-baseline.md task 11
EOF
)"
```

---

## Task 12: Add `metric_type` constants for the new decision points

**Why.** Master plan §5 lists 12 new `metric_type` constants. Adding them in Phase 1 lets later phases just emit; no per-phase constant scaffolding.

**Files:**
- Modify: `crates/vox-db/src/research_metrics_contract.rs`
- Test: `crates/vox-db/tests/metric_type_constants_test.rs`

- [ ] **Step 12.1: Write failing test.**

Create `crates/vox-db/tests/metric_type_constants_test.rs`:

```rust
//! Asserts that all 12 net-new metric_type constants for the orchestrator
//! policy program (P2–P10) are present and non-empty.

use vox_db::research_metrics_contract::*;

#[test]
fn p2_circuit_breaker_constants_exist() {
    assert!(!METRIC_TYPE_CIRCUIT_BREAKER_TRIP.is_empty());
}

#[test]
fn p3_confidence_fusion_constants_exist() {
    assert!(!METRIC_TYPE_SOCRATES_FUSION.is_empty());
}

#[test]
fn p4_tier_routing_constants_exist() {
    assert!(!METRIC_TYPE_MODEL_TIER_ROUTE.is_empty());
}

#[test]
fn p5_plan_mode_constants_exist() {
    assert!(!METRIC_TYPE_PLAN_MODE_DECISION.is_empty());
}

#[test]
fn p6_hitl_constants_exist() {
    assert!(!METRIC_TYPE_HITL_INTERRUPT.is_empty());
    assert!(!METRIC_TYPE_RISK_SCORE.is_empty());
}

#[test]
fn p7_privacy_constants_exist() {
    assert!(!METRIC_TYPE_PRIVACY_ROUTE_DECISION.is_empty());
}

#[test]
fn p8_cache_budget_constants_exist() {
    assert!(!METRIC_TYPE_CACHE_HIT_PREDICTION.is_empty());
    assert!(!METRIC_TYPE_BUDGET_DECISION.is_empty());
}

#[test]
fn p9_calibration_constants_exist() {
    assert!(!METRIC_TYPE_CALIBRATION_RUN.is_empty());
    assert!(!METRIC_TYPE_DRIFT_ALERT.is_empty());
    assert!(!METRIC_TYPE_BANDIT_UPDATE.is_empty());
}

#[test]
fn p10_subagent_constants_exist() {
    assert!(!METRIC_TYPE_SUBAGENT_DISPATCH.is_empty());
    assert!(!METRIC_TYPE_CHAIN_DEPTH_ALERT.is_empty());
}
```

- [ ] **Step 12.2: Run — verify fail.** `cargo test -p vox-db --test metric_type_constants_test`. Expected: compile error per missing constant.

- [ ] **Step 12.3: Add the constants.**

Append to `crates/vox-db/src/research_metrics_contract.rs`. Each constant gets a doc comment naming the phase, decision axis, and OTel SemConv mapping (where it has one).

```rust
// --- Phase 2: Circuit breaker (D6) -------------------------------------
//
// Emitted whenever the doom-loop detector trips. Carries the trip reason
// (NoProgress | StuckOnError | ToolThrash | ActionLoop | Drifting).
// OTel SemConv: gen_ai.agent.error.kind (proposed extension).
pub const METRIC_TYPE_CIRCUIT_BREAKER_TRIP: &str = "orch.circuit_breaker.trip";

// --- Phase 3: Confidence fusion (D3) -----------------------------------
//
// Emitted on every Socrates evaluation. Carries the fused composite score
// and the action selected (Ship / ReSample / Retrieve / SpawnSocrates / Escalate).
// OTel SemConv: gen_ai.agent.confidence (proposed extension).
pub const METRIC_TYPE_SOCRATES_FUSION: &str = "orch.socrates.fusion";

// --- Phase 4: Tier routing (D1) ----------------------------------------
//
// Emitted on every routing decision. Carries the predicted tier, the
// chosen tier (may differ if cascade escalated), and the cascade depth.
// OTel SemConv: gen_ai.request.model + gen_ai.agent.routing.tier (proposed).
pub const METRIC_TYPE_MODEL_TIER_ROUTE: &str = "orch.routing.tier";

// --- Phase 5: Plan-mode trigger (D2) -----------------------------------
//
// Emitted whenever pick_planning_mode() runs. Carries the chosen mode
// (ReAct | PlanThenExecute | ExtendedThinking | ReActWithReflexion).
pub const METRIC_TYPE_PLAN_MODE_DECISION: &str = "orch.plan.mode_decision";

// --- Phase 6: Risk × confidence matrix (D5/D9) -------------------------
//
// HITL_INTERRUPT — fires when the matrix lands at "escalate".
// RISK_SCORE — emitted on every action with non-null compliance / blast.
pub const METRIC_TYPE_HITL_INTERRUPT: &str = "orch.hitl.interrupt";
pub const METRIC_TYPE_RISK_SCORE: &str = "orch.risk.score";

// --- Phase 7: Privacy routing (D8) -------------------------------------
//
// Emitted on every PII-detection-influenced routing decision.
pub const METRIC_TYPE_PRIVACY_ROUTE_DECISION: &str = "orch.privacy.route_decision";

// --- Phase 8: Cache + budget (D7) --------------------------------------
//
// CACHE_HIT_PREDICTION — fired before each call with the prefix-match score.
// BUDGET_DECISION — fired on every per-tenant budget check.
pub const METRIC_TYPE_CACHE_HIT_PREDICTION: &str = "orch.cache.hit_prediction";
pub const METRIC_TYPE_BUDGET_DECISION: &str = "orch.budget.decision";

// --- Phase 9: Calibration (D10) ----------------------------------------
//
// CALIBRATION_RUN — daily background pass; carries ECE per tier.
// DRIFT_ALERT — fires when drift > 2σ from baseline.
// BANDIT_UPDATE — fires on every contextual-bandit weight update.
pub const METRIC_TYPE_CALIBRATION_RUN: &str = "orch.calibration.run";
pub const METRIC_TYPE_DRIFT_ALERT: &str = "orch.calibration.drift_alert";
pub const METRIC_TYPE_BANDIT_UPDATE: &str = "orch.calibration.bandit_update";

// --- Phase 10: Sub-agent dispatch (D4) ---------------------------------
//
// SUBAGENT_DISPATCH — fires per dispatch decision (inline | spawn-one | fan-out).
// CHAIN_DEPTH_ALERT — fires when cumulative chain reliability drops below threshold.
pub const METRIC_TYPE_SUBAGENT_DISPATCH: &str = "orch.subagent.dispatch";
pub const METRIC_TYPE_CHAIN_DEPTH_ALERT: &str = "orch.subagent.chain_depth_alert";
```

- [ ] **Step 12.4: Run — verify pass.** `cargo test -p vox-db --test metric_type_constants_test`. Expected: 13 tests pass.

- [ ] **Step 12.5: Commit.** `git commit -m "feat(vox-db): add 12 metric_type constants for orchestrator policy phases P2-P10"`

---

## Task 13: Contract scaffold — `circuit-breaker.v1`

**Why.** Master plan §3 Gate G4: every contract has a YAML and a JSON Schema, both schema-validated by a loader test. Phase 2 reads from this contract — Phase 1 lays the empty file with a valid schema so Phase 2 only fills bodies.

**Files:**
- Create: `contracts/orchestration/circuit-breaker.v1.yaml`
- Create: `contracts/orchestration/circuit-breaker.v1.schema.json`
- Test: `crates/vox-orchestrator/tests/contract_circuit_breaker_load.rs`

- [ ] **Step 13.1: Write failing loader test.**

Create `crates/vox-orchestrator/tests/contract_circuit_breaker_load.rs`:

```rust
//! Load + schema-validate the circuit-breaker.v1 YAML contract.
//! Phase 1 verifies the scaffold loads; Phase 2 will populate the body.

use std::path::PathBuf;

#[test]
fn circuit_breaker_v1_yaml_exists() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/orchestration/circuit-breaker.v1.yaml");
    assert!(path.exists(), "scaffold yaml at {path:?}");
}

#[test]
fn circuit_breaker_v1_schema_exists() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/orchestration/circuit-breaker.v1.schema.json");
    assert!(path.exists(), "scaffold schema at {path:?}");
}

#[test]
fn circuit_breaker_v1_yaml_validates_against_schema() {
    let yaml_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/orchestration/circuit-breaker.v1.yaml");
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/orchestration/circuit-breaker.v1.schema.json");
    let yaml: serde_yaml::Value = serde_yaml::from_slice(&std::fs::read(yaml_path).unwrap()).unwrap();
    let schema: serde_json::Value = serde_json::from_slice(&std::fs::read(schema_path).unwrap()).unwrap();
    let yaml_as_json = serde_json::to_value(&yaml).unwrap();
    let validator = jsonschema::JSONSchema::compile(&schema)
        .expect("schema compiles");
    let result = validator.validate(&yaml_as_json);
    if let Err(errors) = result {
        for e in errors {
            eprintln!("validation error: {e}");
        }
        panic!("circuit-breaker.v1.yaml fails its schema");
    }
}
```

Add to `crates/vox-orchestrator/Cargo.toml` `[dev-dependencies]` if missing:

```toml
serde_yaml = { workspace = true }
serde_json = { workspace = true }
jsonschema = "0.18"
```

- [ ] **Step 13.2: Run — verify fail.** `cargo test -p vox-orchestrator --test contract_circuit_breaker_load`. Expected: tests fail (files missing).

- [ ] **Step 13.3: Create the scaffold YAML.**

Create `contracts/orchestration/circuit-breaker.v1.yaml`:

```yaml
# Circuit-breaker / doom-loop detector configuration.
#
# Phase 1: scaffold only. Phase 2 (P2 plan) populates the actual thresholds
# from research doc §6.3.
#
# Versioning: this is v1. Breaking changes bump to v2 with a 30-day
# deprecation per the program standards (master plan §10).

version: 1
# Trip thresholds — Phase 2 fills these.
trips:
  no_progress_loops: null
  same_error_loops: null
  tool_calls_no_progress: null
  action_ngram_overlap: null
  semantic_drift_sigma: null
  hard_turn_cap: null

# Graduated warning tiers — Phase 2 fills these.
warnings:
  caution_at_remaining: null
  warning_at_remaining: null

# Action on trip — Phase 2 fills these.
trip_action: null
replanner_max_retries: null
on_replanner_failure: null
```

- [ ] **Step 13.4: Create the JSON Schema.**

Create `contracts/orchestration/circuit-breaker.v1.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://vox.foundation/contracts/orchestration/circuit-breaker.v1.schema.json",
  "title": "circuit-breaker.v1",
  "description": "Doom-loop detector and circuit-breaker configuration. See research doc autonomous-orchestration-policy-research-2026.md §6.3.",
  "type": "object",
  "required": ["version", "trips", "warnings"],
  "properties": {
    "version": { "type": "integer", "const": 1 },
    "trips": {
      "type": "object",
      "properties": {
        "no_progress_loops":     { "type": ["integer", "null"], "minimum": 1 },
        "same_error_loops":      { "type": ["integer", "null"], "minimum": 1 },
        "tool_calls_no_progress":{ "type": ["integer", "null"], "minimum": 1 },
        "action_ngram_overlap":  { "type": ["number",  "null"], "minimum": 0.0, "maximum": 1.0 },
        "semantic_drift_sigma":  { "type": ["number",  "null"], "minimum": 0.0 },
        "hard_turn_cap":         { "type": ["integer", "null"], "minimum": 1 }
      },
      "additionalProperties": false
    },
    "warnings": {
      "type": "object",
      "properties": {
        "caution_at_remaining": { "type": ["integer", "null"], "minimum": 0 },
        "warning_at_remaining": { "type": ["integer", "null"], "minimum": 0 }
      },
      "additionalProperties": false
    },
    "trip_action": {
      "anyOf": [
        { "type": "null" },
        { "type": "string", "enum": ["handoff_to_replanner", "abort", "escalate"] }
      ]
    },
    "replanner_max_retries":   { "type": ["integer", "null"], "minimum": 0 },
    "on_replanner_failure": {
      "anyOf": [
        { "type": "null" },
        { "type": "string", "enum": ["escalate_to_hitl", "abort", "continue_with_partial"] }
      ]
    }
  },
  "additionalProperties": false
}
```

- [ ] **Step 13.5: Run — verify pass.** `cargo test -p vox-orchestrator --test contract_circuit_breaker_load`. Expected: 3 tests pass.

- [ ] **Step 13.6: Commit.** `git commit -m "feat(contracts): scaffold circuit-breaker.v1 (Phase 1; P2 will populate)"`

---

## Task 14: Contract scaffold — `socrates-fusion.v1`

**Files:**
- Create: `contracts/orchestration/socrates-fusion.v1.yaml`
- Create: `contracts/orchestration/socrates-fusion.v1.schema.json`
- Test: `crates/vox-orchestrator/tests/contract_socrates_fusion_load.rs`

- [ ] **Step 14.1: Mirror Task 13's structure.** Loader test asserts existence + schema validation.

- [ ] **Step 14.2: YAML scaffold.**

```yaml
# Socrates fusion + research-action contract. Phase 3 populates.
# See research doc §13.4.

version: 1
signals:
  logprob_entropy:    { weight: null, source: "llm_interactions.logprob_entropy" }
  sep_estimate:       { weight: null, source: "llm_interactions.sep_estimate" }
  self_consistency:   { weight: null, source: "per_claim_resample", fire_when: null }
thresholds:
  ship: null
  resample: null
  retrieve: null
  spawn_socrates: null
  abstain: null
abstention_override:
  if_compliance_tagged: null
  if_user_disabled_socrates: null
```

- [ ] **Step 14.3: Schema.**

Mirror the circuit-breaker schema pattern: `version: const 1`, all numeric fields nullable in v1 scaffold (Phase 3 will fill them and tighten the schema), required keys enumerated.

- [ ] **Step 14.4: Run, commit.** `git commit -m "feat(contracts): scaffold socrates-fusion.v1"`

---

## Task 15: Contract scaffold — `risk-confidence-matrix.v1`

**Files:** parallel to Tasks 13–14. YAML mirrors research doc §13.2.

- [ ] **Step 15.1: Loader test, YAML scaffold, JSON Schema.** Phase 6 populates the matrix table.

- [ ] **Step 15.2: Commit.** `git commit -m "feat(contracts): scaffold risk-confidence-matrix.v1"`

---

## Task 16: Contract scaffold — `tier-routing.v2` (additive over `model-routing.v1`)

**Why.** `contracts/orchestration/model-routing.v1.yaml` and its schema already exist (per pre-plan exploration). Phase 4 needs to add `tier_routing` and `cascade` blocks. Phase 1 lands a `v2` scaffold that is a strict superset of `v1` and validates against an extended schema.

**Files:**
- Create: `contracts/orchestration/model-routing.v2.yaml` (full sample with new optional blocks empty)
- Create: `contracts/orchestration/model-routing.v2.schema.json` (extends v1)
- Test: `crates/vox-orchestrator/tests/contract_model_routing_v2_load.rs`

- [ ] **Step 16.1: Read v1.** Open `contracts/orchestration/model-routing.v1.yaml` and `.schema.json`. Note the existing top-level keys.

- [ ] **Step 16.2: Write loader test that asserts v1 still loads against v1 schema AND v2 loads against v2 schema.**

```rust
// tests assert:
// 1. v1.yaml validates against v1.schema.json (regression)
// 2. v2.yaml validates against v2.schema.json
// 3. v2.yaml ALSO validates against v1.schema.json's "common" subset
//    (proves additivity)
```

- [ ] **Step 16.3: v2 YAML.** Copy v1 verbatim; append commented-out blocks:

```yaml
# === v2 additions (Phase 4 populates) ===
# tier_routing:
#   classifier:
#     type: rule_based
#     rules: []
# cascade:
#   enabled: false
#   start_tier: cheap
#   escalate_threshold: null
#   max_escalations: 1
```

In v1 the additions are commented out so v2 currently equals v1 byte-for-byte modulo comments — no semantic change in P1.

- [ ] **Step 16.4: v2 schema.** Copy v1 schema; add the optional `tier_routing` and `cascade` keys with their sub-schemas. Their bodies are nullable until P4 fills them.

- [ ] **Step 16.5: Run, commit.** `git commit -m "feat(contracts): scaffold model-routing.v2 (additive over v1)"`

---

## Task 17: Contract — `feature-flags.v1`

**Why.** Master plan §6 Rollback policy: every phase ships behind a config gate. The flags live in one contract.

**Files:**
- Create: `contracts/orchestration/feature-flags.v1.yaml`
- Create: `contracts/orchestration/feature-flags.v1.schema.json`
- Test: `crates/vox-orchestrator/tests/contract_feature_flags_load.rs`

- [ ] **Step 17.1: Loader test.** Assert load + schema validation. Assert that all known flags default to `false`.

- [ ] **Step 17.2: YAML.**

```yaml
# Per-phase feature gates. Default-off until P11 flips.
# Master plan §6 (Risk register & rollback).
version: 1
flags:
  # P2
  vox.orchestrator.circuit_breaker.enabled: false
  # P3
  vox.orchestrator.socrates_fusion.enabled: false
  # P4
  vox.orchestrator.tier_cascade.enabled: false
  # P5
  vox.orchestrator.plan_mode_trigger.enabled: false
  # P6
  vox.orchestrator.risk_matrix_hitl.enabled: false
  # P7
  vox.orchestrator.privacy_routing.enabled: false
  # P8
  vox.orchestrator.cache_aware_routing.enabled: false
  vox.orchestrator.compaction_5layer.enabled: false
  vox.orchestrator.tenant_budget.enabled: false
  # P9
  vox.orchestrator.calibration_loop.enabled: false
  vox.orchestrator.drift_detector.enabled: false
  vox.orchestrator.contextual_bandit.enabled: false
  # P10
  vox.orchestrator.subagent_dispatch.enabled: false
  vox.orchestrator.chain_length_cap.enabled: false
```

- [ ] **Step 17.3: Schema.** `version: const 1`; `flags` is an object with `additionalProperties: { type: "boolean" }` and the listed keys as known properties.

- [ ] **Step 17.4: Commit.** `git commit -m "feat(contracts): add feature-flags.v1 (per-phase gates default-off)"`

---

## Task 18: Golden routing tests — pin current behavior

**Why.** Phases 4 and 9 will change routing weights. Without behavior-pinning tests, regressions sneak through. ~30 fixtures cover the existing routing decisions across `TaskCategory × CostPreference`.

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/fixtures/golden_routing/*.json` (30 fixtures)
- Create: `crates/vox-orchestrator/tests/golden_routing_test.rs`

- [ ] **Step 18.1: Write the golden test scaffold.**

```rust
//! Golden behavioral tests for ModelRegistry::best_for_task / best_for.
//! Captures the routing decision for ~30 representative tasks. P4 / P9
//! must update fixtures intentionally — never silently.

use vox_orchestrator::models::{ModelRegistry, TaskCategory};
use vox_orchestrator_test_helpers::{load_golden_fixture, MockModelRegistry};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GoldenCase {
    name: String,
    task_category: String,
    cost_preference: Option<String>,
    expected_model_id: String,
}

#[test]
fn all_golden_routing_cases_pass() {
    let cases: Vec<GoldenCase> = (1..=30)
        .map(|i| {
            let path = format!("golden_routing/case_{i:02}.json");
            load_golden_fixture::<GoldenCase>(&path)
                .unwrap_or_else(|e| panic!("fixture {path}: {e}"))
        })
        .collect();
    let registry = MockModelRegistry::with_default_models().build();
    for case in &cases {
        let cat = parse_task_category(&case.task_category);
        let pick = registry.best_for(cat).unwrap_or_else(||
            panic!("no pick for {} ({})", case.name, case.task_category));
        assert_eq!(pick.id, case.expected_model_id,
            "routing changed for golden case '{}'", case.name);
    }
}

fn parse_task_category(s: &str) -> TaskCategory {
    match s {
        "Code"     => TaskCategory::Code,
        "Chat"     => TaskCategory::Chat,
        "Refactor" => TaskCategory::Refactor,
        // add the others; check the real enum in models/generated.rs
        other => panic!("unknown TaskCategory: {other}"),
    }
}
```

- [ ] **Step 18.2: Generate the 30 fixtures.**

Run a one-off generator (in Rust, in a `#[ignore]` test or a small bin in test-helpers, **not** a `.py`/`.sh` per AGENTS.md) that:
1. Builds `MockModelRegistry::with_default_models()`.
2. Iterates every `TaskCategory` × every `CostPreference`.
3. Calls `best_for()` (or the appropriate variant).
4. Serializes the result as a `case_NN.json` under `fixtures/golden_routing/`.

Approximate generator:

```rust
// crates/vox-orchestrator-test-helpers/src/bin/regen_golden_routing.rs
fn main() {
    use vox_orchestrator_test_helpers::MockModelRegistry;
    use vox_orchestrator::models::TaskCategory;
    let registry = MockModelRegistry::with_default_models().build();
    let categories = TaskCategory::all(); // see models/generated.rs for an iter
    let mut i = 1u32;
    for cat in categories {
        let pick = registry.best_for(cat).expect("registry has model");
        let case = serde_json::json!({
            "name": format!("{cat:?}-default"),
            "task_category": format!("{cat:?}"),
            "cost_preference": null,
            "expected_model_id": pick.id,
        });
        let path = format!(
            "{}/fixtures/golden_routing/case_{i:02}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::create_dir_all(
            std::path::Path::new(&path).parent().unwrap()
        ).unwrap();
        std::fs::write(&path, serde_json::to_vec_pretty(&case).unwrap()).unwrap();
        i += 1;
    }
}
```

If `TaskCategory::all()` doesn't exist on the generated enum, list categories manually. The goal is fixtures, not a comprehensive iterator.

Run: `cargo run --bin regen_golden_routing -p vox-orchestrator-test-helpers`

- [ ] **Step 18.3: Run the test.** `cargo test -p vox-orchestrator --test golden_routing_test`. Expected: passes (just generated the fixtures matching current behavior).

- [ ] **Step 18.4: Document the regen procedure.** Add a comment at the top of the test:

```rust
// To regenerate when behavior changes intentionally:
//   cargo run --bin regen_golden_routing -p vox-orchestrator-test-helpers
// Then review the diff in fixtures/golden_routing/ and commit alongside the
// behavior change. NEVER regenerate without explicit phase-plan authorization.
```

- [ ] **Step 18.5: Commit.**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/golden_routing
git add crates/vox-orchestrator-test-helpers/src/bin
git add crates/vox-orchestrator/tests/golden_routing_test.rs
git commit -m "$(cat <<'EOF'
test(orchestrator): pin current routing behavior with 30 golden fixtures

Establishes the regression boundary for P4 and P9 routing changes.
Includes a regen binary; phase plans must explicitly authorize
fixture regen.

Refs: docs/superpowers/plans/orchestrator/2026-05-08-orchestrator-phase-1-standards-and-baseline.md task 18
EOF
)"
```

---

## Task 19: Tighten `vox-arch-check` — promote orphan detector warn → error

**Why.** Master plan §3 Gate G1 expects each phase to consider tightening arch-check. Phase 1 promotes the orphan detector since its current warn rate should already be low (clean tree). Catching orphan crates at error level prevents new dead code from sneaking in via P2–P10.

**Files:**
- Modify: `docs/src/architecture/layers.toml`
- Possibly modify: any crate currently warning on orphan that should be exempted (e.g., binaries, plugins)

- [ ] **Step 19.1: Run arch-check today and capture orphan warnings.**

```bash
cargo run -p vox-arch-check 2>&1 | tee /tmp/arch.log
grep -i orphan /tmp/arch.log
```

If there are zero orphan warnings, skip to Step 19.4. If there are warnings, list them.

- [ ] **Step 19.2: Audit each orphan warning.**

For each warning:
- If the crate is an intentional binary/plugin (`kind = "binary"` or `kind = "plugin"`): the rule already exempts it; investigate why arch-check is still warning. Likely a `kind` misclassification in `layers.toml`.
- If the crate is genuinely orphan (zero in-tree dependents and not a top-level binary): file a follow-up issue or add `staleness_exempt` only with a comment explaining why.

Resolve all warnings. Either reclassify (kind change) or document the exemption.

- [ ] **Step 19.3: Re-run arch-check.** `cargo run -p vox-arch-check 2>&1 | grep -i orphan` should print nothing.

- [ ] **Step 19.4: Promote the rule to error.**

Edit `docs/src/architecture/layers.toml`. Find the `[guards]` section. Change:

```toml
[guards.orphan]
strict = false   # warn
```

to

```toml
[guards.orphan]
strict = true    # error — promoted in P1 (orchestrator master plan)
```

(The exact key shape depends on the current `layers.toml`. Read it first; the key may be `mode = "warn" | "error"` or similar.)

- [ ] **Step 19.5: Run arch-check — must still exit 0.**

```bash
cargo run -p vox-arch-check
echo "exit code: $?"
```

If non-zero, Step 19.2 missed an orphan. Loop back.

- [ ] **Step 19.6: Add a note to `docs/src/architecture/where-things-live.md`.**

In the "Architectural rules" area, append:

```markdown
- Orphan detector: **strict** (error). Promoted 2026-05-08 in Phase 1 of orchestrator-master-plan.
```

- [ ] **Step 19.7: Commit.**

```bash
git add docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
# any reclassification of crate kinds also goes in this commit
git commit -m "$(cat <<'EOF'
build(arch-check): promote orphan detector warn -> error

P1 master-plan tightening. Catches new dead code at CI time instead of
on review. Existing exempt cases (binaries, plugins) unchanged.

Refs: docs/superpowers/plans/orchestrator/2026-05-08-orchestrator-phase-1-standards-and-baseline.md task 19
EOF
)"
```

---

## Task 20: where-things-live audit

**Why.** Phase 1 has added: 1 crate, 5 benches, 4 contract scaffolds, 1 perf-baseline doc. All need rows. Audit catches anything missed.

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 20.1: Audit the file diff.**

```bash
git log --since="<P1 start>" --name-only --pretty=format: | sort -u
```

For every new file, ensure there's a corresponding row in `where-things-live.md` describing the *concept*, not the file path. The file is *evidence*; the row is *intent*.

Concepts added in P1:
- `vox-orchestrator-test-helpers` crate (Task 1)
- `MockModelRegistry`, `MockBulletinBoard`, `load_golden_fixture` (Tasks 2–4)
- Orchestrator perf benchmark suite (Tasks 5–9)
- Orchestrator perf baseline doc (Task 10)
- v60 schema migration; confidence-fusion telemetry columns (Task 11)
- 12 metric_type constants (Task 12)
- 4 contract scaffolds + feature-flags contract (Tasks 13–17)
- Golden routing fixtures (Task 18)
- Orphan detector strictness change (Task 19)

- [ ] **Step 20.2: Add missing rows.** Each row is one line, alphabetical or grouped by surface.

- [ ] **Step 20.3: Run arch-check + doc-pipeline check.**

```bash
cargo run -p vox-arch-check
cargo run -p vox-doc-pipeline -- --check
```

Both must exit 0.

- [ ] **Step 20.4: Commit.**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): where-things-live rows for P1 additions"
```

---

## Task 21: Run all five quality gates

**Why.** Master plan §3: every phase passes G1–G5 before declaring done. Phase 1 has no new decisions, so G2 and G5 are trivial; the others are real.

- [ ] **Step 21.1: Gate G1 — arch-check.**

```bash
cargo run -p vox-arch-check
echo "G1 exit: $?"
```

Must be 0.

- [ ] **Step 21.2: Gate G2 — telemetry conformance.**

The 12 new metric_type constants exist (Task 12) and are tested. No phase-1 decision points emit metrics yet (P1 is scaffolding). Verify:

```bash
cargo test -p vox-db --test metric_type_constants_test
echo "G2 exit: $?"
```

Must be 0.

- [ ] **Step 21.3: Gate G3 — perf budget.**

P1 captures the baseline; there is no prior budget to assert against. Verify all benches at least *run*:

```bash
cargo bench -p vox-orchestrator
echo "G3 exit: $?"
```

Must be 0.

- [ ] **Step 21.4: Gate G4 — contract conformance.**

```bash
cargo test -p vox-orchestrator --test 'contract_*_load'
echo "G4 exit: $?"
```

Must be 0. All four contract scaffolds + feature-flags load and validate.

- [ ] **Step 21.5: Gate G5 — HITL fallback present.**

P1 introduces no new decisions, so this gate is N/A. **Document N/A** in the phase-end checkpoint (next task) — do not skip silently.

- [ ] **Step 21.6: Run the full test suite as a final sanity.**

```bash
cargo test --workspace
echo "full test exit: $?"
```

Must be 0.

- [ ] **Step 21.7: Commit if anything moved.** Otherwise no commit needed.

---

## Task 22: Regenerate auto-docs and commit the diff

**Why.** Master plan §8.9. Final task of every phase regenerates `SUMMARY.md`, `architecture-index.md`, `feed.xml`. The new architecture doc filed in Task 10 must appear in the regenerated indexes.

**Files:** auto-generated (do not hand-edit; regenerate).

- [ ] **Step 22.1: Run the regenerator.**

```bash
cargo run -p vox-doc-pipeline
```

Expected: writes diffs to `docs/src/SUMMARY.md`, `docs/src/architecture/architecture-index.md`, `docs/src/feed.xml`.

- [ ] **Step 22.2: Verify regen produces no further changes.**

```bash
cargo run -p vox-doc-pipeline -- --check
echo "doc-pipeline check exit: $?"
```

Must be 0. Idempotency is the test.

- [ ] **Step 22.3: Sync ignore files.**

```bash
# Skip if `.voxignore` was not modified in P1.
ls .voxignore  # if no change detected, skip step
# Otherwise:
# vox ci sync-ignore-files
```

- [ ] **Step 22.4: Commit the regen diff.**

```bash
git add docs/src/SUMMARY.md docs/src/architecture/architecture-index.md docs/src/feed.xml
git commit -m "$(cat <<'EOF'
docs: regenerate SUMMARY / architecture-index / feed for P1 additions

Output of cargo run -p vox-doc-pipeline. Picks up
orchestrator-perf-baseline-2026.md filed in P1 task 10.

Auto-generated; do not hand-edit.
EOF
)"
```

- [ ] **Step 22.5: Final phase-end checkpoint.**

Verify the phase exit criteria from master plan §5 row P1:

| Criterion | Status |
|---|---|
| `+test-helpers crate` | done — Tasks 1–4 |
| `+benches/` | done — Tasks 5–9 |
| `+llm_interactions schema v60` | done — Task 11 |
| Scaffolds for 4 contracts | done — Tasks 13–16 (+feature-flags 17) |
| New columns; no new metric_types fired | done — Task 12 (constants exist; not yet emitted, which is correct) |
| Benches green; baseline doc filed | done — Task 10 |
| 5 benches + ~30 golden routing tests | done — Tasks 5–9 + Task 18 |
| HITL surface | N/A (no new decisions) |

If any cell is not "done," loop back to that task before declaring P1 complete.

---

## Phase 1 Sign-Off

When all 22 tasks are checked off and the five gates exit 0, Phase 1 is **complete**.

Next: open the Phase 2 plan (`2026-05-08-orchestrator-phase-2-circuit-breaker.md`) — **not yet written**; the agent executing this should request authorization from the user before proceeding to plan-write Phase 2.

**Self-review pass for the executing agent:**

1. **Spec coverage.** Every cell in master plan §5 row P1 has a corresponding task above? Confirmed.
2. **Placeholder scan.** Search this file for `TBD`, `TODO`, `unimplemented!`. The `unimplemented!()` occurrences in Tasks 2.4 and 8.3 / 9.2 are intentional — they direct the agent to read existing source. Annotate them as such if any reviewer flags.
3. **Type consistency.** `MockModelRegistry::with_default_models()` is referenced consistently. `MockBulletinBoard::new()` matches. `load_golden_fixture` matches. ✓.
4. **HITL gate (G5) is N/A documented.** ✓ (Task 21.5).
5. **No new `.ps1`/`.sh`/`.py` introduced.** ✓ (Task 18.2's regen binary is Rust).
6. **Auto-docs regenerated as final step.** ✓ (Task 22).

---

*End of Phase 1 plan.*
