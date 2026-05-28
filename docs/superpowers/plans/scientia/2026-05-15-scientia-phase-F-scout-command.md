# SCIENTIA Phase F — `vox scientia scout` Single-Command Surface

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Status:** detailed (promoted from outline 2026-05-15).

**Goal:** Provide one command — `vox scientia scout` — that surveys recent activity via Phase A's producers and prints a ranked candidate list with proposed `candidate_class`, top signals, confidence, suggested venue, and a recommended next command. Optional `--watch` mode for cadence-based monitoring.

**Architecture:** Thin CLI wrapper over Phase A's `ProducerRegistry`. The handler instantiates `ProducerRegistry::default_with_codex(codex)`, runs `run_all(&ctx)`, persists newly-seen candidates to `scientia_finding_candidates`, then renders a table (default) or JSON (`--output json`). `--watch` runs the cycle on an interval, diffs against the previous cycle's stored candidates, and emits OS notifications for new high-confidence rows.

**Tech Stack:** Rust 2024; existing `clap` CLI framework (mirror `publication-prepare` pattern at `crates/vox-cli/src/commands/db/publication/prepare.rs`); `vox-scientia-producers` (Phase A — must be detailed-status complete); existing `vox-db` connection helper. Notifications: `notify-rust` if already in workspace deps; otherwise best-effort `println!` fallback.

**Strategic context:** [Gap-map §2 Gap F](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-f--vox-scientia-scout-single-command-surface).

**Out of scope:**
- The detectors themselves (Phase A).
- The venue recommender (Phase E — Phase F gracefully renders `—` for the venue column when Phase E is incomplete).
- The dashboard panel (Phase H).
- Persistent system-service install for `--watch` (foreground-only in Phase F).

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/vox-cli/src/db_cli/subcommands.rs` | Add `Scout` Clap variant under the `Scientia` subtree |
| Modify | `crates/vox-cli/src/commands/scientia.rs` | Facade arm dispatching to handler |
| Create | `crates/vox-cli/src/commands/db/scout.rs` | Scout handler: one-shot + watch loop + rendering |
| Modify | `crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt` | Add `scientia/scout` path |
| Modify | `contracts/cli/command-registry.yaml` | Register `scout` |
| Modify | `docs/src/reference/cli.md` | Document `scout` |
| Modify | `docs/src/how-to/how-to-scientia-publication.md` | Add "start here: `vox scientia scout`" section as the front-door entry-point |
| Modify | `crates/vox-orchestrator/src/mcp_tools/tools/scientia_tools.rs` | Add MCP tool `vox_scientia_scout` |
| Modify | `crates/vox-orchestrator-mcp/src/input_schemas.rs` | Schema for the MCP tool |
| Modify | `crates/vox-orchestrator-mcp/src/dispatch.rs` | Route the MCP tool |
| Modify | `contracts/mcp/tool-registry.canonical.yaml` | Register MCP tool |

LoC budget: ~600 LoC + ~200 tests.

---

## Pre-flight verification

- [ ] **Step P1: Confirm Phase A is detailed-status complete**

```bash
grep "Status:" docs/superpowers/plans/scientia/2026-05-15-scientia-phase-A-signal-producers.md | head -1
```
Expected: `> **Status:** detailed`. If still outline, **stop** — Phase F depends on Phase A's `ProducerRegistry::default_with_codex` and persistence flow being executable.

- [ ] **Step P2: Confirm CLI scaffolding pattern**

Open `crates/vox-cli/src/commands/db/publication/prepare.rs` and note: handler signature, codex acquisition, error type. Phase F mirrors this exactly.

- [ ] **Step P3: Notification crate check**

```bash
grep "notify-rust" Cargo.lock || echo "absent"
```
If absent: Phase F uses best-effort `println!` for notifications and we punt `notify-rust` to a follow-up.

---

## Task 1: Clap variant

**Files:**
- Modify: `crates/vox-cli/src/db_cli/subcommands.rs`

- [ ] **Step 1.1: Write the failing parity test**

The catalog-baseline test will fail until the variant is added. That's our first signal.

```bash
cargo test -p vox-cli command_catalog_paths_baseline 2>&1 | head -10
```
Note the current pass; we'll re-run after the variant and baseline-update.

- [ ] **Step 1.2: Add the variant**

In `subcommands.rs` under the Scientia subtree (locate by searching for the existing `PublicationPrepare` variant — mirror placement):

```rust
/// Scout the current workspace for publication candidates.
Scout {
    /// Window of recent commits to scan.
    #[arg(long, default_value_t = 100)] commit_window: usize,
    /// Days of activity to include.
    #[arg(long, default_value_t = 30)] days_window: u32,
    /// Run continuously on a cadence rather than once.
    #[arg(long)] watch: bool,
    /// Interval for --watch (seconds).
    #[arg(long, default_value_t = 300)] watch_interval_seconds: u64,
    /// Confidence threshold for --watch OS notifications.
    #[arg(long, default_value_t = 0.7)] notify_threshold: f64,
    /// Output format.
    #[arg(long, value_enum, default_value_t = ScoutOutput::Table)] output: ScoutOutput,
    /// Restrict to a single candidate class.
    #[arg(long)] candidate_class: Option<String>,
}
```

Define `ScoutOutput`:
```rust
#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ScoutOutput { Table, Json }
```

- [ ] **Step 1.3: Run to verify Clap accepts the variant**

```bash
cargo check -p vox-cli
```

- [ ] **Step 1.4: Update catalog baseline**

```bash
UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli command_catalog_paths_baseline
```

- [ ] **Step 1.5: Commit**

```bash
git add crates/vox-cli/src/db_cli/subcommands.rs crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt
git commit -m "feat(vox-cli): scientia scout Clap variant"
```

---

## Task 2: Handler skeleton

**Files:**
- Create: `crates/vox-cli/src/commands/db/scout.rs`
- Modify: `crates/vox-cli/src/commands/db/mod.rs` (add `pub mod scout;`)
- Modify: `crates/vox-cli/src/commands/scientia.rs` (dispatch the new variant)

- [ ] **Step 2.1: Stub the handler**

Create `crates/vox-cli/src/commands/db/scout.rs`:

```rust
use anyhow::Result;
use vox_scientia_producers::{ProducerContext, ProducerRegistry};

pub struct ScoutOptions {
    pub commit_window: usize,
    pub days_window: u32,
    pub watch: bool,
    pub watch_interval_seconds: u64,
    pub notify_threshold: f64,
    pub output: super::super::OutputForm,  // adapt to the actual enum location
    pub candidate_class: Option<String>,
}

pub async fn run_scout(opts: ScoutOptions) -> Result<()> {
    let codex = vox_db::VoxDb::connect_default().await?;
    let repo_root = std::env::current_dir()?;
    let ctx = ProducerContext {
        repo_root,
        commit_window: opts.commit_window,
        days_window: opts.days_window,
        now_ms: chrono::Utc::now().timestamp_millis(), // adapt to whichever date crate is in tree
        session_id: format!("scout-{}", uuid_or_timestamp()),
    };
    let registry = ProducerRegistry::default_with_codex(codex.clone());

    if opts.watch {
        run_watch(registry, ctx, opts).await
    } else {
        run_once(registry, &ctx, &opts).await
    }
}

async fn run_once(
    registry: ProducerRegistry,
    ctx: &ProducerContext,
    opts: &ScoutOptions,
) -> Result<()> {
    let events = registry.run_all(ctx).await;
    persist_new_candidates(/* codex, events */).await?;
    render(events, opts);
    Ok(())
}

async fn run_watch(
    _registry: ProducerRegistry,
    _ctx: ProducerContext,
    _opts: ScoutOptions,
) -> Result<()> {
    // Task 5 fills this in.
    Ok(())
}

fn render(_events: Vec<vox_research_events::ResearchEvent>, _opts: &ScoutOptions) {
    // Task 4 fills this in.
}

async fn persist_new_candidates(/* ... */) -> Result<()> {
    // Task 3 fills this in.
    Ok(())
}

fn uuid_or_timestamp() -> String {
    // Use whichever ID generator the workspace already pulls in; fallback timestamp.
    chrono::Utc::now().timestamp_millis().to_string()
}
```

- [ ] **Step 2.2: Wire dispatch**

In `crates/vox-cli/src/commands/scientia.rs`, add the arm:

```rust
ScientiaSubcommand::Scout { commit_window, days_window, watch, watch_interval_seconds, notify_threshold, output, candidate_class } => {
    crate::commands::db::scout::run_scout(ScoutOptions {
        commit_window, days_window, watch, watch_interval_seconds,
        notify_threshold, output, candidate_class,
    }).await
}
```

- [ ] **Step 2.3: Verify it compiles**

```bash
cargo check -p vox-cli
```

- [ ] **Step 2.4: Commit**

```bash
git commit -am "scaffold(vox-cli): scientia scout handler skeleton"
```

---

## Task 3: Persist new candidates to `scientia_finding_candidates`

**Files:**
- Modify: `crates/vox-cli/src/commands/db/scout.rs`

- [ ] **Step 3.1: Write the failing test**

In `crates/vox-cli/tests/scout_persist.rs`:

```rust
// Setup: in-memory codex, register a synthetic producer that emits
// one FindingCandidateProposed event. Call run_once.
// Assert: scientia_finding_candidates row count goes from 0 to 1.
```

- [ ] **Step 3.2: Implement `persist_new_candidates`**

For each `ResearchEvent::FindingCandidateProposed`, build a `FindingCandidateRow` and insert via the Phase A store ops (`vox_db::store::ops_finding_candidates::insert_candidate`). The `signal_fingerprint` is derived from `finding_id` (the producer guarantees fingerprint stability). Existing rows (UNIQUE on `(producer_name, signal_fingerprint)`) are skipped silently — duplicate-insert returns Ok by mapping the unique-violation error.

```rust
async fn persist_new_candidates(
    codex: &vox_db::Codex,
    events: &[vox_research_events::ResearchEvent],
    session_id: &str,
) -> anyhow::Result<usize> {
    use vox_db::store::ops_finding_candidates::{insert_candidate, FindingCandidateRow};
    let now = chrono::Utc::now().timestamp_millis();
    let mut inserted = 0;
    for ev in events {
        if let vox_research_events::ResearchEvent::FindingCandidateProposed {
            finding_id, worthiness_score, ..
        } = ev {
            let row = FindingCandidateRow {
                candidate_id: finding_id.clone(),
                candidate_class: classify_from_finding_id(finding_id),
                publication_id: None,
                title_hint: None,
                internal_signals_json: "[]".into(),
                novelty_evidence_bundle_id: None,
                worthiness_decision_ref: None,
                confidence_json: Some(format!(r#"{{"signal_strength":{}}}"#, worthiness_score)),
                repository_id: Some("vox".into()),  // adapt: read from vox-repository
                producer_name: "scout".into(),
                signal_fingerprint: finding_id.clone(),
                created_at_ms: now,
                updated_at_ms: now,
            };
            match insert_candidate(codex.pool(), &row).await {
                Ok(_) => inserted += 1,
                Err(e) if is_unique_violation(&e) => { /* skip */ },
                Err(e) => return Err(e.into()),
            }
        }
    }
    Ok(inserted)
}

fn classify_from_finding_id(id: &str) -> String {
    if id.starts_with("algimp-") { "algorithmic_improvement".into() }
    else if id.starts_with("repinf-") { "reproducibility_infra".into() }
    else if id.starts_with("teltr-") { "telemetry_trust".into() }
    else { "other".into() }
}
```

- [ ] **Step 3.3: Verify test passes**

- [ ] **Step 3.4: Commit**

---

## Task 4: Table + JSON rendering

**Files:**
- Modify: `crates/vox-cli/src/commands/db/scout.rs`

- [ ] **Step 4.1: Test**

```rust
#[test]
fn table_renders_with_aligned_columns() {
    let events = vec![/* synthetic event */];
    let out = render_table(&events);
    assert!(out.contains("candidate-id"));
    assert!(out.contains("algorithmic_improvement"));
}
```

- [ ] **Step 4.2: Implement renderers**

Plain string rendering, no extra deps:

```rust
fn render_table(events: &[vox_research_events::ResearchEvent]) -> String {
    let mut out = String::new();
    out.push_str("candidate-id                          class                       conf   next\n");
    out.push_str("─────────────────────────────────────────────────────────────────────────────\n");
    for ev in events {
        if let vox_research_events::ResearchEvent::FindingCandidateProposed {
            finding_id, worthiness_score, ..
        } = ev {
            out.push_str(&format!(
                "{:36}  {:26}  {:>5.2}   vox scientia publication-prepare --publication-id {}\n",
                finding_id,
                classify_from_finding_id(finding_id),
                worthiness_score,
                finding_id,
            ));
        }
    }
    out
}

fn render_json(events: &[vox_research_events::ResearchEvent]) -> String {
    serde_json::to_string_pretty(events).unwrap()
}
```

(Use a real table crate like `comfy-table` only if already in tree; otherwise plain string format is fine for Phase F.)

- [ ] **Step 4.3: Verify pass**

- [ ] **Step 4.4: Commit**

---

## Task 5: `--watch` loop

**Files:**
- Modify: `crates/vox-cli/src/commands/db/scout.rs`

- [ ] **Step 5.1: Implement `run_watch`**

```rust
async fn run_watch(
    registry: ProducerRegistry,
    ctx: ProducerContext,
    opts: ScoutOptions,
) -> Result<()> {
    use tokio::time::{interval, Duration};
    let codex = vox_db::VoxDb::connect_default().await?;
    let mut tick = interval(Duration::from_secs(opts.watch_interval_seconds));
    eprintln!("vox scientia scout: watching (every {}s); Ctrl+C to stop", opts.watch_interval_seconds);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let events = registry.run_all(&ctx).await;
                let inserted = persist_new_candidates(&codex.codex(), &events, &ctx.session_id).await?;
                if inserted > 0 {
                    notify_new_candidates(&events, opts.notify_threshold);
                    println!("{}", render_table(&events));
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\nstopped.");
                return Ok(());
            }
        }
    }
}

fn notify_new_candidates(events: &[vox_research_events::ResearchEvent], threshold: f64) {
    for ev in events {
        if let vox_research_events::ResearchEvent::FindingCandidateProposed { finding_id, worthiness_score, .. } = ev {
            if *worthiness_score >= threshold {
                // Best-effort: print prominently. notify-rust integration is a follow-up.
                eprintln!("\x1b[33m[scout] new candidate {}  score={:.2}\x1b[0m", finding_id, worthiness_score);
            }
        }
    }
}
```

- [ ] **Step 5.2: Lifecycle test**

In `tests/scout_watch.rs`:
- Start `run_watch` in a Tokio task.
- After one cycle, send SIGINT (or set up a cancel token).
- Assert clean exit.

- [ ] **Step 5.3: Commit**

---

## Task 6: MCP tool

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `crates/vox-orchestrator/src/mcp_tools/tools/scientia_tools.rs`
- Modify: `contracts/mcp/tool-registry.canonical.yaml`

- [ ] **Step 6.1: Input schema**

```json
{
  "type": "object",
  "properties": {
    "commit_window": {"type": "integer", "default": 100},
    "days_window": {"type": "integer", "default": 30},
    "candidate_class": {"type": "string"}
  }
}
```

- [ ] **Step 6.2: Handler**

Mirror an existing scientia MCP tool (e.g., `vox_scientia_publication_status`). Return JSON-rendered events.

- [ ] **Step 6.3: Registry**

Add to `tool-registry.canonical.yaml`:
```yaml
- name: vox_scientia_scout
  description: Survey workspace for publication candidates via Scientia signal producers.
```

- [ ] **Step 6.4: Parity check**

In `vox-vscode`: `pnpm run compile` (or at minimum `pnpm run generate:mcp-registry && pnpm run check:mcp-parity && pnpm run check:activation-parity`).

- [ ] **Step 6.5: Commit**

---

## Task 7: Documentation

- [ ] **Step 7.1: CLI reference**

In `docs/src/reference/cli.md`, add a `vox scientia scout` section with:
- Synopsis with all flags.
- One-shot example with sample table output.
- `--watch` example.

- [ ] **Step 7.2: How-to entry point**

In `docs/src/how-to/how-to-scientia-publication.md`, add a new "0. Start here" section at the top:

> **First time?** Run `vox scientia scout` in your Vox workspace. It surveys recent commits, benchmark history, and Socrates telemetry and prints candidate findings ranked by confidence — each row tells you the next command to run.

- [ ] **Step 7.3: Commit**

---

## Task 8: Final verification

- [ ] **Step 8.1:** `cargo test --workspace`
- [ ] **Step 8.2:** `cargo run -p vox-arch-check`
- [ ] **Step 8.3:** CLI parity tests pass.
- [ ] **Step 8.4:** MCP parity tests pass.
- [ ] **Step 8.5:** Run `vox scientia scout` against the live repo; verify it surfaces at least one real candidate (depends on Phase A producers having found anything yet — if not, that's diagnostic info, not a Phase F failure).

---

## Acceptance criteria

1. `vox scientia scout` returns a table (or JSON with `--output json`) with the documented columns.
2. `vox scientia scout --watch` runs the cycle on cadence; clean SIGINT exit.
3. New candidates are persisted to `scientia_finding_candidates`; duplicate insertions are silently skipped via the UNIQUE index.
4. `--candidate-class` filter works.
5. CLI parity tests pass; baseline updated.
6. MCP parity tests pass.
7. How-to doc points users at scout as the entry point.

---

## Open questions

- **OQ-F1.** OS notification crate — `notify-rust` if in tree; else punt to follow-up. Verified in pre-flight.
- **OQ-F2.** Confidence threshold for `--watch` notifications — default 0.7 is a guess; tune after live-data observation.
- **OQ-F3.** Cache. Default behavior writes to DB which is itself the cache; no separate `.vox/scout/last.json` file in Phase F.
- **OQ-F4.** Daemon vs one-shot. Foreground-only in Phase F.

---

## Dependencies

- **Upstream (hard):** Phase A — needs `ProducerRegistry::default_with_codex` and `scientia_finding_candidates` table.
- **Upstream (soft):** Phase E — venue column gracefully renders `—` when Phase E config is absent.
- **Downstream:** Phase H (dashboard panel) can read the same `scientia_finding_candidates` table.

---

## Cross-references

- Gap: [gap-map §2 Gap F](../../../src/architecture/scientia-self-publication-gap-map-2026.md#gap-f--vox-scientia-scout-single-command-surface)
- Phase A: [`2026-05-15-scientia-phase-A-signal-producers.md`](./2026-05-15-scientia-phase-A-signal-producers.md)
- Phase E: [`2026-05-15-scientia-phase-E-ai-swe-micro-track.md`](./2026-05-15-scientia-phase-E-ai-swe-micro-track.md)
- CLI mirror pattern: [`crates/vox-cli/src/commands/db/publication/prepare.rs`](../../../../crates/vox-cli/src/commands/db/publication/prepare.rs)
- Command registry: [`contracts/cli/command-registry.yaml`](../../../../contracts/cli/command-registry.yaml)
