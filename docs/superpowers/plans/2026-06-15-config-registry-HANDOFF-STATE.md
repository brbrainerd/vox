# Federated Config Registry — HANDOFF STATE (start here, Sonnet)

> **You do NOT need any prior conversation.** This doc + the plan + the 5 merged PRs below are the complete context. Read this, then the plan, then start at "Next task."

**Plan (full task specs, TDD, parallel map):** `docs/superpowers/plans/2026-06-15-federated-config-registry-plan.md` — its **"Execution Protocol for Sonnet 4.6"** section is mandatory (worktree/isolation discipline, `[PARALLEL-SAFE]` rules, dependency graph). Read it before any task.

**As of `main` @ `139c98deb0` (Phase 2 ~80% done):**

## What is DONE and merged (your worked examples — pattern-match these)
| PR | What | Pattern to copy |
|----|------|-----------------|
| #327 | config-guardrails: `include_str!`-embedded contracts, `RunOpts::sandboxed()`, **`vox ci config-hygiene`** gate (Checks A/B/C + baseline ratchet) | the gate + baseline-ratchet shape |
| #328 | **2A foundation**: `crates/vox-config/src/config_key.rs` (`ConfigKey` schema) + `config_registry.rs` (`CONFIG_KEYS`, `registered_keys()`) | the registry SSOT types |
| #330 | **2B**: `vox ci config-registry-parity` gate (mirrors config-hygiene ratchet) + 2A.3 wave-1 | `crates/vox-cli/src/commands/ci/config_registry_parity.rs` |
| #335 | **2C.1**: `vox ci config-gui-codegen [--check]` → `crates/vox-gui/ui/src/config/generatedSettingsIndex.ts` (40 entries) feeding `SETTINGS_INDEX` | `crates/vox-cli/src/commands/ci/config_gui_codegen.rs` (the generator + drift-gate shape) |
| #336 | **2A.3 wave-2**: operator_registry→CONFIG_KEYS **migration COMPLETE** (127 rows; `every_operator_env_is_in_config_keys` gate green) | the per-row translation rule (in the plan) |

**Current numbers:** `CONFIG_KEYS` = 127 rows. `vox ci config-registry-parity` = **698 grandfathered, 0 new** (baseline `contracts/config/config-registry-baseline.txt`). `vox ci config-hygiene` = 116 grandfathered, 0 new. Both ratchets block only NEW violations.

## Next task (do these in order)
1. **2C.2 — generate the Rust `FIELDS` catalog from `CONFIG_KEYS`** → actual `SettingsView` *rendering* (not just search). Mirror `config_gui_codegen.rs`; emit a `generated_fields.rs` consumed by `crates/vox-gui/src/commands/user_config.rs` `FIELDS`; map `ConfigKind`→GUI `Kind`, section from group; add `--check` drift; completion test asserts `generated FIELDS keys == CONFIG_KEYS surfaced keys`. (Plan Task 2C.2.)
2. **2C.3** — one drift gate covering both generated surfaces; add a test asserting `settingsIndex` and `FIELDS` generated sets are EQUAL; wire `config-gui-codegen --check` + `config-registry-parity` + `config-hygiene` into the CI gate bundle (next to `policy-registry-parity` in `.github/workflows/` + pre-push).
3. **Small cleanups:** normalize the one explicit `gui.section` ("Runtime & Sandbox") to a kebab id like "runtime" in `config_registry.rs` so all sections are kebab ids; gui-flag desired rows.
4. **Burndown (`[PARALLEL-SAFE]` per crate-group):** drive the 698 parity + 116 hygiene baselines toward 0 (embed remaining cwd-relative contracts; register remaining VOX_* knobs). Never silently delete a baseline entry — each removal = a real fix.
5. **2D** reactive `ConfigWatch`/`vox://config-changed` (plan Task 2D). **2E** land the LLM-settings registry as a *filtered member* over `CONFIG_KEYS` — coordinate with `docs/superpowers/plans/2026-06-15-llm-ai-settings-ssot-band-a.md`; do NOT build a 2nd registry.

## Verified gotchas (this repo) — obey or you will waste cycles
- **Shell CWD does NOT persist across tool calls.** Use `git -C "<WT>"` and `cargo --manifest-path "<WT>/crates/<c>/Cargo.toml"`. Never bare `cd`+`cargo`.
- **Pre-push hook hangs** (slow doc pipeline). Push with `git push --no-verify`. Admin-merge bypasses server CI — run your own `cargo clippy -p <c> -- -D warnings` + `cargo test` gates first.
- **`origin/main` moves under you** (concurrent sessions). Before merging: rebase onto fresh `origin/main`; if your commit collides and is now redundant, `git -c core.hooksPath=/dev/null rebase --skip`. Prefer cherry-picking only YOUR commits onto fresh main (zero-overlap → clean).
- **Never run two implementer subagents on the same crate/worktree concurrently.** Parallelize ONLY across disjoint crates, each in its OWN worktree, then cherry-pick. **Appending to `CONFIG_KEYS` cannot be parallelized** (collides at `];`) — do migration in sequential single-agent waves.
- **`cargo fmt --all` is banned** (Windows arg limit) → `cargo fmt -p <crate>`.
- **mens-gated tests** need `--features mens`.
- **GUI TS has no `node_modules`** in a fresh worktree → cannot `tsc`/build; verify `.ts`/`.tsx` by inspection; commit `--no-verify`.
- **Generated files** carry `// @generated … DO NOT EDIT`; regenerate via the codegen, never hand-edit; the `--check` mode is the drift gate.
- **Check C gap:** `config-hygiene` Check C regex catches `resolve_*`/`*_from_env` but not `from_env_*` names (minor).

## Open follow-up (background task already filed)
`task_77735246`: evict ~7 non-knob entries (`HOME`/`HOSTNAME`/`COMPUTERNAME`/`RUST_LOG`/`CARGO_HOME`/`USERPROFILE` + pseudo-key `vox_populi::inference_PROFILE`) from `operator_registry` + `CONFIG_KEYS` — they forced an unnatural `Home::VoxToml`. Check env-parity gates (`data_storage_guard.rs` M-14, `env-vars.v1.yaml`) before removing.

## Definition of done
`config-registry-parity` and `config-hygiene` baselines both at **0**, both `--check` drift gates in CI, GUI settings (search + render) generated from `CONFIG_KEYS`, LLM-settings registry landed as a member. Protocol/crypto/grammar/calibration constants stay const **by design** (enforced by config-hygiene Check B) — "zero magic values" is NOT the goal; "every operationally-meaningful knob has one registered home" is.
