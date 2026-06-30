# Extracting the vox-cli CI Subsystem via `vox-cli-contracts`

> Implementation spec for the headline Tier-3 build-time win: moving the ~45%-of-vox-cli
> CI subsystem into `vox-cli-ci`, gated on a new `vox-cli-contracts` trait seam.
> Produced by a 4-agent design workflow (2026-06-30), verified against source.

## 0. Ground-truth correction (verified against source)

The "external subsystems" ci couples to — audit / policy / runtime / scientia_ledger_contract —
are **not** separate crates. All live **inside `vox-cli`** at
`crates/vox-cli/src/commands/{audit.rs, policy/status_writer.rs, runtime/shell/check_terminal.rs, scientia_ledger_contract.rs}`.
This is decisive:

- The coupling is `ci (vox-cli module) → sibling vox-cli module`. To extract ci into `vox-cli-ci`,
  the seam must **invert** these so vox-cli implements traits that vox-cli-ci consumes
  (dependency points `vox-cli → vox-cli-contracts ← vox-cli-ci`).
- `vox-cli-ci` already depends only on `anyhow/glob/regex/serde/serde_yaml/walkdir/vox-cli-core` —
  no tokio. `vox-cli-contracts` must keep that floor.
- `scientia_ledger_contract.rs` depends only on `vox-bounded-fs + serde_json` → it can **move
  outright** into ci rather than needing a trait.

## 1. `vox-cli-contracts` — the new crate

`crates/vox-cli-contracts/Cargo.toml` (mirrors vox-cli-ci's floor — **no tokio, no vox-cli**):
```toml
[dependencies]
anyhow.workspace = true
serde.workspace = true
serde_yaml.workspace = true
vox-config.workspace = true   # PolicyEntry/PolicyResult/RunStatus already here
```

Three traits + two moved types. Signatures verbatim from the call sites:

### `CheckProvider` (audit seam — `policy_registry.rs:89`)
```rust
pub trait CheckProvider {
    fn load_check_targets(&self, repo_root: &Path) -> anyhow::Result<Vec<vox_config::PolicyEntry>>;
}
```
`CheckManifest`/`CheckItem` serde structs (pure serde over `contracts/ci/check-targets.v1.yaml`,
audit.rs:59) **move into vox-cli-contracts**; vox-cli re-exports them for back-compat.

### `GateStatusWriter` (policy seam — `run_body.rs:754-763`)
```rust
pub trait GateStatusWriter {
    fn current_branch(&self, repo_root: &Path) -> String;
    fn head_commit(&self, repo_root: &Path) -> String;
    fn write_results(&self, repo_root: &Path, branch: &str, commit: &str,
                     ran_at: &str, results: Vec<vox_config::PolicyResult>) -> anyhow::Result<()>;
}
```
`ran_at` stays caller-supplied (the single non-deterministic seam — preserve that).
`gate_status_result` (pure `ok→RunStatus` mapper + 3 unit tests at run_body.rs:776-870) moves
**into vox-cli-ci** unchanged.

### `TerminalPolicyValidator` (runtime seam — `exec_policy_contract.rs`)
```rust
pub trait TerminalPolicyValidator {
    fn default_policy_rel(&self) -> &'static str;       // was DEFAULT_POLICY_REL const
    fn validate_policy_file(&self, repo_root: &Path, policy: &Path) -> anyhow::Result<()>;
    fn run_check_for_ci(&self, payload: &str, policy: Option<&Path>) -> anyhow::Result<()>;
}
```
The `DiskCorpus`/`SMOKE_PAYLOADS`/`REJECT_PAYLOADS` orchestration (exec_policy_contract.rs:19-110)
**moves into vox-cli-ci** and calls the trait; only the `check_terminal` primitives stay in vox-cli.

### No trait for scientia_ledger — **move the modules (corrected placement)**
- `scientia_novelty_ledger_contract.rs` (ci-only; dispatched via `CiCmd::ScientiaNoveltyLedgerContracts`
  in run_body.rs:120) → moves to **vox-cli-ci**.
- **CORRECTION (verified 2026-06-30):** `scientia_ledger_contract.rs` is NOT ci-only — it is also
  called by the non-ci `vox scientia` command (`commands/scientia.rs:21,31`). Moving it to vox-cli-ci
  would invert the dep (vox-cli's scientia.rs → vox-cli-ci). It must go to **vox-cli-contracts**
  instead (deps `vox-bounded-fs` + `vox-jsonschema-util` + `serde_json`, low-floor sync), consumed by
  both vox-cli and vox-cli-ci. Still a plain module move, just to contracts not ci.

> **Status (2026-06-30): PR-1, PR-2, PR-3 LANDED on main** (`4474e04d25`, `1dd265f647`). The crate,
> the 3 traits, `CheckManifest`/`CheckEntry`, `VoxCliProviders`, and the CheckProvider +
> GateStatusWriter in-place wirings are done and tests-green. PR-4 (cmd_enums + dispatcher +
> ~38-guard + scientia move) is the remaining piece — confirmed un-chunkable (run_body's `CiCmd`
> match is the coupling hub), needs a dedicated effort against the clap goldens + freshness gate.

### What does NOT get a trait (the analysis over-counted)
- `scientia_heuristics_parity` / `scientia_worthiness_contract` call `vox_publisher::*` (a real
  external crate) — vox-cli-ci adds `vox-publisher` directly; no seam.
- `code_audit_entries` is `#[cfg(feature="completion-toestub")]` over `vox-code-audit` (clean crate)
  — vox-cli-ci adds the optional dep + feature passthrough; no trait.

**Net contracts surface: 3 traits + `CheckManifest`/`CheckItem`.** Tiny, sync, zero tokio.

## 2. Decoupling steps (verify-in-place before moving)

1. **PR-1: create `vox-cli-contracts`** with the 3 traits + moved `CheckManifest`/`CheckItem`.
   vox-cli adds it as a dep and re-exports `CheckManifest` from `commands/audit.rs`
   (`pub use vox_cli_contracts::CheckManifest;`) so existing serde paths are untouched.
2. **PR-2: vox-cli implements the 3 traits** on a zero-sized `struct VoxCliProviders;` in a new
   `crates/vox-cli/src/commands/ci/providers.rs`, forwarding to the existing
   audit / policy::status_writer / runtime::shell::check_terminal logic.
3. **PR-3: rewire ci call sites to the traits**, still in-place inside vox-cli:
   `policy_registry.rs::audit_check_entries` → `&dyn CheckProvider`; `run_body.rs:754-763` →
   `providers.current_branch(...)`; `exec_policy_contract.rs` → `&dyn TerminalPolicyValidator`;
   thread `VoxCliProviders` from the top dispatcher.
   **Gate: `cargo build -p vox-cli` + `cargo test -p vox-cli` green with ci still in-place** —
   proves the seam holds before any code physically moves.

## 3. The CI move (PR-4, the big one)

**Already in vox-cli-ci**: 22 self-contained guards, dispatched via `vox_cli_ci::module::run()`.

**Moves now:**
- `cmd_enums.rs` (1119 lines: `CiCmd` 100+ variants + 8 nested enums + `gate_policy_id()`) →
  `vox-cli-ci/src/cmd_enums.rs`, clap derives intact (vox-cli-ci adds `clap`). No functional change.
- The dispatcher `run(cmd: CiCmd)` → `vox-cli-ci/src/run.rs` as
  `pub async fn run(cmd, providers) -> Result<()>` (vox-cli-ci gains tokio). `gate_status_result`
  + scientia modules + exec_policy orchestration move with it.

**Two-tier split — do NOT move all 78 guards:**
- **Tier 1 — MOVE** (~60 guards): the 22 already there + light ones needing only clean crate deps
  (`vox-publisher`, `vox-code-audit`, `vox-doc-inventory`, `vox-scaling-policy`,
  `vox-grammar-export`, `vox-bounded-fs`). Add these ~12 deps to vox-cli-ci (all already vox-cli deps).
- **Tier 2 — STAY** in `vox-cli/src/commands/ci/` (~15 heavy guards: `build_timings, pre_push,
  gui_* ×4, config_* ×4, docs_reality_audit, completion_quality, grammar_ssot_parity,
  pipeline_parity, eval_matrix, mens_scorecard, coverage_gates, speech_runtime_suite,
  scaling_audit`). They reach deep into vox-cli internals (`crate::frontend::pnpm_executable`,
  GUI structs, orchestrator); pulling them would force a circular dep. Expose them to the moved
  dispatcher via a **`&str`-keyed callback** (keeps vox-cli-contracts free of clap/CiCmd):
  ```rust
  pub trait HeavyGuardHost {
      fn dispatch_heavy(&self, policy_id: &str, root: &Path) -> Option<anyhow::Result<()>>;
  }
  ```
  vox-cli implements it; the moved `run.rs` calls it as a fallback arm.

**vox-cli shell** (`lib.rs:566-569`): `Ci { cmd: vox_cli_ci::CiCmd }`; dispatch
`vox_cli_ci::run(cmd, &VoxCliProviders).await` (which also impls `HeavyGuardHost`).

## 4. Risks

- **Binary-freshness gate** (`ci.yml`, `ci-health-watchdog.yml`): infra `runner-*` commands are
  exempt (commit `265532e3`). Moving guards changes which crate emits which `vox ci` binary —
  land PR-4 with an explicit freshness re-baseline commit; confirm the gate keys off the `vox`
  binary, not per-crate.
- **Feature unification**: `completion-toestub` (`vox-code-audit`) + publisher features must be
  declared on vox-cli-ci AND forwarded from vox-cli, else unification silently drops
  `code_audit_entries`. Add a `vox-cli-ci/tests/feature_passthrough` assertion.
- **cmd_enums clap golden** (`tests/ci_workflow_contract.rs`, `tests/vox_cli_root_parsing.rs`):
  snapshot `vox ci --help` / parse tree. Move `CiCmd` **byte-identical** — do not tidy variant
  order. Run both as the PR-4 gate.
- **Circular dep**: direction is `vox-cli → {contracts, cli-ci}` and `cli-ci → contracts`. Never
  let contracts depend on vox-cli/vox-cli-ci. The `&str`-keyed `HeavyGuardHost` is what keeps
  Tier-2 guards in vox-cli without cli-ci importing vox-cli.
- **Async leak**: vox-cli-ci gains tokio at PR-4. Contracts stays sync — verify
  `cargo tree -p vox-cli-contracts -i tokio` is empty.

## 5. Effort + PR breakdown + payoff

| PR | Scope | Effort | Gate |
|----|-------|--------|------|
| PR-1 | Create vox-cli-contracts: 3 traits + move CheckManifest/CheckItem | 0.5d | builds; vox-cli re-export compiles |
| PR-2 | `VoxCliProviders` impls in `ci/providers.rs` | 0.5d | impls compile |
| PR-3 | Rewire ci call sites to `&dyn` traits, **ci still in-place** | 1d | `cargo test -p vox-cli` green |
| PR-4 | Move cmd_enums+dispatcher+~60 guards+scientia; `HeavyGuardHost` for 15 Tier-2; shell delegates | 3–4d | clap goldens + freshness re-baseline + feature passthrough |

**Total ~5–6 days.** PRs 1–3 are mechanically safe (no behavior change, verifiable in-place);
PR-4 is the headline split.

**Build-time payoff:** vox-cli-ci absorbs ~45% of vox-cli's compile mass. Once CI is in vox-cli-ci,
editing a guard recompiles only vox-cli-ci + the thin vox-cli shell instead of all of vox-cli —
the incremental-rebuild win for the common "tweak one CI guard" loop is the dominant gain, plus
parallel compilation. The no-tokio `vox-cli-contracts` is the keystone that unlocks it.

## Load-bearing files for the implementer
- `crates/vox-cli/src/commands/ci/policy_registry.rs:89` (CheckProvider seam)
- `crates/vox-cli/src/commands/ci/run_body.rs:754-763` (GateStatusWriter seam) + `:776-870` (gate_status_result to move)
- `crates/vox-cli/src/commands/ci/exec_policy_contract.rs:19-110` (TerminalPolicyValidator seam + orchestration)
- `crates/vox-cli/src/commands/ci/scientia_novelty_ledger_contract.rs` + `crates/vox-cli/src/commands/scientia_ledger_contract.rs` (move outright)
- `crates/vox-cli/src/commands/ci/cmd_enums.rs:1-1119` (CiCmd + nested enums + gate_policy_id)
- `crates/vox-cli/src/lib.rs:566-569` (Cli::Ci wrapper rewrite)
- `crates/vox-cli/src/commands/audit.rs:59` (CheckManifest/CheckItem to move)
- `crates/vox-cli/tests/ci_workflow_contract.rs`, `crates/vox-cli/tests/vox_cli_root_parsing.rs` (clap golden gates for PR-4)
