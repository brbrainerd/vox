---
title: "TOESTUB scaling rules (SSOT)"
description: "SSOT for the scaling/surfaces detector: Rust analysis foundations, JSON envelope schemas, gold/suppression/remediation contracts, rule IDs and severities, suppressions, policy YAML, governance owners, and CI promotion/canary flags."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# TOESTUB scaling rules (SSOT)

Detector id: **`scaling/surfaces`** (`crates/vox-toestub/src/detectors/scaling.rs`).

Strategic architecture companion: [TOESTUB self-healing architecture 2026](toestub-self-healing-architecture-2026.md) (research synthesis, LLM-maintainability guardrails, Populi/MENS feedback loop).

## Rust lexical foundation (shared detectors)

Rust line-oriented rules use `crates/vox-toestub/src/analysis/token_map.rs`, which classifies spans as **`Comment`** vs **`String`** (plus normal / raw / byte string handling) and optional `syn::parse_file` in `RustFileContext`. The engine builds one context per `.rs` file per run and passes it to `DetectionRule::detect`. Findings may set optional `confidence` (`high` / `medium` / `low`). Rules like `stub/placeholder` and `unresolved-ref/fn-call` skip matches in **any** non-code span. **`security/hardcoded-secret` skips matches whose **start** falls in a comment span** but still reports matches inside **string literals** (where secrets usually appear). Use `Finding::fingerprint()` for stable dedup keys across runs.

## JSON output (CLI)

`toestub --format json` and `ToestubEngine::run_and_report` emit a **v1 envelope**: `schema_version`, `tool_version`, `files_scanned`, `rules_applied`, `rust_parse_failures`, optional `unresolved_ref_hot_callers`, `suppressions_applied`, `suppression_counts_by_family`, and `findings` (same shape as before per finding). Schema: [`contracts/toestub/toestub-run-json.v1.schema.json`](../../../contracts/toestub/toestub-run-json.v1.schema.json). Bare findings array schema (e.g. `findings-latest.json` after `scaling-audit` normalization): [`contracts/reports/scaling-audit/findings-array.v1.schema.json`](../../../contracts/reports/scaling-audit/findings-array.v1.schema.json).

**Parse budget:** `vox ci scaling-audit emit-reports` compares envelope `rust_parse_failures` to **`VOX_TOESTUB_MAX_RUST_PARSE_FAILURES`** (see [env-vars SSOT](../reference/env-vars.md)). PR CI runs a full `crates/` JSON audit with a small cap to catch `syn` drift early.

## Contracts (evaluation / suppression / remediation)

- Gold fixtures (draft schema): [`contracts/toestub/gold-dataset.v1.schema.json`](../../../contracts/toestub/gold-dataset.v1.schema.json) — committed cases: [`gold-dataset.v1.json`](../../../contracts/toestub/gold-dataset.v1.json); run `cargo test -p vox-toestub --test gold_dataset`.  
- Structured suppressions (draft): [`contracts/toestub/suppression.v1.schema.json`](../../../contracts/toestub/suppression.v1.schema.json) — example entry: [`suppressions.v1.example.json`](../../../contracts/toestub/suppressions.v1.example.json); load via `toestub --suppressions PATH`.  
- Remediation lane index: [`contracts/reports/toestub-remediation/REMEDIATION-LANES.yaml`](../../../contracts/reports/toestub-remediation/REMEDIATION-LANES.yaml)  
- CI validation: `vox ci scaling-audit verify` checks scaling policy, `findings-latest.json`, remediation delta JSON schema, lanes YAML, and gold dataset JSON.

## Trust surface & promotion artifacts

| Artifact | Role |
|---------|------|
| [`findings-array.v1.schema.json`](../../../contracts/reports/scaling-audit/findings-array.v1.schema.json) | SSOT shape for `findings-latest.json` |
| [`delta-after-remediation.v1.schema.json`](../../../contracts/reports/toestub-remediation/delta-after-remediation.v1.schema.json) | Typed snapshot for trend / remediation delta |
| `emit-reports` outputs | `board.md` (top files), `promotion-metrics.json` (counts + delta pointer) under [`toestub-remediation/`](../../../contracts/reports/toestub-remediation/) |

## Governance (owners)

| Detector family | Owner | Escalation |
|-----------------|-------|------------|
| `scaling/*`, policy literals | platform-ci | Change `contracts/scaling/policy.yaml` + scaling-audit |
| `unresolved-ref/*` | platform-ci | Canary CLI `--canary-crates`; AST corroboration gated per path |
| `stub/*` | platform-ci | severity / copy in `StubDetector` |
| Contracts & gold harness | platform-ci | `contracts/index.yaml` + `scaling-audit verify` |

## Canary rollout

- **`toestub --canary-crates vox-cli,vox-mcp`**: AST-derived hints for unresolved-ref apply only under matching `crates/<name>/` trees. Omit flag (or pass no value) for full-workspace behavior after promotion.
- **`toestub --feature-flags unresolved-regex-fallback`**: When AST hints exist, unresolved-ref normally reports only callees recorded in syn `ExprCall` `call_sites`. This flag allows regex-backed matches through anyway (more true positives from macros; more noise).
- **`promotion-metrics.json`**: Regenerated on `vox ci scaling-audit emit-reports` for post-rollout validation against `findings_total_latest` and the committed remediation delta snapshot.

## Rule IDs (findings)

| Rule id | Severity | Meaning |
|---------|----------|---------|
| `scaling/blocking-in-async` | Info | `std::fs::*` in an `async` fn (use `tokio::fs` / `spawn_blocking`; allowlist in `contracts/scaling/policy.yaml`) |
| `scaling/thread-sleep-async` | Info | `thread::sleep` under async visitor |
| `scaling/path-literal` | Info | String literals matching SSOT path fragments (`mens/runs*`, etc.) — prefer `vox_scaling_policy` |
| `scaling/magic-limit` | Info | Integers in `magic_numeric_hints` from policy |
| `scaling/regex-new-hot` | Warning | `Regex::new(` without `LazyLock`/`OnceLock` on the line |
| `scaling/unbounded-read` | Info | `std::fs::read_to_string` heuristic |
| `scaling/lines-collect-vec` | Info | `.lines()` + `collect::<Vec` |
| `scaling/repeated-json-parse` | Info | `serde_json::from_str` near loop heuristic |
| `scaling/sql-no-limit` | Warning | SQL string with `SELECT` but no `LIMIT` (heuristic) |
| `scaling/http-client-no-timeout` | Warning | `Client::new()` heuristic |
| `scaling/nested-pairwise-loop` | Info | `(i+1)..` inner loop pattern |
| `scaling/cache-miss-hot-read` | Info | `read_to_string` / `fs::read` / `OpenOptions` shortly after a `for` loop header — batch or cache |
| `scaling/large-in-memory-accumulator` | Info | `Vec::with_capacity(N)` with very large `N` — confirm bound or stream |
| `scaling/env-default-duplication` | Info | Same string literal in `unwrap_or("…")` on multiple `std::env::var` lines — centralize |

## Suppressions

Same-line: `// toestub-ignore(scaling)` or `// toestub-ignore(scaling/<rule-suffix>)`.

## Policy

Thresholds and literals: [`contracts/scaling/policy.yaml`](../../../contracts/scaling/policy.yaml).  
Rust accessors: `vox-scaling-policy` crate.

**Severity note:** Scaling findings default to **Info** so `toestub --mode enforce-strict --rules scaling` can pass while audits still surface issues. Raise individual rules to `Warning` when tightening CI.

## CI enforcement promotion (family-by-family)

1. **P0 — audit signal:** Full-repo JSON snapshots via `vox ci scaling-audit emit-reports` (`toestub --mode audit --format json`). Baseline cut: [`contracts/reports/toestub-remediation/baseline-freeze.json`](../../../contracts/reports/toestub-remediation/baseline-freeze.json).
2. **P1 — scoped gate:** `vox ci toestub-scoped` defaults to `legacy` (errors fail). Keep CI on `--mode legacy` across providers for consistent blocking semantics until a deliberate strictness migration is approved.
3. **P2 — scaling strictness:** Use `toestub --rules scaling` with rising `--min-severity` once per-crate overrides and false positives are stable.

Remediation rollup index: [`contracts/reports/scaling-audit/rollup/INDEX.yaml`](../../../contracts/reports/scaling-audit/rollup/INDEX.yaml).

## Programmatic audit limitations (read before trusting counts)

TOESTUB/scaling checks are **heuristic and line-oriented**, not a substitute for the compiler, Miri, profilers, or load tests.

- **Syntax / pattern matching:** Rules flag shapes in source text (`SELECT` without `LIMIT`, `Regex::new(` in a loop, `std::fs` under `async fn`). Legitimate code can match; bad code can evade.
- **Limited symbol resolution:** `unresolved-ref/fn-call` is still single-file for imports, but syn-backed call sites + `fn` tables (and optional canary gating) reduce string-only false positives. Wildcard `use` and `tests/` trees remain special-cased — **blind spots remain**.
- **`unwired/module`:** Only **private** `mod foo;` declarations are flagged; `pub` / `pub(crate)` file-backed modules are assumed to be reached from other files (typical `lib.rs` / `commands/mod.rs` roots).
- **Severity is intentionally conservative:** Many scaling findings are **Info** so audits stay noisy but CI gates stay usable; promote severities only after burn-down.
- **Behavior and performance:** “Scaling” here means *likely* scalability smells, not measured latency or memory. Validate hot paths with benchmarks and production telemetry.

When a finding looks wrong, prefer a one-line `// toestub-ignore(...)` with a short reason, or a **policy override** in [`contracts/scaling/policy.yaml`](../../../contracts/scaling/policy.yaml) for intentional patterns — not silent detector hacks.

