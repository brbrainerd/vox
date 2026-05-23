---
title: "English-Core + Latin Alias Migration Ledger"
description: "Complete migration tracking for nomenclature transition from Latin aliases (vox-dei, vox-ars) -> English canonical names."
category: "Architecture SSOTs"
status: "legacy"
last_updated: "2026-04-05"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# English-Core + Latin Alias Migration Ledger

## Phase 0: Baseline & Inventory Lock

This ledger captures the frozen baseline state of the Vox workspace prior to initiating the English-Core nomenclature migration.

### T001-T005: Core Metadata & Contract Hashes
- **Workspace Members**: 58 packages enumerated under `crates/*` (excluding `crates/vox-py`).
- **Command Registry Hash (`command-registry.yaml`)**: Locked.
- **Operations Catalog Hash (`catalog.v1.yaml`)**: Locked.
- **Capability Registry Hash (`capability-registry.yaml`)**: Locked.
- **Dependency Graph Snapshot**: `cargo metadata --locked --no-deps > migration_cargo_metadata_baseline.json` executed successfully.

### T006-T007: Canonical Concept Domain Map
The following explicit mapping table forms the 1:1 binding between canonical English concepts and Latin aliases:
- `orchestrator` ↔ `dei`
- `skills` ↔ `ars`
- `forge` ↔ `fabrica`
- `database` ↔ `codex`
- `secrets` ↔ `clavis`
- `speech` ↔ `oratio`
- `ml` ↔ `populi`
- `gamification` ↔ `ludus`
- `tutorial` ↔ `schola`
- `package_manager` ↔ `arca`

### T008-T010: CLI Dispatch & Alias Inventory
- **clap-visible aliases (`crates/vox-cli/src/lib.rs`)**: Currently using explicit `visible_alias` strings (e.g., `visible_alias = "secrets"` for `clavis`).
- **Nested Latin Commands (`crates/vox-cli/src/latin_cmd.rs`)**: Contains enums `FabricaCmd`, `DiagCmd`, `ArsCmd` mapping directly to underlying English args structures (`BuildArgs`, `CheckArgs`, etc.).
- **Dispatch Routes (`crates/vox-cli/src/cli_dispatch/mod.rs`)**: Uses `cli_top_level_into_fabrica_or_self` and `run_*_cmd` functions to route aliases to canonical workflows.

### T011-T013: Ecosystem SSOT & CI Baseline
- **CI Checks (`.github/workflows/ci.yml`)**: Includes explicit guards for `codex-ssot`, `check-docs-ssot`, `command-compliance`, `clavis-parity`.
- **Nomenclature Rules (`nomenclature-migration-map.md`)**: Currently positions English as canonical text but Latin as primary CLI structure (`latin_ns`).
- **Orphan Surface Inventory (`orphan-surface-inventory.md`)**: Reflects `vox-dei` as a minimal member, with `vox-orchestrator` handling heavy lifting.

### T014-T018: API & Crate Dependency Baseline
- `vox-dei` currently acts as a slim structural member.
- `vox-ars` exports skill registries and workflows.
- `vox-orchestrator` holds canonical orchestration APIs.
- API exports and paths are logged for safe forwarding shim construction in Phase 3 & 4.

### T019-T023: Build & CI Performance (pre-migration)
- Build timings: Stable.
- Test pass set (`vox-cli`, `vox-mcp`, `vox-orchestrator`): Green.
- Command compliance: Passing.
- Capability sync: Clean.

---

## Migration Risk Log (T024)

### Identified Risks & Mitigations
1. **Dangling Docs Links**: Renaming concept structures might invalidate `docs/src` markdown paths.
   *Mitigation*: Automated doc-inventory verification and link-checker in `.github/workflows/ci.yml`. Phase 6 handles bindings before Phase 7 does any physical directory moves.
2. **LLM Context Disruption**: AI agents are currently heavily context-biased toward `vox-dei` and `vox-ars`. Removing the terms abruptly will degrade code generation accuracy.
   *Mitigation*: Header bindings in `lib.rs` and `Cargo.toml` keywords (Phase 6), plus a deprecated forwarding shim with Tombstone warnings (Phases 3/4).
3. **Broken CI Workflows**: Cargo paths and features inside `.github/workflows/ci.yml` that rely on `vox-dei` (e.g., `ci no-vox-dei-import`).
   *Mitigation*: Phase 5 enforces renaming rules, and we will update all CI scripts iteratively alongside crate logic updates.
4. **Collision of Latin/English CLI arguments**: Passing English args to a Latin alias and causing parse errors, or vice versa.
   *Mitigation*: CLI Interchangeability (Phase 2) builds 1:1 mapping directly in the parsing layer, tested for deterministic output.

archived_date: 2026-04-18
---

## Phase 1: Canonical English Naming in Contract Layer (Completed)

This phase systematically verified and extended the `catalog.v1.schema.json` and its projections.

### T025-T040: Contract Schema and Base Mapping
- Safely extended `catalog.v1.schema.json` inserting `canonical_name` and `latin_aliases` safely without breaking downstream JSON tooling.
- Populated `catalog.v1.yaml` with explicit bounds mapping `dei -> orchestrator`, `ars -> skills`, `fabrica -> forge`, `codex -> database`, etc.

### T041-T044: Projections 
- Automatically generated capabilities and CLI representations mapping via synchronous pipeline updates.

### T045-T054: Built-in Tests & CI Verifiers
- Authored rigid CI safeguards covering T045..T050 directly deeply within `commands::ci::operations_catalog`. Extracted verification checks into `verify_catalog_nomenclature()`.
- Wrote unit tests confirming the system actively rejects structural/alias collisions, retired boundaries, missing core aliases, and enforces `^[a-z]+(-[a-z]+)*$` nomenclature string grammar checks.

### T055-T066: Status
- All compliance checks are actively gated inside `ci command-compliance` and `ci operations-verify` respectively.
- Phase locked and green.

## Phase 3 & 4: Hard-Merges and Shims (Completed)

This phase executed the hard-merges of orphaned Latin crates into their canonical English counterparts to reduce structural fragmentation.

### T067-T080: DEI and ARS Hard-Merges
- Moved all source modules from ox-dei (
oute_telemetry, gent_frontmatter, 
esearch, selection) into ox-orchestrator::dei_shim.
- Moved all source modules from ox-ars (openclaw_adapter, manifest, xecutor, etc.) into ox-skills::ars_shim.
- Converted ox-dei and ox-ars into short-lived forwarding shims (exporting pub use vox_orchestrator::dei_shim::*; and pub use vox_skills::ars_shim::*;).
- Resolved all type inference and import conflicts caused by the boundary shifts.

### T081-T090: CI & Structural Verification
- Updated Cargo.toml dependencies natively to ensure ox-orchestrator and ox-skills inherited required external traits (e.g., ox-socrates-policy, 	okio-tungstenite).
- Executed cargo check -p vox-dei -p vox-ars -p vox-orchestrator -p vox-skills to guarantee parity.
- Executed cargo check -p vox-cli to prove downstream workflow surfaces successfully consumed the shims.
- Executed TOESTUB checks to verify skeleton code structures or structural limits were not violated.
- Phase locked and green.

## Phase 6: Context Binding and Docs Scrubbing (Completed)

This phase neutralized lingering references to archaic ox-dei and ox-ars strings across the repository surface before physical deletion.

### T091-T100: Context Preservation Bindings
- Injected keyswords = ["dei", "vox-dei"] into ox-orchestrator/Cargo.toml and keywords = ["ars", "vox-ars"] into ox-skills/Cargo.toml to actively tether internal AI agent semantic memory to the new crates without requiring full retraining.
- Implemented "Tombstone warning" header descriptions in ox-dei and ox-ars lib.rs shims.

### T101-T110: Documentation and CI Surface Scrubbing
- Scrubbed docs/src markdown paths globally to transition ox-dei to ox-orchestrator and ox-ars to ox-skills while strictly preserving ox-dei-d daemon invocation rules.
- Transitioned reference surfaces inside .github/workflows/ci.yml strictly ensuring workflow script guards accurately match the English-canonical structural footprint.
- Phase locked and green.

## Phase 7 { Physical Deprecation and Deletion (Completed)

This final phase concluded the architectural migration by cleanly erasing the deprecated ox-dei and ox-ars structures from the codebase, confirming the workspace is entirely reliant on the English Core equivalents.

### T111-T120: Dependency Graph Re-wiring
- Eradicated all ox-ars and ox-dei crate-level references across ox-cli, ox-mcp, ox-skills, ox-runtime traversing .toml files directly towards ox-skills and ox-orchestrator.
- Realigned integration test imports inside active members (	ests/ directory imports remapped strictly to ox_skills::ars_shim).

### T121-T130: Physical Structure Deletion
- Purged /crates/vox-dei surface physically from the disk.
- Purged /crates/vox-ars surface physically from the disk.
- Excluded the crates globally from the root Cargo.toml workspace.members.
- Verified absolute compilation success via cargo check --workspace yielding structurally zero errors and complete boundary resilience.
- **Migration Complete and Repository Locked.**


