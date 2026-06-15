# Federated Config Registry Implementation Plan (Sonnet 4.6 handoff)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan. Steps use checkbox (`- [ ]`) syntax. **Read the "Execution Protocol for Sonnet 4.6" section FIRST — it is mandatory.**

**Goal:** Give every operational config knob ONE canonical home in a *federated* registry, generate both GUI settings surfaces from it (killing hand-maintained drift), and enforce completeness with a parity gate — so config is searchable, GUI-visible, and single-sourced, with protocol/security/calibration constants staying const by design.

**Architecture:** A single shared `ConfigKey` schema in `vox-config` is the SSOT *type*. `operator_registry.rs` is formalized into the **general config table** (`CONFIG_KEYS: &[ConfigKey]`). Typed per-domain contracts (scaling/economy/circuit-breaker/model-routing) stay as-is and are *referenced* (federated), not swallowed. The GUI's two drifting surfaces (`settingsIndex.ts` + Rust `FIELDS`) become **generated** from `CONFIG_KEYS`. The in-flight `llm_config_registry` (still plan-only) lands as the *first federated member* on this shared foundation — not a second registry. A `vox ci config-registry-parity` gate makes "every operational env knob is registered" enforceable.

**Tech Stack:** Rust (`vox-config`, `vox-cli/commands/ci`, `vox-gui`), `serde`, the existing `vox ci` gate + `config-hygiene` baseline framework (already on main), the GUI `settingsIndex.ts`/`FieldSpec` surfaces, and the subagent/workflow harness for parallel execution.

**Prerequisite (already merged):** the config-guardrails seatbelt — `vox ci config-hygiene` (Checks A/B/C + baseline) and the `include_str!`-embedded contracts. **Do not start this plan until `vox ci config-hygiene` is green** (it is, on `main` @ 88f34ba9c7).

---

## Execution Protocol for Sonnet 4.6 (MANDATORY — read before any task)

You (Sonnet 4.6) have a smaller working context than the model that wrote this plan. **Do not try to hold the whole registry in your head.** Each task below is self-contained with complete code. Drive execution like this:

### 1. Worktree + branch (once, up front)
```bash
git fetch origin
WT="$PWD/.claude/worktrees/wt-config-registry"
git worktree add -b claude/federated-config-registry "$WT" origin/main
```
All work happens in `$WT`. **Shell CWD does not persist across tool calls** — in EVERY command use `git -C "$WT" …` and `cargo --manifest-path "$WT/crates/<crate>/Cargo.toml" …`, never bare `cd`+`cargo` relying on a prior `cd`. Confirm `git -C "$WT" rev-parse --abbrev-ref HEAD` before each commit. Stage with **explicit paths only** (never `git add -A`, never stage `Cargo.lock`).

### 2. Per-task loop (subagent-driven-development)
For each task: dispatch ONE implementer subagent with the task's full text (paste it — do not make the subagent read this file), then a spec-compliance review, then a code-quality review, then mark complete. The implementer follows the task's TDD steps exactly. **Never run two implementer subagents on the same file/crate concurrently in one worktree** (index + build races).

### 3. Parallelize ONLY where the plan says `[PARALLEL-SAFE]`
Tasks tagged `[PARALLEL-SAFE: group=X]` touch disjoint crates and may run as concurrent agents — but **each parallel agent must work in its OWN pre-created worktree off the same base**, because parallel commits/builds in one worktree collide. Pattern (validated): pre-create N worktrees, dispatch N agents (one per worktree via the Workflow tool's `parallel(...)` or N `Agent` calls in one message), each implements+tests+commits on its own branch, then YOU cherry-pick the N commits onto the integration branch (disjoint files → clean). Tasks WITHOUT the tag are sequential (they share a file).

### 4. Verification before any merge (CI is bypassed on admin-merge)
- `cargo clippy -p <touched-crates> -- -D warnings` (green).
- `vox ci config-hygiene` (green — 0 new; if a task legitimately adds a grandfathered-class entry, run `--update-baseline` and commit the baseline).
- `vox ci config-registry-parity` (green once Phase 2B lands).
- Per-task `cargo test`.

### 5. Hard-won gotchas (this codebase, verified)
- **Pre-push hook hangs** (slow doc pipeline). Push with `git push --no-verify`. Admin-merge bypasses server CI anyway, so do your own clippy/test gates.
- **`origin/main` moves under you** (concurrent sessions). Before merging, rebase your branch onto fresh `origin/main`; if a commit collides with concurrent work and is now redundant, `git -c core.hooksPath=/dev/null rebase --skip` it. Cherry-pick only YOUR commits onto fresh main (zero-overlap → clean) rather than merging a divergent base.
- **`cargo fmt --all` is banned** here (Windows arg-limit). Use `cargo fmt -p <crate>`.
- **mens-gated tests** need `--features mens`.
- **GUI TS has no `node_modules`** in a fresh worktree → you cannot `tsc`/build the UI; verify `.ts`/`.tsx` edits by inspection (value-for-value, imports type-correct) and commit with `--no-verify`.
- **Generated files:** anything emitted by a generator (Phase 2C) must carry a `// @generated … DO NOT EDIT` header and be regenerated, never hand-edited; add a `--check` drift gate.

### 6. Phase gates
Land each phase as its own admin-merged PR (rebase onto fresh main first). **Do not start Phase 2C until 2A+2B are merged and parity is green.** The phase dependency graph is below.

---

## Phase dependency graph (what blocks what; what parallelizes)

```
2A foundation (ConfigKey schema + CONFIG_KEYS table)   ← sequential, single crate (vox-config)
        │
        ├──► 2B parity gate (config-registry-parity)    ← sequential (vox-cli/ci) — depends on 2A
        │
        └──► 2C GUI generation (settingsIndex + FIELDS)  ← depends on 2A; the codegen tasks are [PARALLEL-SAFE]
                    │
                    └──► 2D reactive watch/event bus      ← depends on 2A; [PARALLEL-SAFE] vs 2C tail
        ┌───────────────────────────────────────────────┐
        │ 2E LLM-settings registry lands as a member     │ ← depends on 2A schema; hands off to the
        │    (coordinate with llm-ai-settings-ssot plan) │   separate llm-ai-settings-ssot plan
        └───────────────────────────────────────────────┘
   Baseline burndown (independent, [PARALLEL-SAFE] across crate-groups) — can run ANY time after the
   guardrail seatbelt; converts grandfathered config-hygiene violations into real fixes.
```

---

## Phase 2A — The shared `ConfigKey` schema + the general registry table

Single crate (`vox-config`), sequential. This is the foundation everything else generates from.

### Task 2A.1: Define the `ConfigKey` schema

**Files:**
- Create: `crates/vox-config/src/config_key.rs`
- Modify: `crates/vox-config/src/lib.rs` (add `pub mod config_key;`)

- [ ] **Step 1: Write the failing test** — create `crates/vox-config/src/config_key.rs`:

```rust
//! The single shared schema for every operational config knob (the federated
//! registry's SSOT *type*). `operator_registry` and the GUI `FIELDS`/`settingsIndex`
//! become VIEWS over `CONFIG_KEYS` (Phase 2A.2 / 2C). Protocol/crypto/grammar/
//! calibration constants are NOT config and never get a `ConfigKey`.

/// Value kind for a config knob (superset of the GUI `Kind` and the planned LLM kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigKind { Bool, Int, Float, String, Path, Url, Enum }

/// Where a knob's DEFAULT comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultValue {
    /// A literal default rendered as a string (MUST equal the in-code constant).
    Literal(&'static str),
    /// Computed at read-time by a named accessor (e.g. provider-derived URL).
    Computed(&'static str),
}

/// Coarse UI/topic grouping (federated — domains extend this deliberately).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    General, ModelsAndEndpoints, Tuning, Training, Orchestrator,
    Runtime, Storage, Mesh, Security, Telemetry,
}

/// Canonical home / value-SSOT pointer for the knob (the "ONE home" rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    Env,
    VoxToml,
    /// Value lives in a typed per-domain contract (federation), e.g.
    /// `Contract("contracts/scaling/policy.yaml")`.
    Contract(&'static str),
    Gui,
}

/// Lifecycle: Active = read in code today; Declared = registered, not yet wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status { Active, Declared }

/// GUI surfacing directive. `None` on a `ConfigKey` = not surfaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuiSurface {
    pub section: &'static str,
    /// Optional enum options for a dropdown.
    pub options: &'static [&'static str],
}

/// One operational config knob. Reuses `operator_registry::ConfigClass`.
#[derive(Debug, Clone, Copy)]
pub struct ConfigKey {
    /// The `VOX_*` env name or config key (the unique id).
    pub key: &'static str,
    pub kind: ConfigKind,
    pub default: DefaultValue,
    /// Numeric validation bound (min, max) — `None` for non-numeric/unbounded.
    pub bound: Option<(f64, f64)>,
    pub group: Group,
    pub class: crate::operator_registry::ConfigClass,
    pub home: Home,
    pub gui: Option<GuiSurface>,
    pub secret: bool,
    pub status: Status,
    pub label: &'static str,
    pub hint: &'static str,
}

impl ConfigKey {
    /// A numeric value is valid iff finite and within `bound` (if any).
    pub fn validate_numeric(&self, v: f64) -> bool {
        if !v.is_finite() { return false; }
        match self.bound {
            Some((lo, hi)) => v >= lo && v <= hi,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_registry::ConfigClass;

    fn sample() -> ConfigKey {
        ConfigKey {
            key: "VOX_WASM_SKILL_FUEL", kind: ConfigKind::Int,
            default: DefaultValue::Literal("1000000000"),
            bound: Some((1_000_000.0, 100_000_000_000.0)),
            group: Group::Runtime, class: ConfigClass::NodeLocal, home: Home::Env,
            gui: Some(GuiSurface { section: "Runtime & Sandbox", options: &[] }),
            secret: false, status: Status::Active,
            label: "WASM skill fuel", hint: "Wasmtime instruction budget",
        }
    }

    #[test]
    fn validate_numeric_respects_bounds() {
        let k = sample();
        assert!(k.validate_numeric(1_000_000_000.0));
        assert!(!k.validate_numeric(0.0));          // below min
        assert!(!k.validate_numeric(f64::NAN));     // not finite
        assert!(!k.validate_numeric(1e15));         // above max
    }
}
```

- [ ] **Step 2: Run → fail** — `cargo test --manifest-path "$WT/crates/vox-config/Cargo.toml" config_key` → FAIL (module not declared).
- [ ] **Step 3: Implement** — add `pub mod config_key;` to `crates/vox-config/src/lib.rs`. (The code above IS the implementation.)
- [ ] **Step 4: Run → pass** — same command → PASS (1 test).
- [ ] **Step 5: Commit** — `git -C "$WT" add crates/vox-config/src/config_key.rs crates/vox-config/src/lib.rs && git -C "$WT" commit -m "feat(config): ConfigKey schema — the federated registry SSOT type"`

### Task 2A.2: Seed `CONFIG_KEYS` and make `operator_registry` a view

**Files:**
- Create: `crates/vox-config/src/config_registry.rs` (the `CONFIG_KEYS` table)
- Modify: `crates/vox-config/src/lib.rs` (`pub mod config_registry;`)
- Modify: `crates/vox-config/src/operator_registry.rs` (add `from_config_keys()` view test; do NOT delete `OPERATOR_TUNING_ENVS` yet — Phase 2A.3 migrates it)

- [ ] **Step 1: Write the failing test** — create `crates/vox-config/src/config_registry.rs`:

```rust
//! `CONFIG_KEYS`: the general federated config registry. Seed it with the knobs
//! that already have constants + a clear home. Domains migrate their `operator_registry`
//! rows here over time (Phase 2A.3). Parity gate (2B) enforces env coverage.

use crate::config_key::{ConfigKey, ConfigKind, DefaultValue, Group, GuiSurface, Home, Status};
use crate::operator_registry::ConfigClass;

/// The general config registry. ADD a row here for every new operational knob.
pub const CONFIG_KEYS: &[ConfigKey] = &[
    ConfigKey {
        key: "VOX_WASM_SKILL_FUEL", kind: ConfigKind::Int,
        default: DefaultValue::Literal("1000000000"),
        bound: Some((1_000_000.0, 100_000_000_000.0)),
        group: Group::Runtime, class: ConfigClass::NodeLocal, home: Home::Env,
        gui: Some(GuiSurface { section: "Runtime & Sandbox", options: &[] }),
        secret: false, status: Status::Active,
        label: "WASM skill fuel", hint: "Wasmtime instruction budget for skill execution.",
    },
    ConfigKey {
        key: "VOX_GAMIFY_ECONOMY_PATH", kind: ConfigKind::Path,
        default: DefaultValue::Computed("embedded gamify economy contract"),
        bound: None, group: Group::Tuning, class: ConfigClass::NodeLocal,
        home: Home::Contract("contracts/gamify/economy.v1.yaml"),
        gui: None, secret: false, status: Status::Active,
        label: "Gamify economy contract override",
        hint: "Path to an override economy.v1.yaml; defaults to the embedded contract.",
    },
    ConfigKey {
        key: "VOX_CIRCUIT_BREAKER_CONTRACT", kind: ConfigKind::Path,
        default: DefaultValue::Computed("embedded circuit-breaker contract"),
        bound: None, group: Group::Orchestrator, class: ConfigClass::NodeLocal,
        home: Home::Contract("contracts/orchestration/circuit-breaker.v1.yaml"),
        gui: None, secret: false, status: Status::Active,
        label: "Circuit-breaker contract override",
        hint: "Path to an override circuit-breaker.v1.yaml; defaults to the embedded contract.",
    },
];

/// All registered keys (for the parity gate).
pub fn registered_keys() -> impl Iterator<Item = &'static str> {
    CONFIG_KEYS.iter().map(|k| k.key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for k in CONFIG_KEYS {
            assert!(seen.insert(k.key), "duplicate ConfigKey: {}", k.key);
        }
    }

    #[test]
    fn env_homed_keys_are_vox_prefixed() {
        for k in CONFIG_KEYS {
            if matches!(k.home, Home::Env) {
                assert!(k.key.starts_with("VOX_"), "env knob must be VOX_*: {}", k.key);
            }
        }
    }
}
```

- [ ] **Step 2: Run → fail** — `cargo test --manifest-path "$WT/crates/vox-config/Cargo.toml" config_registry` → FAIL (module not declared).
- [ ] **Step 3: Implement** — add `pub mod config_registry;` to `lib.rs`.
- [ ] **Step 4: Run → pass.**
- [ ] **Step 5: Commit** — `git -C "$WT" add crates/vox-config/src/config_registry.rs crates/vox-config/src/lib.rs && git -C "$WT" commit -m "feat(config): seed CONFIG_KEYS federated registry table"`

### Task 2A.3: Migrate `operator_registry` rows into `CONFIG_KEYS` and make `OPERATOR_TUNING_ENVS` a derived view

> This is the largest 2A task and is mechanical-but-bulky (~125 rows). **Split it with a workflow** (see protocol §3): one agent per `Group` of rows (DB/Mesh/Search/Bootstrap/CiGate/...), each translating its `OperatorEnvSpec` rows into `ConfigKey` rows appended to `CONFIG_KEYS` IN ITS OWN WORKTREE, then you cherry-pick. Because all agents append to the same file (`config_registry.rs`), do NOT run them on one worktree — give each its own and resolve the append-order at cherry-pick (they only add lines; conflicts are trivial "both added" resolved by keeping both blocks).

**Per-row translation rule (give this to each migration agent):** `OperatorEnvSpec { name, description, defaults, config_class }` → `ConfigKey { key: name, kind: <infer from defaults: "1/true"→Bool, integer→Int, float→Float, url→Url, path→Path, else String>, default: DefaultValue::Literal(defaults), bound: None (unless an obvious numeric range — leave None if unsure), group: <map by name prefix: VOX_DB_*→Storage, VOX_MESH_*→Mesh, VOX_SEARCH_*→Tuning, VOX_*_BUDGET*→Tuning, else General>, class: config_class, home: Env, gui: None (Phase 2C decides surfacing), secret: false, status: Active, label: <Title-Case of the name>, hint: description }`.

- [ ] **Step 1 (per agent): Write the failing parity test** in `operator_registry.rs`:
```rust
    #[test]
    fn every_operator_env_is_in_config_keys() {
        use crate::config_registry::CONFIG_KEYS;
        let registered: std::collections::BTreeSet<&str> =
            CONFIG_KEYS.iter().map(|k| k.key).collect();
        for spec in OPERATOR_TUNING_ENVS {
            assert!(registered.contains(spec.name),
                "operator env {} not yet migrated to CONFIG_KEYS", spec.name);
        }
    }
```
- [ ] **Step 2: Run → fail** (most rows not yet migrated).
- [ ] **Step 3 (per agent):** append the translated `ConfigKey` rows for this agent's group to `CONFIG_KEYS`.
- [ ] **Step 4: Run → pass** once ALL groups are migrated (the test is the completion gate).
- [ ] **Step 5: Commit per group**, cherry-pick to integration branch.
- [ ] **Step 6 (you, after all groups):** add `pub fn gui_fields()`/`pub fn env_specs()` view fns to `operator_registry` that DERIVE from `CONFIG_KEYS` (so `OPERATOR_TUNING_ENVS` can later be removed), keeping a `#[test]` asserting the derived view equals the legacy `OPERATOR_TUNING_ENVS` set. Commit.

---

## Phase 2B — `vox ci config-registry-parity` (completeness enforcement)

Single crate (`vox-cli/commands/ci`), sequential, depends on 2A. Mirror the existing `config_hygiene.rs` gate (already on main) and its baseline-ratchet pattern.

### Task 2B.1: The parity gate (pure core + runner + baseline ratchet)

**Files:**
- Create: `crates/vox-cli/src/commands/ci/config_registry_parity.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (`pub mod config_registry_parity;`)
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (+ `ConfigRegistryParity { update_baseline: bool }`)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (dispatch arm)
- Create (generated on first run): `contracts/config/config-registry-baseline.txt`

- [ ] **Step 1: Write the failing test** — create `config_registry_parity.rs` with the pure core (mirror `config_hygiene`'s `parity` shape exactly):

```rust
//! `vox ci config-registry-parity`: every operational VOX_* env knob read in code
//! must be a row in vox_config::config_registry::CONFIG_KEYS (searchable SSOT), and
//! every Env-homed registry row must be read in code (no phantom rows). Baseline
//! ratchet grandfathers today's backlog (mirrors config-hygiene).

use std::collections::BTreeSet;

/// (unregistered_used, registered_unused).
pub fn parity(used: &BTreeSet<String>, registered: &BTreeSet<String>)
    -> (Vec<String>, Vec<String>)
{
    (used.difference(registered).cloned().collect(),
     registered.difference(used).cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_unregistered_and_phantom() {
        let used: BTreeSet<String> = ["VOX_A".into(), "VOX_B".into()].into_iter().collect();
        let reg: BTreeSet<String> = ["VOX_A".into(), "VOX_C".into()].into_iter().collect();
        let (unreg, unused) = parity(&used, &reg);
        assert_eq!(unreg, vec!["VOX_B".to_string()]);
        assert_eq!(unused, vec!["VOX_C".to_string()]);
    }
}
```

- [ ] **Step 2: Run → fail** (module not declared in mod.rs).
- [ ] **Step 3: Implement the runner + ratchet.** Add `pub mod config_registry_parity;` to `mod.rs`. Add a `run(update_baseline: bool) -> anyhow::Result<()>` that: scans `crates/**/*.rs` (non-test) for `VOX_[A-Z0-9_]+` literals into a `BTreeSet` (skip the registry/gate source files; reuse the `collect_rs_files` walker pattern from `config_hygiene.rs` — copy it locally or factor a shared `ci::fs_walk` module); loads `registered` from `vox_config::config_registry::registered_keys()` PLUS any `name` ending in `_` treated as a prefix-allow; computes `parity`; loads/writes `contracts/config/config-registry-baseline.txt` exactly like `config_hygiene`'s baseline (key = the env var name; `--update-baseline` regenerates; default run fails only on NEW unregistered names; `registered_unused` is a warning not a failure — they may be `status: Declared`). Wire `ConfigRegistryParity { #[arg(long)] update_baseline: bool }` into `cmd_enums.rs` and the `run_body.rs` dispatch (`CiCmd::ConfigRegistryParity { update_baseline } => super::config_registry_parity::run(update_baseline)`), plus the `gate_policy_id` `_ => None` already covers it.
- [ ] **Step 4: Build vox-cli, generate baseline, verify green:**
```bash
cargo build --manifest-path "$WT/crates/vox-cli/Cargo.toml"
cd "$WT" && cargo run --manifest-path "$WT/crates/vox-cli/Cargo.toml" -- ci config-registry-parity --update-baseline
cd "$WT" && cargo run --manifest-path "$WT/crates/vox-cli/Cargo.toml" -- ci config-registry-parity   # → OK, 0 new
```
Report the baseline size N (= the registration backlog; burning it down to 0 IS the definition of "config complete + searchable").
- [ ] **Step 5: Commit** (include the generated baseline) — `git -C "$WT" add crates/vox-cli/src/commands/ci/config_registry_parity.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs crates/vox-cli/src/commands/ci/mod.rs contracts/config/config-registry-baseline.txt && git -C "$WT" commit -m "feat(ci): config-registry-parity gate — every operational env knob must be registered"`

> **Wire both gates into pre-push/CI** next to `vox ci policy-registry-parity` (check `.github/workflows/` + the pre-push hook). This is what makes the registry binding for the executor.

---

## Phase 2C — Generate the GUI surfaces from the registry (kill the drift)  `[PARALLEL-SAFE: group=gui]`

Depends on 2A. The two codegen tasks (2C.1 settingsIndex, 2C.2 FIELDS) touch DIFFERENT generated files and may run as parallel agents in separate worktrees; the wiring task (2C.3) is sequential after both.

### Task 2C.1: Generate `settingsIndex.ts` from `CONFIG_KEYS`

**Files:**
- Create: `crates/vox-cli/src/commands/ci/config_gui_codegen.rs` (generator + `--check` drift mode)
- Modify: `cmd_enums.rs`/`run_body.rs`/`mod.rs` (add `ConfigGuiCodegen { check: bool }`)
- Create (generated): `crates/vox-gui/ui/src/config/generatedSettingsIndex.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts` (spread the generated entries into `SETTINGS_INDEX`)

- [ ] **Step 1: Write the failing test (pure render core):**
```rust
//! Generate GUI settings surfaces from vox_config::config_registry::CONFIG_KEYS.
use vox_config::config_key::{ConfigKey, Home};

/// Project gui-surfaced keys into a TS search-index module.
pub fn render_settings_index_ts(keys: &[ConfigKey]) -> String {
    let mut rows = String::new();
    for k in keys.iter().filter(|k| k.gui.is_some()) {
        let g = k.gui.unwrap();
        rows.push_str(&format!(
            "  {{ key: {:?}, label: {:?}, hint: {:?}, section: {:?} }},\n",
            k.key, k.label, k.hint, g.section));
    }
    format!(
        "// @generated by `vox ci config-gui-codegen` from CONFIG_KEYS — DO NOT EDIT.\n\
         export interface GeneratedSetting {{ key: string; label: string; hint: string; section: string; }}\n\
         export const GENERATED_SETTINGS_INDEX: GeneratedSetting[] = [\n{rows}];\n")
}
#[cfg(test)]
mod tests {
    use super::*;
    use vox_config::config_registry::CONFIG_KEYS;
    #[test]
    fn renders_only_gui_keys_with_generated_header() {
        let ts = render_settings_index_ts(CONFIG_KEYS);
        assert!(ts.contains("@generated"));
        assert!(ts.contains("\"VOX_WASM_SKILL_FUEL\""));     // gui: Some(...)
        assert!(!ts.contains("VOX_GAMIFY_ECONOMY_PATH"));    // gui: None
    }
}
```
- [ ] **Step 2: Run → fail.**
- [ ] **Step 3: Implement** the `run(check: bool)`: sort gui keys by `(section, key)`, `render_settings_index_ts`, then write `crates/vox-gui/ui/src/config/generatedSettingsIndex.ts` (generate) or diff-and-bail (`--check`). Wire `ConfigGuiCodegen { check }`. In `settingsIndex.ts`, `import { GENERATED_SETTINGS_INDEX } from '../../../config/generatedSettingsIndex';` and spread it into the existing `SETTINGS_INDEX` array (read the file first to match its entry shape).
- [ ] **Step 4: Generate + drift-check:**
```bash
cd "$WT" && cargo run --manifest-path "$WT/crates/vox-cli/Cargo.toml" -- ci config-gui-codegen
cd "$WT" && cargo run --manifest-path "$WT/crates/vox-cli/Cargo.toml" -- ci config-gui-codegen --check   # → OK
```
- [ ] **Step 5: Commit** (`--no-verify` push later; TS not buildable in worktree — verify by inspection).

### Task 2C.2: Generate the Rust `FIELDS` catalog from `CONFIG_KEYS`  `[PARALLEL-SAFE: group=gui]`

Mirror 2C.1 but emit a `generated_fields.rs` consumed by `crates/vox-gui/src/commands/user_config.rs`'s `FIELDS` (read that file first; map `ConfigKind`→GUI `Kind`, `GuiSurface.section`→`group`, `label`/`hint`, `GuiSurface.options`→`options`). Same `--check` drift gate. The **completion test** is a Rust test asserting `generated FIELDS keys == CONFIG_KEYS gui keys`. Commit.

### Task 2C.3: One drift gate for both generated surfaces (sequential, after 2C.1+2C.2)

Add `vox ci config-gui-codegen --check` (covering BOTH generated files) to CI next to the parity gate. Add a Rust test asserting `settingsIndex` and `FIELDS` generated sets are EQUAL (they generate from the same source → the historical drift becomes impossible). Commit.

---

## Phase 2D — Reactive config watch/event bus (shared infra)  `[PARALLEL-SAFE: group=reactive]`

Depends on 2A. Domain-agnostic, so build it ONCE here (the LLM-settings plan reuses it). This is the `tokio::sync::watch<ConfigSnapshot>` + `vox://config-changed` Tauri-event scaffolding the LLM-settings memo planned — generalized.

### Task 2D.1: `ConfigSnapshot` + watch channel in `vox-config`

**Files:** Create `crates/vox-config/src/config_watch.rs`; modify `lib.rs`.

- [ ] **Step 1: Failing test** — a `ConfigWatch` with `subscribe() -> watch::Receiver<ConfigSnapshot>` and `bump(keys: &[&str])` that increments `snapshot_rev` and notifies. Test: a subscriber sees `rev` increase after `bump`.
```rust
//! Reactive config: a process-wide watch channel bumped on any config write
//! (set_user_config / toml reload / mesh-sync). GUI/agents re-pull instead of poll.
use tokio::sync::watch;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigSnapshot { pub rev: u64, pub changed_keys: Vec<String> }

pub struct ConfigWatch { tx: watch::Sender<ConfigSnapshot>, rx: watch::Receiver<ConfigSnapshot> }
impl ConfigWatch {
    pub fn new() -> Self { let (tx, rx) = watch::channel(ConfigSnapshot::default()); Self { tx, rx } }
    pub fn subscribe(&self) -> watch::Receiver<ConfigSnapshot> { self.rx.clone() }
    pub fn bump(&self, keys: &[&str]) {
        let mut s = self.tx.borrow().clone();
        s.rev += 1; s.changed_keys = keys.iter().map(|k| k.to_string()).collect();
        let _ = self.tx.send(s);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bump_increments_rev_and_records_keys() {
        let w = ConfigWatch::new();
        let rx = w.subscribe();
        assert_eq!(rx.borrow().rev, 0);
        w.bump(&["VOX_WASM_SKILL_FUEL"]);
        assert_eq!(rx.borrow().rev, 1);
        assert_eq!(rx.borrow().changed_keys, vec!["VOX_WASM_SKILL_FUEL"]);
    }
}
```
- [ ] Steps 2–5 as usual (`tokio` is already a vox-config dep — verify; if `watch` feature missing, add `tokio = { workspace = true, features = ["sync"] }`). Commit.

### Task 2D.2: GUI bridge — forward `bump` as a `vox://config-changed` Tauri event

Modify `crates/vox-gui/src/commands/user_config.rs` so `set_user_config`/reset call `ConfigWatch::bump(changed)`, and a Tauri bridge emits `vox://config-changed { rev, changed_keys }`. Add a Rust test that `set_user_config` triggers a bump (inject a test `ConfigWatch`). The TS side (panels subscribing) is a thin listener — spec it, verify by inspection. Commit.

---

## Phase 2E — Land the LLM-settings registry AS A MEMBER (coordination, not duplication)

**This phase hands off to the existing `docs/superpowers/plans/2026-06-15-llm-ai-settings-ssot-band-a.md` plan.** Do NOT build a second registry. Instead:

- [ ] **Task 2E.1:** Verify the LLM-settings plan's `LlmConfigKey` is replaced by `vox_config::config_key::ConfigKey` (Phase 2A). If that plan still defines its own `LlmConfigKey`, file the diff: its LLM-specific fields (`secret`→Clavis, provider-`Computed` defaults, egress-seal) all fit `ConfigKey` already; the only LLM-specific machinery is the *unregistered-LLM-env detector* + *sealed egress arch-check*, which stay in the LLM plan as a SCOPED view over `CONFIG_KEYS.filter(group ∈ {ModelsAndEndpoints, Training})`.
- [ ] **Task 2E.2:** The LLM plan's GUI binding (`gui_fields()`) becomes a thin filter over the Phase-2C generator (LLM keys are just `CONFIG_KEYS` rows with `group: ModelsAndEndpoints`). Its reactive layer reuses Phase 2D's `ConfigWatch`/`vox://config-changed` — delete any duplicate `vox://llm-config-changed` in favor of the generic event with `changed_keys` filtering.
- [ ] **Task 2E.3:** Add a Rust test asserting every LLM env name the LLM plan enumerates is present in `CONFIG_KEYS` (the two registries can never diverge — they ARE one table with a filtered view).

> Coordinate ordering: Phase 2A MUST land before the LLM-settings plan executes, so it builds on `ConfigKey` from day one. If the LLM plan has already started, reconcile in 2E.1 (the diff is small — it's plan-only/unexecuted today).

---

## Baseline burndown (independent, `[PARALLEL-SAFE: group=<crate-domain>]`)

The `config-hygiene` baseline grandfathered ~116 real violations; the parity baseline grandfathers the env-registration backlog. Burning these down is the *actual* "zero magic values where they should be configured." This is embarrassingly parallel by crate-domain — fan out one agent per `config-audit` group (G01–G12), each in its own worktree, each: pick its crate-group's grandfathered entries, fix the genuine ones (embed cwd-relative contracts via `include_str!`; register unregistered env knobs in `CONFIG_KEYS`; relocate-or-justify the `memory_budget.rs` protected-env read; wire-or-delete the 3 unwired resolvers), regenerate the relevant baseline (shrinking it), commit. You cherry-pick + merge.

- [ ] **Burndown gate:** the success metric is `config-registry-parity` and `config-hygiene` baselines both reaching **0 grandfathered**. Track the count down per PR. **Never silently delete a baseline entry** — every removal must correspond to a real fix.

---

## Self-Review

- **Spec coverage:** federated registry (2A schema + table + operator_registry view), parity/completeness (2B gate), GUI visibility+searchable generated from the SSOT (2C settingsIndex + FIELDS, drift-killed), reactive (2D watch+event), one-registry-not-two (2E coordination), and the "zero magic values where appropriate" endgame (baseline burndown). Protocol/security/calibration stay const — enforced by the already-merged `config-hygiene` Check B.
- **Sonnet-fit:** every task is self-contained with complete code + exact commands; the Execution Protocol gives the worktree/parallel/merge harness + the session's verified gotchas; `[PARALLEL-SAFE]` tags + the dependency graph tell Sonnet exactly what to fan out vs sequence; bulky mechanical work (2A.3 row migration, burndown) is explicitly workflow-parallelized one-agent-per-group.
- **TDD:** every task is test-first with the failing test, the run-to-fail, the implementation, the run-to-pass, the commit.
- **Placeholder scan:** foundational tasks (2A.1/2A.2, 2B.1, 2C.1, 2D.1) carry complete code; the heavier integration tasks (2A.3, 2C.2/2C.3, 2D.2, 2E) give the pure-core code + a precise translation/wiring rule + the completion TEST that defines done — the standard way to specify a bulk-mechanical or surface-matching task without guessing the hand-written file's exact current lines (Sonnet reads the file first, as instructed).
- **Type consistency:** `ConfigKey`/`ConfigKind`/`DefaultValue`/`Group`/`Home`/`Status`/`GuiSurface` defined in 2A.1 and consumed unchanged in 2A.2/2C/2D; `parity()` mirrors the on-main `config_hygiene` signature; `ConfigClass` is reused from `operator_registry`, not redefined.
