# Build-Time Measurement Activation and Evidence-Ranked Decoupling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Populate the two committed measurement SSOTs that already-shipped gates read but that nobody ever filled in, then use the resulting blast-radius data to replace 29 unevidenced "Phase 2 decoupling target" labels with a ranked, measured worklist — and retire the per-crate LoC budget that has been ratcheted upward three times without ever binding.

**Architecture:** No new gates and no new tooling. Every gate and every command this plan needs already shipped from the 2026-06-15 and 2026-06-19 plans; what never ran was their *data-generation* steps. This plan executes those steps, calibrates the resulting numbers honestly for the host they were measured on, and then consumes them via the existing `crate-map --top-cuts` analysis to rank decoupling work by measured `blast_s` saved rather than by a two-month-old comment string.

**Tech Stack:** Rust (`vox-cli-ci`, `vox-code-audit`, `vox-graph-reader`), VoxScript (`scripts/crate-build-audit.vox`), `cargo build --timings` HTML ingest, JSON contracts under `contracts/ci/`.

**Spec:** No standalone design doc. This plan continues two existing plans whose code shipped but whose data steps did not — read both before starting:
- [`docs/superpowers/plans/2026-06-19-crate-build-measurement-spine-hardening.md`](2026-06-19-crate-build-measurement-spine-hardening.md) — Tasks 1–3 shipped the `build_crate_summary` function, the `crate-map --write-summary` flag, and the fail-loud `crate-budget` gate. **Task 2 Steps 3–4 ("generate the real committed file", "verify the SSOT has real numbers") never ran.**
- [`docs/superpowers/plans/2026-06-15-build-time-program-measured-phased.md`](2026-06-15-build-time-program-measured-phased.md) — Task 0.1 Step 3 ("record the blast-radius baseline") never ran; every phase-delta row still reads `PENDING-CI`.

## Global Constraints

- **Never fabricate a measurement.** Copied verbatim from the 2026-06-15 plan §"A note on running measurements": *"Do NOT skip the measurement and write a plausible number. If you genuinely cannot measure, say so in the task report and leave the delta cell as `PENDING-CI`."*
- **Never run another cargo command while a timings build is in flight.** Contention corrupts the per-crate self-times being collected, which is the entire product of Task 1.
- **Windows hosts:** never run `cargo fmt --all` (overflows `CreateProcess`; dies with os error 206). Use `cargo fmt -p <crate>`, or `vox run scripts/fmt.vox` for the workspace.
- **Windows hosts:** never run unscoped recursive `rg`/`grep` from the repo root — scope to a crate subdirectory or use a glob.
- **`graphify-out/graph.json` exists**, so a repo hook requires `graphify query "<question>"` for orientation before grepping or reading source files.
- **`contracts/ci/crate-edges.allow.v1.json` `exceptions` entries are USER-AUTHORIZED-ONLY** (AGENTS.md §Dependency Discipline). This plan may *propose* and *rank* removals; it must never add an exception, and must never regenerate a baseline to admit an edge.
- **Measurement-host provenance must be recorded with every number.** The existing ceilings in `contracts/ci/crate-budget.v1.json` were derived from Linux CI as `actual × 1.15`; numbers measured on a different host are valid for *ranking* but not for direct comparison against those ceilings. Task 2 exists solely to keep that distinction honest.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `graphify-out/crate_audit.json` | Generated (gitignored) | Per-crate `compile_s` + LoC + layer, produced by `scripts/crate-build-audit.vox` from the newest `cargo build --timings` HTML. Input to `crate-map`. |
| `contracts/ci/crate-build-map.v1.json` | Modify (committed SSOT) | The gate SSOT `vox ci crate-budget` grades against. Currently `has_compile_times: false`, all 125 crates at `blast_s: 0.0`. Task 1 populates it. |
| `contracts/ci/crate-budget.v1.json` | Modify (committed SSOT) | Keystone `blast_s_ceiling` values. Task 2 adds host provenance so a cross-host comparison can never be made silently. |
| `contracts/reports/crate-top-cuts.v1.json` | Create | Measured ranking of candidate dependency cuts by `blast_s` saved. The evidence artifact that replaces the 29 unevidenced "Phase 2 decoupling target" labels. |
| `docs/src/architecture/layers.toml` | Modify | `[guards] loc_budget` demoted to a reported trend; per-crate `max_loc` values retained as documentation. Task 4. |
| `docs/agents/governance.md` | Modify | Records why per-crate LoC is not a gate and which controls replace it. Task 4. |
| `crates/vox-cli-ci/src/crate_budget.rs` | Modify | Gains a host-provenance mismatch warning. Task 2. |

---

## Task 1: Populate the two measurement SSOTs

Executes the un-run steps of the 2026-06-19 plan (Task 2 Steps 3–4) and the 2026-06-15 plan (Task 0.1 Step 3). No code changes — this task produces *data*, which is exactly why it was skippable and therefore skipped.

**Files:**
- Create: `graphify-out/crate_audit.json` (gitignored intermediate)
- Modify: `contracts/ci/crate-build-map.v1.json`

**Interfaces:**
- Consumes: a `target/cargo-timings/cargo-timing-*.html` produced by `cargo build --timings`.
- Produces: `contracts/ci/crate-build-map.v1.json` with `has_compile_times: true` and non-zero `blast_s` for the 125 workspace crates. Tasks 2 and 3 both read this file.

- [ ] **Step 1: Confirm the box is quiet, then collect timings**

Contention inflates per-crate self-time, and this repo is routinely worked by
more than one agent session at a time on the same machine — separate worktrees
still share CPU and RAM.

**Do not use `tasklist` for this check.** It is not on `PATH` in the Git Bash
shell this harness provides. `tasklist //FI "IMAGENAME eq rustc.exe" //NH |
grep -c rustc` does not error — it prints `0` regardless of what is running,
because `grep -c` counts matches in empty output. That false "idle" reading
caused a duplicate workspace build to be launched on top of a live one during
this plan's own authoring. Use the PowerShell tool:

```powershell
Get-CimInstance Win32_Process -Filter "Name='cargo.exe' OR Name='rustc.exe'" |
  Select-Object ProcessId,Name,@{n='MB';e={[int]($_.WorkingSetSize/1MB)}},
    @{n='Cmd';e={$_.CommandLine.Substring(0,[Math]::Min(110,$_.CommandLine.Length))}} |
  Format-Table -Wrap -AutoSize
```

Expected: no rows. If rows appear, read the `Cmd` column — a peer session's
build is a reason to wait and coordinate, not to proceed. Proceeding produces
a number that looks authoritative and is not, which is the exact failure this
plan exists to end.

Then collect. `-j 6` (rather than the default 12) reduces peak memory. Full-parallelism
builds on this workspace repeatedly die with exit `0xc0000409`. Do not be misled by
the symbolic name Windows prints for that code: `STATUS_STACK_BUFFER_OVERRUN` is
what `__fastfail` reports, and Rust's OOM handler routes through it via `abort()`.
The backtrace is explicit — `memory allocation of 2097152 bytes failed` →
`rust_oom` → `handle_alloc_error` → `<rustc_arena::DroplessArena>::grow` — i.e.
rustc's type-interning arena running out of memory, **not** stack exhaustion.
Raising `RUST_MIN_STACK` does not help and marginally hurts (larger per-thread
reservations add memory pressure); the committed `RUST_MIN_STACK = "8388608"` at
`.cargo/config.toml:100` addresses a genuinely different failure,
`STATUS_STACK_OVERFLOW` (`0xC00000FD`) in libtest's spawned threads. Lower `-j`
and fewer concurrent builds are the fix:

```bash
cargo build --timings --workspace -j 6
```

Expected: completes; `target/cargo-timings/cargo-timing-*.html` exists.

- [ ] **Step 2: Verify the HTML actually carries durations**

The ingest parses an embedded `UNIT_DATA` JS array. A build that was fully cached emits rows with `duration: 0`, which the parser drops — yielding an empty audit and silently re-creating the all-zero problem this task exists to fix.

```bash
ls -t target/cargo-timings/*.html | head -1 | xargs grep -c "UNIT_DATA"
```

Expected: `1`. If `0`, the HTML is malformed — re-run Step 1.

- [ ] **Step 3: Generate the audit dataset**

```bash
vox run --mode interp scripts/crate-build-audit.vox
```

Expected: writes `graphify-out/crate_audit.json` and `graphify-out/CRATE_BUILD_AUDIT.md`. Use `--mode interp`: the script does no heavy compute, and the native lane adds a multi-minute compile.

- [ ] **Step 4: Verify the audit has non-zero compile times before writing the SSOT**

This is the check whose absence let `has_compile_times: false` get committed. Do not skip it.

```bash
python -c "
import json; d = json.load(open('graphify-out/crate_audit.json'))
rows = d if isinstance(d, list) else d.get('crates', [])
nz = [r for r in rows if float(r.get('compile_s') or 0) > 0]
print(f'rows={len(rows)} nonzero_compile_s={len(nz)}')
assert len(nz) > 50, 'audit is empty or near-empty — do NOT write the SSOT'
print('OK')
"
```

Expected: `OK`, with `nonzero_compile_s` in the hundreds.

- [ ] **Step 5: Write the committed SSOT**

```bash
vox graphify crate-map --write-summary contracts/ci/crate-build-map.v1.json
```

- [ ] **Step 6: Verify the SSOT is genuinely populated**

```bash
python -c "
import json; d = json.load(open('contracts/ci/crate-build-map.v1.json'))
assert d['has_compile_times'] is True, 'still count-only — the gate stays toothless'
assert d.get('crates_without_compile_times', 0) == 0, d.get('crates_without_compile_times')
top = sorted(d['crates'], key=lambda c: -c['blast_s'])[:5]
for c in top: print(f\"{c['crate']:<28} blast_s={c['blast_s']:>8.1f} compile_s={c['compile_s']:>6.1f} dependents={c['dependents']}\")
"
```

Expected: `has_compile_times` is `True`, and five crates print with non-zero `blast_s`.

- [ ] **Step 7: Run the gate that has been red-and-unrun**

```bash
cargo run -q -p vox-cli -- ci crate-budget
```

Expected: the gate now *evaluates* instead of bailing with `has_compile_times=false`. It may report `OVER` for one or more keystones — that is a real, actionable verdict and must NOT be silenced here. Record the exact output; Task 2 interprets it.

- [ ] **Step 8: Commit**

```bash
git add contracts/ci/crate-build-map.v1.json
git commit -m "contracts: populate crate-build-map with measured compile times

Executes 2026-06-19-crate-build-measurement-spine-hardening Task 2
Steps 3-4, which never ran: the code shipped but the committed SSOT
stayed at has_compile_times=false with all 125 crates at blast_s 0.0,
so the (correctly fail-loud) crate-budget gate could only ever bail.

Source: cargo build --timings --workspace -j 6, quiesced host."
```

---

## Task 2: Record measurement-host provenance so cross-host comparisons cannot happen silently

The ceilings in `crate-budget.v1.json` were set as *"actual (2026-06-19) × 1.15"* from Linux CI. `blast_s` measured on a different host — a contended Windows box, a laptop on battery, a different core count — is valid for **ranking** but not for **absolute** comparison against those ceilings. Without provenance, a host difference reads as a regression and the gate gets disabled again.

**Files:**
- Modify: `contracts/ci/crate-build-map.v1.json` (add `measured_on`)
- Modify: `crates/vox-cli-ci/src/crate_budget.rs`
- Test: `crates/vox-cli-ci/src/crate_budget.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `contracts/ci/crate-build-map.v1.json` from Task 1.
- Produces: `fn host_mismatch_warning(map_host: Option<&str>, ceiling_host: Option<&str>) -> Option<String>` — returns `Some(warning)` when both are known and differ, `None` otherwise.

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` in `crates/vox-cli-ci/src/crate_budget.rs`:

```rust
#[test]
fn warns_when_map_host_differs_from_ceiling_host() {
    let w = host_mismatch_warning(Some("x86_64-pc-windows-msvc"), Some("x86_64-unknown-linux-gnu"))
        .expect("differing hosts must warn");
    assert!(w.contains("x86_64-pc-windows-msvc"), "warning names the map host: {w}");
    assert!(w.contains("x86_64-unknown-linux-gnu"), "warning names the ceiling host: {w}");
}

#[test]
fn no_warning_when_hosts_match_or_are_unknown() {
    assert!(host_mismatch_warning(Some("x86_64-unknown-linux-gnu"), Some("x86_64-unknown-linux-gnu")).is_none());
    // Absent provenance must not warn — pre-existing files have no `measured_on`,
    // and a spurious warning on every run trains people to ignore it.
    assert!(host_mismatch_warning(None, Some("x86_64-unknown-linux-gnu")).is_none());
    assert!(host_mismatch_warning(Some("x86_64-pc-windows-msvc"), None).is_none());
    assert!(host_mismatch_warning(None, None).is_none());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p vox-cli-ci --lib crate_budget
```

Expected: FAIL, `cannot find function host_mismatch_warning`.

- [ ] **Step 3: Implement**

Add to `crates/vox-cli-ci/src/crate_budget.rs`:

```rust
/// Warn when the blast-radius map and the ceilings were measured on different
/// hosts. `blast_s` scales with the machine, so a Windows-measured map graded
/// against Linux-derived ceilings reports overage that is host variance, not
/// regression. Both unknown or either absent -> no warning: pre-existing files
/// carry no provenance, and a warning that fires every run gets ignored.
fn host_mismatch_warning(map_host: Option<&str>, ceiling_host: Option<&str>) -> Option<String> {
    match (map_host, ceiling_host) {
        (Some(m), Some(c)) if m != c => Some(format!(
            "crate-build-map was measured on '{m}' but the ceilings in \
             contracts/ci/crate-budget.v1.json derive from '{c}'. blast_s scales \
             with the host, so OVER verdicts below may be host variance rather \
             than regression. Re-measure on '{c}', or re-derive the ceilings on \
             '{m}', before treating a failure as real."
        )),
        _ => None,
    }
}
```

Then emit it in `run_crate_budget`, immediately after the `has_compile_times` guard and before the per-keystone loop:

```rust
    if let Some(w) = host_mismatch_warning(
        summary.get("measured_on").and_then(|v| v.as_str()),
        budget.measured_on.as_deref(),
    ) {
        eprintln!("WARN: {w}");
    }
```

Add the field to `BudgetFile`:

```rust
    /// Host triple the ceilings were derived on. `None` for files predating
    /// provenance tracking.
    #[serde(default)]
    pub measured_on: Option<String>,
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p vox-cli-ci --lib crate_budget
```

Expected: PASS.

- [ ] **Step 5: Stamp provenance into both contracts**

Add to `contracts/ci/crate-budget.v1.json`, as a sibling of `schema_version`:

```json
  "measured_on": "x86_64-unknown-linux-gnu",
```

Add the host the Task 1 map was actually generated on to `contracts/ci/crate-build-map.v1.json` — read it from `rustc -vV`, do not assume:

```bash
rustc -vV | grep "^host:"
```

- [ ] **Step 6: Verify the warning fires when hosts differ, and commit**

```bash
cargo run -q -p vox-cli -- ci crate-budget
```

Expected: if Task 1 measured on Windows, a `WARN:` line naming both triples precedes the keystone table. If both are Linux, no warning.

```bash
cargo fmt -p vox-cli-ci
git add crates/vox-cli-ci/src/crate_budget.rs contracts/ci/crate-budget.v1.json contracts/ci/crate-build-map.v1.json
git commit -m "ci(crate-budget): warn when map and ceilings were measured on different hosts

blast_s scales with the measuring machine. The ceilings derive from
Linux CI (actual x 1.15); grading a Windows-measured map against them
reports host variance as regression, which is how a gate gets disabled
instead of fixed."
```

---

## Task 3: Replace the 29 unevidenced "Phase 2 decoupling target" labels with a measured ranking

`contracts/ci/crate-edges.allow.v1.json` holds 34 exceptions, 29 of which carry `reason: "...(pre-existing upward edge; Phase 2 decoupling target)"` dated 2026-07-03. No Phase 2 plan exists, and the ratchet has never been tightened. This task produces the evidence needed to decide which of those 29 are worth cutting — it does **not** cut any.

**Files:**
- Create: `contracts/reports/crate-top-cuts.v1.json`
- Read only: `contracts/ci/crate-edges.allow.v1.json`

**Interfaces:**
- Consumes: `contracts/ci/crate-build-map.v1.json` (Task 1).
- Produces: `contracts/reports/crate-top-cuts.v1.json` — the ranked cut list Task 4's successor work draws from.

- [ ] **Step 1: Generate the ranked cut list**

`--top-cuts` ranks single-edge cuts by total `blast_s` saved. It is meaningless against a zero-valued map, which is why this task depends on Task 1:

```bash
vox graphify crate-map --top-cuts 40 > contracts/reports/crate-top-cuts.v1.json
```

- [ ] **Step 2: Verify the ranking is non-degenerate**

```bash
python -c "
import json; d = json.load(open('contracts/reports/crate-top-cuts.v1.json'))
cuts = d if isinstance(d, list) else d.get('cuts', d.get('top_cuts', []))
assert cuts, 'empty ranking'
nz = [c for c in cuts if float(c.get('blast_s_saved') or c.get('saved') or 0) > 0]
assert nz, 'every cut saves 0.0 — the map is still count-only'
print(f'{len(cuts)} cuts, {len(nz)} with non-zero saving')
for c in cuts[:10]: print(' ', c)
"
```

Expected: a non-empty list with non-zero savings.

- [ ] **Step 3: Intersect the ranking with the 29 grandfathered edges**

This is the deliverable: which grandfathered edges are actually worth the decoupling work, and which are cheap to keep.

```bash
python -c "
import json
allow = json.load(open('contracts/ci/crate-edges.allow.v1.json'))
p2 = {(x['from'], x['to']) for x in allow.get('exceptions', []) if 'Phase 2' in str(x.get('reason',''))}
d = json.load(open('contracts/reports/crate-top-cuts.v1.json'))
cuts = d if isinstance(d, list) else d.get('cuts', d.get('top_cuts', []))
def saved(c): return float(c.get('blast_s_saved') or c.get('saved') or 0)
ranked = {(c.get('from'), c.get('to')): saved(c) for c in cuts}
hits = sorted(((ranked.get(e, 0.0), e) for e in p2), reverse=True)
print(f'{len(p2)} Phase-2 edges; {sum(1 for s,_ in hits if s > 0)} appear in the top-40 ranking')
for s, e in hits:
    print(f'  {s:>8.1f}s  {e[0]} -> {e[1]}' + ('' if s > 0 else '   <- no measured build-time benefit'))
"
```

Expected: a ranked list. Edges printing `0.0` with the trailing marker have **no measured build-time justification** for decoupling — surface them, do not cut them on the strength of a label.

- [ ] **Step 4: Commit the evidence artifact**

```bash
git add contracts/reports/crate-top-cuts.v1.json
git commit -m "contracts: add measured top-cuts ranking for dependency decoupling

29 exceptions in crate-edges.allow.v1.json carry a 'Phase 2 decoupling
target' label from 2026-07-03 with no plan behind it and no evidence
that cutting any of them saves build time. This ranks candidate cuts by
measured blast_s saved so the work can be prioritised, or declined, on
data. Cuts nothing; adds no exception (those are user-authorized-only)."
```

---

## Task 4: Retire the per-crate LoC budget as a gate

`[guards] loc_budget` in `layers.toml` is `warn`, so it produces noise nobody acts on. Eight crates are over budget by ~63K LoC combined, and `vox-orchestrator`'s budget was raised 55K → 60K → 70K and is *still* blown at 85.5K. A limit raised whenever it binds is a lagging indicator, not a constraint. The two controls that actually work already exist: per-file god-object (Error at 500 non-blank lines, enforced as a ratchet on changed files) for agent ergonomics, and `blast_s` (armed by Task 1) for build time.

**Files:**
- Modify: `docs/src/architecture/layers.toml`
- Modify: `docs/agents/governance.md`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks. Independent of Tasks 1–3 and may be done first.

- [ ] **Step 1: Confirm the claim before acting on it**

Do not take the numbers on faith — regenerate them:

```bash
grep -oE "^([a-z0-9-]+)\s*=\s*\{[^}]*max_loc\s*=\s*[0-9_]+" docs/src/architecture/layers.toml \
  | sed -E 's/\s*=\s*\{.*max_loc\s*=\s*/ /; s/_//g' \
  | while read -r crate budget; do
      d="crates/$crate/src"; [ -d "$d" ] || continue
      actual=$(find "$d" -name '*.rs' -type f -exec cat {} + 2>/dev/null | wc -l)
      [ "$actual" -gt "$budget" ] && printf "OVER %-24s %7d / %7d\n" "$crate" "$actual" "$budget"
    done
```

Expected: several `OVER` rows. If none, this task is moot — stop and report.

- [ ] **Step 2: Demote the guard**

In `docs/src/architecture/layers.toml`, replace the `loc_budget` line in `[guards]`:

```toml
# Reported, never blocking. Per-crate LoC is a weak proxy for both goals it was
# serving: build cost is measured directly by blast_s (contracts/ci/crate-build-map.v1.json,
# gated by `vox ci crate-budget`), and agent read/write ergonomics are a per-FILE
# property enforced by arch/god_object (Error at 500 non-blank lines). The
# per-crate number bound nothing: vox-orchestrator's budget was raised
# 55K -> 60K -> 70K and is still exceeded at ~85.5K. Rule 13 (`loc_delta`)
# catches god-crate drift better, because growth is the real signal.
loc_budget        = "off"
```

Verify `"off"` is an accepted value; if the parser only accepts `warn`/`error`, keep `warn` and delete the per-crate `max_loc` values instead, leaving `loc_delta` as the trend guard:

```bash
grep -n "loc_budget\|fn.*guard\|\"warn\"\|\"error\"\|\"off\"" crates/vox-arch-check/src/main.rs | head -20
```

- [ ] **Step 3: Record the rationale where contributors read it**

Append to the "God Object Limit (Multi-Tier)" section of `docs/agents/governance.md`:

```markdown
### Why there is no per-crate LoC gate

Per-crate `max_loc` in `layers.toml` is documentation, not a gate. It conflated
three unrelated goals and was a poor proxy for each:

- **Build cost** is `blast_s` (compile_s x dependents), measured in
  `contracts/ci/crate-build-map.v1.json` and gated by `vox ci crate-budget`.
  LoC correlates weakly — 20K lines of serde structs compile faster than 5K
  lines of heavy generics.
- **Agent read/write ergonomics** is a per-FILE property, enforced by
  `arch/god_object` (Error at 500 non-blank lines). An agent reads files, not
  crates: a 60K-line crate of focused 300-line files is easier to work in than
  a 20K-line crate of eight 2,500-line files.
- **Architectural discipline** is fan-in/fan-out plus `where-things-live.md`.

Some crates are complex by necessity — `vox-compiler` is a compiler. Decomposing
to satisfy an arbitrary crate-level number is work in service of a bad metric.
Growth still matters: Rule 13 (`loc_delta`) warns when a budgeted crate grows
>15% against the last release tag.
```

- [ ] **Step 4: Verify arch-check still passes and commit**

```bash
cargo run -q -p vox-arch-check
```

Expected: runs clean; no `loc_budget` findings.

**Note for the executor:** `layers.toml` lives under `docs/src/architecture/`, which triggers the whole-`crates/` TOESTUB sweep in `enforce-warn` mode (`.github/workflows/ci.yml:764`). Commit `5ece1c52f` removed `@generated` files from that scan set, but ~315 hand-written files still exceed 500 non-blank lines. They are grandfathered by *scoping* (the per-PR gate at `ci.yml:745` only scans changed files), not by suppression — so expect this PR's sweep to surface them. If it blocks, that is a pre-existing condition to report, **not** something to fix by suppressing findings or reverting this task.

```bash
git add docs/src/architecture/layers.toml docs/agents/governance.md
git commit -m "arch: stop gating on per-crate LoC; keep per-file and blast_s

Per-crate max_loc bound nothing — vox-orchestrator's budget was raised
three times (55K/60K/70K) and is still exceeded at 85.5K. Build cost is
measured by blast_s; agent ergonomics is per-file (arch/god_object,
Error at 500 lines). Both already exist and both are enforced."
```

---

## Self-Review

**Spec coverage.** The two predecessor plans' un-run steps are covered by Task 1 (2026-06-19 Task 2 Steps 3–4; 2026-06-15 Task 0.1 Step 3). Three items had no plan at all and are covered here: host-provenance (Task 2), the 29 unevidenced Phase 2 labels (Task 3), the LoC gate (Task 4). Deliberately **not** covered: populating `build-bench-baseline.v1.json`, which needs repeated wall-clock runs on a quiesced host and cannot be produced honestly on a contended one — commit `4cca4f84c` made that gate refuse a placeholder baseline rather than report a false 0%, so it fails loudly until someone measures it properly.

**Placeholder scan.** No TBD/TODO. Every step carries a runnable command and an expected result. Task 3's *output values* are necessarily unknown before Task 1 runs, but its procedure and its pass/fail assertions are exact; the verification asserts non-degeneracy rather than any specific number.

**Type consistency.** `host_mismatch_warning(Option<&str>, Option<&str>) -> Option<String>` is defined once in Task 2 Step 3 and used with that signature in Step 1's tests and in the `run_crate_budget` call site. `BudgetFile::measured_on` is `Option<String>`, read via `.as_deref()`. The map side is read via `summary.get("measured_on").and_then(|v| v.as_str())`, matching the existing `has_compile_times` access pattern in the same function.

**Ordering.** Task 1 → Task 2 → Task 3 is a hard chain (each consumes the prior's artifact). Task 4 is independent and may run first or in parallel.
