# Track A — Backend Control SSOT (ClutchProfile + RiskPosture) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** A single pure-logic source of truth that maps the user's clutch detent and risk posture onto the existing routing/budget/safety knobs, with a YAML contract and parity gate so FE and BE never drift.

**Architecture:** Two new pure enums in `vox-orchestrator/src/mode.rs` — `ClutchProfile` (Free/Efficiency/Balanced/Genius) and `RiskPosture` (High/Moderate/Low) — each with a deterministic `resolve()` that returns a struct of existing knobs (`QualityLevel`, cost preference, free-delegation flag, budget-gate aggressiveness, approval/verification flags, safety-token multiplier, model-lean). A committed `contracts/gui/drive-console.v1.yaml` mirrors the mapping; a parity test asserts code == YAML.

**Tech Stack:** Rust (vox-orchestrator, vox-config), serde, TOML/YAML contract, existing test harness.

**Scope marker:** `[SEQUENTIAL]` — foundational; land before B–F.
**Execution target:** Sonnet 4.6.

---

## Audit Corrections — verified against code 2026-06-20 (read FIRST; overrides stale claims below)

- **CONFIRMED:** `crate::config::CostPreference` import path is correct (`mode.rs:5`); `QualityLevel{Flash,Balanced,Premium}` (`mode.rs:14-20`); `SelectionAxes` presets exact — `COST_FIRST` (70/15/15) `select.rs:287`, `BALANCED` (33/33/34) `:294`, `QUALITY_FIRST` (15/15/70) `:302`, `FAST` (15/70/15) `:310`; `serde_yaml` is a workspace dep of `vox-orchestrator` (`Cargo.toml:95` + build-deps `:131`) → available in tests, **no `cargo add` needed** (delete that instruction in Task 4 Step 2); `pub mod control;` convention OK; the parity test runs automatically under `cargo test` (no separate registry-gate file — the repo's `tool-registry.canonical` style gate is for config registries, not this; Task 4 Step 4 "wire into CI gate" is therefore a no-op — **delete it**, the test self-includes).
- **CONTRACT PATH CONFIRMED:** from `CARGO_MANIFEST_DIR` (= crate root `crates/vox-orchestrator/`) the YAML is `/../../contracts/gui/drive-console.v1.yaml` (up 2). Correct as written. `contracts/gui/` already exists.
- **TASK 5 WAS WRONG — DO NOT DELETE `ExecutionModeProfile`.** It is used in `vox-research-shim` (6 refs: `selection/scorer.rs:61,178-195`, `selection/tests.rs:88,95,113,120`) and re-exported at `vox-orchestrator/src/lib.rs:379`. Deleting it breaks the build. **Replace Task 5 with:** leave `ExecutionModeProfile` in place for now and add a doc-comment `// superseded by ClutchProfile; migrate vox-research-shim then remove` — actual removal is a separate follow-up after the scorer is migrated to consume `ResolvedClutch`. Confirm with `grep -rn "ExecutionModeProfile" crates/` (expect the 6+1 refs above) and STOP if you were about to delete.
- **DESIGN NOTE (code-review):** the plan invents `ApprovalLean` parallel to the real `attention::ApprovalTier{AutoApprove,Confirm,Review,Blocked}` (`attention/budget.rs`). Keep `ApprovalLean` as the *risk-posture intent* (3 values, no `Blocked`) but document the mapping `AutoApproveMore→AutoApprove`, `Confirm→Confirm`, `Review→Review` so Track E wires posture→tier without a second source of truth. Add a `pub fn to_approval_tier(self) -> attention::ApprovalTier` on `ApprovalLean` and a test, so the mapping is code, not prose.

---

## File Structure

- Modify: `crates/vox-orchestrator/src/mode.rs` — add `ClutchProfile`, `RiskPosture`, resolved structs, `resolve()`.
- Create: `contracts/gui/drive-console.v1.yaml` — the detent→knob and posture→gate map (shared SSOT).
- Create: `crates/vox-orchestrator/src/control/drive_console_parity.rs` — parity test code → YAML.
- Modify: `crates/vox-orchestrator/src/lib.rs` — `pub mod control;` (or extend existing module tree).

Reuses (do **not** redefine): `QualityLevel` (`mode.rs:14`), `CostPreference` (`vox-config`), `PoolRule::Free`
(`vox-config/src/model_pool.rs:9`). `SelectionAxes` presets (`models/select.rs:289`) are referenced by name
in the resolved struct (cost/responsiveness/intelligence triple), not imported, to keep this module leaf-pure.

---

### Task 1: `ClutchProfile` enum + resolved knobs

**Files:**
- Modify: `crates/vox-orchestrator/src/mode.rs`

- [ ] **Step 1: Write the failing test** (append to the `#[cfg(test)]` block in `mode.rs`)

```rust
#[cfg(test)]
mod clutch_tests {
    use super::*;
    use crate::config::CostPreference;

    #[test]
    fn clutch_resolves_each_detent() {
        let free = ClutchProfile::Free.resolve();
        assert_eq!(free.quality, QualityLevel::Flash);
        assert_eq!(free.cost_preference, CostPreference::Economy);
        assert!(free.force_free_pool);
        assert!(free.always_delegate_free);
        assert_eq!(free.axes, (70, 15, 15));

        let eff = ClutchProfile::Efficiency.resolve();
        assert_eq!(eff.quality, QualityLevel::Flash);
        assert!(!eff.force_free_pool);
        assert!(!eff.always_delegate_free);
        assert!(eff.delegate_free_when_simple);
        assert_eq!(eff.budget_gate, BudgetAggressiveness::Default);

        let bal = ClutchProfile::Balanced.resolve();
        assert_eq!(bal.quality, QualityLevel::Balanced);
        assert_eq!(bal.axes, (33, 33, 34));

        let genius = ClutchProfile::Genius.resolve();
        assert_eq!(genius.quality, QualityLevel::Premium);
        assert_eq!(genius.cost_preference, CostPreference::Performance);
        assert_eq!(genius.axes, (15, 15, 70));
        assert_eq!(genius.budget_gate, BudgetAggressiveness::Relaxed);
        assert!(!genius.delegate_free_when_simple);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator clutch_resolves_each_detent 2>cargo-clutch.log; tail -30 cargo-clutch.log`
Expected: FAIL — `cannot find type ClutchProfile`.

- [ ] **Step 3: Write minimal implementation** (add above the test block in `mode.rs`)

```rust
/// Budget-gate aggressiveness selected by the clutch. Maps onto the existing
/// downgrade@80%/halt@95% gate (`budget_gate.rs`): `Aggressive` lowers thresholds,
/// `Relaxed` converts halt into a warn (Genius keeps going, with consent surfaced by the UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetAggressiveness {
    Aggressive,
    Default,
    Relaxed,
}

/// Resolved control knobs for one clutch detent. Pure data — no I/O.
/// `axes` is the (cost, responsiveness, intelligence) triple consumed by
/// `SelectionAxes` at the scorer candidate boundary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedClutch {
    pub quality: QualityLevel,
    pub cost_preference: CostPreference,
    pub axes: (u8, u8, u8),
    pub force_free_pool: bool,
    pub always_delegate_free: bool,
    pub delegate_free_when_simple: bool,
    pub budget_gate: BudgetAggressiveness,
}

/// User-facing "how much gas" control. Single SSOT for the four detents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClutchProfile {
    Free,
    #[default]
    Efficiency,
    Balanced,
    Genius,
}

impl ClutchProfile {
    #[must_use]
    pub fn resolve(self) -> ResolvedClutch {
        match self {
            Self::Free => ResolvedClutch {
                quality: QualityLevel::Flash,
                cost_preference: CostPreference::Economy,
                axes: (70, 15, 15),
                force_free_pool: true,
                always_delegate_free: true,
                delegate_free_when_simple: true,
                budget_gate: BudgetAggressiveness::Aggressive,
            },
            Self::Efficiency => ResolvedClutch {
                quality: QualityLevel::Flash,
                cost_preference: CostPreference::Economy,
                axes: (70, 15, 15),
                force_free_pool: false,
                always_delegate_free: false,
                delegate_free_when_simple: true,
                budget_gate: BudgetAggressiveness::Default,
            },
            Self::Balanced => ResolvedClutch {
                quality: QualityLevel::Balanced,
                cost_preference: CostPreference::Economy,
                axes: (33, 33, 34),
                force_free_pool: false,
                always_delegate_free: false,
                delegate_free_when_simple: false,
                budget_gate: BudgetAggressiveness::Default,
            },
            Self::Genius => ResolvedClutch {
                quality: QualityLevel::Premium,
                cost_preference: CostPreference::Performance,
                axes: (15, 15, 70),
                force_free_pool: false,
                always_delegate_free: false,
                delegate_free_when_simple: false,
                budget_gate: BudgetAggressiveness::Relaxed,
            },
        }
    }
}
```

Ensure the `use crate::config::CostPreference;` import already at the top of `mode.rs` (line 5) covers this; add `use serde::{Deserialize, Serialize};` is already present (line 3).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator clutch_resolves_each_detent 2>cargo-clutch.log; tail -20 cargo-clutch.log`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs
git commit -m "feat(orchestrator): ClutchProfile SSOT (detent -> routing/budget knobs)"
```

---

### Task 2: `RiskPosture` enum + resolved gates

**Files:**
- Modify: `crates/vox-orchestrator/src/mode.rs`

- [ ] **Step 1: Write the failing test** (append a new test module)

```rust
#[cfg(test)]
mod risk_tests {
    use super::*;

    #[test]
    fn risk_resolves_each_posture() {
        let high = RiskPosture::High.resolve();
        assert_eq!(high.approval, ApprovalLean::AutoApproveMore);
        assert!(!high.grounding_enforce);
        assert!(!high.socrates_enforce);
        assert_eq!(high.safety_token_multiplier, 1.0);
        assert_eq!(high.model_lean, ModelLean::Neutral); // High lets the clutch pick freely

        let mid = RiskPosture::Moderate.resolve();
        assert_eq!(mid.approval, ApprovalLean::Confirm);
        assert!(mid.grounding_enforce);
        assert!(!mid.socrates_enforce);
        assert_eq!(mid.safety_token_multiplier, 1.0);

        let low = RiskPosture::Low.resolve();
        assert_eq!(low.approval, ApprovalLean::Review);
        assert!(low.grounding_enforce);
        assert!(low.socrates_enforce);
        assert!(low.safety_token_multiplier > 1.0);
        assert_eq!(low.model_lean, ModelLean::Intelligence); // overrides cheap pick
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator risk_resolves_each_posture 2>cargo-risk.log; tail -30 cargo-risk.log`
Expected: FAIL — `cannot find type RiskPosture`.

- [ ] **Step 3: Write minimal implementation** (add above the new test block)

```rust
/// How strongly to gate completion by human/auto approval. Maps onto the existing
/// `ApprovalTier` (`attention/mod.rs`) at the attention gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalLean {
    AutoApproveMore,
    Confirm,
    Review,
}

/// Whether risk nudges the model choice independent of the clutch. `Intelligence`
/// overrides a cheap clutch pick toward an intelligence-weighted candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelLean {
    Neutral,
    Intelligence,
}

/// Resolved safety gates for one risk posture. Pure data — no I/O.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedRisk {
    pub approval: ApprovalLean,
    pub grounding_enforce: bool,
    pub socrates_enforce: bool,
    pub safety_token_multiplier: f64,
    pub model_lean: ModelLean,
}

/// User-facing acceptable-risk control. Higher risk = break things, spend less on safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RiskPosture {
    High,
    #[default]
    Moderate,
    Low,
}

impl RiskPosture {
    #[must_use]
    pub fn resolve(self) -> ResolvedRisk {
        match self {
            Self::High => ResolvedRisk {
                approval: ApprovalLean::AutoApproveMore,
                grounding_enforce: false,
                socrates_enforce: false,
                safety_token_multiplier: 1.0,
                model_lean: ModelLean::Neutral,
            },
            Self::Moderate => ResolvedRisk {
                approval: ApprovalLean::Confirm,
                grounding_enforce: true,
                socrates_enforce: false,
                safety_token_multiplier: 1.0,
                model_lean: ModelLean::Neutral,
            },
            Self::Low => ResolvedRisk {
                approval: ApprovalLean::Review,
                grounding_enforce: true,
                socrates_enforce: true,
                safety_token_multiplier: 1.5,
                model_lean: ModelLean::Intelligence,
            },
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator risk_resolves_each_posture 2>cargo-risk.log; tail -20 cargo-risk.log`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs
git commit -m "feat(orchestrator): RiskPosture SSOT (posture -> approval/verification gates)"
```

---

### Task 3: Clutch × Risk interaction — effective model lean

The spec requires risk to **visibly override** the clutch's model pick (Low risk beats Efficiency's cheap pick).
Encode the interaction in one pure function so FE read-out and BE scorer agree.

**Files:**
- Modify: `crates/vox-orchestrator/src/mode.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod interaction_tests {
    use super::*;

    #[test]
    fn low_risk_overrides_efficiency_cheap_pick() {
        // Efficiency alone leans cost; Low risk forces intelligence.
        let axes = effective_axes(ClutchProfile::Efficiency, RiskPosture::Low);
        assert_eq!(axes, (15, 15, 70));
    }

    #[test]
    fn high_risk_keeps_clutch_axes() {
        let axes = effective_axes(ClutchProfile::Efficiency, RiskPosture::High);
        assert_eq!(axes, (70, 15, 15));
    }

    #[test]
    fn genius_already_intelligent_unchanged_by_low_risk() {
        let axes = effective_axes(ClutchProfile::Genius, RiskPosture::Low);
        assert_eq!(axes, (15, 15, 70));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator effective_axes 2>cargo-int.log; tail -30 cargo-int.log`
Expected: FAIL — `cannot find function effective_axes`.

- [ ] **Step 3: Write minimal implementation**

```rust
/// The (cost, responsiveness, intelligence) triple the scorer should use, after risk
/// overrides the clutch. `ModelLean::Intelligence` (Low risk) forces an
/// intelligence-weighted axis regardless of a cheaper clutch detent.
#[must_use]
pub fn effective_axes(clutch: ClutchProfile, risk: RiskPosture) -> (u8, u8, u8) {
    let base = clutch.resolve().axes;
    match risk.resolve().model_lean {
        ModelLean::Intelligence => (15, 15, 70),
        ModelLean::Neutral => base,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator effective_axes 2>cargo-int.log; tail -20 cargo-int.log`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs
git commit -m "feat(orchestrator): clutch x risk interaction (low risk overrides cheap pick)"
```

---

### Task 4: `drive-console.v1.yaml` contract + parity gate

Mirror the mapping in a committed contract and assert code == YAML, matching the repo's existing
registry-parity gate pattern (e.g. `tool-registry.canonical.yaml`).

**Files:**
- Create: `contracts/gui/drive-console.v1.yaml`
- Create: `crates/vox-orchestrator/src/control/mod.rs`
- Modify: `crates/vox-orchestrator/src/lib.rs` (add `pub mod control;`)

- [ ] **Step 1: Create the contract**

```yaml
# contracts/gui/drive-console.v1.yaml
# SSOT for the chat Drive Console. FE reads this for labels/order; BE asserts parity.
version: 1
clutch:
  - id: free
    quality: flash
    axes: [70, 15, 15]
    force_free_pool: true
  - id: efficiency
    quality: flash
    axes: [70, 15, 15]
    force_free_pool: false
  - id: balanced
    quality: balanced
    axes: [33, 33, 34]
    force_free_pool: false
  - id: genius
    quality: premium
    axes: [15, 15, 70]
    force_free_pool: false
risk:
  - id: high
    approval: auto_approve_more
    grounding_enforce: false
    socrates_enforce: false
  - id: moderate
    approval: confirm
    grounding_enforce: true
    socrates_enforce: false
  - id: low
    approval: review
    grounding_enforce: true
    socrates_enforce: true
```

- [ ] **Step 2: Write the failing parity test**

Create `crates/vox-orchestrator/src/control/mod.rs`:

```rust
//! Drive Console control SSOT parity with `contracts/gui/drive-console.v1.yaml`.

#[cfg(test)]
mod parity_tests {
    use crate::mode::{ClutchProfile, QualityLevel, RiskPosture, ApprovalLean};

    fn quality_str(q: QualityLevel) -> &'static str {
        match q { QualityLevel::Flash => "flash", QualityLevel::Balanced => "balanced", QualityLevel::Premium => "premium" }
    }
    fn approval_str(a: ApprovalLean) -> &'static str {
        match a { ApprovalLean::AutoApproveMore => "auto_approve_more", ApprovalLean::Confirm => "confirm", ApprovalLean::Review => "review" }
    }

    #[test]
    fn code_matches_contract() {
        let raw = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../contracts/gui/drive-console.v1.yaml"),
        ).expect("contract present");
        let doc: serde_yaml::Value = serde_yaml::from_str(&raw).expect("valid yaml");

        for (i, clutch) in [ClutchProfile::Free, ClutchProfile::Efficiency, ClutchProfile::Balanced, ClutchProfile::Genius].into_iter().enumerate() {
            let r = clutch.resolve();
            let row = &doc["clutch"][i];
            assert_eq!(row["quality"].as_str().unwrap(), quality_str(r.quality), "clutch[{i}] quality");
            assert_eq!(row["force_free_pool"].as_bool().unwrap(), r.force_free_pool, "clutch[{i}] pool");
            let axes: Vec<u8> = row["axes"].as_sequence().unwrap().iter().map(|v| v.as_u64().unwrap() as u8).collect();
            assert_eq!((axes[0], axes[1], axes[2]), r.axes, "clutch[{i}] axes");
        }
        for (i, risk) in [RiskPosture::High, RiskPosture::Moderate, RiskPosture::Low].into_iter().enumerate() {
            let r = risk.resolve();
            let row = &doc["risk"][i];
            assert_eq!(row["approval"].as_str().unwrap(), approval_str(r.approval), "risk[{i}] approval");
            assert_eq!(row["grounding_enforce"].as_bool().unwrap(), r.grounding_enforce, "risk[{i}] grounding");
            assert_eq!(row["socrates_enforce"].as_bool().unwrap(), r.socrates_enforce, "risk[{i}] socrates");
        }
    }
}
```

Add `pub mod control;` to `crates/vox-orchestrator/src/lib.rs` (next to the other `pub mod` lines).
Confirm `serde_yaml` is a dev-dependency of `vox-orchestrator`; if not, add it:
`cargo add serde_yaml --dev -p vox-orchestrator`.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator code_matches_contract 2>cargo-parity.log; tail -30 cargo-parity.log`
Expected: PASS. (If FAIL on path, adjust the `concat!` relative path to the workspace root.)

- [ ] **Step 4: Wire into CI gate**

Add a line to the arch/registry parity gate list so the parity test is part of the required check. Find the
existing parity-gate invocation (grep `attention_ledger_parity` or `tool-registry.canonical` under
`scripts/` / `crates/*/ci`) and add `code_matches_contract` to the same gate set. If the gate runs
`cargo test`-by-name, append this test name; if it enumerates a YAML registry list, add `drive-console.v1.yaml`.

- [ ] **Step 5: Commit**

```bash
git add contracts/gui/drive-console.v1.yaml crates/vox-orchestrator/src/control/mod.rs crates/vox-orchestrator/src/lib.rs crates/vox-orchestrator/Cargo.toml
git commit -m "feat(control): drive-console.v1 contract + code/YAML parity gate"
```

---

### Task 5: Clean up the dead `ExecutionModeProfile` (DRY)

`ExecutionModeProfile` (`mode.rs:47`) is dead (audit: "defined but unused"). `ClutchProfile` supersedes it.
Remove it to prevent a second drifting mode taxonomy — but only after confirming zero references.

**Files:**
- Modify: `crates/vox-orchestrator/src/mode.rs`

- [ ] **Step 1: Confirm it is unused**

Run: `grep -rn "ExecutionModeProfile" crates/ --include=*.rs`
Expected: only the definition at `mode.rs:47-54`. If other references exist, STOP and leave it; note in the
plan-audit pass. If only the definition appears, proceed.

- [ ] **Step 2: Remove the enum**

Delete the `ExecutionModeProfile` enum block (`mode.rs:47-54`).

- [ ] **Step 3: Verify the crate still builds + tests pass**

Run: `cargo test -p vox-orchestrator 2>cargo-all.log; tail -30 cargo-all.log`
Expected: PASS, no `unused`/`missing` errors.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator/src/mode.rs
git commit -m "refactor(orchestrator): drop dead ExecutionModeProfile (superseded by ClutchProfile)"
```

---

## Self-Review

**Spec coverage (Track A scope only):** §3.1 clutch→knob map → Task 1. §3.3 risk→gate map → Task 2.
"risk visibly interacts with the clutch" (§3.3) → Task 3. "contracts/gui/drive-console.v1.yaml … parity gate"
(§8) → Task 4. DRY of dead `ExecutionModeProfile` (Problem §1/audit) → Task 5. Attribution capture,
interrupt_task, UI, loop, mission control, dashboard, automation are **explicitly out of Track A** and live in
Tracks B–F (see index).

**Placeholder scan:** none — every step has complete code or an exact command. Task 4 Step 4 names a discovery
step (locate the existing gate) rather than guessing its path; this is a real action, not a placeholder.

**Type consistency:** `ResolvedClutch`/`ResolvedRisk`/`effective_axes`/`BudgetAggressiveness`/`ApprovalLean`/
`ModelLean` names are used identically across tasks 1–4. `axes` is `(u8,u8,u8)` throughout. Contract field
names (`force_free_pool`, `grounding_enforce`, `socrates_enforce`, `axes`) match the parity test reads.

**Downstream consumption (not in Track A):** Track B/D import `ResolvedClutch`/`ResolvedRisk` and call
`effective_axes()` at `models/select::decide` (after pool application) and at the attention/verification gates.
That wiring is deliberately deferred so Track A stays a pure, independently-testable leaf.
