# Centralized Vox Telemetry — Design Spec (SSOT)

**Status:** Approved (brainstorming, 2026-06-19). Plan-only; not executed.
**Owner:** Vox core. **Execution target:** Claude Sonnet 4.6 (parallel subagents / workflows).
**Companion plan:** `docs/superpowers/plans/2026-06-19-centralized-telemetry-program.md`

---

## 1. Problem & goal

Vox already collects rich **local** telemetry (`vox-telemetry` L1 facade, 5-layer config,
spool, ~60 emitter crates) but has **no centralized sink**. We want an opt-in central
store so we can, across the whole user base at once:

- learn which **commands** are common (surface/prioritize them),
- learn which **skills** to surface and *when* (trigger-context analytics),
- learn **how the harness is used** (tool-call mix, session shape),
- learn **common edits** (operation/file-kind patterns — never content),
- continuously improve Vox while **protecting privacy**.

The whole collection path must be a **compile-out plugin** (build Vox with zero telemetry
code present) and must always be **opt-out**, with central upload **opt-in**.

## 2. Decisions (locked in brainstorming)

| Axis | Decision |
|---|---|
| Consent | **Two-tier**: local collection **on by default / opt-out**; central **upload opt-in** (one-time first-run prompt). |
| Datastore | **ClickHouse** (columnar OLAP; built for billions of events + parallel cross-user aggregation). |
| Client wire | **Hybrid**: keep the existing `TelemetryEvent` taxonomy; carry it as **OTLP/HTTP logs JSON**, hand-encoded with `serde`+`reqwest` (no `opentelemetry` SDK on the client) so the backend stays swappable. |
| Server code | **Separate private repo** (`vox-telemetry-server`) — operational infra, never shipped to users. |
| Client exporter crate | **One new workspace crate** `vox-telemetry-otlp` (L3, feature-gated `telemetry-remote`). No heavy deps added to the L1 facade or the ~60 emitters. |
| Executor | **Sonnet 4.6** via subagent-driven development; infra is deployed **incrementally as the plan runs**. |

## 3. Non-negotiable privacy invariants

From 2025–26 CLI-telemetry norms (GitHub-CLI backlash, VS Code/Homebrew/Next.js patterns):

1. **Never collect**: file contents, secrets/tokens, repo names, absolute paths, free-form
   command arguments, prompt/chat text, source diffs' content.
2. **Allowlist only**: command *verbs* + enumerated flags; skill *ids*; operation *types*;
   file *kinds* (extension class); enum/numeric metrics. Anything not on the allowlist is dropped.
3. **Anonymous identity**: a random per-install UUID (no machine/user PII). Repo identity →
   salted hash (salt is per-install, never uploaded).
4. **Redaction before persistence (clean-at-rest)**: the `project`+`redact` pass (pure core of
   `vox-telemetry-otlp`) runs in `SpoolSink` *before* the event is written to the on-disk spool,
   so secrets never touch disk and never leave the process. The uploader only ships
   already-clean records. Covered by guardrail tests (the spooled file must contain no planted secret).
5. **Inspectable & reversible**: `vox telemetry status` (state), `vox telemetry preview`
   (exact payload that would be sent), `vox telemetry off` (kill switch), AND the existing master
   kill-switch `is_master_enabled()` (`VOX_TELEMETRY=off` + org-policy Layer-1) gates remote
   upload too — `is_remote_allowed()` requires it.
6. **Best-effort**: telemetry never blocks a command; `record()` does only pure redaction + a
   guarded disk enqueue (no network); upload is async/off-hot-path and fails open (never panics,
   even on malformed taxonomy → fail-closed empty allowlist).
7. **Server-side k-anonymity** on any aggregate surfaced from the data (k ≥ threshold, TBD-in-research → default k=20).

## 4. Architecture

```
CLIENT (in Vox)                                   SERVER (separate private repo)
┌───────────────────────────────────────┐        ┌──────────────────────────────────┐
│ ~60 emitter crates  → record_event!    │        │  ingest: axum + clickhouse 0.13   │
│ vox-telemetry (L1 facade, ZERO net dep)│        │   (OTLP/HTTP logs receiver)       │
│   • config(5-layer)+install-id+salt+   │ OTLP   │        │                          │
│     consent + master kill-switch       │ logs   │  ClickHouse (columnar)            │
│ vox-cli SpoolSink:project→redact→spool │ JSON   │   • events_raw  (TTL)             │
│   (clean-at-rest; spool ALREADY exists │ /HTTPS │   • MVs: cmd/skill/edit rollups   │
│    in vox-cli, not the facade)         │ ─────► │  Grafana/Metabase dashboards      │
│ vox-telemetry-otlp (L3)                │        │  (skill-surfacing, cmd freq, …)   │
│   • project/redact/otlp_json (PURE)    │        └──────────────────────────────────┘
│   • upload (feat `remote`,COMPILE-OUT) │
└───────────────────────────────────────┘
   client hand-encodes OTLP/HTTP logs JSON (serde+reqwest) — NO opentelemetry SDK on client.
   feature `telemetry-remote` off → zero reqwest/network symbols in the binary.
```

> **Compile-out, precisely:** the `vox-telemetry` facade *always* compiles and is cheap — its
> `record_event!` is a **runtime** no-op until a recorder is registered (it checks
> `global_recorder()`). `vox-telemetry-otlp`'s **pure core** (`project`/`redact`/`otlp_json`,
> serde only) also always compiles — that's what lets `SpoolSink` redact even in non-remote
> builds. The *compile-out unit* is the crate's **`upload` module** behind feature `remote`:
> `--no-default-features` (no `telemetry-remote`) → the binary's dep tree contains zero
> `reqwest`/network symbols. The client carries **no `opentelemetry` SDK** at all (OTLP JSON is
> hand-encoded). Do NOT `#[cfg]`-gate the facade. The success-criteria symbol test targets the
> **binary** (`vox-cli`) dep tree — both the default build and `--no-default-features`.

### Crate plan (lean; avoid ballooning)
- **Keep** `vox-telemetry` (L1) zero-network: only `record_event!`, config, types, spool trait.
  Adding otel/reqwest here would poison build times for all ~60 dependents — **forbidden**.
- **New** `vox-telemetry-otlp` (L3, `kind="library"`): **pure core** (`project`/`redact`/
  `otlp_json`, serde only — always compiles) + feature-gated **`upload`** (`reqwest`+`governor`,
  the compile-out unit). No `opentelemetry` SDK (OTLP JSON hand-encoded). It does NOT implement a
  live recorder — egress is redact-at-spool + a periodic/explicit uploader.
- **Extend** `vox-telemetry::config`: install-UUID + per-install salt + consent persistence +
  `is_remote_allowed() = is_master_enabled() && consent==Granted` (file IO it already does;
  `uuid` already a dep; **no new deps**).
- **Extend** `vox-cli` (L5): `SpoolSink` redacts-before-enqueue; first-run consent prompt +
  `vox telemetry {status,preview,on,off,upload}`. MVP scope = `vox-cli` only (GUI/daemon have no
  recorder registration today → follow-up).
- **arch-check**: add a forbidden-dep rule so no domain crate depends on `vox-telemetry-otlp`
  directly (only the facade); register the new crate in `layers.toml` + `where-things-live.md`.
- **Server** is **not** a workspace crate — separate repo, audited/deployed in its own track.

### Why OTLP **logs** (not metrics/traces)
Product events (command-invoked, skill-activated, edit-performed) are discrete structured
records → map cleanly to OTLP `LogRecord` (`event.name` + typed attributes). This keeps the
existing `TelemetryEvent` field set intact and lets either the **OTel Collector → ClickHouse
exporter** or **our axum service** ingest the same wire format (backend swappable).

## 5. "What to collect" taxonomy (the main thrust)

Existing categories stay (`research_metrics`, `model_calls`, `agent_orchestration`, `build`,
`errors`) and become **opt-in uploadable**. New product categories (each tagged a privacy tier):

| Category | Signal | Fields (allowlisted) | Privacy tier |
|---|---|---|---|
| `command_usage` | which commands are common | verb, enumerated subcommand, allowlisted flag set, exit class, duration bucket | low |
| `skill_activation` | which skill, when | skill id, trigger source (enum), accepted/rejected, surface | low |
| `edit_pattern` | common edits | op type (insert/replace/delete), file-kind class, size bucket, count | medium |
| `harness_usage` | how the harness is used | tool-call kind histogram, session shape (turns, agents spawned), mode | low |
| `error_surface` | failures (extends `errors`) | error class, subsystem, recoverable? | medium |
| `default_decision` | **learn sensible defaults** from real usage at tunable decision points | decision id (enum), chosen value (enum/bucket), outcome (enum: hit_limit / comfortable / throttled / timed_out …), magnitude bucket | low |
| `model_prompt` | feed the **learned per-model prompt layer** (§7) | canonical model id (enum), profile variant id (enum), task category (enum), active-skill set (enum ids), quality/outcome bucket | low |

**`default_decision` — the "sensible defaults" use-case.** Vox picks many tuned constants
(budgets, concurrency, retries, timeouts, cache TTLs). Aggregating *what value was in effect* +
*what outcome it produced* across users lets us set better defaults empirically. A codebase
audit found ~12 high-value sites (committed to the inventory), e.g.: `vox-orchestrator`
`budget/mod.rs` cost/drift/doom-loop/token thresholds; `vox-config` `vox_config.rs` LLM
`max_concurrent`(8)/retry(4); `vox-orchestrator-mcp` `llm_bridge/limits.rs` output-token
cap(8192)/probe TTL(30s)/timeout; `vox-effort-audit` `config.rs` audit concurrency(4);
`vox-audit` `panel.rs` backoff. Emission: a `record_default_decision!(decision_id, chosen,
outcome)` helper (enum-only args) at each site; the **decision id + chosen-value enums are
themselves part of the taxonomy allowlist** so nothing free-form can leak. Outcomes are recorded
near the decision site (runtime-dependent); the redaction/allowlist stays centralized.

The **authoritative emit-site inventory** is produced by the plan's audit phase (graphify +
parallel subagents) → committed as a versioned CSV/JSON SSOT that the server schema and the
client allowlist are both generated from (one source, no drift).

> **⚠ Existing event structs carry prohibited free-form fields.** The current `TelemetryEvent`
> variants (e.g. `ResearchMetricEvent.session_id` can be a repo name like `"bench:myrepo"`;
> `metadata_json` is arbitrary ≤256 KB JSON) violate invariants §3.1–§3.2 if forwarded as-is.
> Migration (plan Track E2) therefore does NOT blanket-forward existing events — each variant
> gets an explicit projection arm that drops free-form strings and salts-hashes any identifier
> suffix. A variant with no privacy-safe mapping is simply not uploaded. The taxonomy parity
> test constrains new categories; the **projection-coverage test** (E2) constrains the existing
> ones.

## 6. Server (separate repo) shape
- **Repo**: `vox-telemetry-server` (private). Stack: Rust `axum` ingest + `clickhouse` 0.13.3,
  or OTel Collector + ClickHouse exporter (decided in infra-audit track — both ingest OTLP).
- **Schema**: `events_raw` (wide, TTL'd) + materialized-view rollups per category; generated
  from the §5 taxonomy SSOT.
- **Hosting**: evaluate **FableForge Coolify** (existing deployment; ClickHouse as a Docker
  service) vs **Hetzner self-managed**. Audit the existing Coolify project/`vox deploy`/compose
  artifacts first; reuse if a Vox project already exists. Deploy incrementally within the plan.
- **Dashboards**: Grafana or Metabase over ClickHouse, one board per §5 category, with the
  k-anonymity floor enforced in the query layer.

## 7. Companion subsystem — learned per-model prompt layer ("Model-Layer")

A learned, model-specific **system-prompt segment** injected *between* the generic
(model-agnostic) Vox skill archive and a concrete model — getting smarter over time from
aggregate outcomes. Coupled to telemetry (the `model_prompt` category feeds it; the central
forest proposes variants). **Decisions (locked):** dedicated `model_prompt_profiles` table +
`ModelPromptRegistry`; **structured-only** central data + **local/opt-in** text mining +
**human/council-approved** preamble text (no raw-prompt upload — honors §3.1); autonomic-gated
promotion (shadow-eval → only `Confirmed` variants inject).

**Reuse map (audited — build almost nothing new):**
- **Learn/promote loop = reuse the model-autonomic confidence machine**
  (`crates/vox-orchestrator/src/models/autonomic.rs`: `ModelConfidence`
  `Provisional→Shadowed→Confirmed→Deprecated`, `should_promote()`, `record_promotion()`),
  evidenced by the existing `model_scoreboard` table. A profile *variant* advances on a measured
  quality-delta vs. the no-profile baseline; only `Confirmed` injects in production.
- **Candidate mining = reuse `vox-similarity` (LSH/minhash) + the advisory `Candidate`
  pattern** from `vox-skill-discovery` (add `CandidateKind::ModelPromptVariant`) — human-gated,
  never auto-applied (mirrors today's skill-discovery).
- **Persistence pattern = mirror `SkillRegistry`** (`vox-plugin-host`): `hydrate_from_db()` +
  fire-and-forget publish; new `model_prompt_profiles` table.
- **Injection point = `build_system_prompt_with_skill()`**
  (`crates/vox-orchestrator-mcp/src/chat_tools/mod.rs:55-187`): add a new ordered segment right
  after the skill catalog/pinned-skill layer. The selected model is already known here. The
  segment MUST be **cache-stable** (the prompt is day-stable to preserve Anthropic/DeepSeek
  prefix caching) — so only a `Confirmed`, versioned profile string is injected, never per-call text.

**Privacy specifics:** profiles keyed by a **canonical model id / `prompt_profile_key`
indirection** (OpenRouter ids churn — never key on a volatile alias). Central `model_prompt`
events carry only enums/buckets (model id, variant id, task, active-skill ids, quality bucket);
the preamble *text* is authored/approved by a human or council, optionally seeded by **local**
prompt mining the user runs on their own machine. Nothing about a user's raw prompts leaves the
device.

**Split-brain guards (audited):** do not add a second model registry — extend the existing one;
learned profiles tune the *prompt for an already-selected model*, never override model
*selection* (council pins win); orphaned-profile lint when a model version retires.

## 8. Out of scope (YAGNI)
Differential-privacy noise injection (research it; ship k-anonymity first), real-time
streaming dashboards, per-user data-subject portal, ML on telemetry, auto-harvesting preamble
text from raw user prompts. Revisit post-MVP.

## 9. Success criteria
- Build `vox` with `--no-default-features` (no `telemetry-remote`) → **zero** otel/reqwest
  symbols in the binary; `record_event!` is a no-op (verified by a symbol/dep test).
- Two-tier consent honored; `preview` shows exactly what `upload` would send; redaction
  guardrail tests pass (no denied field class ever serialized).
- End-to-end: a local event opt-in-uploads → lands in ClickHouse → appears in a dashboard.
- New crate within LoC budget; arch-check green; workspace build-time delta within budget.
- **Model-Layer:** a `Confirmed` per-model profile injects a cache-stable segment after the
  skill layer in `build_system_prompt`; shadow-eval gates promotion; no raw prompt text ever
  uploads; reuses `autonomic.rs` + `vox-similarity` (no second model registry introduced).
```

