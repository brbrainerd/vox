# Vox-DB Audit — Follow-Up Work

> **For agentic workers:** This document is a continuation handoff from the
> audit PR merged on 2026-05-09. Read it top-to-bottom before touching
> anything. All references to "the audit PR" mean the work in
> `docs/superpowers/plans/data-audit/2026-05-08-vox-db-and-memory-audit-pr.md`.

**Goal:** Complete the two items that were correctly deferred from the audit PR
and document the decision framework that governs future DB work.

---

## What the audit PR delivered (do not re-do this)

| Phase | What landed |
|---|---|
| P0-P3 | `data-storage-policy.v1.yaml` is SSOT for turso-import-guard; three new CI checks: `policy-allowlist-parity`, `db-schema-coverage`, `row-serde-lint` + `string-id-lint` (fail-closed) |
| P4 | 9 pure-data types moved from `vox-db` → `vox-db-types`; dead `vox-db` deps removed from `vox-webhook` and `vox-skills` |
| P5 | 44 row/entry types gained `Serialize`/`Deserialize`; compile test at `crates/vox-db-types/tests/serde_uniformity.rs` |
| P6 | Bridges deferred (see below) |
| P7 | `DbAgentId`, `DbSessionId`, `DbTaskId`, `DbCorrelationId`, `DbUserId`, `DbPlanSessionId` in `crates/vox-db-types/src/ids.rs`; all 19 stringly-typed `*_id` fields in `store_types/` migrated; `string-id-lint` fail-closed |
| P8-P9 | `process_supervision.rs` and `lifecycle.rs` doc'd; ADR-004, `where-things-live.md`, `database-nomenclature.md` updated |

---

## Follow-up 1 — Row↔domain bridges (Phase 6 from the audit plan)

### Why it was deferred

No domain types existed that map 1:1 to row types. Specifically:
- `vox-orchestrator::session::state::Session` carries in-memory fields
  (`turns`, `plugin_state`, `total_tokens`) with no DB columns.
- `vox-orchestrator::types::messages::A2AMessage` uses typed newtypes
  (`AgentId`, `MessageId`) and a typed `A2AMessageType` enum; the row has
  delivery-tracking columns (`claim_owner`, `delivery_attempts`) absent from
  the domain.
- No callers manually destructure rows into domain objects — rows are used
  directly as DTOs everywhere.

Bridges to half-matching types would be dead code with a misleading name.

### What needs to exist before bridges make sense

A domain-layer crate — tentatively `vox-agent-types` at L1 — that defines:

```rust
pub struct AgentDefinition { pub id: DbAgentId, pub name: String, ... }
pub struct MemoryRecord    { pub id: i64, pub agent_id: DbAgentId, ... }
pub struct SessionSummary  { pub id: DbSessionId, pub agent_id: DbAgentId, ... }
```

These types would be **pure data** (no async, no connection) and would live
below `vox-orchestrator` in the layer stack. `vox-db-types` (L0) would NOT
depend on them; instead, bridges would live in `vox-agent-types` (L1), which
depends on `vox-db-types`:

```rust
// in vox-agent-types/src/conversions.rs
impl From<vox_db_types::AgentDefEntry> for AgentDefinition { ... }
impl From<vox_db_types::MemoryEntry>   for MemoryRecord    { ... }
```

### Implementation plan (when the prerequisite exists)

**Files to create:**
- `crates/vox-agent-types/src/lib.rs` — new L1 crate, pure types
- `crates/vox-agent-types/src/agent_definition.rs`
- `crates/vox-agent-types/src/memory_record.rs`
- `crates/vox-agent-types/src/session_summary.rs`
- `crates/vox-agent-types/src/conversions.rs`
- `crates/vox-agent-types/Cargo.toml` — deps: `vox-db-types`, `serde`

**Files to modify:**
- `docs/src/architecture/layers.toml` — add `vox-agent-types` at layer 1
- `Cargo.toml` (workspace) — add `vox-agent-types` to `[workspace.members]`
  and `[workspace.dependencies]`
- `docs/src/architecture/where-things-live.md` — add row for the new crate

**Trigger:** open this follow-up PR only after the first consumer crate has a
concrete need to destructure a row into a structured domain object. Do not
create the crate speculatively.

---

## Follow-up 2 — Consumer crate `vox-db-types` direct deps

### What the audit attempted

Phase 4.4 looked for crates that import `vox-db` but only use type imports
(never call connection methods). None were found — every `vox-db` consumer
also calls the facade.

### What to revisit

As the codebase evolves, new crates may be added that only need types (e.g.
a formatter, a display layer, a webhook payload builder). When that happens:

1. Check `Cargo.toml` of the new crate.
2. If it only imports `vox-db` for types: swap to `vox-db-types`.
3. Run `cargo check -p <crate>` to verify.

The guard `vox ci turso-import-guard` will catch any crate that incorrectly
adds a direct turso dep. The layer checker `cargo run -p vox-arch-check` will
catch any invalid upward dependency.

**No immediate action required.**

---

## Decision framework for future DB work

These rules should be internalized before any new DB-related code:

### Adding a new table

1. Add `CREATE TABLE` SQL to `crates/vox-db/src/schema.rs`
   (or the relevant satellite crate if the table is already sanctioned there).
2. `vox ci db-schema-coverage` will fail if the table appears in a non-owner
   crate — fix by moving the SQL.
3. Add the corresponding row struct to `crates/vox-db-types/src/store_types/`
   with `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`.
4. `vox ci row-serde-lint` will fail if the struct is missing derives.
5. If the struct has `*_id` fields, use the `string_id!` macro in
   `crates/vox-db-types/src/ids.rs` to introduce a newtype (or reuse an
   existing one). `vox ci string-id-lint` is fail-closed — it will catch
   any new `pub <x>_id: String` field.

### Adding a new turso-using crate

1. The crate must be listed in `contracts/db/data-storage-policy.v1.yaml`
   under `tiers.a_relational.{owners|allow_direct_access|temporary_exceptions}`.
2. `vox ci turso-import-guard` reads the policy YAML as SSOT — no separate
   `docs/agents/turso-import-allowlist.txt` entry needed for policy owners.
3. For transitional exceptions, add an entry to the txt allowlist with an
   explanatory comment. Remove the entry once the crate is properly
   onboarded.
4. Update `docs/src/architecture/where-things-live.md` with the new crate row.

### Operational JSON state (NOT in the DB)

Some state lives in JSON files, not the DB. This is intentional for two
categories:

- **Tier-D cache** (process supervision, PID files, socket paths): written
  before the DB connection exists; machine-local; no migration concerns.
  Lives in `.vox/process-supervision/`.
- **Orchestrator crash-recovery snapshots**: orchestrator-internal; opaque to
  other consumers; no cross-process visibility needed.

If you find new JSON state and are unsure which tier it belongs to, read
`contracts/db/data-storage-policy.v1.yaml` section `tiers`.

---

## Where to pick up (checklist for the follow-up session)

- [ ] Confirm `cargo run -p vox-cli --bin vox -- ci turso-import-guard --all` exits 0
- [ ] Confirm `cargo run -p vox-cli --bin vox -- ci policy-allowlist-parity` exits 0
- [ ] Confirm `cargo run -p vox-cli --bin vox -- ci db-schema-coverage` exits 0
- [ ] Confirm `cargo run -p vox-cli --bin vox -- ci row-serde-lint` exits 0
- [ ] Confirm `cargo run -p vox-cli --bin vox -- ci string-id-lint` exits 0 (not just report-only)
- [ ] If `vox-agent-types` has since been created: proceed with Follow-up 1
- [ ] If new consumer crates exist with type-only `vox-db` deps: swap to `vox-db-types`
