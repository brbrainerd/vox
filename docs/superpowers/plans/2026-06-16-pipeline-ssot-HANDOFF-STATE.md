# Pipeline SSOT Simplification — HANDOFF STATE (start here)

> **You do NOT need any prior conversation.** This doc + the contract SSOT + the implementation plan are the complete context for the **pipeline / emission / golden-ladder** track. Read this first, then the linked SSOT, then execute "Remaining work."

**Branch (as of handoff):** `feat/vault-decryption-recovery` — **large mixed working tree** (~295 files). Pipeline SSOT work is a **coherent subset**; scientia, GUI, graphify, publisher, and vendored skills dominate the diff. **Split PRs** before merge unless the human explicitly wants one mega-PR.

**Do NOT commit unless the human operator asks.**

---

## Intent (why this work exists)

Make script→emission parity **provable in CI**, not aspirational:

1. **One emission validation gate** (`EmissionProfile`) instead of scattered WebIR checks.
2. **Canonical golden ladder** — 12 fixtures in `contracts/pipeline/canonical-ladder.v1.yaml` drive compile/typecheck/profile tests backward from real `examples/golden/*.vox`.
3. **Honest feature matrix** — `Support::Unverified` for declared-but-unproven cells; ladder-proven decorators explicitly marked.
4. **Script surface lock** — grammar SSOT + decorator enum parity with lexer.
5. **Umbrella CI gate** — `vox ci pipeline-parity` runs grammar → ladder → matrix smoke → k-budget.
6. **Emit harness + diagnostics** — compile-generated script crates; registry-aligned diagnostic codes.

**Binding constraints from the human operator:**

- Do **not** edit the plan file `docs/superpowers/plans/2026-06-14-pipeline-parity-enforcement.md` (read-only contract companion).
- Do **not** use `#[ignore]` on failing ladder tests — fix codegen, harness, or fixtures.
- Use **holistic/sequential** route for related DB/codegen failures (not parallel splits on shared emit paths).
- Automation is **VoxScript-only** (`vox run scripts/…`); no new `.ps1`/`.sh`/`.py` glue.

**Authoritative contract:** [`docs/src/architecture/pipeline-parity-ssot-2026-06-14.md`](../../src/architecture/pipeline-parity-ssot-2026-06-14.md)

**Executable plan (task checkboxes):** [`docs/superpowers/plans/2026-06-14-pipeline-parity-enforcement.md`](2026-06-14-pipeline-parity-enforcement.md)

---

## What is DONE (Phases 1–6 + code-review fixes)

### Phase deliverables (implemented in working tree)

| Phase | Deliverable | Key paths |
|-------|-------------|-----------|
| 1 | `EmissionProfile` SSOT | `crates/vox-codegen/src/emission_profile.rs`, `projection_bundle.rs` (`project_and_validate`) |
| 2 | Canonical ladder + tests | `contracts/pipeline/canonical-ladder.v1.yaml`, `crates/vox-codegen/src/canonical_ladder.rs`, `crates/vox-compiler/tests/emission_ladder_test.rs` |
| 2 | K-budget scoped to ladder | `crates/vox-cli/src/commands/ci/run_body_helpers/syntax_k.rs`, `contracts/eval/complexity-budget.v1.json` (12 fixtures) |
| 3 | Honest matrix | `crates/vox-compiler/src/feature_matrix.rs` (`Support::Unverified`, `is_ladder_proven_decorator`) |
| 4 | Script surface parity | `language_surface.rs`, `grammar_ssot_parity.rs`, `language_surface_ssot_test.rs` |
| 5 | Umbrella gate + CI | `crates/vox-cli/src/commands/ci/pipeline_parity.rs`, `cmd_enums.rs`, `run_body.rs`, `.github/workflows/ci.yml` (~line 805) |
| 6 | Emit harness | `crates/vox-codegen/tests/emit_compile_harness.rs`, diagnostic registry touch-ups |

### Major codegen fixes (no test ignores)

- **DB / CRUD ladder:** `method_emit.rs` — Turso binds, insert return type, `HirDbQueryPlan` routing.
- **`scheduled_tick`:** `Unit` → `()` in Rust emit.
- **`durable_workflow_real`:** workflow runtime deps in script `Cargo.toml`, async workflow/activity emit, durability lower fixes.
- **`error_propagation`:** comparison parens; `VoxJson` accessors in `method_emit.rs`.
- **Ladder harness:** per-fixture package names (`vox-script-{id}`); **`--test-threads=1`** for serial compiles.

### Code-review fix agents (2026-06-16, all integrated)

| Fix | Status | Evidence |
|-----|--------|----------|
| Ladder ignores typecheck | **Fixed** | `check_typecheck_clean` / `assert_typecheck_clean`; YAML-driven `ladder_contract_drives_each_fixture_target` |
| K-budget missing entry warn-only | **Fixed** | Missing ladder fixture → `failures.push` + bail with detail; unit tests in `syntax_k.rs` |
| Grammar parity count-only | **Fixed** | `decorator_feature_lexer_parity_mismatch()` + set equality test |
| Docs-only CI skips tests | **Fixed** | `ci.yml` fail-closed: `docs_changed` + empty affected → `full=true`; contract test in `ci_workflow_contract.rs` |
| Command catalog baseline drift | **Fixed** | `ci/pipeline-parity` added to `command_catalog_paths_baseline.txt` |

### Verified green (reported in session; re-run before push)

```
# Windows — use isolated target dir if vox.exe file-locks on default target/
$env:CARGO_TARGET_DIR = "c:\Users\Owner\vox\target-pipeline-parity"
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-compiler --test emission_ladder_test -- --test-threads=1
# → 19 passed (~234s)

& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-cli k_complexity_budget_tests --lib
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-cli --test ci_workflow_contract selective_ci_fail_closed
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test -p vox-compiler --test language_surface_ssot_test decorator_feature_names_match
```

**Note:** `cargo test` accepts only one test-name filter per invocation; split spot-checks (e.g. two `emission_ladder_test` filters) into separate commands. Avoid parallel `cargo` jobs against the same `CARGO_TARGET_DIR` — ladder contract test ~3 min and may block on `vox.exe` lock.

---

## Remaining work (priority order)

### P0 — Confirm before any PR

1. **End-to-end umbrella gate**
   ```powershell
   $env:CARGO_TARGET_DIR = "c:\Users\Owner\vox\target-pipeline-parity"
   cargo run -p vox-cli -- ci pipeline-parity
   ```
   Steps: grammar SSOT → `emission_ladder_test` (serial) → `feature_matrix_parity_test` → k-budget → matrix coverage print.

   **Last gate run (2026-06-16):** grammar + ladder **PASS**; matrix step failed on `SecretId::VoxSyndicationTemplateProfileEnabled` in `vox-secrets` — **no longer present in tree** (grep clean); re-run should reach matrix + k-budget if workspace compiles.

2. **Stage untracked pipeline files** (still `??` in git — easy to miss):
   - `contracts/pipeline/canonical-ladder.v1.yaml`
   - `crates/vox-codegen/src/canonical_ladder.rs`
   - `crates/vox-codegen/src/emission_profile.rs`
   - `crates/vox-cli/src/commands/ci/pipeline_parity.rs`
   - `crates/vox-compiler/tests/emission_ladder_test.rs`
   - `crates/vox-compiler/tests/k_complexity_script_guard_test.rs` (if part of this PR)

3. **Regenerate / sync SSOT artifacts** after staging:
   ```powershell
   cargo run -p vox-cli -- ci ssot-drift --write   # or let ssot-autoregen bot on same-repo PR
   vox ci pre-push                                 # fast tier minimum
   ```

4. **PR hygiene:** Split pipeline SSOT from unrelated tracks (see "Mixed branch scope" below).

### P1 — Known gaps (optional hardening, not blockers for ladder green)

| Gap | Detail | Suggested fix |
|-----|--------|---------------|
| Matrix `Implemented` vs compile proof | `is_ladder_proven_decorator` marks ~15 decorators `all_targets()` Implemented; ladder only exercises subset | Narrow `Implemented` to proven `(feature, target)` pairs or add emitter tests for `Unverified` |
| Redundant ladder tests | Named per-fixture tests **and** `ladder_contract_drives_each_fixture_target` | Keep dynamic driver; delete redundant named tests in a cleanup PR |
| `print_matrix_coverage` | Counts static matrix cells, not empirical proof | Rename log line or compute from ladder YAML |
| Runtime `EmissionProfile` | Thin for `RustAxum`/`Interpreter` (schema + duplicate routes only) | Extend when axum/interp emit paths need profile validation |
| `feature_matrix_parity_test` | 6 smoke tests for seeded gap cells only | Task 8 full `(Feature, Target)` fixture grid per plan (large) |

### P2 — Plan file follow-through (from audit corrections)

Read plan audit section (2026-06-15) before touching emitters:

- **Wave 2 TS silent drops** — ~6 sites in `codegen_ts/hir_emit/mod.rs`, `web_ir/lower.rs` (not Rust eval — already exhaustive).
- **`unsupported_diagnostic() -> UnsupportedCell`** — shared matrix truth; per-crate adapters (no shared `Diagnostic` type across codegen/compiler).
- **Task 10 builtin registry parity** — three registries, three shapes; needs extractors.

These are **breadth-axis** work beyond the ladder (depth-axis) now landing.

### P3 — Parallel tracks on same branch (NOT pipeline SSOT)

Do not conflate with pipeline handoff; separate PRs recommended:

| Track | Pointer |
|-------|---------|
| Graphify integration | `docs/src/architecture/graphify-integration-research-2026-06-16.md`, `crates/vox-config/src/graphify.rs`, `crates/vox-cli/src/commands/graphify/` |
| GUI operator console | `docs/superpowers/plans/2026-06-16-gui-roadmap-remaining.md` |
| Federated config registry | `docs/superpowers/plans/2026-06-15-config-registry-HANDOFF-STATE.md` |
| Scientia / publisher / scout | `crates/vox-scientia/`, `crates/vox-publisher/`, CLI scientia commands |
| Vault decryption recovery | Branch name suggests primary theme — reconcile with human which PR owns what |

---

## Key files map (pipeline track only)

```
contracts/pipeline/canonical-ladder.v1.yaml     # 12 fixtures × targets × proves tags
contracts/eval/complexity-budget.v1.json        # K budgets for ladder fixture ids only

crates/vox-codegen/src/emission_profile.rs      # validate_bundle / validate_web_module
crates/vox-codegen/src/canonical_ladder.rs        # YAML loader
crates/vox-codegen/src/projection_bundle.rs       # project_and_validate

crates/vox-compiler/src/feature_matrix.rs         # Support matrix SSOT
crates/vox-compiler/src/language_surface.rs       # LEXER_AT_DECORATORS + parity helper
crates/vox-compiler/tests/emission_ladder_test.rs # Ladder driver (typecheck + compile + profile)
crates/vox-compiler/tests/feature_matrix_parity_test.rs
crates/vox-compiler/tests/language_surface_ssot_test.rs

crates/vox-cli/src/commands/ci/pipeline_parity.rs # Umbrella gate
crates/vox-cli/src/commands/ci/grammar_ssot_parity.rs
crates/vox-cli/src/commands/ci/run_body_helpers/syntax_k.rs
crates/vox-cli/tests/ci_workflow_contract.rs
crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt

.github/workflows/ci.yml                          # compiler-gates job runs pipeline-parity
tree-sitter-vox/GRAMMAR_SSOT.md                   # Regenerate via vox grammar --format ssot-markdown
```

---

## CI wiring reference

| Gate | Command / job |
|------|----------------|
| Umbrella | `vox ci pipeline-parity` |
| Ladder only | `cargo test -p vox-compiler --test emission_ladder_test -- --test-threads=1` |
| Matrix smoke | `cargo test -p vox-compiler --test feature_matrix_parity_test` |
| K-budget | `vox ci k-complexity-budget` |
| Grammar | `vox ci grammar-ssot-parity` (via pipeline-parity) |
| Grep gate | `cargo test -p vox-compiler --test parity_grep_gate` |
| Full compiler job | `.github/workflows/ci.yml` → `compiler-gates` |

**Selective CI note:** Docs-only PRs now upgrade to `full=true` when affected set is empty (fail-closed). Contracts outside crate graph already forced full via `affected-crates`.

---

## Windows / agent gotchas

| Issue | Mitigation |
|-------|------------|
| `cargo fmt --all` | **Banned** — use `vox run scripts/fmt.vox` or `cargo fmt -p <crate>` |
| `vox.exe` Access denied (file lock) | `$env:CARGO_TARGET_DIR = "c:\Users\Owner\vox\target-pipeline-parity"` (or `target-ladder-fix`) |
| Ladder parallel flake | Always `--test-threads=1` for `emission_ladder_test` |
| Long compiles (~4–9 min) | Normal for 12 script crate builds; do not kill early (exit `4294967295` = host timeout) |
| `cargo` path in agent shells | `& "$env:USERPROFILE\.cargo\bin\cargo.exe"` |
| Bootstrap without `vox` on PATH | `pwsh -File scripts/windows/vox-dev.ps1 <cmd>` |
| Pre-push hook slow | Human may use `git push --no-verify` after local gates — run `vox ci pre-push` yourself first |

---

## Mixed branch scope (read before reviewing diff)

Integration review ([diff agent](dd9aeb22-9c07-4bec-a39b-7aea053d3ae3)) found **no conflicts** between the four review-fix agents, but the **overall diff is huge**. Pipeline-relevant changes integrate via `pipeline-parity` wiring; everything else is orthogonal.

**Suggested PR split:**

1. **Pipeline SSOT + review fixes** — files in "Key files map" + codegen emit fixes for ladder fixtures + `complexity-budget.v1.json` + CI/docs-only fail-closed + baseline txt.
2. **Graphify** — config + CLI + MCP tools + retrieval contracts.
3. **GUI** — `crates/vox-gui/ui/**`, Playwright specs, surface registry reports.
4. **Scientia / publisher** — scientia crates, publication DB commands, research shim.

---

## Definition of done (pipeline track)

- [ ] `vox ci pipeline-parity` exits 0 locally (serial ladder).
- [ ] All untracked pipeline files committed in the intentional PR (not left as `??`).
- [ ] `vox ci pre-push --complete` green for touched crates (or full tier before merge).
- [ ] `command_catalog_paths_baseline.txt` includes `ci/pipeline-parity` (done in working tree).
- [ ] No `#[ignore]` on ladder tests; no reverted typecheck discarding (`let _ = typecheck_hir_module`).
- [ ] Human confirms PR scope (split vs monolith).

---

## Related handoffs

- Config registry (parallel): [`2026-06-15-config-registry-HANDOFF-STATE.md`](2026-06-15-config-registry-HANDOFF-STATE.md)
- GUI remaining: [`2026-06-16-gui-roadmap-remaining.md`](2026-06-16-gui-roadmap-remaining.md)
- Graphify research: [`docs/src/architecture/graphify-integration-research-2026-06-16.md`](../../src/architecture/graphify-integration-research-2026-06-16.md)
- Where code lives: [`docs/src/architecture/where-things-live.md`](../../src/architecture/where-things-live.md)

---

## Agent execution protocol

1. Read this doc + pipeline SSOT (30 min max).
2. Run P0 verification commands; fix only failures in pipeline scope.
3. Stage/commit **only** when human asks; use conventional commits (`feat`, `fix`, `test`, `ci`).
4. For unrelated failures on this branch, **do not** drive-by fix — note in PR description or separate task.
5. Prefer `vox ci pre-push --full --since <ref>` when iterating on 1–3 crates.

**Last updated:** 2026-06-16 (post code-review fix agents + integration review).
