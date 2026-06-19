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
| Client wire | **Hybrid**: keep the existing `TelemetryEvent` taxonomy; carry it over **OTLP logs** (`opentelemetry-otlp`) so the backend stays swappable. |
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
4. **Redaction at egress**: a redaction pass runs in `vox-telemetry-otlp` *before* any byte
   leaves the process; covered by guardrail tests.
5. **Inspectable & reversible**: `vox telemetry status` (state), `vox telemetry preview`
   (exact payload that would be sent), `vox telemetry off` (kill switch), org-policy hard-off
   (existing Layer-1).
6. **Best-effort**: telemetry never blocks a command; upload is async via the spool and fails open.
7. **Server-side k-anonymity** on any aggregate surfaced from the data (k ≥ threshold, TBD-in-research → default k=20).

## 4. Architecture

```
CLIENT (in Vox)                                   SERVER (separate private repo)
┌───────────────────────────────────────┐        ┌──────────────────────────────────┐
│ ~60 emitter crates  → record_event!    │        │  ingest: axum + clickhouse 0.13   │
│ vox-telemetry (L1 facade, ZERO net dep)│        │   (OTLP/HTTP logs receiver)       │
│   • config (5-layer) + install-id +    │ OTLP   │        │                          │
│     consent state                      │ logs   │  ClickHouse (columnar)            │
│   • spool (exists)                     │ /HTTPS │   • events_raw  (TTL)             │
│   • TelemetryRecorder trait            │ ─────► │   • MVs: cmd/skill/edit rollups   │
│ vox-telemetry-otlp (L3, COMPILE-OUT)   │        │  Grafana/Metabase dashboards      │
│   • redaction pass (egress boundary)   │        │  (skill-surfacing, cmd freq, …)   │
│   • OTLP LogRecord exporter (0.32)     │        └──────────────────────────────────┘
└───────────────────────────────────────┘
        feature = "telemetry-remote"  (off → no otel/reqwest symbols in the binary)
```

> **Compile-out, precisely:** the `vox-telemetry` facade *always* compiles and is cheap — its
> `record_event!` is a **runtime** no-op until a recorder is registered (it checks
> `global_recorder()`). The *compile-out unit* is the **exporter crate** `vox-telemetry-otlp`:
> with `--no-default-features` (no `telemetry-remote`) it is an empty shim and the binary's
> dependency tree contains zero `opentelemetry`/`reqwest` symbols. Do NOT `#[cfg]`-gate the
> facade itself. The success-criteria symbol test targets the **binary** (`vox-cli`) dep tree.

### Crate plan (lean; avoid ballooning)
- **Keep** `vox-telemetry` (L1) zero-network: only `record_event!`, config, types, spool trait.
  Adding otel/reqwest here would poison build times for all ~60 dependents — **forbidden**.
- **New** `vox-telemetry-otlp` (L3, `kind="library"`, feature-gated): implements
  `TelemetryRecorder`, owns the OTLP exporter + redaction. Pulls `opentelemetry*`, `reqwest`,
  `governor`. Registered at binary startup *only* when `telemetry-remote` + consent are on.
- **Extend** `vox-telemetry::config`: install-UUID + consent persistence (uses file IO it
  already does; **no new deps**).
- **Extend** `vox-cli` (L5): first-run consent prompt + `vox telemetry {status,preview,on,off}`.
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

## 7. Out of scope (YAGNI)
Differential-privacy noise injection (research it; ship k-anonymity first), real-time
streaming dashboards, per-user data-subject portal, ML on telemetry. Revisit post-MVP.

## 8. Success criteria
- Build `vox` with `--no-default-features` (no `telemetry-remote`) → **zero** otel/reqwest
  symbols in the binary; `record_event!` is a no-op (verified by a symbol/dep test).
- Two-tier consent honored; `preview` shows exactly what `upload` would send; redaction
  guardrail tests pass (no denied field class ever serialized).
- End-to-end: a local event opt-in-uploads → lands in ClickHouse → appears in a dashboard.
- New crate within LoC budget; arch-check green; workspace build-time delta within budget.
```

