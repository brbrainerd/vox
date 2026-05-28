---
title: "Data Storage SSOT (2026)"
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Single source of truth for data storage architecture"
audience: ["contributors", "agents"]
related:
  - docs/src/architecture/data-storage-migration-backlog-2026.md
  - docs/src/architecture/data-storage-lint-and-ci-spec-2026.md
  - contracts/db/baseline-version-policy.yaml
  - contracts/db/retention-policy.yaml
  - contracts/db/data-storage-policy.v1.yaml
  - AGENTS.md
---

# Data Storage SSOT (2026)

## 1. Status

- **Doc status**: proposed (2026-04-21). Supersedes all prior drafts. Three prior drafts asserted the existence of `vox-schema`, `vox-spool`, and `vox-observability` crates that do not exist in the workspace; those assumptions are retracted here. Every crate name in this document has been verified against `Cargo.toml` `[workspace.dependencies]`.
- **Scope**: how Vox persists, represents, and governs data across libSQL/Turso, `contracts/`, JSONL spools, content-addressed blobs, ephemeral cache, and Rust in-memory types.
- **Companion docs**: [migration backlog](data-storage-migration-backlog-2026.md), [lint and CI spec](data-storage-lint-and-ci-spec-2026.md). This doc is the "why and what"; those are the "how and when".
- **This is a greenfield migration.** We are not preserving field-level backward compatibility with pre-0.4 stores; we preserve semantic intent and re-baseline the DB.

## 2. Why this document exists

Vox has accumulated four overlapping persistence mechanisms, each drifting toward its own de-facto SSoT:

1. **libSQL/Turso** — canonical domain store (`vox-db`, baseline DDL in Rust).
2. **JSON/YAML contracts** — canonical wire and configuration shapes (`contracts/`), but DDL lives in Rust, not in contracts.
3. **Hand-rolled file queues** — `vox-cli::telemetry_spool` (one JSON per file), plus ad-hoc `append(true)` opens scattered through the workspace.
4. **Loose filesystem artifacts** — model weights, training corpora, packaged modules, `vox_hardened.db` (orphan), several databases at the repo root, `target-*` sibling profile dirs.

Additionally, Rust types conflate two jobs: representing libSQL rows and being the wire format. Serialization derives are sprinkled on storage types, making schema churn expensive. Tracing init lives in two places (`vox-actor-runtime::observability`, `vox-cli-core::init_tracing_for_cli`) with duplicated `EnvFilter` setup.

This SSOT declares a four-tier model, identifies concrete findings against the current code, and hands off to the migration backlog and lint spec. Every finding has a fixed ID (F-N) and is cross-referenced by a migration item (M-N) and at least one guard check.

## 3. Non-negotiables (decided constraints)

These were resolved in the audit clarifying pass. They are not up for debate in this doc.

1. **`contracts/` is canonical for wire and DDL shape** — not Rust code. Rust code is generated from contracts. `contracts/db/baseline-version-policy.yaml` already partially encodes this model (it claims to be SSoT for the baseline integer and digest).
2. **libSQL/Turso stays** as the Tier A engine. No sled/heed/redb/rocksdb.
3. **VoxScript-first for migration glue** per `AGENTS.md §VoxScript-First Glue Code`. No `.sh`, no `.ps1`, no `.py` helpers.
4. **Secret plane is `vox-secrets`** per `AGENTS.md §Secret Management (Required, SSOT)`. No direct `env::var("TURSO_*")` outside the vox-secrets allowlist.
5. **Frozen Core crates** (per `contracts/db/data-storage-policy.v1.yaml#frozen_core_crates` — canonical list since `crates/_frozen.md` was retired 2026-05-24) keep their public surface stable. Tier A DDL changes in frozen crates need governance sign-off.
6. **Archive Protocol**: `archive/` and `docs/src/archive/` are frozen inputs. Auditors and agents must not regenerate against them.

## 4. Target state: four-tier data model

### 4.1 Tier A — Relational (libSQL/Turso)

Canonical persistence for anything that has identity, relations, or constraints.

- **Owner**: `vox-db` (API facade + migrations). `vox-package` opens the project-local file through a vox-db handle, not through a string literal.
- **Databases**:
  - User-global: `$VOX_DATA_DIR/store.db` — conversations, agents, events, Arca/Codex core. Baseline DDL assembled from `SCHEMA_FRAGMENTS` in `crates/vox-db/src/schema/manifest.rs`.
  - Project-local: `<repo>/.vox_modules/local_store.db` — package manager index, materialized from `vox.lock`. Regenerable; opened today via bare string literal in `crates/vox-cli/src/commands/{update,sync,search,pm_lifecycle}.rs` (F6).
  - Retired: `research-audit-codex.db` (fold into `store.db` with namespaced prefix, M-24); `vox_hardened.db` at repo root (orphan — no crate opens it — delete in M-25).
- **Schema**: 19 domain fragments (`foundation`, `clavis_cloudless`, `cas_codex`, `conversations`, `agents`, `ci_completion`, `developer_journeys`, `exec_time`, `execution`, `external_review`, `gamification_coordination`, `knowledge`, `mens_intelligence`, `packages`, `publish_cloud`, `scientia`, `toestub_build`, `visus`, `vox_mesh`) under `crates/vox-db/src/schema/domains/`. `BASELINE_VERSION = 59` in `manifest.rs` (bumped from 55 → 58 → 59 in commits `411adcac` and `ff0fdccc` on 2026-04-21; this integer is a moving target — the guard must read the live value, not hard-code it). Contract `contracts/db/baseline-version-policy.yaml` currently pins `repository_baseline_integer: 54` — **live drift**, now 5 versions behind, see F1. The scientia domain gained two telemetry tables on 2026-04-21 (`model_scoreboard`, `model_pricing_catalog`); see §F75.
- **Access policy**: `turso::Connection` / `libsql::Connection` may be opened only inside `vox-db`, `vox-secrets`, `vox-test-harness`. Everywhere else goes through the `vox-db` facade.

### 4.2 Tier B — Append-only JSONL spools

Hot-path event streams that cannot afford a synchronous libSQL write: telemetry events, behavior traces, simulation records, long-running spans.

- **Layout on disk**: `$VOX_SPOOL_DIR/<channel>/YYYY/MM/DD/HH.jsonl`.
- **Record shape**: one JSON object per line, including `event_schema: "vox.telemetry.<event>.v<N>"` so a reader can parse a mixed stream.
- **Rotation**: hourly, daily, or size-based.
- **Writer TODAY**: hand-rolled in `crates/vox-cli/src/telemetry_spool.rs` (one-JSON-per-file, not JSONL); also 51 files across the workspace call `.append(true)` or open `.jsonl` directly without shared rotation or fsync discipline.
- **Writer TARGET**: a single shared writer — see §5.2 for the crate decision.
- **Promoter**: `vox spool promote` drains a channel into Tier A rows, then truncates. Promotion is one-way; after a file is promoted it is deletable.

### 4.3 Tier C — Content-addressed artifacts

Large immutable blobs: model weights, corpus shards, compiled module artifacts, screenshots, large response payloads.

- **Layout**: `$VOX_DATA_DIR/artifacts/sha256/<first2>/<rest-of-hash>`.
- **Indirection**: Tier A rows store only `sha256: TEXT NOT NULL` and `size_bytes: INTEGER NOT NULL`. Any row that references a blob > 4 KiB stores its digest here, never the blob itself.
- **Owner**: TBD — **Open Question #3**. `crates/vox-checksum-manifest` today is a *release asset integrity verifier* (parses `checksums.txt`, verifies downloaded tarballs); it is NOT a blob store. Candidates: (a) extend `vox-db` with a `cas` submodule, (b) introduce a new `vox-cas` crate, (c) re-purpose `vox-bounded-fs`. Decision deferred to a spike in Phase 3 (M-29-spike).
- **Retention**: `contracts/db/retention-policy.yaml` already has a `kind: keep_forever | manual | days | ms_days | expires_lt_now` vocabulary; extend for CAS GC in M-30.

### 4.4 Tier D — Ephemeral cache

Anything under `$VOX_CACHE_DIR` is deleteable without data loss. Includes: download caches, parse caches, generated fixture data, and `target/dogfood/` training dumps from `vox-corpus` (hardcoded today as `CANONICAL_TRAIN_DATA_DIR` in `crates/vox-corpus/src/training/mod.rs:13`).

- **Policy**: no Tier D path may appear as a string literal outside `vox-config`, `vox-cli` init, or explicit tests.
- **TTL**: documented default 30 days; `vox db doctor --gc` expires contents older than that.

### 4.5 What does NOT fit any tier

Three things sit outside the tiers and are tracked separately:

- **Secret plane** (`vox-secrets`, Infisical / Vault / cloudless vault). Not a Vox-owned persistence tier; vox-secrets mediates access.
- **VCS state** (`.git/`, `.jj/`). Off-limits to every Vox crate. Ignored by the guard.
- **Vendored patches** (`patches/`). Cargo patch inputs; not Vox data.

## 5. Crate choices (explicit, with alternatives)

This is the most-changed section of this SSOT. Three prior drafts assumed new crates (`vox-schema`, `vox-spool`, `vox-observability`) without checking the existing workspace. That assumption is retracted. The decisions below are grounded in the real 66-crate workspace.

### 5.1 Schema codegen (contracts → Rust + DDL)

**Decision**: host codegen as a new module tree inside **`vox-jsonschema-util`**, NOT a new top-level crate, until a second distinct consumer appears.

- Why not a new `vox-schema` crate? Because `vox-jsonschema-util` already exists, already holds JSON Schema compilation helpers, and already has a tested surface. Every migration ticket that names "codegen pipeline" lands in a submodule of that crate: `vox_jsonschema_util::codegen::{loader, validator, emit_rust, emit_sql, diff}`.
- If and when a THIRD distinct consumer appears (beyond validator + emitter), consider extracting. Tracked as **Open Question #1**.
- Generated output lands in `crates/vox-db/src/schema/generated/` and `crates/<consumer>/src/generated/`, gitignored under `crates/**/src/generated/`, with a stable hash in a checked-in manifest so CI can verify drift without re-generating on every run.

### 5.2 Tier B JSONL spool writer

**Decision**: promote `crates/vox-cli/src/telemetry_spool.rs` to a first-class, workspace-shared module. New crate `vox-spool` is **created as part of migration item M-03**, in the same PR, not pre-scaffolded.

- Today's `telemetry_spool.rs` is a *one-JSON-per-file* queue (layout: `pending/<uuid>.json`), which is fine for low-volume upload queuing but is the wrong shape for high-volume structured events. M-03 widens the API to JSONL records + rotation, moves it into its own crate, and inverts the dependency so `vox-cli` depends on the library.
- **No pre-scaffolded `crates/vox-spool/` directory exists.** The crate lands only when M-03 executes, at which point M-03 itself updates `[workspace.dependencies]` and writes the `Cargo.toml` / `src/lib.rs` in the same PR.
- Alternative rejected: putting the writer inside `vox-primitives`. Rejected because `vox-primitives` is a tiny-dep / std-only crate (`backoff`, `id`) per its current charter; a writer needs `tokio` + `serde_json` + rotation + fsync, which would pollute primitives.

### 5.3 Observability init

**Decision**: consolidate `crates/vox-actor-runtime/src/observability.rs` and `crates/vox-cli-core/src/lib.rs::init_tracing_for_cli` into a single canonical `vox_actor_runtime::observability::init(policy)` function, and have `vox-cli-core` thinly call it.

- No new `vox-observability` crate. That was a phantom in the previous draft. `vox-actor-runtime` already owns rich observability (request context, ID generation, `tracing_subscriber::fmt` init, `EnvFilter`); promoting it to sole owner is cheaper than extracting.
- Subscriber behavior is parameterized by `contracts/telemetry/subscriber-policy.v1.yaml` (authored in M-40) rather than hardcoded.
- `vox-cli-core::init_tracing_for_cli` becomes a two-line wrapper that passes a `CliProfile` policy variant.

### 5.4 Tier C CAS owner

**Decision deferred** to a 1-week spike in Phase 3 (M-29-spike). Tracked as **Open Question #3**. Default assumption for planning: `vox-db::cas` submodule with a `cas_blobs(sha256 TEXT PRIMARY KEY, size_bytes INTEGER, created_at INTEGER)` index row plus a filesystem layout under `$VOX_DATA_DIR/artifacts/`.

### 5.5 Secret plane

No change. `vox-secrets` stays. Every `VOX_DB_URL` / `VOX_DB_TOKEN` resolution flows through `vox_secrets::resolve_secret(...)`. `VOX_DB_URL` and `VOX_DB_TOKEN` are today read inside `crates/vox-db/src/config.rs`, bypassing vox-secrets; that is F49 and gets fixed in M-20.

## 6. Canonical schema pipeline

```text
 contracts/db/<domain>.v1.yaml            ← hand-edited SSOT
          │
          ▼
 vox_jsonschema_util::codegen::loader      ← parse + validate against index.schema.json
          │
          ├── ::emit_rust  ──→ crates/vox-db/src/schema/generated/<domain>.rs   (row structs, no Ser/De)
          │                     crates/<consumer>/src/generated/<domain>.rs    (wire DTOs with serde)
          │
          ├── ::emit_sql   ──→ crates/vox-db/src/schema/generated/<domain>.sql  (DDL fragments)
          │
          └── ::diff       ──→ vox ci data-storage-guard schema-codegen-drift   (fails on divergence)
```

Invariants:

- Rust row structs (`row/<domain>.rs`) never derive `Serialize` / `Deserialize`.
- Wire DTOs (`dto/<domain>.rs`) derive both, and live in a separate module tree.
- A `From<Row> for Dto` / `TryFrom<Dto> for Row` pair lives in `conv/<domain>.rs`. Generated.

## 7. Findings (F1–F78)

Each finding has an ID, a one-line summary, a file-and-line anchor, and a link to the migration ticket that fixes it. Findings retain their numbers across drafts for stable cross-reference.

### A. Tier A — libSQL / Turso surface

- **F1**. `BASELINE_VERSION: i64 = 59` in `crates/vox-db/src/schema/manifest.rs` (HEAD 2026-04-21 after `ff0fdccc`) disagrees with `repository_baseline_integer: 54` in `contracts/db/baseline-version-policy.yaml`. The contract claims to be SSoT for this integer and for the baseline digest; in practice the Rust constant moved forward three times in the last two days (55 → 58 in `411adcac`, 58 → 59 in `ff0fdccc`) while the contract was not touched. **Live drift, widening.** Fixed by M-20 (digest + version re-sync; must read the Rust constant at build time, not hard-code a number) and gated by existing `vox ci check-codex-ssot` once M-20 wires auto-update. Because the gap is now widening with each orchestration-schema PR, M-20 should land before Phase 2 rather than mid-Phase-2 as originally sequenced.
- **F2**. No delta migration framework — `BASELINE_VERSION` monotonically grows but every prior baseline is collapsed into one monolithic DDL. Users upgrading mid-version need a forward migration path rather than a drop-and-recreate. M-21.
- **F3**. 19 domain fragments are authored in Rust (`crates/vox-db/src/schema/domains/*.rs`). Contract-side YAML equivalents do not exist. Inverts the stated "contracts/ is canonical" policy. M-22 emits the YAML side, M-23 flips ownership.
- **F4**. `research-audit-codex.db` (a second libSQL file at `.vox/research-audit-codex.db`, 1.4 MB on disk) duplicates the ops-storage pattern of `store.db` without a documented reason. Fold into `store.db` with a table-namespaced prefix. M-24.
- **F5**. `vox_hardened.db` at repo root (1.8 MB) has NO code references — zero grep hits under `crates/`. Orphan artifact from a past experiment. **Delete in M-25; do NOT migrate.**
- **F6**. `.vox_modules/local_store.db` is opened via bare string literal in `crates/vox-cli/src/commands/update.rs:26`, `sync.rs:42`, `search.rs:43`, `pm_lifecycle.rs:20` rather than through `vox-db` and `vox-config::paths`. Hard-codes the path in four places. M-26.
- **F7**. `crates/vox-db/src/collection.rs:83` uses string-interpolated SQL (`format!("INSERT INTO {}", ...)`) instead of a bound parameter. Small but real SQLi vector if the caller is wrong. M-27.
- **F8**. `crates/vox-db/src/codex_legacy.rs:50-51` uses fuzzy `is_legacy_schema_chain` heuristics instead of a committed-to baseline digest. M-28.
- **F9**. Row-by-row Turso workaround at `crates/vox-db/src/store/ops_retention.rs:101-111` — documented but not isolated. M-29-docs documents module-level; M-21 considers removing once Turso batch support matures.
- **F10**. Historical note: legacy Turso-prefixed env aliases (`TURSO_*`, `VOX_TURSO_*`) appear across `vox-secrets/src/backend/vox_vault.rs` and `vox-secrets/src/lib.rs` constants. Redundant with the canonical `VOX_DB_URL` / `VOX_DB_TOKEN`. M-30 (sunset).

### B. Contracts — canonical shape

- **F11**. No codegen pipeline exists: contracts are hand-maintained in parallel with Rust structs. `contracts/orchestration/agent-harness.schema.json` (6836 B) is hand-written alongside `crates/vox-orchestrator/src/harness.rs`, and drift is not enforced by any `vox ci` sub-check. M-10 (first codegen inside `vox-jsonschema-util`) and M-11 (first consumer).
- **F12**. `x-vox-version: N` convention is partial — some contracts use it, many don't, and filename `.vN.` suffix usage is inconsistent. No CI check enforces parity. M-12.
- **F13**. `contracts/config/env-vars.v1.yaml` does NOT exist despite being the natural home for env-var documentation. Env contract is "whatever you `env::var(...)` in Rust". M-13 creates the file; M-14 enforces parity.
- **F14**. `contracts/telemetry/events.v1.yaml` does NOT exist; telemetry events are spelled in individual schema JSONs like `completion-run.v1.schema.json` with no index. M-15 authors the event catalog.
- **F15**. `contracts/telemetry/subscriber-policy.v1.yaml` does NOT exist. Subscriber init is hardcoded. M-40 authors.
- **F16**. `schemas/` directory at repo root is empty but tracked. Artifact from an old retired pipeline. Delete in M-05.
- **F17**. `dist/schemas.ts` is a TypeScript frontend artifact — unclear whether it is checked-in source or a committed build output. No regeneration step in CI. M-31 spikes.
- **F18**. OpenAPI contracts (`contracts/populi/*.openapi.yaml`) reference types inline rather than via `$ref` into canonical JSON schemas. Accidental duplicate type definitions. M-16.
- **F19**. Multiple formats per domain: e.g. `contracts/mcp/` mixes `.yaml`, `.schema.json`, `.openapi.yaml`. No rule documents which to prefer. M-17.
- **F20**. No SSOT-digest check on `baseline-version-policy.yaml` — it can drift from `vox_db::schema::schema_baseline_digest_hex()` silently. F1 is today's evidence. M-20 adds the auto-update CI fix.

### C. JSONL / log / sidecar / file-based persistence

- **F21**. `crates/vox-cli/src/telemetry_spool.rs` queues one-JSON-per-file in `pending/<uuid>.json`. Misleading name ("spool" implies append-only rotation). Consolidate with M-03.
- **F22**. 51 files in the workspace call `.jsonl` or `OpenOptions::append(true)` directly, most without a rotation policy or fsync discipline. No shared writer. M-32 inventories, M-33 migrates.
- **F23**. `build_errors.txt` at repo root: raw compiler output, checked in, never refreshed. Stale artifact. M-06 deletes.
- **F24**. `codex-cutover-20260412T065226Z.sidecar.json` and a twin file at repo root: one-off sidecars from a past migration. M-06 deletes.
- **F25**. `test_lexer.rs`, `error.vox`, `prototype_vox_tokenizer.json` at repo root: loose files from past experiments. M-06.
- **F26**. `vox-agent.json`, `vox-schema.json`, `vox.tokens.json` at repo root are referenced by `.voxignore` / `.aiignore` but are checked into git. Contradiction. M-07.
- **F27**. `scratch/` at repo root is a dev-only workspace but has no `.gitkeep` guard. M-08.
- **F28**. `.vox/` contents on a fresh clone are whatever the last session left: `.vox/agents/`, `.vox/artifacts/`, `.vox/bin/`, `.vox/cache/`, `.vox/memory/`, `.vox/sessions/` are all *runtime* directories but many appear in tracked paths. Clean separation between build-time and runtime `.vox/` contents is undocumented. M-09.
- **F29**. No repo-index cache manifest: `vox-corpus` writes to `target/dogfood/` (`CANONICAL_TRAIN_DATA_DIR` in `crates/vox-corpus/src/training/mod.rs:13`), but nothing catalogs what's there, what expires, or who reads it. M-34.
- **F30**. Scattered `OpenOptions::append(true)` calls outside tests. M-03 + grep rule.
- **F31**. `target-{agent-verify,stubcheck,stubcheck2,stubcheck3,stubcheck4,ws-check}` and `target_new` — abandoned per-profile cargo outputs, not in `.gitignore`. M-35.

### D. Rust in-memory types — separation of concerns

- **F32**. `crates/vox-db/src/store/types/` mixes libSQL row structs with `#[derive(Serialize, Deserialize)]`. Row and wire responsibilities conflated. M-50.
- **F33**. Same pattern in `crates/vox-orchestrator/src/types/`. M-51.
- **F34**. Same pattern in `crates/vox-gamify/src/schema/`. M-52.
- **F35**. `ObservationReport` leaks `libsql::Value` across the observability boundary. M-53.
- **F36**. No shared error primitives crate-wide — each crate re-rolls `anyhow::Error` or `thiserror`. M-54.
- **F37**. `#[repr(C)]` discipline is case-by-case. Policy-only, no migration item; each annotated struct needs a one-line justification comment.
- **F38**. `smol_str::SmolStr` is adopted unevenly. Performance benefit worth benchmarking in `vox-actor-runtime`. Deferred — M-55b.
- **F39**. `SmallVec<[T; N]>` not adopted for documented-small vectors (`intent_tags`, CLI `args`). Deferred — M-55c.
- **F40**. `serde(rename_all = ...)` inconsistency: kebab-case in some CLI crates, PascalCase in some wire crates. Canonical: snake_case (default), camelCase allowed for outward HTTP only. M-55.
- **F41**. `crates/vox-orchestrator-types/` is a separate types-only crate — good. Still mixes row + wire; noted under F33. Kept separate to preserve ownership split.
- **F42**. Blocking-in-async risk: `vox-db` already encapsulates its own `block_on` helper behind an allowlist. Other crates import `futures::executor::block_on` or `tokio::task::block_in_place` ad-hoc. M-56 enables clippy `disallowed-methods`.
- **F43**. `bumpalo`-style arena allocation is not used anywhere. Profile-driven; deferred — M-57d.

### E. Config and env vars

- **F44**. `crates/vox-config/src/paths.rs` already exposes `data_dir()`, `default_db_path()`, `state_dir()`, `config_dir()`, `dot_vox_user_dir()`, `mcp_sessions_dir()` — but NOT `VOX_CACHE_DIR`, `VOX_SPOOL_DIR` as first-class overrides. Partial today. M-57 fills in.
- **F45**. `VOX_TELEMETRY_SPOOL_DIR` at `crates/vox-cli/src/telemetry_spool.rs:13` is a one-off name; should be `VOX_SPOOL_DIR`. M-36 (sunset).
- **F46**. `VOX_USER_ID` (read in `crates/vox-config/src/paths.rs:52`) has no documented deterministic default; agent runs produce differing user IDs session-to-session, muddling Tier A rows' `created_by`. M-58.
- **F47**. No env contract file — `VOX_*` env vars are grepped out of source, not declared. M-13 + M-14.
- **F48**. No XDG support — `$VOX_DATA_DIR` defaults to `.vox/` under the working dir, not `$XDG_DATA_HOME/vox/`. M-57.
- **F49**. `VOX_DB_URL` / `VOX_DB_TOKEN` are read directly in `crates/vox-db/src/config.rs`, bypassing `vox-secrets`. AGENTS.md §Secret Management requires vox-secrets as the sole resolver. M-20 routes through vox-secrets.

### F. Observability

- **F50**. Two tracing-subscriber inits: `crates/vox-actor-runtime/src/observability.rs:49` (rich) and `crates/vox-cli-core/src/lib.rs:29` (thin). Duplicated `EnvFilter` setup. M-40 consolidates.
- **F51**. No structured event macro — `tracing::info!(...)` calls do not emit events to the Tier B spool. M-42.
- **F52**. No span registry — span names like `"completion_run"` are authored free-form. M-43.
- **F53**. `build_errors.txt` is the closest thing to a build log, and it's neither structured nor rotated. F23 already tracks deletion; observability angle is M-41.
- **F54**. `tracing-appender` (non-blocking file appender) is not wired in. M-41.
- **F55**. No `RUST_LOG` documentation — devs guess the level strings. M-44.

### G. Governance, CI, regression gates

- **F56**. No `vox ci data-storage-guard` sub-check exists. 42 sub-checks are registered under `crates/vox-cli/src/commands/ci/` but data storage as a cross-cutting concern is not one of them. M-04 scaffolds; M-60 makes it a hard gate.
- **F57**. Golden/frozen fixture policy is ad-hoc — different crates use `insta`, `expect-test`, or hand-rolled expected strings. Tracked as M-64b.
- **F58**. Property / roundtrip tests are missing for Tier A row structs. M-64.
- **F59**. No fuzz targets for wire schemas. M-62.
- **F60**. Tests can touch real `.vox/store.db` if a dev runs them without isolation; no `tempdir` convention enforced. M-65.
- **F61**. No strace / pytrace canary to verify no unexpected file writes during a benchmark run. M-63.

### H. Gaps from prior-draft verification (F62–F74)

- **F62**. `crates/vox-checksum-manifest` was mischaracterized in prior drafts as a content-addressed blob store. It is in fact a *release asset integrity verifier* — parses `checksums.txt`, verifies downloaded tarball bytes (public API: `sample_checksum_basenames()`, `checksum_for_asset()`). It should NOT own Tier C. M-29-spike resolves the real owner.
- **F63**. `vox-observability` was a phantom crate in prior drafts — does not exist and does not need to. See §5.3.
- **F64**. `vox-schema` was a phantom crate in prior drafts. Codegen is a module tree inside `vox-jsonschema-util`. See §5.1. Note: pre-scaffolded `crates/vox-schema/` and `crates/vox-spool/` directories from the prior draft were deleted because `[workspace.members] = ["crates/*", ...]` was auto-including non-compiling scaffolds.
- **F65**. `vox-spool` is not pre-created; it lands only when M-03 executes. See §5.2.
- **F66**. ~~`crates/_frozen.md` Frozen Core Ledger~~ Resolved 2026-05-24: file retired; canonical list is `contracts/db/data-storage-policy.v1.yaml#frozen_core_crates`. M-68 closed.
- **F67**. `dist/schemas.ts` provenance is unknown. M-31 spikes: is it source or build output?
- **F68**. `.jj/` is Jujutsu VCS state; coexists with `.git/`; never touched by Vox crates. Document and allowlist in the guard. M-71.
- **F69**. `tree-sitter-vox/GRAMMAR_SSOT.md` exists and is a grammar SSOT. Link it from Related Documents; wire a parity check. M-72.
- **F70**. `.gitignore` hygiene — no automated BOM check. Policy-only, rolled into M-74.
- **F71**. `patches/` directory is Cargo vendored-patches — third-party, not Vox data. Allowlist in guard. M-73.
- **F72**. `examples/` and `apps/interop/marquee_app/` top-level dirs: if they persist data, routing must go through the tiered model. M-73.
- **F73**. `tools/` and `infra/` top-level dirs: similar to F72. M-73.
- **F74**. `telemetry-trust-ssot.md`, `telemetry-remote-sink-spec.md`, and `telemetry-implementation-blueprint-2026.md` are referenced by `AGENTS.md §Telemetry trust (SSOT)` but do NOT exist in `docs/src/architecture/`. M-40 is blocked on authoring at least the trust SSOT. Tracked here so downstream migrations know the prerequisite.

### I. Findings from 2026-04-21 reconciliation pass (F75–F78)

Added after cross-checking the planning docs against the 24-hour commit window `411adcac..ff0fdccc..d5365d43..29c11a4e..8594df02..6249453b` (seven commits, ~4,000 LOC net, landing orchestration routing SSOT + model admin subcommand tree).

- **F75**. New orchestration contract surface at `contracts/orchestration/` — 14 files at HEAD including `model-routing.v1.yaml`, `model-routing.v1.schema.json`, `providers.v1.yaml`, `model-catalog.bootstrap.v1.json`, plus seven `.schema.json` files (`agent-harness`, `agent-vcs-facade`, `context-lifecycle-telemetry`, `context-work-item`, `journey-envelope.v1`, `orch-daemon-rpc-methods`, `repo-reconstruction`, `vox-generate-code-file-outcomes`). This is a first-class *contract domain* that the SSOT's initial inventory did not list. The `scientia` schema fragment gained two Tier A telemetry tables (`model_scoreboard`, `model_pricing_catalog`) as sinks for this routing surface; the `vox-cli model {costs,discover,explain,pricing,rollup,scoreboard}` subcommand tree (`crates/vox-cli/src/commands/model/`) reads/writes them. Action: SSOT §4 Tier A must enumerate the model telemetry tables as an expected Tier A surface; lint spec's `domain-format-single` must allowlist `contracts/orchestration/` as a mixed-format domain (MCP-like exception); backlog gets M-75 to cross-link the new domain and enforce provider-secret parity. Also added: `run_routing_ssot_guard` is already wired into `data-ssot-guards` (`crates/vox-cli/src/commands/ci/run_body_helpers/data_ssot_guards.rs:165,282`) — do not duplicate in `data-storage-guard`.
- **F76**. **Version-header field name is inconsistent** across contract files. Survey at HEAD:
  - `contracts/db/data-storage-policy.v1.yaml` uses both `version: 1` and `x-vox-version: 1` (redundant).
  - `contracts/orchestration/model-routing.v1.yaml` uses `schema_version: 1`.
  - `contracts/orchestration/providers.v1.yaml` uses `schema_version: 1`.
  - Older JSON schemas use neither (rely on filename + `$id`).

  The lint spec's `version-header-parity` rule (§1.3) was written assuming only `x-vox-version: N`; against HEAD it would false-fail on every orchestration file. Action: (a) extend the rule to accept the set `{x-vox-version, schema_version, version}` as synonyms for now, (b) add M-76 to converge on `x-vox-version` via a contract-renaming migration that also retires the duplicate `version: 1` in the policy file.
- **F77**. `contracts/orchestration/providers.v1.yaml::providers[].secret_id` is a new secrets-parity surface. Each entry's `secret_id` (e.g., `GeminiApiKey`, `OpenRouterApiKey`, `GroqApiKey`) must correspond to a registered ID in `crates/vox-secrets/src/spec/ids.rs` (see `ff0fdccc` diff: `crates/vox-secrets/src/spec/ids.rs` gained +31 lines and `crates/vox-secrets/src/spec/registry/llm.rs` is a new 165-line module registering these). Today this parity is not CI-checked. Action: M-77 adds a guard sub-check `provider-secret-parity`.
- **F78**. **New `model_*` Tier A tables lack a retention-policy entry.** `contracts/db/retention-policy.yaml` does not yet name `model_scoreboard` or `model_pricing_catalog`; without an explicit retention rule these tables grow unboundedly from `llm_interactions` rollups. Action: M-78 adds retention rules and backfills the policy contract; the existing `vox ci data-ssot-guards` path (`run_scientia_consumption_registry_guard` sibling) gets extended to validate every table in a domain fragment has either a retention rule or an explicit `retention: append-only` marker.

## 8. Migration phases (seven)

Phases are a summary. Ticket-level detail is in the [migration backlog](data-storage-migration-backlog-2026.md). Every migration item is broken into numbered sub-steps (1, 2, 3, …) with exact file paths so a future LLM or human contributor can execute without re-deriving context.

- **Phase 0 — Scaffolding**: guard stub, data-storage policy contract schema, `.cursor/rules/data-storage-policy.mdc`, first VoxScript migration script, SSOT/backlog/lint-spec documents cross-indexed.
- **Phase 1 — Contracts-canonical first cut**: contracts version header parity (M-12), first codegen pipeline inside `vox-jsonschema-util` (M-10, M-11), env-vars contract authored (M-13), format consolidation (M-17), OpenAPI drift (M-16), orchestration harness drift gate (M-11).
- **Phase 2 — DDL flip**: emit SQL side of the pipeline (M-22), flip DDL ownership to contracts (M-23), auto-maintain baseline digest (M-20), add delta migration framework (M-21), document Turso workaround (M-29-docs), sunset `TURSO_*` env vars (M-30).
- **Phase 3 — Tier B / C rollout**: promote telemetry spool to new `vox-spool` crate (M-03), CAS owner spike and implementation (M-29-spike + M-30), canonical `VOX_SPOOL_DIR` (M-36), repo index cache manifest (M-34), JSONL sprawl inventory + migration (M-32, M-33), `target-*` cleanup (M-35), `dist/schemas.ts` provenance (M-31).
- **Phase 4 — Observability unification**: single subscriber init (M-40), `tracing-appender` wiring (M-41), structured event macro to Tier B (M-42), span registry (M-43), `RUST_LOG` docs (M-44), `telemetry-trust-ssot.md` authored as F74 prerequisite.
- **Phase 5 — Memory hygiene**: row/dto/conv split in `vox-db` (M-50), orchestrator (M-51), ludus (M-52); `ObservationReport` fix (M-53); shared error primitives (M-54); serde `rename_all` policy (M-55); blocking-in-async deny (M-56); XDG dirs (M-57); env var contract enforcement (M-58); `VOX_USER_ID` default (M-59).
- **Phase 6 — Regression gates (hard)**: promote `data-storage-guard` from stub to blocker (M-60); deny.toml bans (M-61); fuzz (M-62); strace canary (M-63); Tier A roundtrip tests (M-64); test DB isolation (M-65); `.vox/` cleanliness (M-66).
- **Phase 7 — Gap cleanup**: `.vox_modules/local_store.db` path discipline (M-26, already above — Phase 7 adds the ancillary ticket M-67 for fold-plan IF M-29-spike says vox-db should own it); Frozen Core cross-link (M-68); `dist/schemas.ts` gate (if M-31 spike resolves "source", then M-69); `target-*` gitignore (M-70); `.jj/` coexistence (M-71); grammar SSOT parity (M-72); top-level dir READMEs (M-73); `.gitignore` hygiene (M-74).

## 9. Acceptance criteria

This work is considered "done" when all of:

1. `vox ci data-storage-guard` is wired, green, and a hard blocker on `main`.
2. `BASELINE_VERSION` in code and `baseline-version-policy.yaml` agree, byte-for-byte, and the digest is auto-maintained.
3. Every `.jsonl` spool in the workspace flows through the single Tier B writer.
4. No `turso::Connection` or `libsql::Connection` is opened outside the allowlisted crates.
5. Every `VOX_*` env var in `crates/` is listed in `contracts/config/env-vars.v1.yaml`.
6. No struct under `crates/**/src/**/row/` derives `Serialize` or `Deserialize`.
7. `dist/schemas.ts` provenance is resolved: either tracked as a generated artifact (CI regenerates and diffs) or removed.
8. `vox_hardened.db` is deleted from the repo and from `.gitignore`-allowed creation paths.
9. `.cursor/rules/data-storage-policy.mdc` matches the non-negotiables in §3 byte-for-byte.
10. This SSOT moves from `status: proposed` to `status: current`.

## 10. Open questions

1. **Should schema codegen be its own crate?** Today it lives inside `vox-jsonschema-util` (§5.1). When a third consumer arrives, extract to `vox-schema`. Criterion: if `vox_jsonschema_util::codegen::*` exceeds ~1,500 LOC or grows a non-schema consumer, split.
2. **Should `vox-spool` be created as a crate now, or as part of M-03?** Decision: M-03. The prior draft pre-scaffolded it, which wired a non-compiling crate into `[workspace.members]` (via the `crates/*` glob). Reverted.
3. **Tier C owner** — `vox-db::cas` submodule or new `vox-cas` crate? Deferred to M-29-spike (1 week, Phase 3).
4. **Codegen generator choice** — `schemars`, `typify`, `jsonschema-rs`, or hand-rolled? Deferred to M-10. Preference: start with `jsonschema-rs` for validation + hand-rolled emitter for the first domain (agent-harness), measure, then decide whether to adopt `typify`.
5. **Delta migration framework** — forward-only, or forward + backward? Deferred to M-21.
6. **Tier D TTL** — documented 30 days by default. Overridable per-channel? Tracked in M-57.
7. **Golden fixture standard** — `insta` vs `expect-test` vs hand-rolled. M-64b.
8. **`vox-tensor` vs `vox-ml-cli` boundary** — `vox-tensor` (`burn` wrapper + JSONL DataLoader + LoRA config) is a utility; `vox-ml-cli` is the training pipeline consumer. Document boundary and whether any persistent surfaces cross it. M-72.
9. **`dist/schemas.ts` — source or artifact?** Resolved by M-31.

## 11. Related documents

- [migration backlog](data-storage-migration-backlog-2026.md) — M-00 through M-74.
- [lint and CI spec](data-storage-lint-and-ci-spec-2026.md) — guard sub-checks, clippy, deny.toml, grep rules.
- [`contracts/db/baseline-version-policy.yaml`](../../../contracts/db/baseline-version-policy.yaml) — today's partial contract-side SSoT for baseline version + digest.
- [`contracts/db/retention-policy.yaml`](../../../contracts/db/retention-policy.yaml) — retention vocabulary; extended in M-30.
- [`contracts/db/data-storage-policy.v1.yaml`](../../../contracts/db/data-storage-policy.v1.yaml) — machine-checked policy; consumed by the guard.
- `contracts/db/data-storage-policy.v1.yaml#frozen_core_crates` — Frozen Core list (canonical since `crates/_frozen.md` was retired 2026-05-24).
- [`AGENTS.md`](../../../AGENTS.md) — cross-tool policy surface; see `#voxscript-first-glue-code-required`, `#telemetry-trust-ssot`, `#secret-management-required-ssot`, `#archival-protocol-llm-guard`.
- [`tree-sitter-vox/GRAMMAR_SSOT.md`](../../../tree-sitter-vox/GRAMMAR_SSOT.md) — grammar SSoT.
- ADR [023-optional-telemetry-remote-upload.md](../adr/023-optional-telemetry-remote-upload.md) — remote telemetry upload.

## 12. Appendix: Glossary

- **Baseline DDL** — the monolithic SQL run on a fresh database. Today: `vox_db::schema::baseline_sql()` assembled from `SCHEMA_FRAGMENTS`. Target: generated from `contracts/db/**/*.v1.yaml`.
- **Canonical** — the authoritative source when two artifacts disagree. "Contracts are canonical for wire shape" means: if Rust disagrees with the contract, Rust is wrong.
- **Digest parity** — `schema_baseline_digest_hex()` matches `repository_baseline_digest_hex` in `contracts/db/baseline-version-policy.yaml`. Enforced by existing `vox ci check-codex-ssot`.
- **Tier A/B/C/D** — the four persistence tiers in §4. Every persistent surface MUST declare its tier; "tierless" is not allowed.
- **Tierless surface** — a persistent file/table with no declared tier. Treated as a bug by the guard.
- **Guard** 