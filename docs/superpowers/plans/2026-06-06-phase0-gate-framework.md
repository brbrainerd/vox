---
title: Phase 0 — v1.0 Gate Framework Implementation Plan
description: Detailed TDD plan to add tier-aware foundation-first ordering, the --gate all --strict-block-ga GA roll-up, the first real foundation gate, product-binary GA inclusion, and the CR-META criteria-format lint to vox-audit / vox-cli / vox-arch-check.
category: architecture
---

# Phase 0 — v1.0 Gate Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This is Phase 0 of the master roadmap ([`2026-06-06-v1-completion-roadmap.md`](2026-06-06-v1-completion-roadmap.md)).

**Goal:** Turn the existing `vox-audit` `CrlGate`/`Subcommand` registry into the single v1.0 GA spine — tier-aware, foundation-first, with a `vox audit --gate all --strict-block-ga` roll-up that writes `contracts/reports/_snapshot/<UTC>.json`, includes the standalone product binaries, carries the first real foundation gate (CR-F1), and a self-policing CR-META criteria-format lint.

**Architecture:** Five tasks, each TDD. (0.1) `Tier` + `CrlGate::tier()` + foundation-first `all()`/`registry()` ordering. (0.2) wire the already-landed CR-F1 behavioral harness as the registered `behavioral-goldens` foundation gate (proves the "add a gate" pattern + gives CR-F0 real data). (0.3) `--strict-block-ga` GA roll-up with `blocked_by_foundation` semantics + `_snapshot` artifact, surfaced through both the `vox-audit` binary (`main.rs`) and `vox-cli` (`audit.rs`). (0.4) include the standalone `bin/cr-*.rs` product gates in the GA snapshot via a sibling-exe adapter. (0.5) CR-META lint in `vox-arch-check`, wired into the pre-push doc-pipeline.

**Tech Stack:** Rust — `crates/vox-audit` (`lib.rs`, `main.rs`, `subcommands/`, `report.rs`), `crates/vox-cli/src/commands/audit.rs`, `crates/vox-arch-check` (`main.rs` + new `criteria_format.rs`). `cargo test -p <crate>`. No new dependencies.

**No-stubs rule:** Per project policy we do **not** register empty CR-F0/F2–F6/K/U placeholder gates. Phase 0 adds exactly one real foundation gate (CR-F1, whose harness already exists) and the framework around it. Every later gate lands with a real impl in its own phase.

---

## File map

| File | Change |
|---|---|
| `crates/vox-audit/src/lib.rs` | Add `Tier` enum, `CrlGate::tier()`, `F1BehavioralGoldens` variant, reorder `all()`/`registry()`, add `run_ga_snapshot()` + `GaSnapshot`/`GateRow` types, register product-binary gates in the snapshot. Bump `registry_size_matches_gate_count`. |
| `crates/vox-audit/src/subcommands/behavioral_goldens.rs` | **Create** — the CR-F1 `Subcommand` (shells out to `vox run --mode interp` over `examples/golden/*.vox` with `// EXPECT:` lines). |
| `crates/vox-audit/src/subcommands/mod.rs` | `pub mod behavioral_goldens;` |
| `crates/vox-audit/src/ga.rs` | **Create** — `GaSnapshot`, `GateRow`, `product_binary_gates()`, snapshot-build + `blocked_by_foundation` logic + artifact write. |
| `crates/vox-audit/src/main.rs` | Teach `CliCommand::All` to honor a new `--strict-block-ga` flag → call `run_ga_snapshot`; add `BehavioralGoldens` clap variant + `to_gate_name` arm. |
| `crates/vox-cli/src/commands/audit.rs` | Add `--strict-block-ga` to `AuditArgs`; route `--gate all` → `run_ga_snapshot`. |
| `crates/vox-arch-check/src/criteria_format.rs` | **Create** — `check_criteria_format(doc) -> Result<(), Vec<String>>`. |
| `crates/vox-arch-check/src/main.rs` | Detect `--lint criteria-format`; run the lint, write artifact, exit 0/1. |
| `crates/vox-arch-check/tests/criteria_format.rs` | **Create** — lint unit tests. |
| `scripts/check.vox` or the pre-push doc-pipeline | Add the `--lint criteria-format` invocation. |

---

## Task 0.1: `Tier` + `CrlGate::tier()` + foundation-first ordering

**Files:**
- Modify: `crates/vox-audit/src/lib.rs`
- Test: `crates/vox-audit/src/lib.rs` `#[cfg(test)]`

- [ ] **Step 1 — Write the failing test.** Add to the `tests` module in `crates/vox-audit/src/lib.rs`:

```rust
#[test]
fn all_gates_are_ordered_foundation_first() {
    // Tier order is the enum's declaration order (derive(Ord)).
    let tiers: Vec<Tier> = CrlGate::all().map(|g| g.tier()).collect();
    let mut sorted = tiers.clone();
    sorted.sort();
    assert_eq!(
        tiers, sorted,
        "CrlGate::all() must yield gates in non-decreasing tier order \
         (foundation → distribution → gui → product → tooling); got {tiers:?}"
    );
}

#[test]
fn registry_order_matches_all_order() {
    let reg_gates: Vec<CrlGate> = registry().iter().map(|s| s.gate()).collect();
    let all_gates: Vec<CrlGate> = CrlGate::all().collect();
    assert_eq!(reg_gates, all_gates, "registry() and all() must agree on order");
}
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p vox-audit --lib -- all_gates_are_ordered_foundation_first registry_order_matches_all_order` → fails to compile (`Tier` / `tier()` undefined).

- [ ] **Step 3 — Implement.** In `crates/vox-audit/src/lib.rs`, above `CrlGate`:

```rust
/// Release-criteria tier. Declaration order IS the GA evaluation order:
/// foundation gates are evaluated and reported before any downstream gate
/// (see [`run_ga_snapshot`] + CR-F0 in v1-release-criteria.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Foundation,
    Distribution,
    Gui,
    Product,
    Tooling,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Foundation => "foundation",
            Tier::Distribution => "distribution",
            Tier::Gui => "gui",
            Tier::Product => "product",
            Tier::Tooling => "tooling",
        }
    }
}
```

Add a `tier()` method inside `impl CrlGate` (CR-L gates are §3.5 Product; the tooling gate is Tooling). The `F1BehavioralGoldens` arm is added in Task 0.2 — for now `tier()` covers the existing variants:

```rust
/// Which release-criteria tier this gate belongs to.
pub fn tier(self) -> Tier {
    match self {
        CrlGate::L0SpecToApp
        | CrlGate::L1HumanEval
        | CrlGate::L2MensOnDistribution
        | CrlGate::L3RepairCorpus
        | CrlGate::L4PlanFidelity
        | CrlGate::L5AciDefault
        | CrlGate::L6Retirement
        | CrlGate::L7Deploy
        | CrlGate::L8CorpusFeedback => Tier::Product,
        CrlGate::ToolingStdlibCoverage => Tier::Tooling,
    }
}
```

`all()` already yields the CR-L block then the tooling gate — that is already `Product…Tooling`, i.e. non-decreasing. So both tests pass once `Tier`/`tier()` exist. (Foundation enters in Task 0.2; the test then guards that it sorts first.)

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p vox-audit --lib -- all_gates_are_ordered_foundation_first registry_order_matches_all_order`.

- [ ] **Step 5 — Commit.**

```bash
git add crates/vox-audit/src/lib.rs
git commit -m "feat(vox-audit): add Tier + CrlGate::tier() + foundation-first ordering invariant (CR-F0 scaffold)"
```

---

## Task 0.2: Wire CR-F1 behavioral harness as the `behavioral-goldens` foundation gate

**Files:**
- Create: `crates/vox-audit/src/subcommands/behavioral_goldens.rs`
- Modify: `crates/vox-audit/src/subcommands/mod.rs`, `crates/vox-audit/src/lib.rs`, `crates/vox-audit/src/main.rs`
- Reference (logic to mirror): `crates/vox-integration-tests/tests/golden_behavioral_gate.rs`

- [ ] **Step 1 — Write the failing test.** Create `crates/vox-audit/src/subcommands/behavioral_goldens.rs` with only the test first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommonArgs;
    use crate::report::ExitCode;

    #[test]
    fn behavioral_goldens_gate_runs_over_examples() {
        // Requires a `vox` binary on PATH or $VOX_BIN. Skips gracefully (infra
        // error) when absent so the unit test is hermetic in minimal CI; the
        // real gate runs in the full pipeline where vox is built.
        let args = CommonArgs { write_canonical_report: false, ..CommonArgs::default() };
        let outcome = BehavioralGoldensSubcommand.run(&args);
        assert_eq!(outcome.report.thing, "behavioral-goldens");
        // Either it measured (Ok/BarMissed with a threshold) or vox was absent
        // (InfrastructureError). It must never panic and must emit a report.
        assert!(matches!(
            outcome.exit_code,
            ExitCode::Ok | ExitCode::BarMissed | ExitCode::InfrastructureError
        ));
    }

    #[test]
    fn parse_expect_extracts_lines() {
        let src = "// EXPECT: hello\nfn main() { print(\"hello\") }\n// EXPECT: world\n";
        assert_eq!(parse_expect(src), Some("hello\nworld".to_string()));
        assert_eq!(parse_expect("fn main() {}\n"), None);
    }
}
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p vox-audit --lib -- behavioral_goldens` → fails to compile (`BehavioralGoldensSubcommand`, `parse_expect` undefined).

- [ ] **Step 3 — Implement** the gate above the test module in the same file:

```rust
//! `vox audit --gate behavioral-goldens` — CR-F1 foundation gate.
//!
//! Runs every `examples/golden/*.vox` that carries `// EXPECT:` lines under
//! `vox run --mode interp` and asserts stdout matches the concatenated EXPECT
//! block. Mirrors the landed integration harness
//! `crates/vox-integration-tests/tests/golden_behavioral_gate.rs`; this is the
//! registered-gate form so `vox audit --gate all` and the GA snapshot see it.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};

pub struct BehavioralGoldensSubcommand;

/// Resolve the `vox` binary: `$VOX_BIN` override, else `vox` on PATH.
fn vox_bin() -> String {
    std::env::var("VOX_BIN").unwrap_or_else(|_| "vox".to_string())
}

/// Collect the `// EXPECT:` lines (in source order) joined by newlines.
/// Returns None when the golden declares no expectations.
pub(crate) fn parse_expect(src: &str) -> Option<String> {
    let mut lines = Vec::new();
    for raw in src.lines() {
        let t = raw.trim_start();
        if let Some(rest) = t.strip_prefix("// EXPECT:") {
            lines.push(rest.trim().to_string());
        }
    }
    if lines.is_empty() { None } else { Some(lines.join("\n")) }
}

impl Subcommand for BehavioralGoldensSubcommand {
    fn gate(&self) -> CrlGate {
        CrlGate::F1BehavioralGoldens
    }

    fn description(&self) -> &'static str {
        "CR-F1: behavioral goldens — `// EXPECT:` stdout matches `vox run --mode interp`."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let thing = CrlGate::F1BehavioralGoldens.thing_name();
        let golden_dir = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join("examples").join("golden"));

        let entries = match std::fs::read_dir(&golden_dir) {
            Ok(e) => e,
            Err(io) => {
                return RunOutcome {
                    report: AuditReport::infra_error(
                        thing,
                        format!("cannot read golden dir {}: {io}", golden_dir.display()),
                    ),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };

        let mut total = 0u32;
        let mut passed = 0u32;
        let mut failures: Vec<String> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("vox") {
                continue;
            }
            let src = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                // Read failures are a real problem, not a skip (matches the
                // CodeRabbit fix on the integration harness).
                Err(io) => {
                    return RunOutcome {
                        report: AuditReport::infra_error(
                            thing,
                            format!("cannot read {}: {io}", path.display()),
                        ),
                        exit_code: ExitCode::InfrastructureError,
                    };
                }
            };
            let Some(expected) = parse_expect(&src) else { continue };
            total += 1;

            let out = std::process::Command::new(vox_bin())
                .args(["run", "--mode", "interp"])
                .arg(&path)
                .output();
            match out {
                Ok(o) => {
                    let got = String::from_utf8_lossy(&o.stdout).trim_end().to_string();
                    if got == expected.trim_end() {
                        passed += 1;
                    } else {
                        failures.push(format!(
                            "{}: expected {:?}, got {:?}",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            expected,
                            got
                        ));
                    }
                }
                Err(io) => {
                    // vox binary absent / not executable → infra error, not a
                    // measurement failure.
                    return RunOutcome {
                        report: AuditReport::infra_error(
                            thing,
                            format!("failed to exec `{}`: {io}", vox_bin()),
                        ),
                        exit_code: ExitCode::InfrastructureError,
                    };
                }
            }
        }

        let pass_rate = if total == 0 { 0.0 } else { passed as f64 / total as f64 };
        let met = total > 0 && passed == total;
        let mut report = AuditReport::complete(
            thing,
            format!("count:{total}"),
            total,
            Results { overall_pass_rate: pass_rate, median_pass_rate: None, per_llm: Vec::new() },
        );
        report.threshold = Some(Threshold { target: args.threshold.unwrap_or(1.0), met });
        if !met {
            report.note = Some(if total == 0 {
                "no goldens with `// EXPECT:` lines found".to_string()
            } else {
                format!("{}/{} behavioral goldens diverged: {}", total - passed, total, failures.join("; "))
            });
        }
        RunOutcome {
            report,
            exit_code: if met { ExitCode::Ok } else { ExitCode::BarMissed },
        }
    }
}
```

> **Note (landed code):** the simple `Command::output()` above is the design
> sketch. The shipped gate hardens the per-golden run with a wall-clock timeout
> (`VOX_GOLDEN_TIMEOUT_SECS`, default 30s), drains stdout on a helper thread to
> avoid pipe-fill deadlock, and maps outcomes through a `GoldenRun { Done,
> TimedOut, SpawnErr }` enum — so a hanging/broken `vox` can't hang the gate. A
> timeout is a behavioral failure (not an infra error). See the committed
> `crates/vox-audit/src/subcommands/behavioral_goldens.rs`.

- [ ] **Step 4 — Register the variant.** In `crates/vox-audit/src/lib.rs`:
  - Add `F1BehavioralGoldens,` as the **first** variant of `CrlGate`.
  - `thing_name`: add `CrlGate::F1BehavioralGoldens => "behavioral-goldens",`.
  - `tier`: add `CrlGate::F1BehavioralGoldens => Tier::Foundation,` (as the first match arm).
  - `block_ga`: add `CrlGate::F1BehavioralGoldens` to the `matches!` set (foundation gates block GA).
  - `all()`: put `CrlGate::F1BehavioralGoldens,` **first** in the array.
  - `registry()`: put `Box::new(subcommands::behavioral_goldens::BehavioralGoldensSubcommand),` **first** in the vec.
  - Bump `registry_size_matches_gate_count` from `10` to `11` (and its message).
  - In `subcommands/mod.rs` add `pub mod behavioral_goldens;`.

- [ ] **Step 5 — Wire the CLI** in `crates/vox-audit/src/main.rs`: add `BehavioralGoldens` to `CliCommand` (with a doc line `/// CR-F1: behavioral goldens.`) and `CliCommand::BehavioralGoldens => Some("behavioral-goldens"),` to `to_gate_name`.

- [ ] **Step 6 — Run, expect PASS.** `cargo test -p vox-audit` (the new gate tests + the existing registry round-trip tests in lib.rs — `every_gate_has_a_subcommand_in_registry`, `registry_size_matches_gate_count`, `all_gates_are_ordered_foundation_first` — all green). Then smoke it: `VOX_BIN=$(which vox) cargo run -p vox-audit -- behavioral-goldens --no-canonical-report`.

- [ ] **Step 7 — Commit.**

```bash
git add crates/vox-audit/src
git commit -m "feat(vox-audit): register CR-F1 behavioral-goldens as first foundation gate"
```

---

## Task 0.3: `--strict-block-ga` GA roll-up + `_snapshot` artifact

**Files:**
- Create: `crates/vox-audit/src/ga.rs`
- Modify: `crates/vox-audit/src/lib.rs` (`pub mod ga;`), `crates/vox-audit/src/main.rs`, `crates/vox-cli/src/commands/audit.rs`

- [ ] **Step 1 — Write the failing test.** Create `crates/vox-audit/src/ga.rs` with the test first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn row(thing: &str, tier: crate::Tier, met: bool) -> GateRow {
        GateRow {
            thing: thing.into(),
            tier: tier.as_str().into(),
            met,
            blocked_by_foundation: false,
            exit_code: if met { 0 } else { 1 },
            external_infra: false,
        }
    }

    #[test]
    fn red_foundation_blocks_all_downstream() {
        let rows = vec![
            row("behavioral-goldens", crate::Tier::Foundation, false),
            row("retirement", crate::Tier::Product, true),
        ];
        let snap = GaSnapshot::from_rows(rows, /* strict */ true);
        assert!(snap.foundation_red);
        let downstream = snap.gates.iter().find(|g| g.thing == "retirement").unwrap();
        assert!(downstream.blocked_by_foundation, "product row must be blocked when foundation is red");
        assert!(!snap.ga_met);
        assert_ne!(snap.exit_code, 0);
    }

    #[test]
    fn all_green_passes_ga() {
        let rows = vec![
            row("behavioral-goldens", crate::Tier::Foundation, true),
            row("retirement", crate::Tier::Product, true),
        ];
        let snap = GaSnapshot::from_rows(rows, true);
        assert!(!snap.foundation_red);
        assert!(snap.ga_met);
        assert_eq!(snap.exit_code, 0);
    }

    #[test]
    fn external_infra_red_does_not_block_when_not_strict() {
        // A built-but-unrun external_infra gate is honest-red; non-strict GA
        // reports it but exits 0.
        let mut r = row("cr-p2", crate::Tier::Product, false);
        r.external_infra = true;
        let snap = GaSnapshot::from_rows(vec![r], /* strict */ false);
        assert!(!snap.ga_met);
        assert_eq!(snap.exit_code, 0, "non-strict run never fails the build");
    }
}
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p vox-audit --lib -- ga::` → undefined `GaSnapshot`/`GateRow`.

- [ ] **Step 3 — Implement** in `crates/vox-audit/src/ga.rs`:

```rust
//! GA roll-up — `vox audit --gate all --strict-block-ga`.
//!
//! Runs every registered gate (foundation first per CR-F0), folds in the
//! standalone product binaries (Task 0.4), and writes
//! `contracts/reports/_snapshot/<UTC>.json`. If any foundation gate is red,
//! every downstream gate is forced `blocked_by_foundation` and GA fails.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GateRow {
    pub thing: String,
    pub tier: String,
    pub met: bool,
    pub blocked_by_foundation: bool,
    pub exit_code: i32,
    #[serde(default)]
    pub external_infra: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GaSnapshot {
    pub schema_version: u32,
    pub measured_at: String,
    pub strict_block_ga: bool,
    pub foundation_red: bool,
    pub ga_met: bool,
    pub exit_code: i32,
    pub gates: Vec<GateRow>,
}

impl GaSnapshot {
    /// Build the snapshot from already-evaluated rows, applying CR-F0
    /// foundation-blocking and computing the GA verdict + exit code.
    pub fn from_rows(mut gates: Vec<GateRow>, strict_block_ga: bool) -> Self {
        let foundation_red = gates.iter().any(|g| g.tier == "foundation" && !g.met);
        if foundation_red {
            for g in gates.iter_mut() {
                if g.tier != "foundation" {
                    g.blocked_by_foundation = true;
                }
            }
        }
        // GA is met when every non-tooling gate is met (external_infra gates
        // included — their honest-red state must be cleared for GA) and no
        // foundation gate is red.
        let ga_met = !foundation_red
            && gates
                .iter()
                .filter(|g| g.tier != "tooling")
                .all(|g| g.met);
        let exit_code = if strict_block_ga && !ga_met { 1 } else { 0 };
        Self {
            schema_version: 1,
            measured_at: now_rfc3339(),
            strict_block_ga,
            foundation_red,
            ga_met,
            exit_code,
            gates,
        }
    }

    /// Write to `contracts/reports/_snapshot/<YYYY-MM-DD>.json` under the root.
    pub fn write_canonical(&self, root: &std::path::Path) -> std::io::Result<()> {
        let dir = root.join("contracts").join("reports").join("_snapshot");
        std::fs::create_dir_all(&dir)?;
        let date = today_yyyymmdd();
        let body = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(dir.join(format!("{date}.json")), body)
    }
}

fn now_rfc3339() -> String { chrono::Utc::now().to_rfc3339() }
fn today_yyyymmdd() -> String { chrono::Utc::now().format("%Y-%m-%d").to_string() }
```

- [ ] **Step 4 — Add the runner** `run_ga_snapshot` to `crates/vox-audit/src/lib.rs` (public): run `registry()` gates → map each `RunOutcome` to a `GateRow` (`met = outcome.exit_code == ExitCode::Ok && !outcome.report.incomplete`); append the product-binary rows from Task 0.4 (`ga::product_binary_gates()` — stub it to return `vec![]` in this task so the code compiles, fill in 0.4); build `GaSnapshot::from_rows`; write canonical; return `GaSnapshot`. Add `pub mod ga;`.

```rust
pub fn run_ga_snapshot(args: &CommonArgs, strict_block_ga: bool) -> ga::GaSnapshot {
    use report::ExitCode;
    let mut rows: Vec<ga::GateRow> = registry()
        .into_iter()
        .map(|sub| {
            let g = sub.gate();
            let outcome = sub.run(args);
            ga::GateRow {
                thing: g.thing_name().to_string(),
                tier: g.tier().as_str().to_string(),
                met: outcome.exit_code == ExitCode::Ok && !outcome.report.incomplete,
                blocked_by_foundation: false,
                exit_code: outcome.exit_code.as_i32(),
                external_infra: false,
            }
        })
        .collect();
    rows.extend(ga::product_binary_gates(args)); // 0.4 fills this in
    let snap = ga::GaSnapshot::from_rows(rows, strict_block_ga);
    if args.write_canonical_report {
        let _ = snap.write_canonical(&workspace_root());
    }
    snap
}
```

- [ ] **Step 5 — Surface in the `vox-audit` binary** (`main.rs`): add a top-level `#[arg(long, global = true)] strict_block_ga: bool` to `Cli`; in the `CliCommand::All` arm, when `cli.strict_block_ga` is set, call `run_ga_snapshot(&common, true)`, print its JSON, and `return ProcessExitCode::from(snap.exit_code as u8)`. (Leave the existing non-strict `All` behavior — render every report — intact for `cli.strict_block_ga == false`, or route both through the snapshot and only gate the exit code on `strict_block_ga`.)

- [ ] **Step 6 — Surface in `vox-cli`** (`crates/vox-cli/src/commands/audit.rs`): add `#[arg(long)] pub strict_block_ga: bool` to `AuditArgs`; in `run()`, before the single-gate branch, handle `if args.gate.as_deref() == Some("all") { let snap = vox_audit::run_ga_snapshot(&common, args.strict_block_ga); println!("{}", serde_json::to_string_pretty(&snap)?); std::process::exit(snap.exit_code); }`. This makes the criteria doc's `vox audit --gate all --strict-block-ga` real.

- [ ] **Step 7 — Run, expect PASS.** `cargo test -p vox-audit` then smoke: `VOX_BIN=$(which vox) cargo run -p vox-audit -- all --strict-block-ga --no-canonical-report` (expect non-zero today: downstream CR-L gates are honest-red until later phases — that is correct CR-F0 behavior).

- [ ] **Step 8 — Commit.**

```bash
git add crates/vox-audit/src crates/vox-cli/src/commands/audit.rs
git commit -m "feat(vox-audit): --gate all --strict-block-ga GA roll-up with foundation-blocking _snapshot (CR-F0)"
```

---

## Task 0.4: Include the standalone product binaries in the GA snapshot

The CR-A/D/E/P checks are standalone `bin/cr-*.rs` exes (e.g. `bin/cr-a1.rs`), **outside** `registry()`. GA must see them. Full conversion to `Subcommand`s is Phase 5; here we surface their pass/fail by running the sibling executables built into the same target dir.

**Files:**
- Modify: `crates/vox-audit/src/ga.rs`

- [ ] **Step 1 — Write the failing test** in `ga.rs`:

```rust
#[test]
fn product_binary_descriptors_cover_existing_bins() {
    // The descriptor list must match the bins declared in Cargo.toml.
    let names: Vec<&str> = product_binary_descriptors().iter().map(|d| d.bin).collect();
    for expected in ["cr-a1", "cr-a2", "cr-a4", "cr-d3", "cr-e1", "cr-e2", "cr-p1", "cr-p2"] {
        assert!(names.contains(&expected), "missing descriptor for {expected}");
    }
    // P-gates are external_infra; A/E/D are not.
    let p1 = product_binary_descriptors().into_iter().find(|d| d.bin == "cr-p1").unwrap();
    assert!(p1.external_infra);
    let a1 = product_binary_descriptors().into_iter().find(|d| d.bin == "cr-a1").unwrap();
    assert!(!a1.external_infra);
}
```

- [ ] **Step 2 — Run, expect FAIL** (`product_binary_descriptors` undefined).

- [ ] **Step 3 — Implement** in `ga.rs`:

```rust
pub struct ProductBin {
    pub bin: &'static str,
    pub thing: &'static str,
    pub external_infra: bool,
}

/// Descriptors for the standalone `bin/cr-*.rs` product gates. CR-P* are
/// external_infra (live deploy / soak). cr-p3/cr-e3 are not yet bins — they
/// land in Phase 6.
pub fn product_binary_descriptors() -> Vec<ProductBin> {
    vec![
        ProductBin { bin: "cr-a1", thing: "cr-a1", external_infra: false },
        ProductBin { bin: "cr-a2", thing: "cr-a2", external_infra: false },
        ProductBin { bin: "cr-a4", thing: "cr-a4", external_infra: false },
        ProductBin { bin: "cr-d3", thing: "cr-d3", external_infra: false },
        ProductBin { bin: "cr-e1", thing: "cr-e1", external_infra: false },
        ProductBin { bin: "cr-e2", thing: "cr-e2", external_infra: false },
        ProductBin { bin: "cr-p1", thing: "cr-p1", external_infra: true },
        ProductBin { bin: "cr-p2", thing: "cr-p2", external_infra: true },
    ]
}

/// Run each product binary by resolving its sibling path next to the current
/// executable; record a GateRow. A missing binary is recorded as a non-met
/// row with exit_code -1 (honest: "not measured") rather than a panic.
pub fn product_binary_gates(_args: &crate::CommonArgs) -> Vec<GateRow> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    product_binary_descriptors()
        .into_iter()
        .map(|d| {
            let (met, code) = match &exe_dir {
                Some(dir) => {
                    let mut path = dir.join(d.bin);
                    if cfg!(windows) { path.set_extension("exe"); }
                    if path.exists() {
                        match std::process::Command::new(&path).output() {
                            Ok(o) => (o.status.success(), o.status.code().unwrap_or(-1)),
                            Err(_) => (false, -1),
                        }
                    } else {
                        (false, -1)
                    }
                }
                None => (false, -1),
            };
            GateRow {
                thing: d.thing.to_string(),
                tier: "product".to_string(),
                met,
                blocked_by_foundation: false,
                exit_code: code,
                external_infra: d.external_infra,
            }
        })
        .collect()
}
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p vox-audit --lib -- ga::`.

- [ ] **Step 5 — Commit.**

```bash
git add crates/vox-audit/src/ga.rs
git commit -m "feat(vox-audit): surface standalone CR-A/D/E/P binaries in the GA snapshot"
```

---

## Task 0.5: CR-META criteria-format lint in `vox-arch-check`

**Files:**
- Create: `crates/vox-arch-check/src/criteria_format.rs`, `crates/vox-arch-check/tests/criteria_format.rs`
- Modify: `crates/vox-arch-check/src/main.rs`
- Modify: the pre-push doc-pipeline invocation (e.g. `scripts/check.vox` or the relevant `vox ci` step)

- [ ] **Step 1 — Write the failing test.** Create `crates/vox-arch-check/tests/criteria_format.rs`:

```rust
use vox_arch_check::criteria_format::check_criteria_format;

#[test]
fn flags_block_missing_if_failing() {
    let doc = "\
**[CR-X] Some criterion.** Foo.
- `verify_cmd`: `cargo run -p vox-audit -- foo`
- `artifact_path`: `contracts/reports/foo/<UTC>.json`
";
    let errs = check_criteria_format(doc).unwrap_err();
    assert!(errs.iter().any(|e| e.contains("CR-X") && e.contains("if_failing")),
        "expected a CR-X / if_failing error; got {errs:?}");
}

#[test]
fn well_formed_block_passes() {
    let doc = "\
**[CR-Y] Good.** Bar.
- `verify_cmd`: `cargo run -p vox-audit -- bar`
- `artifact_path`: `contracts/reports/bar/<UTC>.json`
- `if_failing`: do the thing.
";
    assert!(check_criteria_format(doc).is_ok(), "{:?}", check_criteria_format(doc));
}

#[test]
fn real_criteria_doc_is_well_formed() {
    let root = env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(root)
        .join("../../docs/src/architecture/v1-release-criteria.md");
    let doc = std::fs::read_to_string(&path).expect("read criteria doc");
    let res = check_criteria_format(&doc);
    assert!(res.is_ok(), "live criteria doc must self-pass CR-META: {res:?}");
}
```

- [ ] **Step 2 — Run, expect FAIL.** `cargo test -p vox-arch-check --test criteria_format` → `criteria_format` module not found. (Note: `vox-arch-check` must expose `pub mod criteria_format;` from a `lib.rs`. If the crate is binary-only today, add a minimal `src/lib.rs` with `pub mod criteria_format;` and keep `main.rs` using it via the crate name.)

- [ ] **Step 3 — Implement** `crates/vox-arch-check/src/criteria_format.rs`:

```rust
//! CR-META: every `[CR-*]` block in v1-release-criteria.md must declare
//! `verify_cmd`, `artifact_path`, and a non-empty `if_failing`. A block is the
//! text from one `[CR-...]` marker up to (but not including) the next, or EOF.

/// Returns `Ok(())` when every criterion block is well-formed, else the list
/// of human-readable violations.
pub fn check_criteria_format(doc: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let blocks = split_blocks(doc);
    if blocks.is_empty() {
        return Err(vec!["no [CR-*] criterion blocks found".to_string()]);
    }
    for (id, body) in blocks {
        for field in ["verify_cmd", "artifact_path", "if_failing"] {
            if !field_present(&body, field) {
                errors.push(format!("[{id}] missing `{field}`"));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Field is present if the body contains a backticked `field` followed by a
/// `:` and at least one non-whitespace char after it on that segment.
fn field_present(body: &str, field: &str) -> bool {
    let needle = format!("`{field}`");
    let Some(pos) = body.find(&needle) else { return false };
    let after = &body[pos + needle.len()..];
    // Skip an optional `:` / `·` separator and surrounding spaces, then require
    // a non-empty, non-newline payload (the value).
    let trimmed = after.trim_start_matches([':', '·', ' ', '*']);
    trimmed
        .lines()
        .next()
        .map(|l| !l.trim().is_empty())
        .unwrap_or(false)
}

/// Split into `(id, body)` pairs keyed on `[CR-<id>]` markers.
fn split_blocks(doc: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = doc.as_bytes();
    let mut search_from = 0usize;
    let mut markers: Vec<(usize, String)> = Vec::new();
    while let Some(rel) = doc[search_from..].find("[CR-") {
        let start = search_from + rel;
        // Parse the id up to the closing ']'.
        if let Some(end_rel) = doc[start..].find(']') {
            let id = doc[start + 1..start + end_rel].to_string(); // e.g. "CR-F2"
            markers.push((start, id));
            search_from = start + end_rel + 1;
        } else {
            break;
        }
        let _ = bytes; // keep byte access available if refined later
    }
    for i in 0..markers.len() {
        let (start, ref id) = markers[i];
        let end = markers.get(i + 1).map(|(s, _)| *s).unwrap_or(doc.len());
        out.push((id.clone(), doc[start..end].to_string()));
    }
    out
}
```

- [ ] **Step 4 — Run, expect PASS.** `cargo test -p vox-arch-check --test criteria_format`. If `real_criteria_doc_is_well_formed` fails, fix the offending block in `v1-release-criteria.md` (that is the lint doing its job).

- [ ] **Step 5 — Wire the CLI** in `crates/vox-arch-check/src/main.rs`. Near the top of `main()`, before the normal arch-check `run`:

```rust
let argv: Vec<String> = std::env::args().collect();
if let Some(i) = argv.iter().position(|a| a == "--lint") {
    if argv.get(i + 1).map(String::as_str) == Some("criteria-format") {
        return run_criteria_format_lint();
    }
}
```

and add:

```rust
fn run_criteria_format_lint() -> ExitCode {
    let root = vox_audit_workspace_root(); // reuse the existing workspace-root locator
    let path = root.join("docs/src/architecture/v1-release-criteria.md");
    let doc = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(e) => { eprintln!("criteria-format: cannot read {}: {e}", path.display()); return ExitCode::from(2); }
    };
    let (met, errors) = match crate::criteria_format::check_criteria_format(&doc) {
        Ok(()) => (true, Vec::new()),
        Err(errs) => (false, errs),
    };
    // Artifact (best-effort) — mirrors bin/cr-a1.rs shape.
    let out_dir = root.join("contracts/reports/arch/criteria-format");
    let _ = std::fs::create_dir_all(&out_dir);
    let body = serde_json::json!({
        "schema_version": 1, "criterion": "CR-META",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "errors": errors, "threshold": { "target": "all blocks well-formed", "met": met },
    });
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let _ = std::fs::write(out_dir.join(format!("{date}.json")), serde_json::to_string_pretty(&body).unwrap_or_default());
    if met { ExitCode::SUCCESS } else {
        for e in &errors { eprintln!("CR-META: {e}"); }
        ExitCode::from(1)
    }
}
```

(Resolve the workspace root with whatever locator `main.rs` already uses — it computes `workspace_root` for its rules; reuse that value rather than introducing a new helper.)

- [ ] **Step 6 — Run, expect PASS.** `cargo run -p vox-arch-check -- --lint criteria-format` → exit 0; artifact at `contracts/reports/arch/criteria-format/<date>.json`.

- [ ] **Step 7 — Wire into the doc-pipeline.** Add `cargo run -p vox-arch-check -- --lint criteria-format` to the pre-push doc-pipeline step (the same gate that enforces frontmatter). Confirm the criteria doc self-polices on push.

- [ ] **Step 8 — Commit.**

```bash
git add crates/vox-arch-check/src crates/vox-arch-check/tests scripts
git commit -m "feat(vox-arch-check): CR-META criteria-format lint (--lint criteria-format) + doc-pipeline wiring"
```

---

## Phase-0 exit criteria (verify before declaring done)

- [ ] `cargo test -p vox-audit` green (new gate + ga tests + existing registry round-trips, size bumped to 11).
- [ ] `cargo test -p vox-arch-check` green (criteria-format unit + live-doc tests).
- [ ] `VOX_BIN=$(which vox) cargo run -p vox-audit -- all --strict-block-ga --no-canonical-report` runs, foundation row (`behavioral-goldens`) first, downstream CR-L rows marked `blocked_by_foundation` while honest-red, exits non-zero (correct — later phases turn it green).
- [ ] `vox audit --gate all --strict-block-ga` works through `vox-cli`.
- [ ] `cargo run -p vox-arch-check -- --lint criteria-format` exits 0; artifact written.
- [ ] `vox run scripts/fmt.vox` clean (or `cargo fmt -p vox-audit -p vox-arch-check`); `cargo run -p vox-arch-check` (normal mode) still green.

## Self-review notes

- **No new stubs:** the only gate added is CR-F1, whose harness already exists; CR-F0–F6/K/U are NOT registered as empty placeholders. The product-binary fold reuses existing real binaries.
- **Type consistency:** `GateRow`/`GaSnapshot` field names are used identically across `ga.rs`, the `run_ga_snapshot` mapper, and the tests. `CrlGate::F1BehavioralGoldens` is referenced identically in `lib.rs`, `behavioral_goldens.rs`, and `main.rs`.
- **Determinism:** the GA roll-up logic (`from_rows`) is tested on injected rows, never the live repo. The behavioral gate degrades to `InfrastructureError` (not panic / not false-fail) when `vox` is absent.
- **Honest red:** a built-but-unrun `external_infra` gate reports `met:false` and is shown in the snapshot; only `--strict-block-ga` turns that into a non-zero exit, matching the criteria doc's GA acceptance.
