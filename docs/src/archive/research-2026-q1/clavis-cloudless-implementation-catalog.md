---
title: "Clavis Cloudless Implementation Catalog"
description: "Task-by-task execution catalog for hardened Clavis Cloudless rollout, keyed to plan todo IDs."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-06"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Clavis Cloudless Implementation Catalog

This catalog converts the hardened execution plan into mechanical implementation instructions keyed by todo ID, with explicit file targets, expected code changes, and verification checks.

## Execution rules

- Run tasks in dependency order from the hardened plan.
- Do not add new direct `std::env::var` secret reads outside Clavis source modules.
- Any new `SecretId` must update Clavis SSOT docs and parity checks.
- Enforce fail-closed behavior in strict profiles.

## Workstream A tasks

### `a1-threat-model-v1`

- Source of truth: `docs/src/architecture/clavis-cloudless-threat-model-v1.md`.
- Ensure actor classes and secret-flow boundaries reference current code anchors.
- Verify consistency with `docs/src/architecture/clavis-secrets-env-research-2026.md`.

### `a2-source-policy-matrix`

- Keep source matrix in `docs/src/architecture/clavis-cloudless-threat-model-v1.md`.
- Add class-to-source constraints before modifying resolver behavior.

### `a3-break-glass-governance`

- Define activation, audit, TTL, and rotation requirements in runbook.
- Reference CI/audit instrumentation tasks in Workstreams E and G.

## Workstream B tasks

### `b1-secret-spec-metadata`

Target files:

- `crates/vox-clavis/src/lib.rs`
- `crates/vox-clavis/src/types.rs` (if new enums/status carriers are needed)

Required additions:

- `secret_class`
- `material_kind`
- `persistable_account_secret`
- `device_local_only`
- `allowed_sources`
- `rotation_policy`

### `b2-spec-completeness-assertions`

Target files:

- `crates/vox-clavis/src/lib.rs`
- `crates/vox-clavis/src/tests.rs` or new tests file

Required checks:

- All `SecretId` entries define all metadata fields.
- Test fails if any spec entry omits metadata.

### `b3-resolver-profile-types`

Target file: `crates/vox-clavis/src/resolver.rs`

Required changes:

- Add strict/lenient profile type.
- Deterministic source-order matrix per profile.

### `b4-resolver-rejection-statuses`

Target files:

- `crates/vox-clavis/src/types.rs`
- `crates/vox-clavis/src/resolver.rs`

Required statuses:

- `RejectedLegacyAlias`
- `RejectedSourcePolicy`
- `RejectedClassPolicy`

### `b5-resolver-strict-tests`

Target files:

- `crates/vox-clavis/src/tests.rs`
- `crates/vox-clavis/tests/*`

Required tests:

- profile x source permutations
- malformed/empty source values
- unavailable backend behavior

## Workstream C tasks

### `c1-cloudless-record-schema`

Target files:

- VoxDB schema modules under `crates/vox-db/src/schema/`
- storage ops modules under `crates/vox-db/src/store/`

Schema minimum:

- account identifier
- secret id
- ciphertext
- key reference
- version
- updated timestamp
- rotation metadata
- consistency metadata

### `c2-envelope-encryption`

Target files:

- `crates/vox-clavis/src/backend/vox_vault.rs` (or new backend module)
- encryption helpers in clavis backend area

Required:

- DEK per record
- KEK reference and rewrap support
- explicit key versioning

### `c3-cloudless-backend-adapter`

Target files:

- `crates/vox-clavis/src/backend/mod.rs`
- `crates/vox-clavis/src/lib.rs`
- new backend implementation module(s)

Required:

- CRUD adapter using VoxDB encrypted rows
- strict-profile no-plaintext fallback

### `c4-sync-replication-tests`

Target files:

- `crates/vox-db/tests/*`
- `crates/vox-clavis/tests/*`

Test dimensions:

- canonical vs project store
- replica-latest read consistency handling
- stale replica deterministic failure behavior

### `c5-backup-restore-harness`

Target files:

- `crates/vox-db/tests/*`
- optional ops tooling in `crates/vox-cli/src/commands/*`

Required:

- encrypted backup/restore verification
- corrupted ciphertext/key reference tests

## Workstream D tasks

### `d1-mcp-gateway-migration`

Target files:

- `crates/vox-orchestrator/src/mcp_tools/http_gateway.rs`
- `crates/vox-clavis/src/lib.rs`

Required:

- replace direct bearer env reads with Clavis secret resolution

### `d2-runtime-registry-migration`

Target file: `crates/vox-runtime/src/llm/types.rs`

Required:

- remove secret-material dependence on arbitrary `api_key_env` in strict path
- keep non-secret endpoint config flexibility where needed

### `d3-publisher-openreview-migration`

Target file: `crates/vox-publisher/src/publication_preflight.rs`

Required:

- replace token env probing with Clavis ID-based resolution

### `d4-orchestrator-social-migration`

Target file: `crates/vox-orchestrator/src/config/impl_env.rs`

Required:

- route social credentials through Clavis, not direct env reads

### `d5-db-compat-hardcut`

Target file: `crates/vox-db/src/config.rs`

Required:

- strict-profile behavior rejects compatibility aliases by policy boundary

### `d6-consumer-strict-suite`

Target files:

- tests across `vox-mcp`, `vox-runtime`, `vox-publisher`, `vox-orchestrator`, `vox-db`

Required:

- strict and lenient profile regression coverage

## Workstream E tasks

### `e1-secret-env-guard-strict`

Target file: `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs`

Required:

- hard-cut strict mode for secret-env-guard
- clear allowlist semantics

### `e2-dataflow-leak-guards`

Target files:

- `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs`
- command wiring files under `crates/vox-cli/src/commands/ci/`

Required:

- detect secret serialization anti-patterns
- detect model-context leak patterns

### `e3-guard-negative-fixtures`

Target files:

- `crates/vox-cli/tests/fixtures/*`

Required:

- seeded failing fixtures for each guard category

## Workstream F tasks

### `f1-clavis-ssot-refresh`

Target file: `docs/src/reference/clavis-ssot.md`

Required:

- source-policy matrix
- hard-cut semantics examples

### `f2-env-vars-contract-refresh`

Target files:

- `docs/src/reference/env-vars.md`
- `docs/src/reference/mcp-http-gateway-contract.md`
- `contracts/mcp/http-gateway.openapi.yaml`

Required:

- sync docs/contracts with new auth/source semantics

### `f3-cloudless-ops-runbook`

Target file:

- `docs/src/operations/clavis-cloudless-ops-runbook.md`

Required:

- key custody, backup, restore, rotate, incident flow

### `f4-break-glass-runbook`

Target file:

- `docs/src/operations/clavis-break-glass-runbook.md`

Required:

- JIT access workflow, audit evidence, expiry and rotation controls

## Workstream G tasks

### `g1-no-secret-log-tests`

Target files:

- integration tests in affected crates

Required:

- assert zero secret value leakage in logs/traces/payload contexts

### `g2-fuzz-and-chaos-suite`

Target files:

- resolver tests in `vox-clavis`
- backend fault tests in `vox-db`/`vox-clavis`

### `g3-revocation-rotation-suite`

Target files:

- `vox-clavis` tests for rotation/revocation policies by material kind

## Workstream H tasks

### `h1-feature-flag-choreography`

Target files:

- clavis and consumer config surfaces; docs for flag semantics

Required rollout:

- shadow -> canary -> enforce -> decommission

### `h2-go-no-go-gates`

Target files:

- CI command helpers and release checklist docs

Required:

- machine-checkable promotion/rollback criteria

### `h3-post-cutover-audit`

Target files:

- reporting command and/or query path in CLI/DB surfaces

Required:

- policy violation report for cutover validation

### `h4-compat-code-sunset`

Target files:

- all temporary compatibility branches introduced during cutover

Required:

- removal checklist and completion verification

## Verification matrix

Before declaring completion:

1. `secret-env-guard` and `clavis-parity` pass.
2. new strict guards pass on baseline and fail on negative fixtures.
3. all migrated callsites have strict-profile tests.
4. contracts and docs remain synchronized.
5. cutover rehearsal passes in CI profile.


