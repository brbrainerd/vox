# Vox Console Discovery Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Sandbox note:** In the Claude worktree sandbox, dispatched subagents are read-only (shell/write DENIED). Execute implementation tasks in the main session, not via parallel implementer subagents. Reviews (read-only) may still be delegated.

**Goal:** Add a "Vox Console" surface to the `vox-gui` Tauri app — a Warp-model discovery terminal with fish-style as-you-type command discovery, a real PTY for full terminal control with tabs, a persistent help/tips rail driven by a local spaced-repetition memory of what the user has seen, and first-class agent visibility wired to the existing orchestrator daemon.

**Architecture:** New `discovery` module in `vox-gamify` owns a per-user exposure ledger + FSRS-style scheduler (Codex/turso table, registered through `vox-db` `BASELINE_VERSION`). New Rust modules in `vox-gui/src/commands` add a `portable-pty` manager (one PTY per tab) and suggest/help Tauri commands that rank candidates from the existing command catalog + shell history. A new React surface (`components/surfaces/Console/`) owns its input line (enabling ghost text), renders PTY output via `xterm.js` with OSC 133 block markers, shows the discovery rail, and reuses the existing `vox://orch-status` / `vox://agent-events` streams + A2A path for the agent strip/tabs/inbox. The surface registers in `surface-registry.v1.yaml` as `live_backend`.

**Tech Stack:** Rust (Tauri 2, tokio, turso/Codex, `portable-pty`), TypeScript/React 19 (Vite, `@xterm/xterm`, `@testing-library/react`, vitest).

---

## File Structure

**Backend — discovery ledger + scheduler (`vox-gamify`):**
- Create: `crates/vox-gamify/src/discovery/mod.rs` — module root, re-exports.
- Create: `crates/vox-gamify/src/discovery/ledger.rs` — exposure ledger upserts/queries (`discovery_state` table).
- Create: `crates/vox-gamify/src/discovery/fsrs.rs` — FSRS-style memory-state update (pure functions).
- Create: `crates/vox-gamify/src/discovery/rank.rs` — frecency + novelty + due-ness ranking (pure functions).
- Modify: `crates/vox-gamify/src/lib.rs` — `pub mod discovery;`.
- Create: `crates/vox-db/src/schema/domains/sql/discovery.sql` — `discovery_state` DDL.
- Create: `crates/vox-db/src/schema/domains/discovery.rs` — `include_str!` wrapper.
- Modify: `crates/vox-db/src/schema/manifest.rs` — register fragment + bump `BASELINE_VERSION`.
- Modify: `crates/vox-db/src/schema/domains/mod.rs` — `pub mod discovery;`.

**Backend — PTY manager + Tauri commands (`vox-gui`):**
- Create: `crates/vox-gui/src/commands/pty.rs` — PTY session manager + Tauri commands.
- Create: `crates/vox-gui/src/commands/discovery.rs` — suggest/help/record Tauri commands.
- Modify: `crates/vox-gui/src/main.rs` — register commands in `generate_handler!`, manage PTY state.
- Modify: `crates/vox-gui/Cargo.toml` — add `portable-pty`.

**Frontend — Console surface (`vox-gui/ui`):**
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/Console.tsx` — surface root (layout A: terminal + rail + agent strip).
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/InputEditor.tsx` — owned prompt with ghost text.
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/TerminalTab.tsx` — xterm.js view + OSC 133 block parsing.
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/DiscoveryRail.tsx` — help/tips pane.
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/AgentStrip.tsx` — live agent chips.
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/suggest.ts` — client-side candidate matching over the catalog.
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/*.test.tsx` — vitest specs (per component).
- Modify: `crates/vox-gui/ui/src/App.tsx` — `View` union + import + `renderView` case `'console'`.
- Modify: `crates/vox-gui/ui/src/transport.ts` — PTY + discovery invoke/listen wrappers.
- Modify: `crates/vox-gui/ui/package.json` — add `@xterm/xterm` + `@xterm/addon-fit`.
- Modify: `contracts/gui/surface-registry.v1.yaml` — add `console` surface (tier `live_backend`).
- Modify: `docs/src/architecture/where-things-live.md` — add rows for the new modules.

---

## Phase 1 — Discovery ledger schema (vox-db)

### Task 1: Add the `discovery_state` table to the Arca baseline

**Files:**
- Create: `crates/vox-db/src/schema/domains/sql/discovery.sql`
- Create: `crates/vox-db/src/schema/domains/discovery.rs`
- Modify: `crates/vox-db/src/schema/domains/mod.rs`
- Modify: `crates/vox-db/src/schema/manifest.rs`
- Test: `crates/vox-db/tests/db_connection_tests.rs`

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-db/tests/db_connection_tests.rs`:

```rust
#[tokio::test]
async fn test_discovery_state_table_exists() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.unwrap();
    // Inserting a row proves the table + columns exist after baseline init.
    db.connection()
        .execute(
            "INSERT INTO discovery_state \
             (user_id, action_id, seen_count, used_count, last_seen_ms, last_used_ms, \
              dwell_ms_total, fsrs_stability, fsrs_difficulty, fsrs_due_ms) \
             VALUES ('u1','vox.scientia.review',1,0,10,0,0,0.0,0.0,0)",
            turso::params![],
        )
        .await
        .unwrap();
    let mut rows = db
        .connection()
        .query(
            "SELECT seen_count FROM discovery_state WHERE user_id='u1' AND action_id='vox.scientia.review'",
            turso::params![],
        )
        .await
        .unwrap();
    let n = rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap();
    assert_eq!(n, 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-db test_discovery_state_table_exists -- --nocapture`
Expected: FAIL — `no such table: discovery_state`.

- [ ] **Step 3: Create the SQL DDL**

Create `crates/vox-db/src/schema/domains/sql/discovery.sql`:

```sql
-- Per-user command/concept exposure ledger for the Vox Console discovery engine.
-- One row per (user, action-manifest id). Created lazily on first sight; rows for
-- action ids absent from the current manifest are simply never resurfaced.
CREATE TABLE IF NOT EXISTS discovery_state (
    user_id         TEXT    NOT NULL,
    action_id       TEXT    NOT NULL,
    seen_count      INTEGER NOT NULL DEFAULT 0,
    used_count      INTEGER NOT NULL DEFAULT 0,
    last_seen_ms    INTEGER NOT NULL DEFAULT 0,
    last_used_ms    INTEGER NOT NULL DEFAULT 0,
    dwell_ms_total  INTEGER NOT NULL DEFAULT 0,
    fsrs_stability  REAL    NOT NULL DEFAULT 0.0,
    fsrs_difficulty REAL    NOT NULL DEFAULT 0.0,
    fsrs_due_ms     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, action_id)
);

CREATE INDEX IF NOT EXISTS idx_discovery_state_due
    ON discovery_state(user_id, fsrs_due_ms);
```

- [ ] **Step 4: Create the domain module**

Create `crates/vox-db/src/schema/domains/discovery.rs`:

```rust
//! Arca SQL: Vox Console discovery exposure ledger (design §discovery ledger).
pub const SCHEMA_DISCOVERY: &str = include_str!("sql/discovery.sql");
```

- [ ] **Step 5: Register the module**

In `crates/vox-db/src/schema/domains/mod.rs`, add alongside the other `pub mod` lines:

```rust
pub mod discovery;
```

- [ ] **Step 6: Register the fragment and bump the baseline**

In `crates/vox-db/src/schema/manifest.rs`, add a `SchemaFragment` to the `SCHEMA_FRAGMENTS` array (after the gamification fragment):

```rust
SchemaFragment {
    name: "discovery",
    sql: domains::discovery::SCHEMA_DISCOVERY,
},
```

Then bump `BASELINE_VERSION` by 1, updating the trailing comment (replace `N` with the current value + 1):

```rust
pub const BASELINE_VERSION: i64 = N; // +1: discovery_state (Vox Console exposure ledger)
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test -p vox-db test_discovery_state_table_exists -- --nocapture`
Expected: PASS.

- [ ] **Step 8: Run the baseline-version smoke test**

Run: `cargo test -p vox-db test_db_memory_smoke -- --nocapture`
Expected: PASS (asserts `schema_version() == BASELINE_VERSION`).

- [ ] **Step 9: Commit**

```bash
git add crates/vox-db/src/schema/domains/sql/discovery.sql crates/vox-db/src/schema/domains/discovery.rs crates/vox-db/src/schema/domains/mod.rs crates/vox-db/src/schema/manifest.rs crates/vox-db/tests/db_connection_tests.rs
git commit -m "feat(vox-db): add discovery_state table for console exposure ledger"
```

---

## Phase 2 — FSRS scheduler + ranking (pure functions)

### Task 2: FSRS-style memory update

**Files:**
- Create: `crates/vox-gamify/src/discovery/mod.rs`
- Create: `crates/vox-gamify/src/discovery/fsrs.rs`
- Modify: `crates/vox-gamify/src/lib.rs`
- Test: in-module `#[cfg(test)]` in `fsrs.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gamify/src/discovery/fsrs.rs` with only the types + test (no impl yet):

```rust
//! Minimal FSRS-style spaced-repetition state update. Deterministic, no LLM.
//! `stability` is roughly "days until ~90% recall"; `difficulty` in [1,10].

/// A discovery item's memory state. `due_ms` is an absolute epoch-ms timestamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryState {
    pub stability: f64,
    pub difficulty: f64,
    pub due_ms: i64,
}

/// Outcome of an exposure: did the user actually *use* the surfaced command?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recall {
    /// Saw it in the rail/tips but did not invoke it.
    Seen,
    /// Invoked the command (strong signal).
    Used,
}

const DAY_MS: i64 = 86_400_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_use_sets_positive_stability_and_future_due() {
        let next = update(None, Recall::Used, 1_000);
        assert!(next.stability >= 1.0, "stability {}", next.stability);
        assert!(next.due_ms > 1_000);
    }

    #[test]
    fn seen_without_use_keeps_due_soon() {
        let used = update(None, Recall::Used, 0);
        let seen_again = update(Some(used), Recall::Seen, used.due_ms);
        // A "seen but not used" review must not push the item far out; it should
        // remain due sooner than a successful "used" review would have.
        let used_again = update(Some(used), Recall::Used, used.due_ms);
        assert!(seen_again.due_ms < used_again.due_ms);
    }

    #[test]
    fn repeated_use_grows_stability_monotonically() {
        let a = update(None, Recall::Used, 0);
        let b = update(Some(a), Recall::Used, a.due_ms);
        assert!(b.stability > a.stability);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-gamify discovery::fsrs -- --nocapture`
Expected: FAIL — `cannot find function update`.

- [ ] **Step 3: Implement `update`**

Add to `crates/vox-gamify/src/discovery/fsrs.rs` (above the `#[cfg(test)]` block):

```rust
/// Update memory state given a review outcome at `now_ms`.
///
/// `prev == None` is the first-ever exposure. Deterministic: same inputs → same
/// output, so it is trivially testable and replay-safe.
pub fn update(prev: Option<MemoryState>, recall: Recall, now_ms: i64) -> MemoryState {
    match prev {
        None => {
            // Initial state. "Used" earns more initial stability than "Seen".
            let (stability, difficulty) = match recall {
                Recall::Used => (3.0, 5.0),
                Recall::Seen => (1.0, 6.0),
            };
            MemoryState {
                stability,
                difficulty,
                due_ms: now_ms + (stability * DAY_MS as f64) as i64,
            }
        }
        Some(p) => {
            // Difficulty drifts down on use, up on a passive "seen".
            let difficulty = match recall {
                Recall::Used => (p.difficulty - 0.5).clamp(1.0, 10.0),
                Recall::Seen => (p.difficulty + 0.3).clamp(1.0, 10.0),
            };
            // Stability grows on use (easier items grow faster); a passive "seen"
            // grows it only slightly so the item resurfaces again soon.
            let growth = match recall {
                Recall::Used => 1.0 + (11.0 - difficulty) / 10.0,
                Recall::Seen => 1.05,
            };
            let stability = (p.stability * growth).max(p.stability + 0.1);
            MemoryState {
                stability,
                difficulty,
                due_ms: now_ms + (stability * DAY_MS as f64) as i64,
            }
        }
    }
}
```

- [ ] **Step 4: Create the module root and register it**

Create `crates/vox-gamify/src/discovery/mod.rs`:

```rust
//! Vox Console discovery engine: per-user exposure ledger, FSRS-style scheduler,
//! and suggestion ranking. Local and deterministic — no LLM.

pub mod fsrs;
pub mod rank;
pub mod ledger;

pub use fsrs::{update as fsrs_update, MemoryState, Recall};
```

In `crates/vox-gamify/src/lib.rs`, add near the other `pub mod` declarations:

```rust
pub mod discovery;
```

> Note: `rank` and `ledger` are created in Tasks 3–4; until then, comment those two `pub mod` lines or create empty stub files. Prefer creating the files in this task as empty modules (`//! placeholder`) to keep the crate compiling, then fill them in the next tasks.

- [ ] **Step 5: Create empty sibling modules so the crate compiles**

Create `crates/vox-gamify/src/discovery/rank.rs` and `crates/vox-gamify/src/discovery/ledger.rs`, each containing only:

```rust
//! (implemented in a later task)
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p vox-gamify discovery::fsrs -- --nocapture`
Expected: PASS (3 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gamify/src/discovery/ crates/vox-gamify/src/lib.rs
git commit -m "feat(vox-gamify): FSRS-style discovery scheduler"
```

### Task 3: Suggestion ranking

**Files:**
- Modify: `crates/vox-gamify/src/discovery/rank.rs`
- Test: in-module `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

Replace the contents of `crates/vox-gamify/src/discovery/rank.rs` with:

```rust
//! Suggestion ranking: frecency (usage) + novelty (never-seen boost) + due-ness.
//! Pure scoring so it is deterministic and unit-testable.

/// The ranking inputs for one candidate command.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub action_id: String,
    pub used_count: u32,
    pub last_used_ms: i64,
    pub seen_count: u32,
    /// FSRS due timestamp; 0 when never tracked.
    pub due_ms: i64,
    /// True when the typed prefix matches this command's path/alias.
    pub prefix_match: bool,
}

/// Score a candidate at `now_ms`. Higher = surface sooner. Prefix matches always
/// outrank non-matches; among matches, blend recent usage, novelty, and due-ness.
pub fn score(c: &Candidate, now_ms: i64) -> f64 {
    let prefix = if c.prefix_match { 1000.0 } else { 0.0 };
    // Frecency: log of usage, decayed by days since last use.
    let days_since = ((now_ms - c.last_used_ms).max(0) as f64) / 86_400_000.0;
    let frecency = (c.used_count as f64 + 1.0).ln() / (1.0 + days_since);
    // Novelty: never-seen commands get a fixed boost that fades as seen_count rises.
    let novelty = if c.seen_count == 0 { 5.0 } else { 1.0 / c.seen_count as f64 };
    // Due-ness: items past their FSRS due time are worth resurfacing.
    let due = if c.due_ms != 0 && c.due_ms <= now_ms { 3.0 } else { 0.0 };
    prefix + frecency + novelty + due
}

/// Rank candidates best-first. Stable on ties by `action_id` for determinism.
pub fn rank(mut candidates: Vec<Candidate>, now_ms: i64) -> Vec<Candidate> {
    candidates.sort_by(|a, b| {
        score(b, now_ms)
            .partial_cmp(&score(a, now_ms))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.action_id.cmp(&b.action_id))
    });
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, used: u32, seen: u32, prefix: bool) -> Candidate {
        Candidate {
            action_id: id.into(),
            used_count: used,
            last_used_ms: 0,
            seen_count: seen,
            due_ms: 0,
            prefix_match: prefix,
        }
    }

    #[test]
    fn prefix_matches_outrank_everything() {
        let out = rank(vec![cand("b", 99, 99, false), cand("a", 0, 0, true)], 1);
        assert_eq!(out[0].action_id, "a");
    }

    #[test]
    fn never_seen_beats_seen_among_equal_usage() {
        let out = rank(vec![cand("seen", 0, 10, true), cand("fresh", 0, 0, true)], 1);
        assert_eq!(out[0].action_id, "fresh");
    }

    #[test]
    fn due_items_get_resurfaced() {
        let mut due = cand("due", 0, 5, false);
        due.due_ms = 1; // due in the past relative to now=1000
        let not_due = cand("notdue", 0, 5, false);
        let out = rank(vec![not_due, due], 1000);
        assert_eq!(out[0].action_id, "due");
    }
}
```

- [ ] **Step 2: Run test to verify it fails, then passes**

Run: `cargo test -p vox-gamify discovery::rank -- --nocapture`
Expected: initially the file was a placeholder so the test module didn't exist; after pasting, run again → PASS (3 tests). (If you paste impl + tests together, this is a single PASS run; confirm all three pass.)

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gamify/src/discovery/rank.rs
git commit -m "feat(vox-gamify): discovery suggestion ranking (frecency+novelty+due)"
```

---

## Phase 3 — Exposure ledger (DB-backed)

### Task 4: Ledger upsert + query against `discovery_state`

**Files:**
- Modify: `crates/vox-gamify/src/discovery/ledger.rs`
- Test: `crates/vox-gamify/tests/discovery_ledger_tests.rs` (create)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gamify/tests/discovery_ledger_tests.rs`:

```rust
use vox_gamify::discovery::ledger;
use vox_gamify::discovery::Recall;

#[tokio::test]
async fn record_seen_then_used_accumulates() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.unwrap();
    ledger::record(&db, "u1", "vox.scientia.review", Recall::Seen, 2_000, 1_000)
        .await
        .unwrap();
    ledger::record(&db, "u1", "vox.scientia.review", Recall::Used, 5_000, 0)
        .await
        .unwrap();
    let row = ledger::get(&db, "u1", "vox.scientia.review")
        .await
        .unwrap()
        .expect("row exists");
    assert_eq!(row.seen_count, 1);
    assert_eq!(row.used_count, 1);
    assert_eq!(row.dwell_ms_total, 1_000);
    assert!(row.fsrs_due_ms > 5_000);
}

#[tokio::test]
async fn due_query_returns_overdue_items() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.unwrap();
    ledger::record(&db, "u1", "vox.populi.status", Recall::Seen, 1, 0)
        .await
        .unwrap();
    // Far-future "now" makes the seeded item overdue.
    let due = ledger::due_action_ids(&db, "u1", i64::MAX, 10).await.unwrap();
    assert!(due.contains(&"vox.populi.status".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-gamify --test discovery_ledger_tests -- --nocapture`
Expected: FAIL — `record`, `get`, `due_action_ids`, `DiscoveryRow` not found.

- [ ] **Step 3: Implement the ledger**

Replace `crates/vox-gamify/src/discovery/ledger.rs` with:

```rust
//! DB-backed exposure ledger over the `discovery_state` table. Mirrors the
//! `crates/vox-gamify/src/db/counters.rs` connection+breaker pattern.

use anyhow::Result;
use turso::params;
use vox_db::Codex;

use super::fsrs::{self, MemoryState, Recall};

/// One materialized ledger row.
#[derive(Debug, Clone)]
pub struct DiscoveryRow {
    pub seen_count: u32,
    pub used_count: u32,
    pub last_seen_ms: i64,
    pub last_used_ms: i64,
    pub dwell_ms_total: i64,
    pub fsrs_stability: f64,
    pub fsrs_difficulty: f64,
    pub fsrs_due_ms: i64,
}

/// Fetch the current row for (user, action), if any.
pub async fn get(db: &Codex, user_id: &str, action_id: &str) -> Result<Option<DiscoveryRow>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT seen_count, used_count, last_seen_ms, last_used_ms, dwell_ms_total, \
             fsrs_stability, fsrs_difficulty, fsrs_due_ms \
             FROM discovery_state WHERE user_id=?1 AND action_id=?2",
            params![user_id, action_id],
        )
        .await?;
    match rows.next().await? {
        None => Ok(None),
        Some(r) => Ok(Some(DiscoveryRow {
            seen_count: r.get::<i64>(0).unwrap_or(0).max(0) as u32,
            used_count: r.get::<i64>(1).unwrap_or(0).max(0) as u32,
            last_seen_ms: r.get::<i64>(2).unwrap_or(0),
            last_used_ms: r.get::<i64>(3).unwrap_or(0),
            dwell_ms_total: r.get::<i64>(4).unwrap_or(0),
            fsrs_stability: r.get::<f64>(5).unwrap_or(0.0),
            fsrs_difficulty: r.get::<f64>(6).unwrap_or(0.0),
            fsrs_due_ms: r.get::<i64>(7).unwrap_or(0),
        })),
    }
}

/// Record an exposure. `recall` distinguishes seen-vs-used; `dwell_ms` adds to the
/// running dwell total (pass 0 for `Used`). Updates the FSRS memory state.
pub async fn record(
    db: &Codex,
    user_id: &str,
    action_id: &str,
    recall: Recall,
    now_ms: i64,
    dwell_ms: i64,
) -> Result<()> {
    let prev = get(db, user_id, action_id).await?.map(|r| MemoryState {
        stability: r.fsrs_stability,
        difficulty: r.fsrs_difficulty,
        due_ms: r.fsrs_due_ms,
    });
    let next = fsrs::update(prev, recall, now_ms);
    let (seen_inc, used_inc) = match recall {
        Recall::Seen => (1_i64, 0_i64),
        Recall::Used => (0_i64, 1_i64),
    };
    let (last_seen, last_used) = match recall {
        Recall::Seen => (now_ms, 0),
        Recall::Used => (0, now_ms),
    };
    let (uid, aid) = (user_id.to_string(), action_id.to_string());
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO discovery_state \
                 (user_id, action_id, seen_count, used_count, last_seen_ms, last_used_ms, \
                  dwell_ms_total, fsrs_stability, fsrs_difficulty, fsrs_due_ms) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) \
                 ON CONFLICT(user_id, action_id) DO UPDATE SET \
                   seen_count=seen_count+?3, \
                   used_count=used_count+?4, \
                   last_seen_ms=MAX(last_seen_ms,?5), \
                   last_used_ms=MAX(last_used_ms,?6), \
                   dwell_ms_total=dwell_ms_total+?7, \
                   fsrs_stability=?8, fsrs_difficulty=?9, fsrs_due_ms=?10",
                params![
                    uid.as_str(),
                    aid.as_str(),
                    seen_inc,
                    used_inc,
                    last_seen,
                    last_used,
                    dwell_ms,
                    next.stability,
                    next.difficulty,
                    next.due_ms
                ],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Action ids whose FSRS due time is at or before `now_ms`, soonest-first, capped.
pub async fn due_action_ids(
    db: &Codex,
    user_id: &str,
    now_ms: i64,
    limit: u32,
) -> Result<Vec<String>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT action_id FROM discovery_state \
             WHERE user_id=?1 AND fsrs_due_ms<=?2 AND fsrs_due_ms>0 \
             ORDER BY fsrs_due_ms ASC LIMIT ?3",
            params![user_id, now_ms, limit as i64],
        )
        .await?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        if let Ok(id) = r.get::<String>(0) {
            out.push(id);
        }
    }
    Ok(out)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-gamify --test discovery_ledger_tests -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gamify/src/discovery/ledger.rs crates/vox-gamify/tests/discovery_ledger_tests.rs
git commit -m "feat(vox-gamify): DB-backed discovery exposure ledger"
```

---

## Phase 4 — Discovery Tauri commands (vox-gui backend)

### Task 5: suggest / help / record commands

**Files:**
- Create: `crates/vox-gui/src/commands/discovery.rs`
- Modify: `crates/vox-gui/src/main.rs`
- Test: in-module `#[cfg(test)]` in `discovery.rs` (logic-only; Tauri commands themselves are thin)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/src/commands/discovery.rs`:

```rust
//! Tauri commands powering the Console discovery engine: candidate suggestion,
//! per-command help lookup, and exposure recording. Ranking/ledger logic lives in
//! `vox_gamify::discovery`; these commands adapt the command catalog to it.

use serde::Serialize;
use vox_cli::command_catalog::{build_catalog, CommandCatalogEntry};

/// A single suggestion returned to the UI.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suggestion {
    /// Canonical action id, e.g. "vox.scientia.review".
    pub action_id: String,
    /// Full command line fragment to complete to, e.g. "scientia review".
    pub completion: String,
    pub about: String,
}

/// Build the canonical action id for a catalog entry: "vox" + dotted path.
pub fn action_id_for(entry: &CommandCatalogEntry) -> String {
    let mut parts = vec!["vox".to_string()];
    parts.extend(entry.path.iter().cloned());
    parts.join(".")
}

/// Filter the catalog to entries whose dotted path starts with the typed words.
/// `typed` is the text after the leading "vox " (may be empty). Runnable leaves
/// only (entries with no subcommands), capped at `limit`.
pub fn match_catalog(
    entries: &[CommandCatalogEntry],
    typed: &str,
    limit: usize,
) -> Vec<Suggestion> {
    let needle: Vec<&str> = typed.split_whitespace().collect();
    let mut out = Vec::new();
    for e in entries {
        if e.has_subcommands {
            continue;
        }
        let path_str = e.path.join(" ");
        let matches = needle.is_empty()
            || path_str.starts_with(&needle.join(" "))
            || e.aliases.iter().any(|a| a.starts_with(typed));
        if matches {
            out.push(Suggestion {
                action_id: action_id_for(e),
                completion: path_str,
                about: e.about.clone(),
            });
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

#[tauri::command]
pub fn discovery_suggest(typed: String, limit: Option<usize>) -> Result<Vec<Suggestion>, String> {
    let catalog = build_catalog();
    let typed = typed.strip_prefix("vox ").unwrap_or(&typed).trim().to_string();
    Ok(match_catalog(&catalog.entries, &typed, limit.unwrap_or(8)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_lists_runnable_leaves() {
        let catalog = build_catalog();
        let s = match_catalog(&catalog.entries, "", 5);
        assert!(!s.is_empty());
        assert!(s.len() <= 5);
    }

    #[test]
    fn prefix_filters_to_matching_paths() {
        let catalog = build_catalog();
        // "config" is a stable top-level group across the CLI.
        let s = match_catalog(&catalog.entries, "config", 20);
        assert!(
            s.iter().all(|x| x.completion.starts_with("config") || x.action_id.contains("config")),
            "got: {:?}",
            s.iter().map(|x| &x.completion).collect::<Vec<_>>()
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-gui discovery:: -- --nocapture`
Expected: FAIL — module not yet declared in the crate (compile error: unresolved module).

- [ ] **Step 3: Declare the module**

In `crates/vox-gui/src/commands/mod.rs` (or wherever `commands` submodules are declared — match the existing pattern, e.g. next to `pub mod catalog;`), add:

```rust
pub mod discovery;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-gui discovery:: -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 5: Add the help + record commands**

Append to `crates/vox-gui/src/commands/discovery.rs`:

```rust
/// Rich help for one action id, for the discovery rail.
#[derive(Debug, Clone, Serialize)]
pub struct ActionHelp {
    pub action_id: String,
    pub about: String,
    pub args: Vec<ArgHelp>,
    pub example: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArgHelp {
    pub name: String,
    pub help: String,
    pub required: bool,
}

#[tauri::command]
pub fn discovery_help(action_id: String) -> Result<Option<ActionHelp>, String> {
    let catalog = build_catalog();
    let entry = catalog
        .entries
        .iter()
        .find(|e| action_id_for(e) == action_id);
    Ok(entry.map(|e| {
        let args = e
            .arguments
            .iter()
            .map(|a| ArgHelp {
                name: a.long.clone().unwrap_or_else(|| a.name.clone()),
                help: a.help.clone().unwrap_or_default(),
                required: a.required,
            })
            .collect();
        ActionHelp {
            action_id: action_id.clone(),
            about: e.about.clone(),
            args,
            example: format!("vox {}", e.path.join(" ")),
        }
    }))
}

/// Record an exposure (seen/used) for the current user. `used=true` ⇒ Recall::Used.
#[tauri::command]
pub async fn discovery_record(
    action_id: String,
    used: bool,
    now_ms: i64,
    dwell_ms: i64,
) -> Result<(), String> {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::default_local())
        .await
        .map_err(|e| e.to_string())?;
    let recall = if used {
        vox_gamify::discovery::Recall::Used
    } else {
        vox_gamify::discovery::Recall::Seen
    };
    vox_gamify::discovery::ledger::record(&db, current_user_id(), &action_id, recall, now_ms, dwell_ms)
        .await
        .map_err(|e| e.to_string())
}

/// Single-user desktop build: the local profile id. Matches how other gamify
/// call sites resolve the user (see existing usage of the gamify profile).
fn current_user_id() -> &'static str {
    "local"
}
```

> Implementer note: `vox_db::DbConfig::default_local()` and the `"local"` user id are placeholders for the project's real local-DB accessor and current-user resolution. Before writing this step, grep for how `vox-gui` already opens the Codex (e.g. in `commands/scientia.rs` or `commands/memory.rs`) and reuse that exact accessor + user-id source. Replace both here to match. Do NOT invent a new DB-open path.

- [ ] **Step 6: Register the three commands**

In `crates/vox-gui/src/main.rs`, add to the `tauri::generate_handler![...]` list (alongside `commands::catalog::get_command_catalog`):

```rust
commands::discovery::discovery_suggest,
commands::discovery::discovery_help,
commands::discovery::discovery_record,
```

- [ ] **Step 7: Run the full crate test + build**

Run: `cargo test -p vox-gui discovery:: -- --nocapture` then `cargo build -p vox-gui`
Expected: tests PASS; build succeeds.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/src/commands/discovery.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): discovery suggest/help/record Tauri commands"
```

---

## Phase 5 — PTY manager (vox-gui backend)

### Task 6: portable-pty session manager + Tauri commands

**Files:**
- Modify: `crates/vox-gui/Cargo.toml`
- Create: `crates/vox-gui/src/commands/pty.rs`
- Modify: `crates/vox-gui/src/main.rs`
- Test: in-module `#[cfg(test)]` in `pty.rs`

- [ ] **Step 1: Add the dependency**

In `crates/vox-gui/Cargo.toml` `[dependencies]`, add:

```toml
portable-pty = "0.8"
```

Run: `cargo build -p vox-gui` to fetch/compile the dep.
Expected: builds (no code using it yet).

- [ ] **Step 2: Write the failing test**

Create `crates/vox-gui/src/commands/pty.rs`:

```rust
//! Per-tab PTY sessions for the Vox Console. Each tab owns one PTY running the
//! user's shell; bytes stream to the UI as Tauri events, input is written back.
//! Windows spawns use ConPTY via portable-pty (no flashing console windows).

use std::collections::HashMap;
use std::sync::Mutex;

/// The default shell command per platform. Configurable later via settings.
pub fn default_shell() -> String {
    if cfg!(windows) {
        "pwsh".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string())
    }
}

/// Registry of live PTY sessions keyed by tab id. Managed by Tauri state.
#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<String, PtySession>>,
}

struct PtySession {
    writer: Box<dyn std::io::Write + Send>,
}

impl PtyManager {
    pub fn has(&self, tab_id: &str) -> bool {
        self.sessions.lock().unwrap().contains_key(tab_id)
    }

    pub fn count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shell_is_nonempty() {
        assert!(!default_shell().is_empty());
    }

    #[test]
    fn manager_starts_empty() {
        let m = PtyManager::default();
        assert_eq!(m.count(), 0);
        assert!(!m.has("tab-1"));
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-gui pty:: -- --nocapture`
Expected: FAIL — module not declared (unresolved module `pty`).

- [ ] **Step 4: Declare the module and run the test**

In `crates/vox-gui/src/commands/mod.rs` add `pub mod pty;`.
Run: `cargo test -p vox-gui pty:: -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 5: Implement spawn / write / resize / close**

Append to `crates/vox-gui/src/commands/pty.rs`:

```rust
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tauri::Emitter;

/// Tauri event carrying a chunk of PTY output. Payload: { tab_id, data (utf8 lossy) }.
pub const PTY_OUTPUT_EVENT: &str = "vox://pty-output";
/// Tauri event signalling a PTY exited. Payload: { tab_id }.
pub const PTY_EXIT_EVENT: &str = "vox://pty-exit";

#[derive(serde::Serialize, Clone)]
struct PtyChunk {
    tab_id: String,
    data: String,
}

#[derive(serde::Serialize, Clone)]
struct PtyExit {
    tab_id: String,
}

#[tauri::command]
pub fn pty_spawn(
    app: tauri::AppHandle,
    manager: tauri::State<'_, PtyManager>,
    tab_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let cmd = CommandBuilder::new(default_shell());
    let _child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    manager
        .sessions
        .lock()
        .unwrap()
        .insert(tab_id.clone(), PtySession { writer });

    // Stream output on a blocking thread (portable-pty reader is sync).
    let app_handle = app.clone();
    let id = tab_id.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let chunk = PtyChunk {
                        tab_id: id.clone(),
                        data: String::from_utf8_lossy(&buf[..n]).to_string(),
                    };
                    let _ = app_handle.emit(PTY_OUTPUT_EVENT, chunk);
                }
            }
        }
        let _ = app_handle.emit(PTY_EXIT_EVENT, PtyExit { tab_id: id });
    });
    Ok(())
}

#[tauri::command]
pub fn pty_write(
    manager: tauri::State<'_, PtyManager>,
    tab_id: String,
    data: String,
) -> Result<(), String> {
    use std::io::Write;
    let mut sessions = manager.sessions.lock().unwrap();
    let session = sessions.get_mut(&tab_id).ok_or("no such pty tab")?;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| e.to_string())?;
    session.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pty_close(manager: tauri::State<'_, PtyManager>, tab_id: String) -> Result<(), String> {
    manager.sessions.lock().unwrap().remove(&tab_id);
    Ok(())
}
```

> Note: `portable-pty` opens ConPTY on Windows internally, which does not flash a console window, so the `quiet_command` `CREATE_NO_WINDOW` flag is not separately required for the PTY child. Keep the `quiet_command` discipline for any *non-PTY* `std::process::Command` spawns added elsewhere.

- [ ] **Step 6: Register state + commands**

In `crates/vox-gui/src/main.rs`:
- Add `.manage(commands::pty::PtyManager::default())` to the builder (next to the other `.manage(...)` calls).
- Add to `generate_handler![...]`:

```rust
commands::pty::pty_spawn,
commands::pty::pty_write,
commands::pty::pty_close,
```

- [ ] **Step 7: Build and test**

Run: `cargo test -p vox-gui pty:: -- --nocapture` then `cargo build -p vox-gui`
Expected: tests PASS; build succeeds.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/Cargo.toml crates/vox-gui/src/commands/pty.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): portable-pty per-tab terminal session manager"
```

---

## Phase 6 — Frontend: transport, deps, registry

### Task 7: Transport wrappers + xterm deps + surface registry

**Files:**
- Modify: `crates/vox-gui/ui/package.json`
- Modify: `crates/vox-gui/ui/src/transport.ts`
- Modify: `contracts/gui/surface-registry.v1.yaml`
- Test: `crates/vox-gui/ui/src/transport.console.test.ts` (create)

- [ ] **Step 1: Add xterm dependencies**

In `crates/vox-gui/ui/package.json` `dependencies`, add:

```json
"@xterm/xterm": "^5.5.0",
"@xterm/addon-fit": "^0.10.0"
```

Run: `pnpm install` (in `crates/vox-gui/ui`).
Expected: installs cleanly.

- [ ] **Step 2: Write the failing test**

Create `crates/vox-gui/ui/src/transport.console.test.ts`:

```typescript
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import { discoverySuggest, ptyWrite } from './transport';

describe('console transport', () => {
  beforeEach(() => invokeMock.mockReset());

  it('discoverySuggest forwards typed + limit to invoke', async () => {
    invokeMock.mockResolvedValue([{ action_id: 'vox.config.show', completion: 'config show', about: '' }]);
    const out = await discoverySuggest('config', 5);
    expect(invokeMock).toHaveBeenCalledWith('discovery_suggest', { typed: 'config', limit: 5 });
    expect(out[0].action_id).toBe('vox.config.show');
  });

  it('ptyWrite forwards tab id + data', async () => {
    invokeMock.mockResolvedValue(undefined);
    await ptyWrite('tab-1', 'ls\n');
    expect(invokeMock).toHaveBeenCalledWith('pty_write', { tabId: 'tab-1', data: 'ls\n' });
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run (in `crates/vox-gui/ui`): `pnpm vitest run src/transport.console.test.ts`
Expected: FAIL — `discoverySuggest`/`ptyWrite` not exported.

- [ ] **Step 4: Add the wrappers**

Append to `crates/vox-gui/ui/src/transport.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface Suggestion {
  action_id: string;
  completion: string;
  about: string;
}

export interface ActionHelp {
  action_id: string;
  about: string;
  args: { name: string; help: string; required: boolean }[];
  example: string;
}

export function discoverySuggest(typed: string, limit = 8): Promise<Suggestion[]> {
  return invoke<Suggestion[]>('discovery_suggest', { typed, limit });
}

export function discoveryHelp(actionId: string): Promise<ActionHelp | null> {
  return invoke<ActionHelp | null>('discovery_help', { actionId });
}

export function discoveryRecord(actionId: string, used: boolean, nowMs: number, dwellMs: number): Promise<void> {
  return invoke('discovery_record', { actionId, used, nowMs, dwellMs });
}

export function ptySpawn(tabId: string, cols: number, rows: number): Promise<void> {
  return invoke('pty_spawn', { tabId, cols, rows });
}

export function ptyWrite(tabId: string, data: string): Promise<void> {
  return invoke('pty_write', { tabId, data });
}

export function ptyClose(tabId: string): Promise<void> {
  return invoke('pty_close', { tabId });
}

export const PTY_OUTPUT_EVENT = 'vox://pty-output';
export const PTY_EXIT_EVENT = 'vox://pty-exit';

export function listenPtyOutput(onChunk: (tabId: string, data: string) => void): Promise<UnlistenFn> {
  return listen<{ tab_id: string; data: string }>(PTY_OUTPUT_EVENT, (e) =>
    onChunk(e.payload.tab_id, e.payload.data),
  );
}

export function listenPtyExit(onExit: (tabId: string) => void): Promise<UnlistenFn> {
  return listen<{ tab_id: string }>(PTY_EXIT_EVENT, (e) => onExit(e.payload.tab_id));
}
```

> Note: `transport.ts` already imports `invoke`/`listen` at the top (see existing `listenOrchStatus`). If duplicate-import lint fires, fold these into the existing import lines rather than adding new ones.

- [ ] **Step 5: Run test to verify it passes**

Run: `pnpm vitest run src/transport.console.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 6: Register the surface**

In `contracts/gui/surface-registry.v1.yaml`, add (keeping alphabetical order among surfaces):

```yaml
- view_key: console
  cli_group: null
  representation_tier: live_backend
  nav_label: Console
  nav_icon: command
  nav_group: develop
  notes: warp-model discovery terminal
```

Run: `cargo run -p vox-cli -- ci gui-surface-registry --write` (regenerates `surfaceRegistry.generated.ts`).
Expected: the generated TS gains a `console` entry; gate exits 0.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/package.json crates/vox-gui/ui/pnpm-lock.yaml crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/transport.console.test.ts contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
git commit -m "feat(vox-gui): console transport wrappers, xterm deps, surface registry entry"
```

---

## Phase 7 — Frontend: Console surface

### Task 8: Input editor with ghost text

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/InputEditor.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Console/InputEditor.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/src/components/surfaces/Console/InputEditor.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

vi.mock('../../../../transport', () => ({
  discoverySuggest: vi.fn().mockResolvedValue([
    { action_id: 'vox.config.show', completion: 'config show', about: 'show config' },
  ]),
}));

import { InputEditor } from './InputEditor';

describe('InputEditor', () => {
  beforeEach(() => cleanup());

  it('shows ghost text for the top suggestion as you type', async () => {
    render(<InputEditor onSubmit={vi.fn()} onActiveSuggestion={vi.fn()} />);
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'vox config' } });
    await waitFor(() => expect(screen.getByTestId('ghost').textContent).toContain('config show'));
  });

  it('accepts ghost text on Tab and submits on Enter', async () => {
    const onSubmit = vi.fn();
    render(<InputEditor onSubmit={onSubmit} onActiveSuggestion={vi.fn()} />);
    const input = screen.getByRole('textbox') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'vox config' } });
    await waitFor(() => expect(screen.getByTestId('ghost').textContent).toBeTruthy());
    fireEvent.keyDown(input, { key: 'Tab' });
    expect(input.value).toBe('vox config show');
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onSubmit).toHaveBeenCalledWith('vox config show');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/components/surfaces/Console/InputEditor.test.tsx`
Expected: FAIL — cannot resolve `./InputEditor`.

- [ ] **Step 3: Implement the component**

Create `crates/vox-gui/ui/src/components/surfaces/Console/InputEditor.tsx`:

```tsx
import React, { useEffect, useRef, useState } from 'react';
import { discoverySuggest, type Suggestion } from '../../../../transport';

interface Props {
  onSubmit: (line: string) => void;
  /** Called with the currently-highlighted suggestion's action id (for the rail). */
  onActiveSuggestion: (actionId: string | null) => void;
}

/**
 * The console prompt. The shell never receives a keystroke until Enter, so we own
 * completion entirely: as the user types, the top catalog suggestion renders as
 * ghost text after the cursor; Tab/→ accepts it, Enter submits the line.
 */
export function InputEditor({ onSubmit, onActiveSuggestion }: Props) {
  const [value, setValue] = useState('');
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const debounce = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (debounce.current) clearTimeout(debounce.current);
    if (!value.startsWith('vox')) {
      setSuggestions([]);
      onActiveSuggestion(null);
      return;
    }
    debounce.current = setTimeout(() => {
      discoverySuggest(value, 8)
        .then((s) => {
          setSuggestions(s);
          onActiveSuggestion(s[0]?.action_id ?? null);
        })
        .catch(() => setSuggestions([]));
    }, 120);
    return () => {
      if (debounce.current) clearTimeout(debounce.current);
    };
  }, [value, onActiveSuggestion]);

  // The ghost is the remaining text of the top completion beyond what's typed.
  const top = suggestions[0];
  const typedTail = value.replace(/^vox\s*/, '');
  const ghost =
    top && top.completion.startsWith(typedTail) && typedTail.length > 0
      ? top.completion.slice(typedTail.length)
      : '';

  const acceptGhost = () => {
    if (ghost) setValue(`vox ${top!.completion}`);
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if ((e.key === 'Tab' || e.key === 'ArrowRight') && ghost) {
      e.preventDefault();
      acceptGhost();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const line = value.trim();
      if (line) onSubmit(line);
      setValue('');
      setSuggestions([]);
    }
  };

  return (
    <div style={{ position: 'relative', fontFamily: 'monospace' }}>
      <span aria-hidden style={{ position: 'absolute', left: 0, color: '#9ca3af', pointerEvents: 'none' }}>
        {value}
        <span data-testid="ghost">{ghost}</span>
      </span>
      <input
        role="textbox"
        aria-label="console input"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={onKeyDown}
        spellCheck={false}
        autoComplete="off"
        style={{ width: '100%', background: 'transparent', border: 'none', outline: 'none', fontFamily: 'monospace' }}
      />
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/components/surfaces/Console/InputEditor.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Console/InputEditor.tsx crates/vox-gui/ui/src/components/surfaces/Console/InputEditor.test.tsx
git commit -m "feat(vox-gui): console input editor with catalog ghost text"
```

### Task 9: Discovery rail

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/DiscoveryRail.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Console/DiscoveryRail.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/src/components/surfaces/Console/DiscoveryRail.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const recordMock = vi.fn().mockResolvedValue(undefined);
vi.mock('../../../../transport', () => ({
  discoveryHelp: vi.fn().mockResolvedValue({
    action_id: 'vox.scientia.review',
    about: 'Review queued nanopubs',
    args: [{ name: '--limit', help: 'max items', required: false }],
    example: 'vox scientia review',
  }),
  discoveryRecord: (...a: unknown[]) => recordMock(...a),
}));

import { DiscoveryRail } from './DiscoveryRail';

describe('DiscoveryRail', () => {
  beforeEach(() => { cleanup(); recordMock.mockClear(); });

  it('renders help for the active action id', async () => {
    render(<DiscoveryRail actionId="vox.scientia.review" nowMs={1000} />);
    await waitFor(() => expect(screen.getByText('Review queued nanopubs')).toBeTruthy());
    expect(screen.getByText('vox scientia review')).toBeTruthy();
  });

  it('records a seen exposure for the displayed action', async () => {
    render(<DiscoveryRail actionId="vox.scientia.review" nowMs={1000} />);
    await waitFor(() => expect(recordMock).toHaveBeenCalled());
    expect(recordMock.mock.calls[0][0]).toBe('vox.scientia.review');
    expect(recordMock.mock.calls[0][1]).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/components/surfaces/Console/DiscoveryRail.test.tsx`
Expected: FAIL — cannot resolve `./DiscoveryRail`.

- [ ] **Step 3: Implement the component**

Create `crates/vox-gui/ui/src/components/surfaces/Console/DiscoveryRail.tsx`:

```tsx
import React, { useEffect, useRef, useState } from 'react';
import { discoveryHelp, discoveryRecord, type ActionHelp } from '../../../../transport';

interface Props {
  /** The action id currently under the cursor / top suggestion, or null. */
  actionId: string | null;
  /** Epoch ms (passed in so the component stays deterministic/testable). */
  nowMs: number;
}

/**
 * Persistent right-hand rail (layout A). Resolves the active action to its help
 * and records a "seen" exposure (with dwell) so the spaced-repetition scheduler
 * learns what the user has been shown.
 */
export function DiscoveryRail({ actionId, nowMs }: Props) {
  const [help, setHelp] = useState<ActionHelp | null>(null);
  const shownAt = useRef<number>(nowMs);

  useEffect(() => {
    if (!actionId) {
      setHelp(null);
      return;
    }
    shownAt.current = nowMs;
    let live = true;
    discoveryHelp(actionId).then((h) => live && setHelp(h)).catch(() => {});
    return () => {
      live = false;
    };
  }, [actionId, nowMs]);

  // Record a "seen" exposure when the active action settles (debounced by 2s of
  // dwell — matches spec §discovery rail). Fire-and-forget.
  useEffect(() => {
    if (!actionId) return;
    const DWELL_MS = 2000;
    const t = setTimeout(() => {
      discoveryRecord(actionId, false, nowMs + DWELL_MS, DWELL_MS).catch(() => {});
    }, DWELL_MS);
    return () => clearTimeout(t);
  }, [actionId, nowMs]);

  if (!help) {
    return (
      <aside aria-label="discovery" style={{ width: 280, padding: 12, fontSize: 12 }}>
        <p style={{ color: '#9ca3af' }}>Start typing a vox command to see help and tips.</p>
      </aside>
    );
  }

  return (
    <aside aria-label="discovery" style={{ width: 280, padding: 12, fontSize: 12 }}>
      <h3 style={{ fontSize: 13, margin: '0 0 6px' }}>{help.example}</h3>
      <p style={{ margin: '0 0 8px' }}>{help.about}</p>
      {help.args.length > 0 && (
        <ul style={{ margin: 0, paddingLeft: 16 }}>
          {help.args.map((a) => (
            <li key={a.name}>
              <code>{a.name}</code>
              {a.required ? ' (required)' : ''} — {a.help}
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/components/surfaces/Console/DiscoveryRail.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Console/DiscoveryRail.tsx crates/vox-gui/ui/src/components/surfaces/Console/DiscoveryRail.test.tsx
git commit -m "feat(vox-gui): console discovery rail with help + seen recording"
```

### Task 10: Terminal tab (xterm.js + PTY wiring)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/TerminalTab.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Console/TerminalTab.test.tsx`

- [ ] **Step 1: Write the failing test**

Because xterm.js touches the real DOM/canvas, the test asserts wiring (spawn on mount, write on submit) with xterm mocked.

Create `crates/vox-gui/ui/src/components/surfaces/Console/TerminalTab.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const spawnMock = vi.fn().mockResolvedValue(undefined);
const writeMock = vi.fn().mockResolvedValue(undefined);
vi.mock('../../../../transport', () => ({
  ptySpawn: (...a: unknown[]) => spawnMock(...a),
  ptyWrite: (...a: unknown[]) => writeMock(...a),
  ptyClose: vi.fn().mockResolvedValue(undefined),
  listenPtyOutput: vi.fn().mockResolvedValue(() => {}),
  listenPtyExit: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    open() {}
    write() {}
    onData() {}
    dispose() {}
    loadAddon() {}
    get cols() { return 80; }
    get rows() { return 24; }
  },
}));
vi.mock('@xterm/addon-fit', () => ({ FitAddon: class { fit() {} } }));

import { TerminalTab } from './TerminalTab';

describe('TerminalTab', () => {
  beforeEach(() => { cleanup(); spawnMock.mockClear(); writeMock.mockClear(); });

  it('spawns a PTY for its tab id on mount', async () => {
    render(<TerminalTab tabId="tab-1" pendingLine={null} />);
    await waitFor(() => expect(spawnMock).toHaveBeenCalledWith('tab-1', 80, 24));
  });

  it('writes a submitted line to the PTY', async () => {
    const { rerender } = render(<TerminalTab tabId="tab-1" pendingLine={null} />);
    await waitFor(() => expect(spawnMock).toHaveBeenCalled());
    rerender(<TerminalTab tabId="tab-1" pendingLine={{ text: 'ls', seq: 1 }} />);
    await waitFor(() => expect(writeMock).toHaveBeenCalledWith('tab-1', 'ls\n'));
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/components/surfaces/Console/TerminalTab.test.tsx`
Expected: FAIL — cannot resolve `./TerminalTab`.

- [ ] **Step 3: Implement the component**

Create `crates/vox-gui/ui/src/components/surfaces/Console/TerminalTab.tsx`:

```tsx
import React, { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import {
  ptySpawn,
  ptyWrite,
  ptyClose,
  listenPtyOutput,
  listenPtyExit,
} from '../../../../transport';

/** A line the parent wants written to this PTY. `seq` changes each submit so the
 *  effect re-fires even when the same text is sent twice. */
export interface PendingLine {
  text: string;
  seq: number;
}

interface Props {
  tabId: string;
  pendingLine: PendingLine | null;
}

/**
 * Renders one PTY-backed terminal via xterm.js. Spawns the PTY on mount, streams
 * output in, and forwards both interactive keystrokes (xterm onData) and
 * parent-submitted lines to the backend.
 */
export function TerminalTab({ tabId, pendingLine }: Props) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<Terminal | null>(null);

  useEffect(() => {
    const term = new Terminal({ convertEol: true, fontFamily: 'monospace', fontSize: 13 });
    const fit = new FitAddon();
    term.loadAddon(fit);
    if (hostRef.current) term.open(hostRef.current);
    try { fit.fit(); } catch { /* jsdom: no layout */ }
    termRef.current = term;

    term.onData((d) => { ptyWrite(tabId, d).catch(() => {}); });

    let unOut: (() => void) | undefined;
    let unExit: (() => void) | undefined;
    listenPtyOutput((id, data) => { if (id === tabId) term.write(data); }).then((u) => (unOut = u));
    listenPtyExit((id) => { if (id === tabId) term.write('\r\n[process exited]\r\n'); }).then((u) => (unExit = u));

    ptySpawn(tabId, term.cols || 80, term.rows || 24).catch(() => {});

    return () => {
      unOut?.();
      unExit?.();
      ptyClose(tabId).catch(() => {});
      term.dispose();
    };
  }, [tabId]);

  useEffect(() => {
    if (pendingLine && termRef.current) {
      ptyWrite(tabId, `${pendingLine.text}\n`).catch(() => {});
    }
  }, [pendingLine, tabId]);

  return <div ref={hostRef} aria-label="terminal" style={{ height: '100%', width: '100%' }} />;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/components/surfaces/Console/TerminalTab.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Console/TerminalTab.tsx crates/vox-gui/ui/src/components/surfaces/Console/TerminalTab.test.tsx
git commit -m "feat(vox-gui): xterm.js terminal tab wired to PTY backend"
```

### Task 11: Agent strip

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/AgentStrip.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Console/AgentStrip.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/src/components/surfaces/Console/AgentStrip.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';

import { AgentStrip } from './AgentStrip';

describe('AgentStrip', () => {
  beforeEach(() => cleanup());

  it('renders a chip per agent with its state', () => {
    render(
      <AgentStrip
        agents={[
          { id: 'a1', name: 'sci-runner', state: 'running' },
          { id: 'a2', name: 'quantize-01', state: 'queued' },
        ]}
        onOpen={vi.fn()}
      />,
    );
    expect(screen.getByText('sci-runner')).toBeTruthy();
    expect(screen.getByText('quantize-01')).toBeTruthy();
  });

  it('calls onOpen with the agent id when a chip is clicked', () => {
    const onOpen = vi.fn();
    render(<AgentStrip agents={[{ id: 'a1', name: 'sci-runner', state: 'running' }]} onOpen={onOpen} />);
    fireEvent.click(screen.getByText('sci-runner'));
    expect(onOpen).toHaveBeenCalledWith('a1');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/components/surfaces/Console/AgentStrip.test.tsx`
Expected: FAIL — cannot resolve `./AgentStrip`.

- [ ] **Step 3: Implement the component**

Create `crates/vox-gui/ui/src/components/surfaces/Console/AgentStrip.tsx`:

```tsx
import React from 'react';

export interface AgentChip {
  id: string;
  name: string;
  state: string;
}

interface Props {
  agents: AgentChip[];
  onOpen: (agentId: string) => void;
}

/**
 * Persistent strip of live agents. Data comes from the same `vox://orch-status`
 * snapshot the Dashboard uses (mapped by the parent), so the two never disagree.
 * Clicking a chip asks the parent to open that agent as a tab.
 */
export function AgentStrip({ agents, onOpen }: Props) {
  if (agents.length === 0) {
    return <div aria-label="agents" style={{ padding: '4px 10px', fontSize: 11, color: '#9ca3af' }}>no agents</div>;
  }
  return (
    <div aria-label="agents" style={{ display: 'flex', gap: 8, padding: '4px 10px', fontSize: 11 }}>
      {agents.map((a) => (
        <button
          key={a.id}
          onClick={() => onOpen(a.id)}
          style={{ borderRadius: 10, padding: '2px 8px', cursor: 'pointer' }}
          title={`${a.name} · ${a.state}`}
        >
          {a.name} · {a.state}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/components/surfaces/Console/AgentStrip.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Console/AgentStrip.tsx crates/vox-gui/ui/src/components/surfaces/Console/AgentStrip.test.tsx
git commit -m "feat(vox-gui): console agent strip (chips from orchestrator status)"
```

### Task 12: Console surface root + App wiring

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/Console.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/Console.test.tsx`
- Modify: `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/src/components/surfaces/Console/Console.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

vi.mock('../../../../transport', () => ({
  discoverySuggest: vi.fn().mockResolvedValue([]),
  discoveryHelp: vi.fn().mockResolvedValue(null),
  discoveryRecord: vi.fn().mockResolvedValue(undefined),
  ptySpawn: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  ptyClose: vi.fn().mockResolvedValue(undefined),
  listenPtyOutput: vi.fn().mockResolvedValue(() => {}),
  listenPtyExit: vi.fn().mockResolvedValue(() => {}),
  listenOrchStatus: vi.fn().mockRejectedValue(new Error('not in tauri')),
}));
vi.mock('@xterm/xterm', () => ({
  Terminal: class { open() {} write() {} onData() {} dispose() {} loadAddon() {} get cols() { return 80; } get rows() { return 24; } },
}));
vi.mock('@xterm/addon-fit', () => ({ FitAddon: class { fit() {} } }));

import { Console } from './Console';

describe('Console', () => {
  beforeEach(() => cleanup());

  it('renders the terminal, input, and discovery rail', async () => {
    render(<Console pushToast={vi.fn()} />);
    expect(screen.getByLabelText('terminal')).toBeTruthy();
    expect(screen.getByRole('textbox')).toBeTruthy();
    await waitFor(() => expect(screen.getByLabelText('discovery')).toBeTruthy());
  });

  it('submitting a line in the input forwards it to the terminal write path', async () => {
    const t = await import('../../../../transport');
    render(<Console pushToast={vi.fn()} />);
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'echo hi' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(t.ptyWrite).toHaveBeenCalledWith('console-1', 'echo hi\n'));
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/components/surfaces/Console/Console.test.tsx`
Expected: FAIL — cannot resolve `./Console`.

- [ ] **Step 3: Implement the surface root**

Create `crates/vox-gui/ui/src/components/surfaces/Console/Console.tsx`:

```tsx
import React, { useEffect, useState } from 'react';
import { InputEditor } from './InputEditor';
import { DiscoveryRail } from './DiscoveryRail';
import { TerminalTab, type PendingLine } from './TerminalTab';
import { AgentStrip, type AgentChip } from './AgentStrip';
import { listenOrchStatus } from '../../../../transport';

interface Props {
  pushToast: (t: { kind: string; message: string }) => void;
}

/**
 * Vox Console surface (layout A): agent strip on top, terminal on the left,
 * persistent discovery rail on the right, owned input editor along the bottom.
 * A single PTY tab ("console-1") in v1; multi-tab is additive.
 */
export function Console({ pushToast }: Props) {
  const [pending, setPending] = useState<PendingLine | null>(null);
  const [activeAction, setActiveAction] = useState<string | null>(null);
  const [agents, setAgents] = useState<AgentChip[]>([]);
  const seq = React.useRef(0);
  const tabId = 'console-1';
  const nowMs = Date.now();

  useEffect(() => {
    let un: (() => void) | undefined;
    listenOrchStatus((status: any) => {
      const list: AgentChip[] = (status?.agents ?? []).map((a: any) => ({
        id: String(a.id ?? a.agent_id ?? ''),
        name: String(a.name ?? a.id ?? 'agent'),
        state: a.paused ? 'paused' : a.in_progress > 0 ? 'running' : 'queued',
      }));
      setAgents(list);
    })
      .then((u) => (un = u))
      .catch(() => {/* not in tauri / daemon down — strip shows "no agents" */});
    return () => un?.();
  }, []);

  const submit = (line: string) => {
    seq.current += 1;
    setPending({ text: line, seq: seq.current });
  };

  const openAgentTab = (agentId: string) => {
    // v1: surface the agent id via toast; full agent-tab streaming is Task 13.
    pushToast({ kind: 'info', message: `agent ${agentId} — open tab (coming in agent-tab task)` });
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <AgentStrip agents={agents} onOpen={openAgentTab} />
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div style={{ display: 'flex', flexDirection: 'column', flex: 1, minWidth: 0 }}>
          <div style={{ flex: 1, minHeight: 0 }}>
            <TerminalTab tabId={tabId} pendingLine={pending} />
          </div>
          <div style={{ borderTop: '1px solid rgba(255,255,255,0.08)', padding: '6px 10px' }}>
            <InputEditor onSubmit={submit} onActiveSuggestion={setActiveAction} />
          </div>
        </div>
        <DiscoveryRail actionId={activeAction} nowMs={nowMs} />
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/components/surfaces/Console/Console.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Wire into App.tsx**

In `crates/vox-gui/ui/src/App.tsx`:
- Add `'console'` to the `View` union type.
- Add the import near the other surface imports:

```typescript
import { Console } from './components/surfaces/Console/Console';
```

- Add a case in the `renderView` switch:

```typescript
case 'console':
  return <Console pushToast={pushToast} />;
```

- [ ] **Step 6: Run the full UI test suite + typecheck**

Run (in `crates/vox-gui/ui`): `pnpm vitest run` then `pnpm tsc --noEmit` (or the repo's typecheck script).
Expected: all tests PASS; no type errors.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Console/Console.tsx crates/vox-gui/ui/src/components/surfaces/Console/Console.test.tsx crates/vox-gui/ui/src/App.tsx
git commit -m "feat(vox-gui): Console surface root wired into App (layout A)"
```

---

## Phase 8 — Agent tabs, send-to-agent, cross-menu deep links

### Task 13: Agent event tab + send-to-agent composer + Dashboard deep link

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/AgentTab.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/AgentTab.test.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/SendToAgent.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Console/SendToAgent.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Console/Console.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` (add "Open in Console")

- [ ] **Step 1: Write the failing test for AgentTab**

Create `crates/vox-gui/ui/src/components/surfaces/Console/AgentTab.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

let emit: ((e: { id: number; timestamp_ms: number; kind: { type: string; agent_id?: string } }) => void) | null = null;
vi.mock('../../../../transport', () => ({
  listenAgentEvents: vi.fn().mockImplementation((cb: any) => { emit = cb; return Promise.resolve(() => {}); }),
}));

import { AgentTab } from './AgentTab';

describe('AgentTab', () => {
  beforeEach(() => { cleanup(); emit = null; });

  it('appends events matching its agent id', async () => {
    render(<AgentTab agentId="a1" />);
    await waitFor(() => expect(emit).toBeTruthy());
    emit!({ id: 1, timestamp_ms: 0, kind: { type: 'task_started', agent_id: 'a1' } });
    await waitFor(() => expect(screen.getByText(/task_started/)).toBeTruthy());
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `pnpm vitest run src/components/surfaces/Console/AgentTab.test.tsx`
Expected: FAIL — cannot resolve `./AgentTab`.

- [ ] **Step 3: Implement AgentTab**

Create `crates/vox-gui/ui/src/components/surfaces/Console/AgentTab.tsx`:

```tsx
import React, { useEffect, useState } from 'react';
import { listenAgentEvents, type AgentEventFrame } from '../../../../transport';

interface Props {
  agentId: string;
}

/**
 * A console tab showing one agent's live event stream. Reuses the existing
 * `vox://agent-events` Tauri stream (same source as the Dashboard), filtering to
 * this agent. Read-only view; spawning/controlling agents is done via commands.
 */
export function AgentTab({ agentId }: Props) {
  const [lines, setLines] = useState<string[]>([]);

  useEffect(() => {
    let un: (() => void) | undefined;
    listenAgentEvents((e: AgentEventFrame) => {
      const id = (e.kind as any).agent_id;
      if (id && String(id) !== agentId) return;
      setLines((prev) => [...prev.slice(-499), `${e.timestamp_ms} ${e.kind.type}`]);
    })
      .then((u) => (un = u))
      .catch(() => {});
    return () => un?.();
  }, [agentId]);

  return (
    <div aria-label="agent events" style={{ fontFamily: 'monospace', fontSize: 12, padding: 8, overflowY: 'auto', height: '100%' }}>
      {lines.length === 0 ? <p style={{ color: '#9ca3af' }}>waiting for events…</p> : lines.map((l, i) => <div key={i}>{l}</div>)}
    </div>
  );
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `pnpm vitest run src/components/surfaces/Console/AgentTab.test.tsx`
Expected: PASS.

- [ ] **Step 5: Write the failing test for SendToAgent**

Create `crates/vox-gui/ui/src/components/surfaces/Console/SendToAgent.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';

import { SendToAgent } from './SendToAgent';

describe('SendToAgent', () => {
  beforeEach(() => cleanup());

  it('submits the block text + chosen agent to onSend', () => {
    const onSend = vi.fn();
    render(
      <SendToAgent
        block={'echo hi\nhi'}
        agents={[{ id: 'a1', name: 'sci-runner', state: 'running' }]}
        onSend={onSend}
        onClose={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByLabelText('note'), { target: { value: 'fyi' } });
    fireEvent.click(screen.getByText('Send'));
    expect(onSend).toHaveBeenCalledWith('a1', 'echo hi\nhi', 'fyi');
  });
});
```

- [ ] **Step 6: Implement SendToAgent (verify fail first)**

Run the test to confirm it fails (`cannot resolve`), then create `crates/vox-gui/ui/src/components/surfaces/Console/SendToAgent.tsx`:

```tsx
import React, { useState } from 'react';
import type { AgentChip } from './AgentStrip';

interface Props {
  block: string;
  agents: AgentChip[];
  onSend: (agentId: string, block: string, note: string) => void;
  onClose: () => void;
}

/** Small composer to send a terminal block to an agent's A2A inbox. */
export function SendToAgent({ block, agents, onSend, onClose }: Props) {
  const [target, setTarget] = useState(agents[0]?.id ?? '');
  const [note, setNote] = useState('');
  return (
    <div role="dialog" aria-label="send to agent" style={{ padding: 12 }}>
      <select aria-label="agent" value={target} onChange={(e) => setTarget(e.target.value)}>
        {agents.map((a) => (
          <option key={a.id} value={a.id}>{a.name}</option>
        ))}
      </select>
      <pre style={{ maxHeight: 80, overflow: 'auto', fontSize: 11 }}>{block}</pre>
      <input aria-label="note" value={note} onChange={(e) => setNote(e.target.value)} placeholder="note (optional)" />
      <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
        <button onClick={() => onSend(target, block, note)}>Send</button>
        <button onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}
```

> Implementer note: wire `onSend` in `Console.tsx` to the existing A2A send path. Grep `vox-gui/src/commands` for an existing A2A / message-send Tauri command; if none exists, add a thin command that calls the orchestrator daemon's A2A send (the `a2a_messages` path identified in the spec). If adding a backend command, give it its own TDD task mirroring Task 5 before wiring the button.

- [ ] **Step 7: Run both new component tests**

Run: `pnpm vitest run src/components/surfaces/Console/SendToAgent.test.tsx src/components/surfaces/Console/AgentTab.test.tsx`
Expected: PASS.

- [ ] **Step 8: Add "Open in Console" deep link on the Dashboard**

In `Dashboard.tsx`, add a per-agent affordance (button/link) that switches the active view to `'console'` and passes the agent id. Follow the existing view-switch mechanism (the parent passes a setter; match how other surfaces request navigation). Add a vitest assertion in the Dashboard's existing test file that clicking it invokes the navigation callback with `('console', agentId)`.

> Implementer note: inspect how `App.tsx` exposes view switching to surfaces (props vs context) and reuse it. Do not introduce a new navigation mechanism.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Console/AgentTab.tsx crates/vox-gui/ui/src/components/surfaces/Console/AgentTab.test.tsx crates/vox-gui/ui/src/components/surfaces/Console/SendToAgent.tsx crates/vox-gui/ui/src/components/surfaces/Console/SendToAgent.test.tsx crates/vox-gui/ui/src/components/surfaces/Console/Console.tsx crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx
git commit -m "feat(vox-gui): agent tabs, send-to-agent composer, dashboard deep link"
```

---

## Phase 9 — Integration gates + docs

### Task 14: Surface-registry gate, arch-check, where-things-live, full sweeps

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`
- (verification only) all gates

- [ ] **Step 1: Add where-things-live rows**

In `docs/src/architecture/where-things-live.md`, add rows mapping the new concepts to crates:
- "Console discovery ledger / spaced repetition" → `crates/vox-gamify/src/discovery/`
- "Console PTY terminal sessions" → `crates/vox-gui/src/commands/pty.rs`
- "Console discovery Tauri commands" → `crates/vox-gui/src/commands/discovery.rs`
- "Console UI surface" → `crates/vox-gui/ui/src/components/surfaces/Console/`

- [ ] **Step 2: Run the surface-registry gate**

Run: `cargo run -p vox-cli -- ci gui-surface-registry`
Expected: exit 0 (console entry present, `view_key` wired in App.tsx).

- [ ] **Step 3: Run arch-check**

Run: `cargo run -p vox-arch-check`
Expected: exit 0 (no new illegal edges; `vox-gui → vox-gamify` already exists per Cargo.toml; `portable-pty` is an external dep).

- [ ] **Step 4: Run the full Rust test suite for touched crates**

Run: `cargo test -p vox-db -p vox-gamify -p vox-gui`
Expected: all PASS.

- [ ] **Step 5: Run the full UI suite + typecheck**

Run (in `crates/vox-gui/ui`): `pnpm vitest run && pnpm tsc --noEmit`
Expected: all PASS; no type errors.

- [ ] **Step 6: Format touched crates (Windows-safe)**

Run: `cargo fmt -p vox-db -p vox-gamify -p vox-gui` (never `cargo fmt --all` on Windows).
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): where-things-live rows for Vox Console discovery engine"
```

---

## Out of scope (v1) — do not implement

- **OSC 133 block segmentation + per-block copy/re-run affordances.** The spec
  lists these as part of the terminal core, but full block parsing requires a
  shell-profile marker-injection step and a stateful parser that is a project of
  its own. v1 ships raw xterm scrollback; the send-to-agent composer (Task 13)
  operates on the user's *current text selection* in the terminal, not a parsed
  block. Block segmentation is the first follow-up after v1 lands. (Flagged to
  the maintainer during plan self-review — accept or pull into v1 before
  executing Task 10.)
- LLM next-need prediction / co-occurrence mining (schema leaves room; future work).
- Multi-tab beyond the single `console-1` PTY tab + agent tabs (additive later).
- Warp fork / Warp Workflows export.
- Console over the HTTP gateway / web dashboard.
- Replacing the existing Catalog forms surface.

## Verification summary (run before finishing the branch)

```bash
cargo test -p vox-db -p vox-gamify -p vox-gui
cargo run -p vox-cli -- ci gui-surface-registry
cargo run -p vox-arch-check
( cd crates/vox-gui/ui && pnpm vitest run && pnpm tsc --noEmit )
```
All must be green before invoking superpowers:finishing-a-development-branch.
