# Crate Build-Time Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut the workspace's default debug-build cost by feature-gating CI-only / optional code out of default builds and shrinking the churn surface of the most-depended-on crate, without changing any runtime behavior of default builds.

**Architecture:** Each task targets ONE crate, makes a surgical Cargo `[features]` or module-relocation change, verifies the crate compiles in BOTH the gated-off and gated-on configurations, confirms the targeted heavy dependency / code is actually dropped from the default build, and re-measures. Tasks are independent and individually shippable — execute in order (highest confidence first) but any can be skipped.

**Tech Stack:** Rust (cargo, `--timings`, `cargo tree`), `cargo nextest`, the in-repo measurement tool `scripts/crate-build-audit.vox` (run via the `vox` CLI), and the `layers.toml` budget system enforced by `vox-arch-check`.

---

## Context for the implementer (read this first — you have zero prior context)

### The measurement harness you will use

A dependency + build-time audit tool already exists: **`scripts/crate-build-audit.vox`**. It reads `cargo metadata`, `docs/src/architecture/layers.toml`, and the latest `cargo build --timings` HTML, and writes:
- `graphify-out/crate_audit.json` — per-crate `{layer, loc, fan_in, fan_out, compile_s, max_loc, deps}`
- `graphify-out/CRATE_BUILD_AUDIT.md` — ranked tables + computed optimization targets

Refresh the dependency/LoC map (fast, no build):
```bash
vox run --mode interp scripts/crate-build-audit.vox
```
Refresh compile-time numbers too (cold rebuild, minutes):
```bash
VOX_AUDIT_BUILD=1 vox run --mode interp scripts/crate-build-audit.vox
```

For a **single-crate** before/after measurement (much faster than a full rebuild), use this pattern throughout the plan:
```bash
cargo clean -p <crate>
/usr/bin/env time -p cargo build -p <crate> 2>&1 | tail -1   # Linux/mac
# On Windows PowerShell: Measure-Command { cargo build -p <crate> }
```
To confirm a dependency is actually gone from a build config:
```bash
cargo tree -p <crate> -e normal | grep -i <dep>    # empty output = dep dropped
```

### The budget system

`docs/src/architecture/layers.toml` assigns each crate a `layer`, and optional `max_loc` / `max_dependents`. `vox-arch-check` warns when a crate exceeds budget. Run it any time with:
```bash
cargo run -p vox-arch-check --quiet
```

### Current measured baseline (cold build, 106 workspace crates, third-party cached)

| Crate | compile self-time | LoC | fan-in | Note |
|---|---|---|---|---|
| `vox-orchestrator-mcp` | 63.6s | 40,744 | 5 | over `max_loc=40,000` |
| `vox-cli` | 53.7s | ~90,279 | 4 | default member; over `max_loc=90,000` |
| `vox-audit` | 26.8s | 8,963 | 1 | 3.0 s/kLoC anomaly — 9 binary units |
| `vox-orchestrator` | 26.8s | 68,904 | 10 | |
| `vox-db` | 21.2s | 38,211 | **26** | #1 rebuild blast-radius |
| `vox-sql` | 7.7s | 2,395 | 3 | 3.2 s/kLoC — tri-backend |

### Two facts that override intuition (verified — do NOT re-litigate)

1. **A `vox-db-types` leaf crate ALREADY exists** (`crates/vox-db-types`, L0, deps: serde/serde_json/chrono only). `vox-db` already re-exports it. **No dependent can drop its `vox-db` dependency** — all 26 hold a live `VoxDb`/`Codex` handle. So the vox-db win is *narrow*: move pure-data types OUT of vox-db's churny modules into the existing leaf, so editing those types stops rebuilding 26 crates. Do not attempt to make dependents bypass vox-db.
2. **The `vox-cli-thin` / `OrchestratorClient` trait facade was already attempted and ABANDONED in the 2026-05-08 reorg Phase 7** ("large refactor work for limited additional payoff and is not pursued" — `docs/src/architecture/build-time-log.md`). **Do NOT revive it.** Per-command feature gates are the supported approach.

### Crates that are intrinsically expensive — do NOT chase these

Investigation confirmed these are already correctly feature-gated and their cost is intrinsic to a genuinely-needed dependency. **Spending effort here is wasted:**
- `vox-lsp` (6.3s) — `tower-lsp-server` + full `vox-compiler`; already has a `db` feature. Intrinsic.
- `vox-plugin-speech` (9.4s) — `candle-core`; already gated behind `stt-candle` (default) with `--no-default-features` dropping it entirely. Already correct.

---

## File Structure

No new files except one moved Rust module and the plan-time edits. Per task:

- **Task 1:** Modify `crates/vox-audit/Cargo.toml` (add `[features]`, add `required-features` to 8 `[[bin]]`). Modify any CI/script that runs a `cr-*` bin.
- **Task 2:** Modify `crates/vox-orchestrator-mcp/Cargo.toml` (default-feature list). Modify consumers that need news tools in default builds.
- **Task 3:** Modify `crates/vox-sql/Cargo.toml` (add `[features]`, make backends optional). Modify consumers selecting a non-default backend.
- **Task 4:** Create `crates/vox-db-types/src/retrieval.rs` (moved). Modify `crates/vox-db-types/src/lib.rs`, `crates/vox-db/src/lib.rs`, delete `crates/vox-db/src/retrieval.rs`.
- **Task 5 (larger):** Create a new sibling crate or fold `crates/vox-orchestrator-mcp/src/llm_bridge/` into the egress facade. Modify `layers.toml`.
- **Task 6 (optional, caveated):** Modify `crates/vox-cli/Cargo.toml`, `crates/vox-cli/src/commands/mod.rs`, CI workflows, and `lefthook.yml`.

---

## Task 1: vox-audit — gate the 8 CI-only `cr-*` gate binaries behind a `ci-gates` feature

**Why first:** Highest-confidence, lowest-risk win. `vox-audit` builds **9 binary compilation units**; 8 of them (`cr-e1`, `cr-a1`, `cr-d3`, `cr-a4`, `cr-a2`, `cr-e2`, `cr-p1`, `cr-p2`) are CI gate-runners never run during interactive development. Gating them out of the default build removes 8 of 9 codegen units from `cargo build -p vox-audit`.

**Files:**
- Modify: `crates/vox-audit/Cargo.toml`
- Modify: any CI workflow / script that invokes a `cr-*` bin (enumerate in Step 1)

- [ ] **Step 1: Baseline — measure and enumerate**

```bash
cargo clean -p vox-audit
cargo build -p vox-audit 2>&1 | tail -1     # note the wall time; this builds all 9 bins
# Enumerate every cr-* invocation that must keep working:
grep -rn "cr-e1\|cr-a1\|cr-d3\|cr-a4\|cr-a2\|cr-e2\|cr-p1\|cr-p2" .github/ scripts/ contracts/ 2>/dev/null
```
Expected: build succeeds; the grep lists every place a `cr-*` bin is run (criteria `verify_cmd` in `contracts/`, any workflow). Record these — each must build vox-audit with `--features ci-gates` after this change.

- [ ] **Step 2: Add the `ci-gates` feature and gate the 8 bins**

In `crates/vox-audit/Cargo.toml`, add a `[features]` table immediately after the `[dependencies]` block's end (before `[[bin]]`):
```toml
[features]
# CI-only gate-runner binaries (cr-*). They are invoked by CI criteria verify_cmds,
# never during interactive development, so they are excluded from the default build
# to cut 8 of 9 codegen units. CI builds with `--features ci-gates`.
default = []
ci-gates = []
```
Then add `required-features = ["ci-gates"]` to EACH of the 8 `cr-*` `[[bin]]` targets. For example, change:
```toml
[[bin]]
name = "cr-e1"
path = "src/bin/cr-e1.rs"
```
to:
```toml
[[bin]]
name = "cr-e1"
path = "src/bin/cr-e1.rs"
required-features = ["ci-gates"]
```
Do this for all eight: `cr-e1`, `cr-a1`, `cr-d3`, `cr-a4`, `cr-a2`, `cr-e2`, `cr-p1`, `cr-p2`. **Leave the `vox-audit` umbrella bin (`name = "vox-audit"`) ungated.**

- [ ] **Step 3: Verify the default build drops the 8 bins**

```bash
cargo clean -p vox-audit
cargo build -p vox-audit 2>&1 | tail -1     # should be noticeably faster
ls target/debug/cr-* 2>/dev/null            # expect: no such files (bins not built)
test -x target/debug/vox-audit && echo "umbrella bin OK"
```
Expected: faster wall time; `cr-*` bins absent; umbrella bin present.

- [ ] **Step 4: Verify the gated-on build still compiles all bins**

```bash
cargo build -p vox-audit --features ci-gates 2>&1 | tail -1
ls target/debug/cr-e1 target/debug/cr-p2 && echo "cr bins build under feature"
cargo nextest run -p vox-audit 2>&1 | tail -5
```
Expected: all `cr-*` bins build; tests pass.

- [ ] **Step 5: Update every `cr-*` invocation site found in Step 1 to pass `--features ci-gates`**

For each site from Step 1, ensure the build/run uses the feature. Example for a workflow step that runs a gate bin:
```yaml
# before
- run: cargo run -p vox-audit --bin cr-p1
# after
- run: cargo run -p vox-audit --features ci-gates --bin cr-p1
```
Also check `.github/workflows/cr-l8-corpus-feedback.yml:235` (`cargo build -p vox-audit -p vox-code-audit`): if any step downstream of it runs a `cr-*` bin, add `--features ci-gates` to that build. The umbrella subcommands (`cargo run -p vox-audit -- corpus-feedback`, `-- stdlib-coverage`) are UNAFFECTED — leave them as-is.

- [ ] **Step 6: Verify arch-check + commit**

```bash
cargo run -p vox-arch-check --quiet 2>&1 | tail -3
git add crates/vox-audit/Cargo.toml .github/ scripts/ contracts/
git commit -m "perf(vox-audit): gate 8 CI-only cr-* bins behind ci-gates feature

Cuts 8 of 9 binary codegen units from the default cargo build -p vox-audit.
CI gate-runners build with --features ci-gates; the umbrella vox-audit bin and
its subcommands are unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: vox-orchestrator-mcp — drop `news-publish` from the default features

**Why:** `news-publish` is already `#[cfg(feature = "news-publish")]`-gated over `scientia_tools/` (3,553 LoC) + `news_tools.rs` (450 LoC) and pulls the heavy `vox-publisher` dependency, yet it is in the `default` list so every default build pays for it. Fan-in is only 5 crates, so the blast radius of requiring opt-in is tiny.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml`
- Modify: consumers that need news tools in their default build (enumerate in Step 1)

- [ ] **Step 1: Baseline + find consumers that rely on news-publish by default**

```bash
cargo clean -p vox-orchestrator-mcp
cargo build -p vox-orchestrator-mcp 2>&1 | tail -1
cargo tree -p vox-orchestrator-mcp -e normal | grep -i vox-publisher   # present today
# Who depends on vox-orchestrator-mcp, and do they need scientia/news tools?
grep -rln "vox-orchestrator-mcp" crates/*/Cargo.toml
grep -rln "scientia_tools\|news_tools\|news-publish" crates/vox-gui/ crates/vox-orchestrator-d/ crates/vox-cli/ 2>/dev/null
```
Expected: `vox-publisher` present in tree; the greps reveal whether `vox-gui`, the daemon (`vox-orchestrator-d`), or `vox-cli` (via `mcp-server`) reference news/scientia tooling. Record any that do — they must add the feature in Step 3.

- [ ] **Step 2: Move `news-publish` out of `default`**

In `crates/vox-orchestrator-mcp/Cargo.toml`, change:
```toml
default = ["news-publish", "toestub-gate", "json-schema"]
```
to:
```toml
default = ["toestub-gate", "json-schema"]
```
Leave the `news-publish = [...]` feature definition itself unchanged.

- [ ] **Step 3: Re-add the feature for any consumer that needs it**

For each consumer found in Step 1 that needs news/scientia tools, add the feature to ITS dependency line. Example, if `vox-orchestrator-d` needs it, in `crates/vox-orchestrator-d/Cargo.toml`:
```toml
# before
vox-orchestrator-mcp = { workspace = true }
# after
vox-orchestrator-mcp = { workspace = true, features = ["news-publish"] }
```
If a consumer's own Cargo already has a `news-publish` feature that forwards, wire it through there instead. If NO consumer needs it in a default build (likely — it's a release/publish-time capability), skip this step.

- [ ] **Step 4: Verify both configs compile + tests pass**

```bash
cargo clean -p vox-orchestrator-mcp
cargo build -p vox-orchestrator-mcp 2>&1 | tail -1                    # faster; no vox-publisher
cargo tree -p vox-orchestrator-mcp -e normal | grep -i vox-publisher  # expect: empty
cargo check -p vox-orchestrator-mcp --features news-publish 2>&1 | tail -1
cargo nextest run -p vox-orchestrator-mcp 2>&1 | tail -5
# Downstream still builds (the CLI mcp-server path):
cargo check -p vox-cli --features mcp-server 2>&1 | tail -1
```
Expected: default build faster and `vox-publisher` gone from the tree; feature-on build compiles; tests pass; CLI still checks.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/Cargo.toml crates/*/Cargo.toml
git commit -m "perf(orchestrator-mcp): drop news-publish from default features

scientia_tools/ + news_tools.rs (~4k LoC) and the vox-publisher dep are already
cfg-gated behind news-publish; remove it from default so interactive builds skip
them. Consumers needing news tooling opt in explicitly.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: vox-sql — feature-gate the SQL backends (libSQL default; Postgres/MySQL opt-in)

**Why:** `vox-sql` unconditionally compiles `sqlx` with BOTH `postgres` and `mysql` drivers PLUS `turso` (libSQL) — three full wire-protocol/async DB stacks — for every build, though most consumers use one engine. The crate's own description ("Engine-agnostic … (libSQL, Postgres, MySQL)") makes the engine the natural feature axis.

**Files:**
- Modify: `crates/vox-sql/Cargo.toml`
- Modify: consumers selecting Postgres/MySQL (enumerate in Step 1)

- [ ] **Step 1: Baseline + find which backends consumers actually use**

```bash
cargo clean -p vox-sql
cargo build -p vox-sql 2>&1 | tail -1
grep -rln "vox-sql" crates/*/Cargo.toml          # the 3 dependents
grep -rn "postgres\|mysql\|Postgres\|MySql\|sqlx" crates/vox-sql/src/ | head
```
Expected: build time noted; you learn which dependents (3) use vox-sql and whether any select postgres/mysql at runtime (vs default libSQL).

- [ ] **Step 2: Make the backends optional + add a feature axis**

In `crates/vox-sql/Cargo.toml`, change the `turso` and `sqlx` lines to be optional and add a `[features]` table:
```toml
turso = { workspace = true, optional = true }
sqlx = { version = "0.9.0", default-features = false, features = ["runtime-tokio"], optional = true }
```
Add (after `[dependencies]`):
```toml
[features]
# One feature per SQL engine. Default to libSQL (turso); Postgres/MySQL pull their
# own sqlx wire-protocol driver only when explicitly enabled.
default = ["libsql"]
libsql = ["dep:turso"]
postgres = ["dep:sqlx", "sqlx/postgres"]
mysql = ["dep:sqlx", "sqlx/mysql"]
```
Then guard the backend-specific code with `#[cfg(feature = "...")]`. Find the Postgres/MySQL impls (from Step 1's grep, e.g. in `crates/vox-sql/src/lib.rs`, `ddl.rs`, `type_map.rs`) and gate each `sqlx`-using item:
```rust
#[cfg(feature = "postgres")]
mod postgres_backend;     // or #[cfg(any(feature = "postgres", feature = "mysql"))] on shared sqlx code
```
Gate the `turso`-using libSQL code behind `#[cfg(feature = "libsql")]`. (The exact items are revealed by `cargo check` errors in Step 3 — fix each "unresolved import `sqlx`/`turso`" by adding the matching `#[cfg(feature = ...)]`.)

- [ ] **Step 3: Verify each backend config compiles**

```bash
cargo check -p vox-sql 2>&1 | tail -3                                   # default = libsql only
cargo tree -p vox-sql -e normal | grep -i "sqlx" && echo "UNEXPECTED: sqlx in default" || echo "sqlx dropped from default OK"
cargo check -p vox-sql --no-default-features --features postgres 2>&1 | tail -3
cargo check -p vox-sql --no-default-features --features mysql 2>&1 | tail -3
cargo check -p vox-sql --features "postgres,mysql" 2>&1 | tail -3       # all engines (CI/parity)
cargo nextest run -p vox-sql --features "postgres,mysql" 2>&1 | tail -5
```
Expected: default build excludes `sqlx` from the tree; each single-backend config compiles; the all-engines config + tests pass.

- [ ] **Step 4: Re-add backends for any consumer that needs them**

For each dependent from Step 1 that uses Postgres/MySQL, add the feature to its `vox-sql` dependency line, e.g.:
```toml
vox-sql = { workspace = true, features = ["postgres", "mysql"] }
```
Then `cargo check -p <that-consumer>` to confirm. If all 3 dependents only use libSQL, skip.

- [ ] **Step 5: Commit**

```bash
cargo run -p vox-arch-check --quiet 2>&1 | tail -3
git add crates/vox-sql/Cargo.toml crates/vox-sql/src/ crates/*/Cargo.toml
git commit -m "perf(vox-sql): feature-gate SQL backends (libsql default, postgres/mysql opt-in)

Default builds no longer compile the sqlx postgres+mysql wire drivers; consumers
select engines via features. CI parity builds with --features postgres,mysql.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: vox-db — move the pure `retrieval` contracts into the existing `vox-db-types` leaf

**Why:** `crates/vox-db/src/retrieval.rs` is pure-data (imports only `serde`; zero `turso`, zero `async fn` — verified) but lives in the heavy `vox-db` crate, so editing retrieval types triggers rebuilds of all 26 `vox-db` dependents. Moving it to the existing L0 `vox-db-types` leaf and re-exporting it from `vox-db` shrinks the churn surface (the 4 retrieval consumers — `vox-cli`, `vox-orchestrator`, `vox-orchestrator-mcp`, `vox-search` — rebuild only on leaf changes, and the leaf is tiny/stable). No dependent import changes are needed because `vox-db` keeps re-exporting the symbols.

**Files:**
- Create: `crates/vox-db-types/src/retrieval.rs` (moved content)
- Modify: `crates/vox-db-types/src/lib.rs` (add `pub mod retrieval;`)
- Modify: `crates/vox-db/src/lib.rs` (replace `pub mod retrieval;` with a re-export of `vox_db_types::retrieval`)
- Delete: `crates/vox-db/src/retrieval.rs`

- [ ] **Step 1: Baseline — confirm purity and capture the re-export list**

```bash
grep -c "async fn\|turso\|Connection" crates/vox-db/src/retrieval.rs     # expect: 0
sed -n '189,200p' crates/vox-db/src/lib.rs                                # the `pub use retrieval::{...}` block — copy its symbol list
```
Expected: 0 matches (pure). Record the exact symbol list re-exported at `lib.rs:189` (e.g. `SearchBackend, SearchCorpus, SearchIntent, SearchPlan, SearchDiagnostics, RetrievalMode, RetrievalQuery, RetrievalResult, RetrievalEvidenceSource, SearchRefinementAction, heuristic_search_plan, fuse_hybrid_results`).

- [ ] **Step 2: Move the file into the leaf**

```bash
git mv crates/vox-db/src/retrieval.rs crates/vox-db-types/src/retrieval.rs
```
In `crates/vox-db-types/src/lib.rs`, add alongside the other `pub mod` lines (e.g. after `pub mod research;`):
```rust
/// Hybrid retrieval planning types + rank fusion (pure contracts; no DB runtime).
pub mod retrieval;
```

- [ ] **Step 3: Replace vox-db's module with a re-export**

In `crates/vox-db/src/lib.rs`, replace line 132 (`pub mod retrieval;`) with:
```rust
/// Hybrid retrieval helpers — re-exported from the pure-data `vox-db-types` leaf
/// so edits to these contracts no longer rebuild every vox-db dependent.
pub use vox_db_types::retrieval;
```
Leave the existing `pub use retrieval::{ ... };` block (around line 189) UNCHANGED — it now re-exports from the leaf transparently, so downstream `vox_db::SearchPlan` etc. keep resolving.

- [ ] **Step 4: Verify the workspace compiles and the 4 consumers are unchanged**

```bash
cargo check -p vox-db-types 2>&1 | tail -3
cargo check -p vox-db 2>&1 | tail -3
for c in vox-cli vox-orchestrator vox-orchestrator-mcp vox-search; do
  cargo check -p "$c" 2>&1 | tail -1
done
cargo nextest run -p vox-db -p vox-db-types 2>&1 | tail -5
```
Expected: all compile with NO edits to the consumers; tests pass. If `vox-db/src/retrieval.rs` had referenced any non-leaf `vox-db` type, `cargo check -p vox-db-types` will error — in that case, also move that referenced type into the leaf (it must itself be pure-data) or revert and report.

- [ ] **Step 5: Confirm the churn-surface win**

```bash
# Editing a retrieval type now recompiles vox-db-types + its dependents, NOT all of vox-db.
touch crates/vox-db-types/src/retrieval.rs
cargo build -p vox-search --timings 2>&1 | tail -1
# In target/cargo-timings/cargo-timing.html, confirm vox-db itself did not recompile from this edit
# (only vox-db-types + downstream). Compare against: touch crates/vox-db/src/lib.rs (rebuilds vox-db, 21s).
```
Expected: a retrieval-type edit no longer forces vox-db's 21s recompile.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-db/src/lib.rs crates/vox-db-types/
git commit -m "perf(vox-db): move pure retrieval contracts into vox-db-types leaf

retrieval.rs is serde-only (no turso/async); relocating it to the L0 leaf and
re-exporting from vox-db means editing search-plan/fusion types stops rebuilding
all 26 vox-db dependents. Downstream imports (vox_db::SearchPlan, ...) unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5 (larger refactor): vox-orchestrator-mcp — extract `llm_bridge/` to fix the LoC-budget overrun

**Why:** `vox-orchestrator-mcp` is 40,744 LoC, over its `max_loc = 40,000` budget. `src/llm_bridge/` (4,199 LoC) is the largest single cohesive subsystem and is provider-egress logic that architecturally belongs behind the LLM egress boundary anyway (project rule: all LLM call sites go through `vox_actor_runtime::llm`). Extracting it both fixes the budget and creates a parallel compile unit.

**This is a multi-hour refactor.** Do it only after Tasks 1–4. Treat it as its own mini-plan:

**Files:**
- Create: `crates/vox-mcp-llm-bridge/` (new crate) OR move into an existing egress crate
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`, `crates/vox-orchestrator-mcp/src/dispatch.rs`, `crates/vox-orchestrator-mcp/Cargo.toml`
- Modify: `docs/src/architecture/layers.toml` (register the new crate) and `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Map the seam**

```bash
ls -R crates/vox-orchestrator-mcp/src/llm_bridge/
grep -rn "llm_bridge" crates/vox-orchestrator-mcp/src/ | grep -v "src/llm_bridge/"   # all external references
```
Expected: the list of every `llm_bridge::` reference from outside the module (mostly `dispatch.rs`). These become the new crate's public API surface — keep it minimal.

- [ ] **Step 2: Decide the destination (checkpoint with a human)**

Either (a) a new `crates/vox-mcp-llm-bridge` crate at the appropriate layer, or (b) fold into `vox-actor-runtime::llm` if the bridge is genuinely generic egress. Inspect what `llm_bridge/` imports from its parent crate — if it reaches into orchestrator-mcp internals heavily, (a) is safer. **Pause and confirm the destination before moving code.**

- [ ] **Step 3: Move + wire**

`git mv` the module into the chosen crate, add the crate to the workspace `members` (already `crates/*`), define its `Cargo.toml` (deps it actually uses), and replace the references in `vox-orchestrator-mcp` with `use vox_mcp_llm_bridge::...`. Add the dependency to `crates/vox-orchestrator-mcp/Cargo.toml`.

- [ ] **Step 4: Register in the SSOTs**

Add the new crate to `docs/src/architecture/layers.toml` with its `layer` (and `kind`/`max_loc` if needed) and a row in `docs/src/architecture/where-things-live.md`. Both are `error`-level arch-check rules — a missing entry fails the gate.

- [ ] **Step 5: Verify**

```bash
cargo check -p vox-orchestrator-mcp 2>&1 | tail -3
cargo check -p vox-mcp-llm-bridge 2>&1 | tail -3
cargo nextest run -p vox-orchestrator-mcp -p vox-mcp-llm-bridge 2>&1 | tail -5
cargo run -p vox-arch-check --quiet 2>&1 | tail -5    # expect: no max_loc warning on vox-orchestrator-mcp
```
Expected: both crates compile; tests pass; arch-check no longer flags the LoC budget.

- [ ] **Step 6: Commit**

```bash
git add crates/ docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "refactor(orchestrator-mcp): extract llm_bridge into its own crate

Moves the 4.2k-LoC provider-egress subsystem out of vox-orchestrator-mcp, putting
it back under max_loc=40000 and creating a parallel compile unit. Egress logic now
lives behind a dedicated crate boundary.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6 (OPTIONAL — read the caveat): vox-cli — gate `commands/ci/` behind a `cli-ci` feature

**Why it's tempting:** `commands/ci/` is 29,575 LoC (~33% of vox-cli) and is ungated, so every default `cargo build -p vox-cli` (the rebuild every dev pays) compiles it.

**CAVEAT — confirm before doing this:** The local pre-push hook delegates to `vox ci pre-push` (`lefthook.yml`), so **any developer who runs git hooks needs `vox ci` built locally** and would have to build vox-cli `--features cli-ci` anyway — at which point they pay the cost. The benefit accrues only to devs who never invoke `vox ci` locally. This is a weaker win than Tasks 1–5 and adds cross-cutting feature plumbing (CI + hooks + dispatch). **Do this only if profiling confirms a meaningful default-build win for the common workflow, and after checkpointing with a human.**

**Files:**
- Modify: `crates/vox-cli/Cargo.toml`, `crates/vox-cli/src/commands/mod.rs` (module decl + dispatch arm)
- Modify: every CI workflow building/running `vox ci`, and `lefthook.yml`

- [ ] **Step 1: Baseline + enumerate all `vox ci` build/run sites**

```bash
cargo clean -p vox-cli && cargo build -p vox-cli --features completion-toestub 2>&1 | tail -1
grep -rn "vox ci \|ci pre-push\|-p vox-cli" .github/workflows/ lefthook.yml scripts/ | grep -i "ci" | head -40
```
Expected: baseline time; the full list of places that need `--features cli-ci`.

- [ ] **Step 2: Add the feature and gate the module**

In `crates/vox-cli/Cargo.toml [features]`, add `cli-ci = []` and decide whether to include it in `default` (recommended: NOT default, so the win is realized). In `crates/vox-cli/src/commands/mod.rs`, gate the `ci` module declaration and its dispatch arm:
```rust
#[cfg(feature = "cli-ci")]
pub mod ci;
```
and in the command dispatch match, gate the `Commands::Ci(...)` arm with `#[cfg(feature = "cli-ci")]` (and add a `#[cfg(not(feature = "cli-ci"))]` arm that prints a clear "rebuild with --features cli-ci" error, so a default binary fails gracefully rather than silently).

- [ ] **Step 3: Verify both configs**

```bash
cargo build -p vox-cli 2>&1 | tail -1                                   # default: no ci/, faster
cargo build -p vox-cli --features cli-ci 2>&1 | tail -1                 # ci/ present
target/debug/vox ci pre-push 2>&1 | head -3 || true                     # default binary: graceful error
cargo nextest run -p vox-cli --features cli-ci 2>&1 | tail -5
```
Expected: default build faster and lacks `vox ci`; feature build has it; tests pass.

- [ ] **Step 4: Add `cli-ci` to every CI + hook site from Step 1**

Update each workflow that runs `vox ci <gate>` to build with `--features cli-ci` (the `setup` job's artifact build at `.github/workflows/ci.yml:90` is the key one — change `--features completion-toestub` to `--features completion-toestub,cli-ci`). Update `lefthook.yml` pre-push commands that need `vox ci`. **Miss one and CI/hooks break** — this is why the enumeration in Step 1 must be exhaustive.

- [ ] **Step 5: Verify arch-check + commit**

```bash
cargo run -p vox-arch-check --quiet 2>&1 | tail -3
git add crates/vox-cli/ .github/ lefthook.yml
git commit -m "perf(vox-cli): gate commands/ci behind cli-ci feature

ci/ (~29.5k LoC, ~33% of the crate) is CI-only; gating it out of default builds
speeds the common dev rebuild. CI and pre-push hooks build with --features cli-ci.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Final verification (after all chosen tasks)

- [ ] **Re-measure the whole workspace and compare to baseline**

```bash
VOX_AUDIT_BUILD=1 vox run --mode interp scripts/crate-build-audit.vox
cat graphify-out/CRATE_BUILD_AUDIT.md       # compare compile_s + over-budget list vs the baseline table above
cargo run -p vox-arch-check --quiet         # expect: fewer/no max_loc warnings
```
Expected outcomes:
- `vox-audit` default build drops ~8 codegen units (Task 1).
- `vox-orchestrator-mcp` default build drops `vox-publisher` + ~4k LoC (Task 2), and (if Task 5 done) is back under `max_loc`.
- `vox-sql` default build drops the `sqlx` postgres+mysql drivers (Task 3).
- A retrieval-type edit no longer rebuilds 26 crates (Task 4).
- `vox ci`/`vox audit cr-*` still work in CI with their features enabled.

- [ ] **Confirm no behavior change in default workflows**

```bash
cargo nextest run --workspace --exclude vox-gui 2>&1 | tail -10
```
Expected: full suite green. Feature-gated code is still exercised by tests that enable the features.

---

## Self-Review notes (already applied)

- **Spec coverage:** Tasks cover the four highest-ROI hotspots (vox-audit, vox-orchestrator-mcp, vox-sql, vox-db) + the budget-overrun refactor (Task 5) + the caveated vox-cli option (Task 6). `vox-lsp`/`vox-plugin-speech` are explicitly out of scope (intrinsic — documented above) so the executor doesn't waste effort.
- **No placeholders:** every Cargo/code change shows the exact before/after; every verification has an exact command + expected output. Where a set of sites must be found (cr-* invocations, mcp/sql consumers, vox ci sites), the exact `grep` and the handling rule are given rather than a vague "find and update."
- **Type/name consistency:** feature names (`ci-gates`, `news-publish`, `libsql`/`postgres`/`mysql`, `cli-ci`) and crate/symbol names match across tasks and the `cargo` invocations that consume them.
