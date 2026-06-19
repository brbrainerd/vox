# Centralized Vox Telemetry — Program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in centralized telemetry pipeline (compile-out plugin client → OTLP → ClickHouse server in a separate private repo), migrate the full existing `vox-telemetry` package onto it, and audit the whole codebase for what to collect — privacy-first, build-lean.

**Architecture:** Keep the zero-dep `vox-telemetry` L1 facade; add one feature-gated L3 exporter crate (`vox-telemetry-otlp`) that implements `TelemetryRecorder`, redacts at egress, and ships events as OTLP LogRecords. A separate private repo (`vox-telemetry-server`) ingests OTLP into ClickHouse and surfaces dashboards. Consent is two-tier (local opt-out / remote opt-in). Spec SSOT: `docs/superpowers/specs/2026-06-19-centralized-telemetry-design.md`.

**Tech Stack:** Rust (workspace + new server repo), `opentelemetry`/`opentelemetry-otlp`/`opentelemetry_sdk` `0.32.x`, `tracing-opentelemetry` (pin compatible w/ 0.32 via `cargo add`), `reqwest`, `governor` (client backoff), `axum` + `clickhouse` `0.13.3` (server), ClickHouse, Grafana/Metabase, Docker/Coolify (FableForge) or Hetzner.

---

## Shared Conventions (read once, apply to every task)

**Execution model (Sonnet 4.6):** Tracks marked `[PARALLEL-SAFE]` may be dispatched to
concurrent subagents; `[SEQUENTIAL]` tracks have a hard dependency noted inline. Within a
track, execute tasks in order (TDD: red → green → commit). Use the dispatching-parallel-agents
skill to fan out independent tracks; review between tasks per subagent-driven-development.

**Track dependency DAG:**
```
Track A (Audit & Foundations)  ──┬──► Track B (Client exporter)  ─┐
  [SEQUENTIAL — runs first]       │                               ├─► Track E (Migration & wiring)  [SEQUENTIAL, last]
                                  └──► Track C (Server repo)  ──► Track D (Deploy infra)  ─┘
                                       [PARALLEL-SAFE w/ B]      [SEQUENTIAL after C]

Track F (Model-Layer: learned per-model prompt)  [soft-depends on A taxonomy + E `model_prompt`
  category; PARALLEL-SAFE with B/C/D — touches the orchestrator/model subsystem, not the egress path]
```
- **A is the gate.** It produces the **taxonomy SSOT** (`contracts/telemetry/collection-taxonomy.v1.json`) and the **infra-audit report**, which B, C, and D all consume. Do not start B/C until A's SSOT is committed.
- **B and C run in parallel** (different surfaces: workspace client vs separate server repo).
- **D** deploys what C builds (needs C). **E** wires emit sites + flips migration on (needs A+B; soft-needs D for the live end-to-end test).

**Version pinning (verify-then-pin, June 2026 floors):** In Track A you will check crates.io /
`cargo add --dry-run` and record EXACT resolved versions in
`contracts/telemetry/pinned-versions.md`. Floors: `opentelemetry*` ≥ `0.32.0`, `clickhouse` =
`0.13.3`, `governor` (workspace already pins `0.10` — reuse it). **WARNING — the otel bump is
NOT a free additive pin.** The workspace root `Cargo.toml` already pins `opentelemetry* = 0.29`
/ `tracing-opentelemetry = 0.30`, and those are **already consumed** by the existing
tracing/span stack (`vox-foundation` `otel` feature et al.). Going 0.29→0.32 crosses multiple
breaking otel releases and is a **repo-wide breaking migration**, not a new dependency. Task A4
MUST decide the scope (see A4 Step 4). **Resolved by the v2 architecture correction:** the **client no longer depends on the
`opentelemetry` SDK at all** — it hand-encodes the OTLP/HTTP logs JSON envelope with
`serde`+`reqwest` (see Track B correction box). This SIDESTEPS the entire bump: the workspace
`opentelemetry* = 0.29` pins (traces-only, consumed by `vox-foundation`'s `otel` feature) are
**left untouched**, and no `logs` feature is added to any shared dep (which would have been a
workspace-global feature-unification change, NOT the "additive" no-op the first draft claimed).
The `clickhouse 0.13.3` + `opentelemetry-proto` pins live ONLY in the separate server repo.
A4 just records the final client pins (`reqwest`/`governor`, already in the workspace) and
confirms no client otel dep was introduced. A workspace-wide 0.29→0.32 bump, if ever wanted for
the traces stack, is an unrelated follow-up out of scope here.

**Crate hygiene (non-negotiable):**
- New deps go in the **workspace root `[workspace.dependencies]`** then referenced
  `{ workspace = true }`. Run `cargo hakari generate` after adding (workspace-hack parity).
- **Never** add `opentelemetry*`/`reqwest` to `vox-telemetry` (L1 facade) or any emitter.
  They live ONLY in `vox-telemetry-otlp` (L3).
- Register `vox-telemetry-otlp` in `docs/src/architecture/layers.toml` AND
  `docs/src/architecture/where-things-live.md` in the SAME commit it's created (Rule 12 parity is `error`).
- Run `cargo run -p vox-arch-check` before every commit that adds a crate/dep.

**Commit & ledger protocol:** Commit per TDD step as the skill dictates. At the **end of each
Track**, append one ledger entry to `docs/superpowers/antigravity-handoff-ledger.md` using the
existing `AGH-XXXX` format (next sequential id), recording: track id, commits, test counts,
follow-ups, and the verification command output. This is the "automatic ledger update" — it is
a required final task in every track below (Task *.LEDGER).

**Build/test commands:** touched-crate clippy `cargo clippy -p <crate> -- -D warnings`;
format `cargo fmt -p <crate>` (NEVER `--all`); arch `cargo run -p vox-arch-check`.

---

## Track A — Audit & Foundations  `[SEQUENTIAL — gate for all others]`

Produces: (1) the codebase **emit-site inventory**, (2) the **collection taxonomy SSOT**,
(3) the **infra-audit report**, (4) **pinned versions**. No product code ships in Track A
except the SSOT contract files + the codegen that reads them.

### Task A1: Inventory existing telemetry emit sites (parallel subagents)

**Files:**
- Create: `contracts/telemetry/emit-site-inventory.csv`
- Create: `contracts/telemetry/INVENTORY_METHOD.md`

- [ ] **Step 1: Dispatch the inventory sweep.** Using dispatching-parallel-agents, fan out
  read-only Explore subagents over `crates/` in ~8-crate clusters (respect the timeout: scope
  each agent to a crate subset, not the whole tree). Each agent returns, for its cluster, every
  call to `record_event!`, `record_task_started`, `fill_task_root_summary`, and every
  `TelemetryEvent::*` construction, as rows: `crate,file,line,event_type,category,fields_used`.
- [ ] **Step 2: Merge** agent outputs into `emit-site-inventory.csv` (dedupe by file:line).
  Document the cluster list + method in `INVENTORY_METHOD.md` so it is reproducible.
- [ ] **Step 3: Cross-check** against `crates/vox-telemetry/src/types.rs` `METRIC_TYPE_*` /
  event structs — every event type must appear in the inventory or be marked `unused`.
- [ ] **Step 4: Commit.**
```bash
git add contracts/telemetry/emit-site-inventory.csv contracts/telemetry/INVENTORY_METHOD.md
git commit -m "docs(telemetry): inventory existing emit sites across workspace"
```

### Task A2: Identify NEW collection sites for product categories

**Files:**
- Modify: `contracts/telemetry/emit-site-inventory.csv` (append proposed rows, `status=proposed`)

- [ ] **Step 1: Locate command dispatch.** Read `crates/vox-cli/src/cli_dispatch/mod.rs` and
  `crates/vox-cli/src/commands/` to find the single command-dispatch chokepoint (where verb +
  subcommand are known). Record the file:line as the `command_usage` emit site.
- [ ] **Step 2: Locate skill activation.** Grep for the skill-invocation path (skill registry /
  `SkillManifest` consumers). Record the `skill_activation` emit site.
- [ ] **Step 3: Locate edit application.** Find where file edits/patches are applied by the
  agent runtime. Record the `edit_pattern` emit site (op type + file-kind only).
- [ ] **Step 4: Locate harness tool-call dispatch** (orchestrator-mcp tool dispatch). Record
  the `harness_usage` emit site.
- [ ] **Step 5: Append** all proposed sites to the inventory with `status=proposed` + the
  allowlisted field set from spec §5. **Commit.**
```bash
git add contracts/telemetry/emit-site-inventory.csv
git commit -m "docs(telemetry): propose new product-category emit sites"
```

### Task A2b: Inventory `default_decision` sites (the "sensible defaults" surface)

**Files:**
- Create: `contracts/telemetry/default-decision-sites.csv`

- [ ] **Step 1: Confirm the audited anchors.** Verify each of these tunable-default sites still
  exists (line numbers may drift — confirm by content) and record
  `decision_id,crate,file:line,default_value,chosen_enum,outcome_enum`:
  - `vox-orchestrator` `src/budget/mod.rs` — cost threshold (~50_000 µ$), drift threshold (~0.5),
    doom-loop threshold (~2.0), fallback token cap (~100_000), alert ratio (~0.8);
  - `vox-config` `src/config/vox_config.rs` — LLM `max_concurrent` (8), retry max (4);
  - `vox-orchestrator-mcp` `src/llm_bridge/limits.rs` — output-token cap (8192), probe cache TTL
    (30s), probe timeout (2s/1s);
  - `vox-effort-audit` `src/config.rs` — max concurrent audit jobs (4);
  - `vox-audit` `src/panel.rs` — client backoff (base 30s / max 600s).
- [ ] **Step 2:** For each, define the **bucketed enum** for the chosen value and the **outcome
  enum** (e.g. `{hit_limit, near_limit, comfortable}`) — all enum/bucket, never raw numbers that
  could fingerprint. Append to `default-decision-sites.csv` with `status=proposed`. **Commit.**
```bash
git add contracts/telemetry/default-decision-sites.csv
git commit -m "docs(telemetry): inventory default_decision tuning sites"
```

### Task A3: Author the collection-taxonomy SSOT

**Files:**
- Create: `contracts/telemetry/collection-taxonomy.v1.json`
- Create: `contracts/telemetry/SCHEMA.md`
- Test: `crates/vox-telemetry/tests/taxonomy_ssot_parity.rs`

- [ ] **Step 1: Write the SSOT.** One JSON object: `{ "version": 1, "k_anonymity": 20,
  "categories": [ { "name": "command_usage", "privacy_tier": "low", "fields": [ {"name":"verb",
  "type":"enum","allowed":[...]}, {"name":"duration_bucket","type":"enum","allowed":["lt1s",...]} ], "otlp_event_name":"vox.command", "upload_default": true } , ... ] }`.
  Populate every category from spec §5; every field carries `type` ∈ {enum,int,bool,hash} and
  enums carry an explicit `allowed` list (this list IS the redaction allowlist).
- [ ] **Step 2: Write the failing parity test** asserting (a) the JSON parses, (b) every
  category in spec §5 is present, (c) no field has type `string`/`free` (privacy invariant #2):
```rust
#[test]
fn taxonomy_has_no_freeform_string_fields() {
    let txt = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/telemetry/collection-taxonomy.v1.json")
    ).unwrap();
    let v: serde_json::Value = serde_json::from_str(&txt).unwrap();
    for cat in v["categories"].as_array().unwrap() {
        for f in cat["fields"].as_array().unwrap() {
            let ty = f["type"].as_str().unwrap();
            assert!(matches!(ty, "enum"|"int"|"bool"|"hash"),
                "field {} in {} is free-form '{}' — privacy invariant #2 violated",
                f["name"], cat["name"], ty);
        }
    }
}
```
- [ ] **Step 3: Run** `cargo test -p vox-telemetry taxonomy_has_no_freeform_string_fields` → PASS.
- [ ] **Step 4:** Write `SCHEMA.md` (human description of each category/field + why each is privacy-safe). **Commit.**
```bash
git add contracts/telemetry/collection-taxonomy.v1.json contracts/telemetry/SCHEMA.md crates/vox-telemetry/tests/taxonomy_ssot_parity.rs
git commit -m "feat(telemetry): collection-taxonomy v1 SSOT + privacy parity test"
```

### Task A4: Infra audit (FableForge / Coolify / existing Vox project)

**Files:**
- Create: `docs/superpowers/specs/2026-06-19-telemetry-infra-audit.md`

- [ ] **Step 1: Inventory in-repo infra.** Read `infra/docker-compose.yml`, `infra/README.md`,
  `infra/coolify/` (if present), `docs/src/reference/deployment-compose.md`,
  `docs/src/reference/cli-vox-deploy.md`, and the `vox deploy` command source. Record: what
  Coolify project(s) exist, what services are defined, whether a Vox project already exists on
  FableForge, and how `vox deploy` targets it. NOTE: `vox-eval.compose.yml` at repo root is an
  **eval harness, not deployment infra** — do not treat it as the telemetry topology; there is
  no ClickHouse service today, you are adding one.
- [ ] **Step 2: Decide ingest topology.** In the audit doc, choose between **(a) OTel Collector
  + ClickHouse exporter** (less code, standard) vs **(b) axum + `clickhouse` crate** (typed,
  fewer moving parts). Record the decision + rationale. Default recommendation: (b) for a thin,
  auditable MVP; (a) if the Collector is already deployed.
- [ ] **Step 3: Decide hosting.** FableForge-Coolify ClickHouse container (reuse existing
  project if found) vs Hetzner self-managed. Record decision + the concrete target.
- [ ] **Step 4: Pin versions + scope the otel decision.** Enumerate existing `opentelemetry*`
  consumers: `grep -rn "opentelemetry" --include=Cargo.toml crates/` (known: `vox-foundation`'s
  `otel` feature). Decide and RECORD in `pinned-versions.md`: (a) keep `vox-telemetry-otlp` on
  the shared **0.29** workspace pin (recommended MVP — additive, no migration), OR (b) bump the
  workspace pin 0.29→0.32 (only if the enumerated consumer set is small and you budget the
  API-break fixes). Do NOT introduce a *second* otel major version in the tree (duplicate-dep
  bloat + workspace-hack churn). Record EXACT resolved `clickhouse`/`governor` versions too.
  **Commit** both files.
```bash
git add docs/superpowers/specs/2026-06-19-telemetry-infra-audit.md contracts/telemetry/pinned-versions.md
git commit -m "docs(telemetry): infra audit + pinned dependency versions"
```

### Task A.LEDGER: Append Track A ledger entry
- [ ] Append `AGH-XXXX` to `docs/superpowers/antigravity-handoff-ledger.md` summarizing Track A
  (files created, taxonomy version, infra decision, pinned versions, follow-ups). Commit.

---

## Track B — Client exporter `vox-telemetry-otlp`  `[depends on A; PARALLEL-SAFE with C]`

> ### ⚠ ARCHITECTURE CORRECTION (plan-review v2 — read before any B task)
> The first draft assumed a *live* OTLP network export inside the sync `record()` path. That is
> wrong and would panic. Ground truth (verified):
> - `TelemetryRecorder::record(&self, &TelemetryEvent)` is **sync**, on the caller's hot path
>   (`crates/vox-telemetry/src/recorder.rs:9`). Network in `record()` = `tokio::spawn` panic
>   outside a runtime (the existing `SpoolSink` guards this with `Handle::try_current()`,
>   `crates/vox-cli/src/telemetry_sink.rs:35`).
> - **A real spool already exists** — but in **`vox-cli`** (`crates/vox-cli/src/telemetry_spool.rs`:
>   `enqueue` / `list_pending` / `read_payload` / `ack` / async `upload_pending`), NOT in
>   `vox-telemetry`. The exporter crate is forbidden from depending up into `vox-cli`.
> - `set_global_recorder` is **first-write-wins / irreversible** (`recorder.rs:13`); `vox-cli`
>   already calls it once at startup in `init_telemetry_sinks` (`crates/vox-cli/src/lib.rs:~673-711`,
>   building a `CompositeRecorder`). A *second* `set_global_recorder` to "add the remote leg" is a
>   **silent no-op**.
> - `TelemetryEvent` is **internally-tagged** (`#[serde(tag="event_type")]`) and
>   **`#[non_exhaustive]`** (`crates/vox-telemetry/src/types.rs:272-273`) — serialization is a
>   FLAT `{"event_type":"research_metric",...}`, and any external `match` needs a `_ => None` arm.
>
> **Corrected design (redact-before-spool; no live network in `record()`):**
> 1. `vox-telemetry-otlp` splits into a **pure core** (always compiles: `project` + `redact` +
>    OTLP-logs-JSON encoder — serde only, ZERO otel/reqwest) and a **feature-gated `remote`
>    uploader** (`reqwest`-based POST of OTLP/HTTP logs JSON; the compile-out unit).
> 2. The **redaction happens at enqueue**: `vox-cli`'s `SpoolSink` calls the pure
>    `project_event`→`redact_event` BEFORE writing, so the on-disk spool only ever holds
>    **clean, OTLP-shaped** records (secrets never hit disk). `record()` stays pure+sync (no
>    network), reusing the existing `Handle::try_current()` disk-write guard.
> 3. The **uploader** (feature `remote`) reads pending clean records and POSTs OTLP/HTTP logs to
>    the central endpoint — async, off the hot path, in `vox telemetry upload` + an optional
>    periodic flush. Extends the existing `upload_pending` pattern.
> 4. The remote sink is **always in the composite, gated INTERNALLY** per-`record()` by
>    `is_remote_allowed()` — never conditionally registered (which can't work; #8 above). Adding
>    the sink = pushing it into the `sinks` Vec inside the single `init_telemetry_sinks`
>    construction, not a second `set_global_recorder`.
> 5. **No `opentelemetry` SDK on the client.** We hand-encode the OTLP/HTTP **logs** JSON
>    envelope (stable schema) with `serde` + `reqwest` — this sidesteps the missing `logs`
>    feature on the workspace `opentelemetry-otlp`/`_sdk` pins (which are traces-only today) and
>    its workspace-global feature-unification blast radius. A4 confirms; the server still speaks
>    standard OTLP so the backend stays swappable.
>
> **Task order is corrected to: B3 (consent+salt+master-switch) → B2 (project/redact, needs the
> salt) → B1 (crate scaffold can come first or here) → B4 (SpoolSink integration) → B5 (uploader
> + CLI).** The headings below keep their numbers; follow the dependency order in this box.

### Task B1: Scaffold the feature-gated L3 crate

**Files:**
- Create: `crates/vox-telemetry-otlp/Cargo.toml`
- Create: `crates/vox-telemetry-otlp/src/lib.rs`
- Modify: `Cargo.toml` (root: add to `members` + `[workspace.dependencies]` for `governor`)
- Modify: `docs/src/architecture/layers.toml`
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Create `Cargo.toml`** (heavy deps optional behind `remote`):
```toml
[package]
name = "vox-telemetry-otlp"
description = "Feature-gated OTLP exporter for vox-telemetry: redacts at egress and ships events as OTLP LogRecords. Compiles to a no-op shim unless the `remote` feature is on."
version.workspace = true
edition.workspace = true
license.workspace = true

[features]
default = []
# Pure core (project/redact/otlp_json) ALWAYS compiles — serde only, zero network.
# `remote` adds ONLY the async uploader (the compile-out unit). We hand-encode the OTLP/HTTP
# logs JSON envelope, so NO `opentelemetry*` SDK deps are needed here (they are traces-only in
# the workspace pin and dragging the `logs` feature in would be a workspace-global change).
remote = ["dep:reqwest", "dep:governor", "dep:tokio"]

[dependencies]
vox-telemetry = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
reqwest = { workspace = true, optional = true }
governor = { workspace = true, optional = true }
tokio = { workspace = true, optional = true }
workspace-hack = { workspace = true }

[lints]
workspace = true
```
- [ ] **Step 2: Register in `layers.toml`** under `[crates]`:
  `vox-telemetry-otlp = { layer = 3, max_loc = 3_000 }` and add the where-things-live.md row
  ("L3 — feature-gated telemetry egress; only the binary surface registers it").
- [ ] **Step 3: Add the forbidden-dep arch rule** in `layers.toml`:
```toml
[[forbidden_deps]]
crate    = "vox-telemetry-otlp"
forbidden = ["vox-orchestrator", "vox-cli", "vox-gui"]
reason   = "egress crate must not depend up into surfaces; surfaces register it, not vice-versa"
```
  And document the inverse convention in SCHEMA.md: domain crates depend on `vox-telemetry`, never on `-otlp`.
- [ ] **Step 4: Stub `lib.rs`** with the no-op default path:
```rust
//! OTLP egress for vox-telemetry. The pure core (project/redact/otlp_json) always compiles
//! (serde only). The `remote` feature adds the async network uploader — the compile-out unit:
//! `--no-default-features` builds contain zero `reqwest`/network symbols.
#![cfg_attr(not(feature = "remote"), allow(unused))]

// Pure core — ALWAYS compiled (serde only, no network). vox-cli's SpoolSink calls these so the
// spool is redacted/clean even in builds without the `remote` feature.
pub mod project;     // TelemetryEvent (internally-tagged, non_exhaustive) -> (category, flat map)
pub mod redact;      // taxonomy-allowlist guard over the projected map
pub mod otlp_json;   // RedactedRecord -> OTLP/HTTP logs JSON envelope

// Feature-gated egress: the async uploader (reqwest). THIS is the compile-out unit.
#[cfg(feature = "remote")]
pub mod upload;
```
- [ ] **Step 5:** `cargo run -p vox-arch-check` → green; `cargo build -p vox-telemetry-otlp` (no features) → builds, pulls zero otel deps. **Commit.**
```bash
git add crates/vox-telemetry-otlp Cargo.toml docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "feat(telemetry-otlp): scaffold feature-gated L3 exporter crate"
```

### Task B2: Projection + redaction (egress boundary) — TDD

> **Critical design note (corrected after plan-review v2).** `TelemetryEvent` is an
> **internally-tagged, `#[non_exhaustive]` enum** (`crates/vox-telemetry/src/types.rs:272-273`:
> `#[serde(tag = "event_type", rename_all = "snake_case")]`): `serde_json::to_value` yields a
> **FLAT** `{"event_type":"research_metric","session_id":"...","metadata_json":"..."}` — there is
> no nesting. Because it is `#[non_exhaustive]`, an external `match` **MUST** end in a mandatory
> `_ => None` arm (it will not compile otherwise). The real variant set to handle (from
> `types.rs:274+`) includes `ResearchMetric, ModelCall, Error, BuildSummary, TaskRootSummary,
> AiFixture, LintFinding, LintAutofix, RepairAttempt, RepairOutcome, AuditRun, SelectionDecision,
> ModelDiscovery, ModelClassification, ConfidencePromotion` (+ the 5 new product variants) — map
> only the product-relevant ones; everything else → `_ => None` (not uploaded). Several variants
> carry **free-form `String` fields prohibited by spec §3** (`ResearchMetricEvent.session_id` can
> be `"bench:myrepo"` — a repo name; `metadata_json` is arbitrary ≤256 KB JSON). Privacy is
> therefore enforced in TWO
> layers: **(1) `project_event`** — a hand-written per-variant map that emits a `(category,
> flat_map)` containing ONLY allowlisted, transformed fields (this is where free-form strings
> are dropped/hashed); **(2) `redact_event`** — a generic taxonomy-driven allowlist filter that
> runs AFTER projection as defense-in-depth so nothing outside the SSOT can ever serialize.

**Files:**
- Create: `crates/vox-telemetry-otlp/src/project.rs` (variant → category projection + transforms)
- Create: `crates/vox-telemetry-otlp/src/redact.rs` (taxonomy allowlist guard)
- Test: `crates/vox-telemetry-otlp/tests/projection_redaction_guardrail.rs`

- [ ] **Step 1: Write the failing guardrail test using a REAL `TelemetryEvent`** (not a hand-built
  flat JSON) carrying a secret in both a free-form field and the session id:
```rust
use vox_telemetry::{TelemetryEvent, ResearchMetricEvent};
use vox_telemetry_otlp::project::project_event;
use vox_telemetry_otlp::redact::redact_event;

#[test]
fn real_research_metric_never_leaks_session_or_metadata() {
    let ev = TelemetryEvent::ResearchMetric(ResearchMetricEvent {
        session_id: "bench:my_private_repo".into(),                 // repo name — prohibited
        metric_type: "syntax_k".into(),
        metric_value: Some(1.0),
        metadata_json: Some("{\"token\":\"ghp_MUST_NEVER_LEAVE\"}".into()), // free-form — prohibited
    });
    let (category, flat) = project_event(&ev).expect("variant is projected");
    let red = redact_event(&category, &flat).expect("known category");
    let s = serde_json::to_string(&red).unwrap();
    assert!(!s.contains("my_private_repo"), "repo name leaked");
    assert!(!s.contains("ghp_MUST_NEVER_LEAVE"), "metadata secret leaked");
    assert!(!s.contains("token"), "free-form metadata key leaked");
    // The session PREFIX (enum class) is allowed; the suffix must be hashed, not raw.
    assert!(s.contains("syntax_k"));
}
```
- [ ] **Step 2: Run** `cargo test -p vox-telemetry-otlp --features remote real_research_metric` → FAIL.
- [ ] **Step 3: Implement `project.rs`** — a `pub fn project_event(e: &TelemetryEvent) ->
  Option<(String, serde_json::Map<String, Value>)>` with an **explicit `match` over every
  variant**. For each variant emit only allowlisted fields, applying transforms:
  - `session_id` → split on `':'`: keep the prefix as an enum (`SESSION_PREFIX_*` from the
    facade) under key `session_prefix`; the suffix is **salted-hashed** (`hash` type) under
    `session_suffix_hash` (salt = the per-install salt from B3; never uploaded) — never the raw value.
  - `metadata_json` / any free-form `String` → **dropped entirely** (do not attempt to parse).
  - numeric/enum/bool fields → passed through under their taxonomy field name.
  A variant with no product-relevant mapping returns `None` (it simply isn't uploaded).
  Provide the full match arm for `ResearchMetric`, `ModelCall`, `Error`, `BuildSummary`,
  `TaskRootSummary`, and the 5 new product variants; remaining variants → `None` with a
  `// TODO(track-E): map if product-relevant` marker that the E1 inventory drives.
- [ ] **Step 4: Implement `redact.rs`** — taxonomy-driven guard consuming the projected map:
```rust
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use serde_json::Value;

static TAXONOMY: &str =
    include_str!("../../../contracts/telemetry/collection-taxonomy.v1.json");

#[derive(Debug, Serialize)]
pub struct RedactedRecord { pub event_name: String, pub attrs: serde_json::Map<String, Value> }

fn allowlist() -> &'static HashMap<String, (String, HashSet<String>)> {
    static CELL: OnceLock<HashMap<String, (String, HashSet<String>)>> = OnceLock::new();
    CELL.get_or_init(|| {
        // NEVER panic on the telemetry hot path (spec §3.6). On any parse problem, degrade to an
        // EMPTY allowlist → redact_event returns None → nothing uploads (fail-closed/safe).
        // The taxonomy is also validated at compile-test time by A3's parity test.
        let mut m = HashMap::new();
        let Ok(v) = serde_json::from_str::<Value>(TAXONOMY) else { return m; };
        let Some(cats) = v["categories"].as_array() else { return m; };
        for cat in cats {
            let (Some(name), Some(ev), Some(fields)) =
                (cat["name"].as_str(), cat["otlp_event_name"].as_str(), cat["fields"].as_array())
            else { continue; };
            let set = fields.iter().filter_map(|f| f["name"].as_str().map(str::to_string)).collect();
            m.insert(name.to_string(), (ev.to_string(), set));
        }
        m
    })
}

/// Second-layer guard: keep ONLY taxonomy-allowlisted scalar fields for `category`.
pub fn redact_event(category: &str, flat: &serde_json::Map<String, Value>) -> Option<RedactedRecord> {
    let (event_name, allowed) = allowlist().get(category)?;
    let mut attrs = serde_json::Map::new();
    for (k, val) in flat {
        if allowed.contains(k) && !val.is_object() && !val.is_array() {
            attrs.insert(k.clone(), val.clone());
        }
    }
    Some(RedactedRecord { event_name: event_name.clone(), attrs })
}
```
- [ ] **Step 5: Run** the test → PASS. Add tests: (a) an unknown category → `redact_event` returns
  `None`; (b) every `SESSION_PREFIX_*` constant projects to a stable `session_prefix` enum value.
- [ ] **Step 6: Commit.**
```bash
git add crates/vox-telemetry-otlp/src/project.rs crates/vox-telemetry-otlp/src/redact.rs crates/vox-telemetry-otlp/tests/projection_redaction_guardrail.rs
git commit -m "feat(telemetry-otlp): variant projection + taxonomy redaction (two-layer egress guard)"
```

### Task B3: Consent + anonymous install-id in `vox-telemetry::config` — TDD

**Files:**
- Modify: `crates/vox-telemetry/src/config.rs`
- Test: `crates/vox-telemetry/tests/consent_install_id.rs`

- [ ] **Step 1: Write failing test** — `install_id()` returns a stable UUID across calls and
  `consent_state()` is `Unset` by default, `Granted`/`Denied` after `set_remote_consent`:
```rust
use vox_telemetry::config::{install_id, ConsentState, remote_consent, set_remote_consent};
#[test]
fn install_id_is_stable_and_consent_defaults_unset() {
    let a = install_id(); let b = install_id();
    assert_eq!(a, b, "install id must be stable within a run");
    assert_eq!(remote_consent(), ConsentState::Unset);
}
```
- [ ] **Step 2: Run** → FAIL (symbols missing).
- [ ] **Step 3: Implement** in `config.rs` (all new fns live in this module so they can call the
  module-private `user_config_path()` — if any are needed elsewhere, promote that helper to
  `pub(crate)`, do NOT duplicate it):
  - a `pub enum ConsentState { Unset, Granted, Denied }`;
  - `pub fn install_id() -> String` — reads-or-creates a v4 UUID via **`uuid::Uuid::new_v4()`**
    (`uuid` is ALREADY a direct dep of `vox-telemetry` — `Cargo.toml`, workspace pin `uuid =
    {version="1", features=["v4","serde"]}`; do NOT hand-roll an RNG and do NOT add `getrandom`),
    persisted as `~/.config/vox/install-id` next to the user config;
  - `pub fn install_salt() -> [u8; 16]` — a second random per-install value persisted as
    `install-salt`, used by `project.rs` to salt session-suffix hashes; **never uploaded**;
  - `pub fn remote_consent() -> ConsentState` reads `[telemetry] remote_consent =
    "granted|denied"`; `pub fn set_remote_consent(s: ConsentState)` writes it;
  - `pub fn is_remote_allowed() -> bool` = **`is_master_enabled() && remote_consent()==Granted`**.
    The existing `is_master_enabled()` (`config.rs:126`) is the hard kill (org-policy file +
    `VOX_TELEMETRY=off|0|false`); it MUST gate remote upload too, or a machine with telemetry
    hard-off would still upload once consent was granted (a §3.5 privacy breach). A stray
    `remote_upload=true` without consent must also stay false.
- [ ] **Step 4: Run** → PASS. Add tests (use `#[serial]` like the existing config tests, since
  they mutate env): (a) `set_remote_consent(Denied)` ⇒ `is_remote_allowed()==false` even with
  `remote_upload=true`; (b) **`VOX_TELEMETRY=off` ⇒ `is_remote_allowed()==false` even with
  consent `Granted`** (master-switch precedence).
- [ ] **Step 5: Commit.**
```bash
git add crates/vox-telemetry/src/config.rs crates/vox-telemetry/tests/consent_install_id.rs
git commit -m "feat(telemetry): two-tier consent state + anonymous install id"
```

### Task B4: Redact-before-spool in `vox-cli`'s `SpoolSink` — TDD

> Per the correction box: there is no live network recorder. The existing `SpoolSink`
> (`crates/vox-cli/src/telemetry_sink.rs`) already runs in the composite at startup. We make it
> **project + redact BEFORE enqueue** so the on-disk spool only holds clean OTLP-shaped records,
> and so a build without the `remote` feature still never writes secrets to disk. `record()`
> stays pure+sync (project/redact are pure); the existing `Handle::try_current()` guard handles
> the disk write. NO new `set_global_recorder` call.

**Files:**
- Create: `crates/vox-telemetry-otlp/src/otlp_json.rs` (pure OTLP/HTTP logs JSON encoder — serde only)
- Modify: `crates/vox-cli/src/telemetry_sink.rs` (`SpoolSink::record`)
- Modify: `crates/vox-cli/Cargo.toml` (add `vox-telemetry-otlp` dep — pure core, no `remote` yet)
- Test: `crates/vox-cli/tests/spool_is_redacted.rs`

- [ ] **Step 1: Read first.** Open `crates/vox-cli/src/telemetry_sink.rs:26-50` and
  `crates/vox-cli/src/telemetry_spool.rs` (`enqueue` signature) — replicate the existing
  `Handle::try_current()` guard; do not change the sync trait contract.
- [ ] **Step 2: Failing test** — register a `SpoolSink` over a temp dir, `record` a
  `ResearchMetric` whose `session_id="bench:secret_repo"` + `metadata_json` carries a token; read
  the spooled file and assert it contains neither the repo name nor the token, and that it is a
  valid OTLP-logs JSON object with `event.name`.
- [ ] **Step 3:** Implement `otlp_json.rs`: `pub fn to_otlp_log(rec: &RedactedRecord, install_id:
  &str) -> serde_json::Value` building the stable OTLP/HTTP logs envelope
  (`resourceLogs[0].resource.attributes` includes `vox.install_id`; `scopeLogs[0].logRecords[0]`
  has `body`/`attributes` from `rec`). Pure serde — no otel SDK, no `logs` feature.
- [ ] **Step 4:** In `SpoolSink::record`, before enqueue: `if let Some((cat, flat)) =
  project_event(event) { if let Some(red) = redact_event(&cat, &flat) { let payload =
  to_otlp_log(&red, &install_id()); /* enqueue payload via existing guard */ } }`. Events that
  project to `None` are simply not spooled.
- [ ] **Step 5: Run** → PASS. **Commit.**
```bash
git add crates/vox-telemetry-otlp/src/otlp_json.rs crates/vox-cli/src/telemetry_sink.rs crates/vox-cli/Cargo.toml crates/vox-cli/tests/spool_is_redacted.rs
git commit -m "feat(telemetry): redact+OTLP-encode before spool enqueue (clean-at-rest)"
```

### Task B5: Feature-gated uploader + first-run prompt + `vox telemetry` CLI — TDD

**Files:**
- Create: `crates/vox-telemetry-otlp/src/upload.rs` (feature `remote`: async OTLP POST of pending spool records)
- Create: `crates/vox-cli/src/commands/telemetry/mod.rs`
- Modify: `crates/vox-cli/src/lib.rs` (the `init_telemetry_sinks` chokepoint + first-run prompt hook; CONFIRM the function name/line before editing — it is `init_telemetry_sinks` near `lib.rs:673`, NOT `cli_dispatch/mod.rs`)
- Modify: `crates/vox-cli/Cargo.toml` (enable `vox-telemetry-otlp/remote` behind a NEW cli feature `telemetry-remote`, default OFF)
- Test: `crates/vox-cli/tests/telemetry_cli.rs`, `crates/vox-telemetry-otlp/tests/upload_gating.rs`

- [ ] **Step 1: Failing tests** — (a) `vox telemetry status` prints state machine-readably;
  `vox telemetry preview` prints the exact redacted OTLP JSON that WOULD upload and sends
  nothing; (b) `upload_pending_otlp(dir, endpoint)` is a **no-op returning Ok(0) when
  `is_remote_allowed()==false`** (consent Unset OR `VOX_TELEMETRY=off`), and POSTs+acks each
  pending record when allowed (assert against a tiny tokio mock server).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** `upload.rs` (feature `remote`): async `upload_pending_otlp` that
  short-circuits on `!is_remote_allowed()`, else `list_pending`→POST OTLP/HTTP→`ack`, with a
  `governor` rate limit and best-effort error swallowing (never panics). Wire `vox telemetry
  {status,preview,on,off,upload}`: `on`→`set_remote_consent(Granted)`, `off`→`Denied`,
  `upload`→call the uploader, `preview`→project+redact a synthetic-or-pending record and print.
  The **first-run prompt** fires once at startup (consent==Unset && interactive TTY) listing
  exactly what is/isn't collected (from SCHEMA.md), default = do not upload. An optional periodic
  flush calls the uploader off the hot path. NOTE: the `SpoolSink` from B4 is ALWAYS in the
  composite; the `remote` feature only adds the *uploader*, and consent/master-switch gate at
  upload time — so runtime `vox telemetry on/off` takes effect on the next upload (no recorder
  re-registration needed).
- [ ] **Step 4: Run** → PASS. **Symbol tests:** assert BOTH `cargo build -p vox-cli
  --no-default-features` AND the **default** `vox` binary build contain zero `reqwest`/otel
  upload symbols unless `telemetry-remote` is explicitly enabled (guards feature-unification leak).
- [ ] **Step 5: Commit.**
```bash
git add crates/vox-telemetry-otlp/src/upload.rs crates/vox-cli/src/commands/telemetry crates/vox-cli/src/lib.rs crates/vox-cli/Cargo.toml crates/vox-cli/tests/telemetry_cli.rs crates/vox-telemetry-otlp/tests/upload_gating.rs
git commit -m "feat(cli): feature-gated OTLP uploader + vox telemetry CLI + first-run consent"
```

### Task B.LEDGER: Append Track B ledger entry
- [ ] Append `AGH-XXXX` (client exporter delivered, test counts, no-symbol verification, follow-ups). Commit.

---

## Track C — Server repo `vox-telemetry-server`  `[depends on A; PARALLEL-SAFE with B]`

> Built in a **separate private repo**, not the Vox workspace. Clone/init it outside `crates/`.
> All schema/DDL is **generated from** `contracts/telemetry/collection-taxonomy.v1.json` (copy it
> into the server repo as a vendored contract so client and server share one source).

### Task C1: Init server repo + vendor the taxonomy
- [ ] **Step 1:** `git init vox-telemetry-server`; add `rust-toolchain`, workspace `Cargo.toml`,
  and copy `collection-taxonomy.v1.json` into `contracts/`. Add `clickhouse = "0.13.3"`,
  `axum`, `tokio`, `serde`, `serde_json`, `opentelemetry-proto` (OTLP log decoding).
- [ ] **Step 2:** Commit `chore: init vox-telemetry-server with vendored taxonomy contract`.

### Task C2: Generate ClickHouse schema from the taxonomy — TDD
**Files:** `src/schema.rs`, `migrations/0001_events_raw.sql`, `tests/schema_gen.rs`
- [ ] **Step 1: Failing test** — a `gen_ddl(taxonomy)` fn yields a `CREATE TABLE events_raw`
  with one column per allowlisted field across categories (typed: enum→`LowCardinality(String)`,
  int→`Int64`, bool→`UInt8`, hash→`String`), plus `event_name LowCardinality(String)`,
  `install_id String`, `ts DateTime64`, and a `TTL ts + INTERVAL 180 DAY` clause.
- [ ] **Step 2:** Run → FAIL. **Step 3:** Implement `gen_ddl`. **Step 4:** Run → PASS.
- [ ] **Step 5:** Emit `migrations/0001_events_raw.sql` + per-category materialized views
  (`mv_command_usage`, `mv_skill_activation`, `mv_edit_pattern`, …) that roll up counts by
  day/field. **Commit.**

### Task C3: OTLP ingest endpoint (axum) — TDD
**Files:** `src/ingest.rs`, `src/main.rs`, `tests/ingest_roundtrip.rs`
- [ ] **Step 1: Failing test** — POST an OTLP/HTTP logs payload (one `vox.command` record) to
  `/v1/logs`; assert the handler decodes it, **re-applies server-side field allowlisting**
  (defense in depth — never trust the client), and inserts one row whose unlisted attributes
  were dropped.
- [ ] **Step 2–4:** Implement decode (`opentelemetry-proto`) → allowlist filter → `clickhouse`
  batch insert; run tests green.
- [ ] **Step 5:** Add `/healthz` + a k-anonymity-enforcing read view helper. **Commit.**

### Task C4: Dashboards
- [ ] Add `dashboards/` with Grafana/Metabase JSON: one board per §5 category (command
  frequency, skill-surface timing, edit-pattern mix, harness usage), each query enforcing the
  `k_anonymity` floor from the taxonomy. Commit.

### Task C.LEDGER: Append Track C ledger entry (note: separate repo — reference its commit hashes).

---

## Track D — Deploy infra incrementally  `[SEQUENTIAL after C; needs A4 decision]`

> Executes the hosting decision from A4. Deploy **as you go**: stand up ClickHouse first,
> verify, then ingest, then dashboards. Use the existing `vox deploy` / Coolify project if A4
> found one; otherwise create a new Coolify project on FableForge (or Hetzner).

### Task D1: Provision ClickHouse
- [ ] Add a `clickhouse` service to the chosen compose/Coolify project (single-node, volume-
  backed, internal network only). Apply `migrations/0001`. Verify `SELECT 1` + table exists.
  Commit the compose/infra change. **Deploy.**

### Task D2: Deploy ingest service
- [ ] Build + deploy `vox-telemetry-server` as a container behind TLS (Coolify-managed cert).
  Set the public OTLP endpoint URL. Smoke-test `/healthz` + a hand-crafted OTLP POST → row in
  ClickHouse. Record the endpoint in `contracts/telemetry/pinned-versions.md`. **Deploy.**

### Task D3: Deploy dashboards
- [ ] Deploy Grafana/Metabase pointed at ClickHouse (read-only user). Import `dashboards/`.
  Verify a board renders with synthetic data. **Deploy.**

### Task D.LEDGER: Append Track D ledger entry (live endpoint, deploy method, verification output).

---

## Track E — Migration & wiring  `[SEQUENTIAL — last; needs A,B; soft-needs D]`

### Task E1: Wire new product-category emit sites (TDD, one task per site)
- [ ] For each `status=proposed` row in the inventory (A2): write a failing test that the site
  emits the right `TelemetryEvent` with only allowlisted fields when telemetry is on; implement
  the `record_event!` call at the chokepoint; run green; commit. (command_usage, skill_activation,
  edit_pattern, harness_usage, error_surface — 5 sites, 5 commits.)

### Task E1b: Add `record_default_decision!` + instrument the default-decision sites — TDD
- [ ] **Step 1:** Add a `DefaultDecisionEvent { decision_id: String, chosen: String, outcome:
  String, magnitude_bucket: Option<i64> }` variant to `TelemetryEvent` (mirror the existing
  variant pattern in `types.rs`; add the `METRIC_TYPE_DEFAULT_DECISION` const + re-export) and a
  thin `record_default_decision!(decision_id, chosen, outcome)` macro wrapping `record_event!`.
  Add the `default_decision` category to the taxonomy SSOT (A3) — `decision_id`/`chosen`/`outcome`
  are `enum` fields whose `allowed` lists come from `default-decision-sites.csv` (A2b).
- [ ] **Step 2:** For each `status=proposed` row in `default-decision-sites.csv`, write a failing
  test that the site emits the right `decision_id`+`chosen` enum, then add the
  `record_default_decision!` call at the site (passing the bucketed enum, never the raw number),
  run green, commit. Do these as small per-site commits. Keep enums in sync with the SSOT
  (the projection-coverage test from E2 Step 1 will catch any free-form leak).

### Task E2: Route ALL existing categories through the exporter (full package migration)
- [ ] **Step 1: Projection coverage gate.** Before any existing category may upload, every
  `TelemetryEvent` variant that is NOT mapped in `project.rs` (B2) must return `None` (not
  upload) — there is no silent passthrough. Write a test that iterates a representative instance
  of EACH variant through `project_event`→`redact_event` and asserts the serialized output
  contains none of a set of planted secrets (repo name, token, raw `metadata_json`). This is the
  §3 invariant gate for the existing package, and it REPLACES the earlier (wrong) "gate
  identically" framing — existing structs carry free-form strings and need per-field projection,
  not blanket forwarding.
- [ ] **Step 2:** No new recorder registration is needed — the `SpoolSink` (B4) already redacts
  + spools every mapped category, and the B5 uploader ships them when `is_remote_allowed()`.
  So Step 2 is: confirm each existing mapped category (`research_metrics`, `model_calls`,
  `build`, `errors`, `agent_orchestration`) has a projection arm and an enabled
  `upload_default`, and add an integration test that an existing event end-to-ends through
  spool→uploader→mock endpoint only with consent. **Scope = `vox-cli` only for the MVP.**
  `vox-gui` and the orchestrator daemon have **no recorder registration today** (verified: no
  `set_global_recorder` in `crates/vox-gui/`) — wiring telemetry into those surfaces is net-new
  and is filed as an explicit follow-up (E-followup), not assumed here.

### Task E3: End-to-end verification
- [ ] With the live endpoint (Track D), set consent Granted in a scratch profile, run a command,
  and assert the event lands in ClickHouse and appears in the dashboard. Record output. (If D not
  yet live, run against a local docker ClickHouse and mark the prod check as a follow-up.)

### Task E4: Final guardrails
- [ ] Add a CI gate (`vox ci` sub-check or arch-check rule) asserting: (a) no emitter crate
  depends on `vox-telemetry-otlp`; (b) the taxonomy parity test runs; (c) a `--no-default-
  features` build of `vox-cli` contains zero otel symbols. Commit.

### Task E.LEDGER: Append Track E ledger entry + a program-completion summary entry.

---

## Track F — Model-Layer: learned per-model prompt  `[soft-deps A+E; PARALLEL-SAFE with B/C/D]`

> Reuses the audited machinery; introduces NO second model registry. Anchors verified:
> autonomic loop `crates/vox-orchestrator/src/models/autonomic.rs` (`ModelConfidence`,
> `should_promote`, `record_promotion`); scoreboard `crates/vox-db/src/schema/domains/scientia.rs`
> (`model_scoreboard`); similarity `crates/vox-similarity` + `vox-skill-discovery` Candidate;
> registry pattern `crates/vox-plugin-host/src/skill_registry.rs` (`hydrate_from_db`); injection
> `crates/vox-orchestrator-mcp/src/chat_tools/mod.rs:55-187` (`build_system_prompt_with_skill`).

### Task F1: `model_prompt_profiles` table + `ModelPromptRegistry` — TDD
**Files:** new migration in `crates/vox-db/src/schema/domains/scientia.rs`; new
`crates/vox-orchestrator/src/models/prompt_profiles.rs`; test alongside.
- [ ] **Step 1: Failing test** — `ModelPromptRegistry::hydrate_from_db()` loads rows into a
  `HashMap<prompt_profile_key, Vec<ModelPromptProfile>>`; `active_profile(key)` returns the
  single `Confirmed` variant (or `None`).
- [ ] **Step 2:** Add table `model_prompt_profiles` (PK `(prompt_profile_key, variant_id)`; cols
  `preamble_text TEXT`, `confidence` enum, `quality_delta REAL`, `applications INT`,
  `created_at_ms`, `approved_by TEXT`). Mirror the `model_scoreboard` schema style + bump
  `BASELINE_VERSION` + yaml digest per the existing migration convention.
- [ ] **Step 3:** Implement `ModelPromptProfile` type + `ModelPromptRegistry` (mirror
  `SkillRegistry`: `Mutex<HashMap>` + `hydrate_from_db` + fire-and-forget `publish`). Run green. Commit.

### Task F2: canonical-id keying (`prompt_profile_key`) — TDD
- [ ] **Step 1: Failing test** — `prompt_profile_key(&ModelSpec)` returns a stable key that does
  NOT change across OpenRouter alias churn (e.g. strips provider-route volatility; falls back to
  `model.id` only when no canonical slug). **Step 2-3:** implement on `ModelSpec` (no new field
  required if derivable; else add `Option<String>` defaulting to `id`); test alias-churn stability. Commit.

### Task F3: inject the cache-stable segment in `build_system_prompt` — TDD
**Files:** modify `crates/vox-orchestrator-mcp/src/chat_tools/mod.rs` (thread selected model in);
new segment fn; test.
- [ ] **Step 1: Read first** `chat_tools/mod.rs:55-187` + `plan.rs:236-261` (model resolved
  before prompt build). **Step 2: Failing test** — given a `Confirmed` profile for the selected
  model, `build_system_prompt` includes a `## Model guidance ({id})` segment placed AFTER the
  skill catalog/pinned-skill layer and BEFORE the volatile budget/temporal segments (cache-stable
  region); given no `Confirmed` profile, the prompt is byte-identical to today.
- [ ] **Step 3:** Thread the selected `ModelSpec` into `build_system_prompt_with_skill`; look up
  `registry.active_profile(prompt_profile_key(model))`; inject only the `Confirmed` variant text.
  Run green. Commit. (Guard: never inject per-call/volatile text — would break prefix caching.)

### Task F4: autonomic-gated promotion via shadow-eval — TDD
- [ ] **Step 1: Failing test** — a variant starts `Provisional`; `should_promote_profile(variant,
  scoreboard_delta)` advances `Provisional→Shadowed` on candidate acceptance and
  `Shadowed→Confirmed` only when shadow-eval quality_delta ≥ threshold over ≥N applications;
  emits a `ConfidencePromotionEvent`-style record. **Step 2-3:** implement by REUSING
  `autonomic::should_promote` semantics (same thresholds/state enum); persist state to F1's table.
  Run green. Commit.
- [ ] **Step 2 (shadow-eval harness):** run a `Shadowed` variant in shadow (compare baseline vs
  baseline+variant quality on the same calls) without injecting into production; record the delta.

### Task F5: candidate mining (advisory) + local prompt mining — TDD
- [ ] **Step 1:** Add `CandidateKind::ModelPromptVariant` to `vox-skill-discovery`; a miner that
  clusters per-model structured outcomes (via `vox-similarity`) into proposed variants — ADVISORY
  output only (never auto-applies), mirroring `dedup_skills`/`validate_ssot`. **Local-only** prompt
  text mining is opt-in and stays on-device; central input is the structured `model_prompt` events.
- [ ] **Step 2:** A `vox model-layer suggest` CLI surfaces candidates for human/council approval;
  approval writes a `Provisional` profile via F1. Tests + commit.

### Task F6: `model_prompt` telemetry category + forest feedback — TDD
- [ ] Add the `model_prompt` category to the taxonomy SSOT (A3) — enum/bucket fields only
  (canonical model id, variant id, task, active-skill ids, quality bucket). Emit a
  `record_model_prompt!` event at the injection/outcome site. The server (Track C) rolls these up;
  a server-side job proposes cross-user candidate variants that re-enter as advisory F5 candidates
  (the "forest" feedback) — **structured signals only, never raw prompt text**.

### Task F.LEDGER: Append Track F ledger entry (Model-Layer delivered; reuse map; follow-ups).

---

## Self-Review (run after writing; checklist, not a subagent)

- **Spec coverage:** §2 decisions → Shared Conventions + crate plan (B1) ✓; §3 privacy
  invariants → redaction (B2), consent (B3), preview/status (B5), server-side re-allowlist (C3),
  k-anonymity (C2/C4/E) ✓; §4 architecture → B1/B4/C ✓; §5 taxonomy → A3 + E1 + F6 ✓; §6 server →
  C+D ✓; §7 Model-Layer → Track F (F1-F6) ✓; §9 success criteria → E3/E4 + F3 ✓.
- **Placeholder scan:** audit-dependent values (DDL columns, emit sites) are **generated from
  the A3 SSOT via a stated procedure**, not left as "TBD" — each such task carries the generator
  + acceptance test. No bare TODOs.
- **Type consistency:** `TelemetryRecorder`/`set_global_recorder`/`global_recorder`/
  `CompositeRecorder`/`record_event!` are the real facade symbols (verified against
  `crates/vox-telemetry/src/{lib,recorder,config,types}.rs`). NEW symbols introduced once and
  reused consistently: `ConsentState`, `install_id`, `install_salt`, `is_remote_allowed`,
  `OtlpRecorder`, `project_event`, `redact_event`, `RedactedRecord`, `gen_ddl`. (Plan-review
  correction: the earlier `event_to_json` symbol was removed — projection (`project_event`) is
  the enum→flat step; do not reintroduce it.)
- **Post-review v1 corrections:** B2 projects the real enum + drops/hashes free-form `String`
  fields; E2 enforces per-variant projection coverage; B3 uses `uuid::Uuid::new_v4()`.
- **Post-review v2 corrections (architecture):** (1) `TelemetryEvent` is **internally-tagged +
  `#[non_exhaustive]`** (flat serialization; mandatory `_ => None` arm) — note corrected. (2) No
  live network in sync `record()` — egress is **redact-before-spool** + a feature-gated uploader
  (resolves the `tokio::spawn`-panic and the first-write-wins `set_global_recorder` no-op). (3)
  The real spool is in **`vox-cli`**, not the facade. (4) `is_remote_allowed()` now includes
  **`is_master_enabled()`** (org-policy / `VOX_TELEMETRY=off` precedence). (5) Client carries **no
  `opentelemetry` SDK** — OTLP JSON hand-encoded, dissolving the 0.29→0.32 / `logs`-feature blast
  radius. (6) Taxonomy parse **fail-closed** (no hot-path panic). (7) MVP scope = `vox-cli`;
  GUI/daemon registration is net-new follow-up. (8) Added `default_decision` category +
  `record_default_decision!` + A2b/E1b for the "sensible defaults" surface.
- **Gaps to flag at execution:** otel pin scope decision — keep 0.29 vs bump 0.32 (resolve in
  A4 Step 4); whether FableForge already hosts a Vox project (A4 Step 1); OTel-Collector-vs-axum
  ingest (A4 Step 2); the exact projection arm for each less-common `TelemetryEvent` variant
  (drive from the E1 inventory; default `None`/no-upload until mapped).

---

## Execution handoff

Plan complete. Recommended: **subagent-driven** — run Track A inline (it gates everything),
then dispatch Tracks B and C to parallel subagents, then D, then E, reviewing between tasks.
Use superpowers:subagent-driven-development.
