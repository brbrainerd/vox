---
title: "C3 — vox-cli-ci extraction plan (2026-05-15)"
description: "Plan for extracting vox-cli/src/commands/ci/ (22K LoC, 74 files) into a new vox-cli-ci crate. Identifies the three shared-module blockers, the correct move via vox-cli-core, and a 6-task TDD breakdown."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-15"
training_eligible: false
---

# C3 — `vox-cli-ci` extraction plan (2026-05-15)

**Origin:** [`2026-05-08-crate-org-followup-design.md`](2026-05-08-crate-org-followup-design.md) §Track C, item C3.  
**Planned entry in `layers.toml`:** `vox-cli-ci = { plan = "...", layer = 3 }` (update to point here).

## TL;DR

- `vox-cli/src/commands/ci/` is **22,458 LoC** across 74 files — 31% of `vox-cli`'s 71K total.
- `vox-cli` is at **71,296 / 90,000 LoC** (79% budget), growing with every new CI guard.
- The `ci/` subdir uses 5 sibling modules: `artifact_policy`, `command_contract`, `command_registry_model`, `utils`, and `commands` (self-referential).
- Three of those (`artifact_policy`, `command_contract`, `command_registry_model` — 366 LoC combined) are ALSO used by non-ci code in `build_service.rs` and `command_catalog.rs`. They cannot move directly to `vox-cli-ci`.
- **Fix:** move the three shared modules to `vox-cli-core` first. Then `vox-cli-ci` depends on `vox-cli-core` for shared types and the `ci/` code moves cleanly.
- `crate::utils::install_policy` and `crate::utils::release_artifacts` (subsets of `utils/`) are the only `utils` imports in `ci/`. These can move to `vox-cli-core` or `vox-cli-ci` directly if they're not used elsewhere.
- **Estimated post-extraction sizes:** `vox-cli-ci` ~23K LoC, `vox-cli` ~48K LoC (down 31% from current).

---

## 1. Current state

```
vox-cli  71,296 LoC  (budget: 90,000 — 79% used)
  └── src/commands/ci/  22,458 LoC  (31%)
```

The `ci/` subdir grows with every new `vox ci <guard>` command. Growth is steady because new CI guards are the cheapest layer for the LLM agents to add policy enforcement. At the current rate (~24K in one month), `vox-cli` will exceed budget before Q3 2026.

---

## 2. Sibling dependency analysis

`vox-cli/src/commands/ci/**/*.rs` imports these `crate::` modules:

| Module | LoC | Used by non-ci? | Can move where? |
|---|---:|---|---|
| `crate::artifact_policy` | 135 | Yes (`build_service.rs`) | → `vox-cli-core` |
| `crate::command_contract` | 178 | Yes (`command_catalog.rs`) | → `vox-cli-core` |
| `crate::command_registry_model` | 53 | Declared `pub mod` in `lib.rs` | → `vox-cli-core` |
| `crate::utils::install_policy` | ~80 | Unknown — check | → `vox-cli-core` if shared, else `vox-cli-ci` |
| `crate::utils::release_artifacts` | ~120 | Unknown — check | → `vox-cli-core` if shared, else `vox-cli-ci` |
| `crate::commands::ci::*` | — | Self-referential | Stays within `vox-cli-ci` as `crate::*` |

**Verification command before starting:**
```powershell
# Check utils submodule usage outside ci/
grep -rn "utils::install_policy\|utils::release_artifacts" crates/vox-cli/src/ --include="*.rs" |
  Where-Object { $_ -notmatch "commands/ci/" }
```

---

## 3. Extraction architecture

### Before

```
vox-cli (L5)
  ├── lib.rs (declares all modules including commands::ci)
  ├── artifact_policy.rs (used by build_service + ci/)
  ├── command_contract.rs (used by command_catalog + ci/)
  ├── command_registry_model.rs (pub mod)
  ├── commands/ci/**   ← 22K LoC
  └── build_service.rs, command_catalog.rs, …
```

### After

```
vox-cli-core (L3)  ← add 3 shared modules here
  ├── artifact_policy.rs
  ├── command_contract.rs
  └── command_registry_model.rs

vox-cli-ci (L3)  ← new crate
  ├── Cargo.toml  (dep: vox-cli-core, vox-arch-check infra)
  └── src/  ← all 74 files from commands/ci/

vox-cli (L5)  ← now 48K LoC
  ├── lib.rs  (cmd: Ci { cmd: vox_cli_ci::CiCmd })
  ├── Cargo.toml  (add dep: vox-cli-ci)
  ├── build_service.rs  (use vox_cli_core::artifact_policy)
  └── command_catalog.rs  (use vox_cli_core::command_contract)
```

### Dep graph

```
vox-cli (L5)
  ├── vox-cli-core (L3)
  └── vox-cli-ci (L3)
        └── vox-cli-core (L3)
```

No cycles. `vox-cli` → `vox-cli-ci` → `vox-cli-core` is strictly acyclic.

---

## 4. Task breakdown

### C3-T1 — Shared-module usage audit (1h)

For each of the 5 sibling module imports, verify exact usage:

```powershell
# Which non-ci files use artifact_policy?
grep -rn "artifact_policy" crates/vox-cli/src/ --include="*.rs" | Where-Object { $_ -notmatch "commands/ci/" }
# Same for command_contract, command_registry_model, utils::install_policy, utils::release_artifacts
```

Produce final table: module → destination (vox-cli-core or vox-cli-ci). Update this doc with findings.

### C3-T2 — Move shared modules to `vox-cli-core` (2–3h)

For each module destined for `vox-cli-core`:

1. `git mv crates/vox-cli/src/<module>.rs crates/vox-cli-core/src/<module>.rs`
2. Add `pub mod <module>;` to `crates/vox-cli-core/src/lib.rs`.
3. In `vox-cli/src/lib.rs`: remove the old `mod <module>` declaration.
4. In all `vox-cli` callers of the moved module: replace `crate::<module>::` with `vox_cli_core::<module>::` (or `use vox_cli_core::<module>;`).
5. `cargo check -p vox-cli` must pass.

For `utils` submodules going to `vox-cli-core`:
- If `install_policy` and `release_artifacts` are not used outside `ci/`, move them to `vox-cli-ci/src/utils/` instead.

### C3-T3 — Create `vox-cli-ci` skeleton (30m)

```
crates/vox-cli-ci/
  Cargo.toml
  src/
    lib.rs
```

`Cargo.toml` needs:
- `name = "vox-cli-ci"`
- `description = "vox ci subcommand: workspace CI guards (SSOT checks, test inventory, build timings, feature matrix, doc inventory)."`
- `vox-cli-core = { workspace = true }`
- `workspace-hack = { workspace = true }`
- All the external deps currently pulled by `commands/ci/` files (these will surface as compile errors in C3-T4 — address iteratively)

Add to `layers.toml`:
```toml
vox-cli-ci = { layer = 3, max_loc = 28_000 }
```

Add to `where-things-live.md` L3 section:
```
| [vox-cli-ci](../../../crates/vox-cli-ci/) | vox ci subcommand: all CI guards and workspace-health checks. Extracted from vox-cli commands/ci/ in C3. |
```

Remove from `[planned]` table in `layers.toml`.

### C3-T4 — Move `commands/ci/` into `vox-cli-ci` (4–8h)

1. `git mv crates/vox-cli/src/commands/ci crates/vox-cli-ci/src/`
2. In every moved file:
   - `crate::artifact_policy::*` → `vox_cli_core::artifact_policy::*`
   - `crate::command_contract::*` → `vox_cli_core::command_contract::*`
   - `crate::command_registry_model::*` → `vox_cli_core::command_registry_model::*`
   - `crate::utils::install_policy::*` → `crate::utils::install_policy::*` (if moved to vox-cli-ci/src/utils/) OR `vox_cli_core::utils::install_policy::*`
   - `crate::commands::ci::` → `crate::` (since they're now at crate root)
   - `super::ci::` references in ci sub-modules → `super::` (check for these)
3. Add `pub use` in `vox-cli-ci/src/lib.rs` for the top-level `CiCmd` type.
4. `cargo check -p vox-cli-ci` must pass.

**Iterative dep resolution:** each `cargo check` error will surface missing external deps — add them to `vox-cli-ci/Cargo.toml` one by one. Expected deps: `anyhow`, `tokio`, `serde`, `serde_json`, `regex`, `tracing`, `chrono`, `reqwest` (maybe behind `vox-http-client`), `vox-compiler`, `vox-db`, etc.

### C3-T5 — Wire `CiCmd` into `vox-cli` (1h)

1. Add `vox-cli-ci = { path = "../vox-cli-ci" }` to `crates/vox-cli/Cargo.toml`.
2. In `crates/vox-cli/src/lib.rs`:
   - Remove `mod commands` declaration for `ci` sub-module (or keep the `ci` sub-module but re-export from `vox-cli-ci`).
   - Change the `Ci` arm in the `VoxCmd` enum: `cmd: commands::ci::CiCmd` → `cmd: vox_cli_ci::CiCmd`.
3. `cargo check -p vox-cli` must pass.
4. `cargo check --workspace --exclude vox-gui` must pass.

### C3-T6 — Verification and cleanup (1h)

1. `cargo run -p vox-arch-check` — clean ✓.
2. `cargo test -p vox-cli-ci` — all tests pass.
3. `cargo test -p vox-cli` — all tests pass.
4. Update `layers.toml`:
   - `vox-cli` `max_loc`: lower from 90K to 55K (or actual post-extraction count + 10% margin).
5. Update this plan doc: mark complete, record actual post-extraction LoC counts.

---

## 5. External dep inventory for `vox-cli-ci/Cargo.toml`

Run this before C3-T3 to pre-populate the dep list:

```powershell
# Find all 'use' statements in ci/ that reference external crates
grep -rh "^use " crates/vox-cli/src/commands/ci/ --include="*.rs" |
  Select-String -Pattern "^use (vox_|crate::)" -NotMatch |
  ForEach-Object { ($_ -split "::")[0] -replace "^use ", "" } |
  Sort-Object -Unique
```

Expected result includes: `anyhow`, `chrono`, `indexmap`, `regex`, `serde`, `serde_json`, `serde_yaml`, `tokio`, `tracing`.

Workspace deps that ci/ likely needs from `vox-*` crates (verify before adding):
- `vox-arch-check` (compile-time validation tools)
- `vox-code-audit` (detector rule access)
- `vox-compiler` (CI compilation checks)
- `vox-db` (build-timing DB writes, schema coverage)
- `vox-doc-pipeline` (doc inventory)
- `vox-http-client` (link checking, deploy status)
- `vox-scientia` (novelty ledger)

---

## 6. Decision checklist before starting

- [ ] `vox-cli` LoC has exceeded 80% of budget (72,000 LoC), OR a new CI guard was rejected for adding too much to `vox-cli`.
- [ ] C3-T1 dep audit is complete and the shared-module destinations are confirmed.
- [ ] No other active large PR touches `vox-cli/src/commands/ci/` (merge conflicts would be severe).
- [ ] CI is green on `main` before branching.
- [ ] Not overlapping with any active `vox-cli` binary surface PR (risk of `lib.rs` conflicts).

---

## 7. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `ci/` files use `vox-cli` internals not caught by the dep audit | Medium | High — compile failure in C3-T4 | Iterate `cargo check -p vox-cli-ci` after each file move; don't batch too many at once |
| Moving `command_contract` to `vox-cli-core` breaks `vox-cli` consumers | Low | Medium | Fix consumers in C3-T2 before starting C3-T4 |
| `vox-cli-ci` acquires heavy deps (vox-compiler, vox-db) that make it not a fast-leaf | High | Low — expected for an L3 crate | Accept: `vox-cli-ci` is L3 "heavy runtime" category, not a leaf |
| Large diff causes review friction | High | Low | Mechanically verify with pre-commit hooks; keep CI green throughout |
| AGENTS.md enforcement notes still reference `vox-cli` for ci guards | Low | Low | `grep -rn "vox-cli.*ci\|ci.*vox-cli" docs/ AGENTS.md` after C3-T6; fix any stale references |
