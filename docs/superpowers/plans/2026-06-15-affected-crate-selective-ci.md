# Affected-Crate Selective CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make PR-time CI build/test/clippy only the crates affected by a PR's changes (changed crates + their reverse-dependency closure), instead of rebuilding the whole 110-crate workspace every push — while keeping a full-workspace build at the merge-queue gate so main stays sound.

**Architecture:** A committed crate-dependency graph (`contracts/ci/crate-graph.v1.json`, drift-gated) feeds a tested `vox ci affected-crates` subcommand that maps a PR's `git diff` → changed crates → reverse-dep closure (or a `full` verdict when a sentinel file changes). The `setup` job emits the affected crate list; the three heavy jobs (`tests`, `lints`, `audits`) consume it on `pull_request` (run `-p <affected>`) and fall back to `--workspace` on `merge_group`/`push:main` (the authoritative gate, which also runs once per batch). A shadow-mode rollout proves parity before the fast path becomes load-bearing.

**Tech Stack:** Rust (the `vox-cli` `ci` subcommand surface), `cargo metadata`, `cargo nextest`/`cargo clippy`/`cargo check` with `-p` scoping, GitHub Actions (`.github/workflows/ci.yml`), `serde_json`.

---

## Feasibility verdict & design rationale (read first)

The two audits established:

| Finding | Source | Consequence for this plan |
|---|---|---|
| Resolver v2 + **`workspace-hack`** (70 direct / **92 of 110** transitive dependents) | `Cargo.toml` `resolver="2"`, `.config/hakari.toml` | Affected-only is a **sound test/clippy *selection*** but **not** sound *build isolation* (feature unification can differ in a full build). → **Keep `--workspace` at the merge gate; affected-only is PR-time only.** |
| Real blast radius: `vox-gui`→1, `vox-cli`→4, `vox-ast`/`vox-compiler`/`vox-db`→48, `vox-config`→62, sentinel→110 | `cargo metadata` closure | Big win on leaf/feature PRs; small win on base-crate PRs. Worth it — most PRs are leaf-ish. |
| Sentinel files force full: root `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml`, `rust-toolchain.toml`, `crates/workspace-hack/**`, `.config/hakari.toml` | audit §3 | Hard-coded sentinel list; any match → `full=true`. |
| `merge_group` has **no base-ref** (`ci.yml:53-56` forces everything true) | `ci.yml` setup job | **Design choice: `merge_group` = full build (the authoritative gate).** Avoids the no-base problem AND closes the feature-unification soundness gap AND batches (one full build per group of ≤5 PRs). |
| `ci-summary` aggregator rejects `skipped` (`if [ "$r" != "success" ]`) | `ci.yml:955-976` | **Blocker:** if any of the 5 required jobs skips, the gate goes RED. Must add a `skipped`-as-pass clause **before** any job is made skippable. |
| Only `tests` consumes the existing `rust_changed`/`docs_changed`, and even then never skips the workspace compile | `ci.yml:726-744` | The coarse flag exists but is unused; we extend the same `setup` job. |
| No first-party proc-macro crates; 13 `build.rs` crates (handled by normal reverse-dep edges) | audit §4 | No extra fallback needed for proc-macros/build scripts. |
| Prior art: `mutation-pr.yml` uses `tj-actions/changed-files` per-crate matrix; `all-features-matrix` enumerates 24 crates | `ci.yml`, `mutation-pr.yml` | Patterns to mirror; we standardize on the tested subcommand instead of inline action logic. |
| sccache is **local-dir** (`/cache/sccache`), 0% cross-worktree hits | `ci.yml:20-24` | Out of scope here, but noted: affected-only compounds with sccache; a distributed sccache (the GHA backend already used in `ci-fallback-hosted.yml`) is a separate follow-up. |

**The load-bearing design decision:** `pull_request` push → **affected-only** (fast). `merge_group` + `push:main` + nightly → **full `--workspace`** (sound). Because the merge queue runs the full build on the batched result before merging, a wrong affected-only result on a PR can only *delay at the queue*, never break main. This makes the optimization safe to ship.

**Why a tested Rust subcommand, not inline bash:** the affected computation is correctness-critical (a bug = false-green at PR time). Pure functions (sentinel match, file→crate, reverse-dep BFS) get Rust unit tests. It mirrors the existing `vox ci <gate>` surface. Per `AGENTS.md`, automation glue is `.vox`/`vox` — this is a `vox ci` subcommand, not a `.sh`.

---

## Handoff context for the executor (Claude Sonnet 4.6)

> **This plan is being handed to Claude Sonnet 4.6 to execute.** Sonnet 4.6 is capable but, relative to the author, more prone to editing from memory, fabricating plausible-but-wrong API signatures, and declaring success without running the verification. The guardrails below are not optional polish — they are the difference between a correct landing and a false-green CI gate. Read this section in full before touching Task 1.

### Hard guardrails for Sonnet 4.6 (non-negotiable)

- **ALWAYS `Read` the exact file before editing it.** Do NOT edit from memory or from this plan's excerpts. **The plan's line numbers (e.g. `ci.yml:955`) are approximate and the file has already drifted** — see the pre-flight checks below, where the real anchors differ from the quoted lines. Re-locate every edit site by searching for the quoted **anchor string**, never by jumping to a line number.
- **Do NOT invent API signatures.** Before calling any `vox_*` crate function or assuming a struct field, `Grep` for its definition and `Read` it. If this plan references a function/module/path that does not exist as written, **STOP and report** — do not fabricate a plausible substitute and do not "make it compile" by guessing the real name.
- **Run EVERY verification command the task specifies and paste the actual output.** Do not claim a test passed without running it. If a build or test fails, fix the **root cause**. Do NOT comment out, `#[ignore]`, `#[allow(...)]`-away, or weaken an assertion to make it pass. A weakened test on a correctness-critical CI gate is worse than no test.
- **One task at a time.** Commit after each task with the **exact** commit message given in that task. Do not batch multiple tasks into one commit and do not skip the commits — the phased rollout depends on each task being an isolated, revertable unit.
- **If a step's expected output does not match reality, STOP and report the discrepancy** with the actual output you observed. Do not improvise past it (e.g. do not "adjust" the assertion, sentinel list, or graph by hand to match a surprising result — a surprise means an assumption is wrong, and that is exactly the bug this plan exists to prevent).
- **Never use `--no-verify`,** never run `cargo fmt --all` (it overflows the Windows arg limit → `os error 206`; format a single crate with `cargo fmt -p <crate>` or run `vox run scripts/fmt.vox`), and **never push to `main` directly.** All workflow/CI changes land via PR — the merge-queue full build is the soundness backstop that makes this whole optimization safe, so it must not be bypassed.

### Pre-flight checks (resolve these FOUR before writing any code)

These are the open verification items from the Self-Review, promoted here as **mandatory pre-flight grep/read steps**. Do them first, in order, and report the resolved path/shape for each before starting Task 1. (The author already ran these once; the expected answers are noted so you can confirm — if your result differs, the codebase drifted again and you must STOP and report.)

1. **Host crate/module for `vox ci` subcommands.** The plan's File Structure says `affected.rs`/`affected_cmd.rs` go in `crates/vox-cli-ci/src/` — **VERIFY this.** `Grep` for an existing advisory check name (`runner-policy-check`, `gui-visual-review`) and for the *subcommand registry* (`Grep -n "ssot-drift"` across `crates/vox-cli/`). Read the registration: the **enum variant** (`CiCmd` in `crates/vox-cli/src/commands/ci/cmd_enums.rs`, shape `#[command(name = "…")] Variant,`) and the **dispatch arm** (`crates/vox-cli/src/commands/ci/run_body.rs`, shape `CiCmd::Variant => handler(&root),`). Mirror that shape exactly. **Expected answer:** the *check logic* crate is `vox-cli-ci` (e.g. `runner_policy_check.rs`), but the `vox ci <name>` **enum + dispatch lives in `crates/vox-cli/src/commands/ci/`** — so the plan's "Modify `crates/vox-cli-ci/src/lib.rs` (the `ci` dispatch)" line is imprecise; the dispatch you must edit is in `vox-cli`, not `vox-cli-ci`. Report the exact two file paths you will edit before writing `affected.rs`.
2. **`ssot-drift` sub-check registration shape.** `Grep -n "run_ssot_drift\|ssot_drift" crates/vox-cli/src/commands/ci/` to see how an existing sub-check is added. **Expected answer:** `CiCmd::SsotDrift => run_ssot_drift(&root)` in `run_body.rs`, with `run_ssot_drift` defined under `run_body_helpers/` and re-exported (see the `use super::…run_ssot_drift…` line). Add your `check_graph` call as a sub-check in the same helper module, mirroring an existing sub-check's call/return shape.
3. **Does `setup` already build `vox-cli`?** Read the `setup` job in `.github/workflows/ci.yml` *before* adding any build step. **Expected answer: YES — it already does.** `setup` runs `cargo build -p vox-cli --locked --features completion-toestub` and uploads `target/debug/vox` as an artifact; every downstream job downloads it and `chmod +x target/debug/vox`. **This changes Task 9 materially:** do NOT add a second `cargo build -p vox-cli --quiet` (see the Critique section). Compute affected using the binary the `setup` job already produces, after that build step.
4. **The real clippy allow-list.** The plan's `cargo clippy … -A clippy::… …` uses `…` as a stand-in. `Grep -n "A clippy::" .github/workflows/ci.yml` and copy the list **verbatim**. **Expected answer (the `lints` job invocation):**
   ```
   cargo clippy --workspace --all-targets --exclude vox-gui -- \
     -D warnings \
     -A clippy::items_after_test_module \
     -A clippy::collapsible_match \
     -A clippy::collapsible_if \
     -A clippy::should_implement_trait \
     -A clippy::doc_overindented_list_items \
     -A clippy::doc_lazy_continuation
   ```
   Preserve every `-A` flag and the `-D warnings`. (There is a second, identical list in the `guards-fast`/early clippy step — keep both in sync.)

---

## Critique & improvements vs the codebase

A frank pass over the plan against the real tree (`crates/vox-cli/src/commands/ci/`, `.github/workflows/ci.yml`). The Rust/YAML blocks are sound; these are the seams a less-careful execution will tear.

- **Risk — `cargo build -p vox-cli` in `setup` adds latency to every PR (and the plan double-builds it).** Pre-flight check 3 establishes that `setup` **already** builds `vox-cli` and ships `target/debug/vox` as an artifact. Task 9's freshly-added `cargo build -p vox-cli --quiet` is therefore **redundant** — it would compile the CLI a second time on the critical path of every PR. **Improvement:** in `setup`, compute affected crates *after the existing build step*, invoking the already-built `./target/debug/vox ci affected-crates …` (do not add a new `cargo build`). If you ever do need a build that isn't already present, the cheaper options are (a) reuse the uploaded `target/debug/vox` artifact, or (b) a pre-built `vox` from the self-hosted runner image if one is on `PATH` — `Grep` the runner image / container setup before assuming. **Recommend measuring first:** capture the `setup` job wall-time before and after, and only keep the affected computation in `setup` if it adds < ~10s on top of the build that's already there.
- **Risk — `affected_p_args` is interpolated into a shell `cargo` command (word-splitting / injection surface).** Tasks 11–12 expand `$P_ARGS` / `${{ needs.setup.outputs.affected_p_args }}` straight into a `cargo …` line. Crate names are `[a-z0-9-]`, so this is **low-risk in practice**, but it is an unvalidated string crossing a shell boundary. **Improvement:** have the subcommand (`run_affected_cmd`, Task 6) **validate each crate name against `^[a-zA-Z0-9_-]+$` before emitting** it, and hard-error if a name fails (a non-conforming name means the graph is corrupt). In the YAML, quote the expansion where it's a single token and rely on intentional word-splitting only for the `-p a -p b` form — document at the `run:` site that `P_ARGS` is trusted *because* the subcommand validated it. Belt-and-suspenders, but cheap insurance on a gate.
- **Gap — doctests are skipped at PR time.** Task 11 runs `cargo test --workspace --doc` **only** in the `FULL` (gate) path. That is acceptable (doctests run at the merge gate and nightly), but the plan never states it as an intentional gap. **Make it explicit:** "PR-time affected runs do NOT run doctests; doctests are covered by the full `--workspace --doc` at `merge_group` + nightly. A doctest-only regression is therefore invisible until the merge gate — accepted, because it cannot land on `main` (the gate runs full)." Add this line to the Task 13 SSOT doc.
- **Gap — a NEW crate added in a PR.** If a PR adds `crates/vox-new/`, the committed `crate-graph.v1.json` won't contain it until regenerated, so `file_to_crate` maps its files to a crate that isn't a graph node (it still becomes a seed, but its reverse-deps are unknown). **This is caught by the Task 2 drift gate** (the graph is stale → `--check` fails with the regen hint → CI red), which is the correct behavior. **State the interaction explicitly** in the SSOT doc and at Task 2: *"Adding or removing a crate requires regenerating `crate-graph.v1.json` in the same PR; the `ssot-drift` graph-drift sub-check enforces this — a PR that adds a crate without regenerating the graph fails CI with a regenerate hint."* This turns a silent correctness hole into a loud, self-explaining failure.
- **Improvement — the `compiler-gates` conditional uses substring `contains`.** Task 12 Step 3 gates on `contains(needs.setup.outputs.affected_crates, 'vox-compiler')`. GitHub Actions `contains` on a string is a **substring** match, so a hypothetical `vox-compiler-foo` (or `vox-compilerd`) would **false-match** and pull in `compiler-gates` unnecessarily — and worse, a substring check is brittle if crate names ever overlap. **Improvement:** match whole tokens. Cleanest: have the subcommand emit a dedicated boolean output (e.g. `affects_compiler=true|false`, computed by exact set membership of `vox-compiler`/`vox-codegen`/`vox-integration-tests` in the affected `BTreeSet`) and gate on `needs.setup.outputs.affects_compiler == 'true'` instead of substring-checking the space-joined list. (`vox-compilerd` is a real crate name in this tree — `Grep` confirms — so this false-match is not hypothetical.)
- **Improvement — Phase 4 shadow-mode acceptance is qualitative.** Task 10's acceptance gate ("no case where the full `--workspace` test found a failure in a crate that was NOT in the affected set") is described as something the executor eyeballs over several PRs. On a correctness-critical gate, "eyeballed" is the wrong bar. **Improvement:** make it **data-driven**. Add a small shadow comparator step that, on the full run, parses the nextest/junit results, and for any **failing** test whose crate is NOT in `affected_crates`, emits `::warning title=affected-ci shadow-miss::<crate> failed but was not in the affected set`. Then the Phase-4 acceptance gate becomes mechanical: *"flip Phase 5 only after N PRs with zero `shadow-miss` warnings,"* rather than a human judgment call. (The junit XML is already produced — Task 11 references `--tool-config-file "vox-ci:${junit_cfg}"` — so the comparator has structured input to read.)

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `crates/vox-cli-ci/src/affected.rs` | **Create** | Pure logic: graph type, sentinel match, file→crate, reverse-dep closure, `compute_affected()`. Unit-tested. |
| `crates/vox-cli-ci/src/affected_cmd.rs` | **Create** | CLI glue: read changed-files file + graph JSON, call `compute_affected`, emit `$GITHUB_OUTPUT` lines; `--regen`/`--check` for the graph SSOT. |
| `contracts/ci/crate-graph.v1.json` | **Create** | Committed workspace dep graph (`crate → forward workspace deps`); the affected input (no `cargo metadata` at CI time). |
| `crates/vox-cli-ci/src/lib.rs` (or the `ci` dispatch) | **Modify** | Register `affected-crates` subcommand + its enum variant + dispatch arm. |
| `.github/workflows/ci.yml` | **Modify** | (a) `setup` builds `vox-cli` + emits `full`/`affected_crates`; (b) `ci-summary` treats `skipped` as pass; (c) `tests`/`lints`/`audits` scope to affected on PR, full on merge_group/push. |
| `docs/src/ci/affected-crate-selective-ci.md` | **Create** | SSOT doc: the rule, the PR-vs-gate split, the sentinel list, the graph-drift gate. |

> **Where this lives:** the audit found the advisory CI checks live in `vox-cli-ci` (e.g. `gui_visual_review.rs`, `runner_scale.rs`). Confirm the exact crate/module that hosts `vox ci` subcommands by reading one sibling (`crates/vox-cli-ci/src/`), and place `affected.rs`/`affected_cmd.rs` there, matching its registration pattern exactly.

**Phasing (each phase = shippable):**
- **Phase 1 (Tasks 1–3):** crate-graph SSOT + generator + drift gate. (No CI behavior change.)
- **Phase 2 (Tasks 4–7):** the `affected-crates` computation + subcommand (TDD). (No CI behavior change.)
- **Phase 3 (Tasks 8–9):** wire `setup` outputs + fix `ci-summary` skip handling. (Still full builds; outputs unused = safe.)
- **Phase 4 (Task 10):** **shadow mode** — compute affected, log it, still run full, compare. Build confidence.
- **Phase 5 (Tasks 11–13):** flip `tests`/`lints`/`audits` to affected-on-PR / full-on-gate; docs; remove shadow.

---

## Task 1: Generate the crate-graph SSOT

**Files:**
- Create: `contracts/ci/crate-graph.v1.json`
- Create: `crates/vox-cli-ci/src/affected_cmd.rs` (the `--regen` path only, here)

> *Sonnet note: the code uses `&vec![]` as the `unwrap_or` fallback inside loops — Rust 1.96 will warn `temporary value dropped while borrowed` on `as_array().unwrap_or(&vec![])`. Bind the empty vec to a `let empty = vec![];` once and pass `&empty`, or use `.unwrap_or_default()` on an owned iterator; do NOT delete the null-safety. Also add `use serde_json;` is unnecessary (it's a crate, referenced by path) — but `serde::{Deserialize, Serialize}` and `std::collections::{BTreeMap, BTreeSet}` imports ARE required; the block compiles only with them present.*

- [ ] **Step 1: Write the graph generator (`--regen`)**

In `affected_cmd.rs`, implement a function that shells `cargo metadata --format-version 1 --no-deps` is insufficient (need resolve graph); use `cargo metadata --format-version 1` and parse:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Default)]
pub struct CrateGraph {
    pub schema_version: u32,
    /// crate name -> its workspace (first-party) dependency names
    pub crates: BTreeMap<String, Vec<String>>,
}

/// Build the graph from `cargo metadata` JSON (parsed value).
pub fn graph_from_metadata(meta: &serde_json::Value) -> CrateGraph {
    // workspace member package ids
    let members: std::collections::BTreeSet<String> = meta["workspace_members"]
        .as_array().unwrap_or(&vec![]).iter()
        .filter_map(|v| v.as_str().map(String::from)).collect();
    // id -> name
    let mut id_name = BTreeMap::new();
    for p in meta["packages"].as_array().unwrap_or(&vec![]) {
        if let (Some(id), Some(name)) = (p["id"].as_str(), p["name"].as_str()) {
            id_name.insert(id.to_string(), name.to_string());
        }
    }
    let mut crates: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in meta["resolve"]["nodes"].as_array().unwrap_or(&vec![]) {
        let id = node["id"].as_str().unwrap_or("");
        if !members.contains(id) { continue; }
        let name = match id_name.get(id) { Some(n) => n.clone(), None => continue };
        let mut deps: Vec<String> = node["deps"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|d| d["pkg"].as_str())
            .filter(|pid| members.contains(*pid))
            .filter_map(|pid| id_name.get(pid).cloned())
            .collect();
        deps.sort(); deps.dedup();
        crates.insert(name, deps);
    }
    CrateGraph { schema_version: 1, crates }
}
```

- [ ] **Step 2: Add the `--regen` CLI path**

```rust
pub fn regen_graph(out_path: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let graph = graph_from_metadata(&meta);
    let json = serde_json::to_string_pretty(&graph).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(out_path.parent().unwrap()).ok();
    std::fs::write(out_path, json + "\n").map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: Generate the committed graph**

Run (from repo root): `cargo run -p vox-cli -- ci affected-crates --regen --out contracts/ci/crate-graph.v1.json`
(After Task 6 wires the subcommand; for now, a temporary `cargo test`-driven call or a scratch `main` is fine — but the cleanest order is to do Task 6's registration first, then run this. If you implement Task 6 before this step, run it here.)
Expected: `contracts/ci/crate-graph.v1.json` with ~110 entries. Sanity: `vox-gui` has a non-trivial `deps` list; `workspace-hack` appears.

- [ ] **Step 4: Commit**

```bash
git add contracts/ci/crate-graph.v1.json crates/vox-cli-ci/src/affected_cmd.rs
git commit -m "feat(ci): generate committed crate-dependency graph SSOT"
```

---

## Task 2: Crate-graph drift gate (TDD)

**Files:**
- Modify: `crates/vox-cli-ci/src/affected_cmd.rs` (add `--check`)
- Wire into the existing `ssot-drift` gate

> *Sonnet note: pre-flight check 2 — the real `ssot-drift` dispatch is `CiCmd::SsotDrift => run_ssot_drift(&root)` in `crates/vox-cli/src/commands/ci/run_body.rs`, and `run_ssot_drift` lives in `run_body_helpers/`. Add `check_graph` as a sub-check INSIDE that helper (mirror an existing sub-check's signature + error propagation), not by hand-editing `run_body.rs`'s match arm. Also: this gate is what catches a PR that adds a crate without regenerating the graph — make it HARD-fail, not advisory.*

- [ ] **Step 1: Write the failing test**

In `affected_cmd.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graph_from_metadata_extracts_workspace_edges() {
        let meta: serde_json::Value = serde_json::json!({
            "workspace_members": ["a 0.1 (path+file:///a)", "b 0.1 (path+file:///b)"],
            "packages": [
                {"id":"a 0.1 (path+file:///a)","name":"a"},
                {"id":"b 0.1 (path+file:///b)","name":"b"},
                {"id":"serde 1.0 (registry)","name":"serde"}
            ],
            "resolve": {"nodes": [
                {"id":"a 0.1 (path+file:///a)","deps":[{"pkg":"b 0.1 (path+file:///b)"},{"pkg":"serde 1.0 (registry)"}]},
                {"id":"b 0.1 (path+file:///b)","deps":[]}
            ]}
        });
        let g = graph_from_metadata(&meta);
        assert_eq!(g.crates["a"], vec!["b"]);   // serde dropped (not a workspace member)
        assert_eq!(g.crates["b"], Vec::<String>::new());
    }
}
```

- [ ] **Step 2: Run it** — `cargo test -p vox-cli-ci affected_cmd::tests::graph_from_metadata` → PASS (the impl from Task 1 satisfies it). If FAIL, fix `graph_from_metadata`.

- [ ] **Step 3: Implement `--check` (regenerate-in-memory, diff against committed)**

```rust
pub fn check_graph(committed: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new("cargo").args(["metadata","--format-version","1"])
        .output().map_err(|e| e.to_string())?;
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let fresh = serde_json::to_string_pretty(&graph_from_metadata(&meta)).map_err(|e| e.to_string())? + "\n";
    let on_disk = std::fs::read_to_string(committed).unwrap_or_default();
    if fresh != on_disk {
        return Err(format!("crate-graph drift: regenerate with `vox ci affected-crates --regen --out {}`", committed.display()));
    }
    Ok(())
}
```

- [ ] **Step 4: Wire into ssot-drift**

Find the `ssot-drift` gate dispatch (the audit / memory references `vox ci ssot-drift`). Add a call to `check_graph(Path::new("contracts/ci/crate-graph.v1.json"))` as one of its sub-checks (advisory or hard — make it **hard**: a stale graph means affected-only could miss a new crate). Mirror an existing sub-check's registration.

- [ ] **Step 5: Verify + commit**

Run: `cargo run -p vox-cli -- ci affected-crates --check` → exits 0 (graph is fresh). Touch a crate's deps, re-run → fails with the regen hint.
```bash
git add crates/vox-cli-ci/src/affected_cmd.rs
git commit -m "feat(ci): crate-graph drift gate (regenerate-on-change)"
```

---

## Task 3: `Cargo.toml` member sanity (TDD guard)

**Files:**
- Modify: `crates/vox-cli-ci/src/affected.rs` (the SENTINELS const — defined here, used in Task 4)

> *Sonnet note: this is a `Create` of `affected.rs`, not a `Modify` (the file does not exist yet — the header says "Modify" but Task 6's File Structure lists it as Create). After creating it, the module must be declared (`mod affected;` / `pub mod affected;`) in the crate's `lib.rs` or it won't compile into the test target — `Grep` how a sibling like `runner_policy_check` is declared and mirror it.*

- [ ] **Step 1: Define the sentinel list with a test**

Create `crates/vox-cli-ci/src/affected.rs` (start):

```rust
/// Files whose change forces a full-workspace build (affected-only is unsound for these).
/// Derived from the feasibility audit §3.
pub const SENTINEL_EXACT: &[&str] = &["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".config/hakari.toml"];
pub const SENTINEL_PREFIX: &[&str] = &[".cargo/", "crates/workspace-hack/"];

/// True if a changed path forces a full build.
pub fn is_sentinel(path: &str) -> bool {
    SENTINEL_EXACT.contains(&path) || SENTINEL_PREFIX.iter().any(|p| path.starts_with(p))
}

#[cfg(test)]
mod sentinel_tests {
    use super::*;
    #[test] fn root_cargo_toml_is_sentinel() { assert!(is_sentinel("Cargo.toml")); }
    #[test] fn per_crate_cargo_toml_is_not() { assert!(!is_sentinel("crates/vox-gui/Cargo.toml")); }
    #[test] fn cargo_config_is_sentinel() { assert!(is_sentinel(".cargo/config.toml")); }
    #[test] fn lockfile_is_sentinel() { assert!(is_sentinel("Cargo.lock")); }
    #[test] fn hakari_pkg_is_sentinel() { assert!(is_sentinel("crates/workspace-hack/Cargo.toml")); }
    #[test] fn hakari_config_is_sentinel() { assert!(is_sentinel(".config/hakari.toml")); }
    #[test] fn normal_crate_file_is_not() { assert!(!is_sentinel("crates/vox-gui/src/App.tsx")); }
}
```

> Critical: `Cargo.toml` (root) is a sentinel but `crates/X/Cargo.toml` is NOT (it's a per-crate change). `is_sentinel` uses exact match for the root one, so `crates/.../Cargo.toml` correctly falls through.

- [ ] **Step 2: Run** — `cargo test -p vox-cli-ci affected::sentinel_tests` → 7 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-cli-ci/src/affected.rs
git commit -m "feat(ci): affected-crate sentinel-file detection (TDD)"
```

---

## Task 4: file→crate mapping (TDD)

**Files:** Modify `crates/vox-cli-ci/src/affected.rs`

> *Sonnet note: `file_to_crate` returns `Option<&str>` borrowing from the input `path` — do not "fix" a borrow-checker complaint by changing it to `Option<String>` without also updating the `.map(String::from)` call sites in Task 6's `compute_affected`. Run the exact `cargo test -p vox-cli-ci affected::file_crate_tests` and paste the "4 passed" line; do not assume.*

- [ ] **Step 1: Add the failing test + impl**

```rust
/// Map a changed file path to its owning workspace crate, if any.
/// `crates/<name>/...` -> Some("<name>"); anything else -> None.
pub fn file_to_crate(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("crates/")?;
    let name = rest.split('/').next()?;
    if name.is_empty() { None } else { Some(name) }
}

#[cfg(test)]
mod file_crate_tests {
    use super::*;
    #[test] fn maps_crate_src() { assert_eq!(file_to_crate("crates/vox-db/src/lib.rs"), Some("vox-db")); }
    #[test] fn maps_crate_toml() { assert_eq!(file_to_crate("crates/vox-gui/Cargo.toml"), Some("vox-gui")); }
    #[test] fn non_crate_is_none() { assert_eq!(file_to_crate("docs/foo.md"), None); }
    #[test] fn bare_crates_dir_is_none() { assert_eq!(file_to_crate("crates/"), None); }
}
```

- [ ] **Step 2: Run** — `cargo test -p vox-cli-ci affected::file_crate_tests` → 4 PASS.

- [ ] **Step 3: Commit** — `git commit -am "feat(ci): file→crate mapping (TDD)"`

---

## Task 5: reverse-dependency closure (TDD)

**Files:** Modify `crates/vox-cli-ci/src/affected.rs`

> *Sonnet note: the `use std::collections::{BTreeMap, BTreeSet, VecDeque};` line is required and `VecDeque` in particular is easy to forget — the BFS won't compile without it. If Task 3 already imported `BTreeMap`/`BTreeSet` at file top, do NOT duplicate the import here (Rust errors on a redundant `use`); consolidate into one top-of-file `use`.*

- [ ] **Step 1: Add the failing test + impl**

```rust
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// graph: crate -> its forward (workspace) deps. Returns: for each crate, who depends on it.
pub fn invert(graph: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, BTreeSet<String>> {
    let mut rev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (krate, deps) in graph {
        rev.entry(krate.clone()).or_default(); // ensure node exists
        for dep in deps {
            rev.entry(dep.clone()).or_default().insert(krate.clone());
        }
    }
    rev
}

/// BFS over the inverted graph: all crates transitively depending on any seed, incl. seeds.
pub fn reverse_closure(graph: &BTreeMap<String, Vec<String>>, seeds: &BTreeSet<String>) -> BTreeSet<String> {
    let rev = invert(graph);
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut q: VecDeque<String> = seeds.iter().cloned().collect();
    while let Some(c) = q.pop_front() {
        if !out.insert(c.clone()) { continue; }
        if let Some(dependents) = rev.get(&c) {
            for d in dependents { if !out.contains(d) { q.push_back(d.clone()); } }
        }
    }
    out
}

#[cfg(test)]
mod closure_tests {
    use super::*;
    fn g() -> BTreeMap<String, Vec<String>> {
        // a -> b -> c ; d -> b
        BTreeMap::from([
            ("a".into(), vec!["b".into()]),
            ("b".into(), vec!["c".into()]),
            ("c".into(), vec![]),
            ("d".into(), vec!["b".into()]),
        ])
    }
    #[test] fn leaf_change_only_itself() {
        let s = BTreeSet::from(["a".to_string()]);
        assert_eq!(reverse_closure(&g(), &s), BTreeSet::from(["a".to_string()]));
    }
    #[test] fn base_change_pulls_all_dependents() {
        let s = BTreeSet::from(["c".to_string()]);
        // c's dependents: b; b's dependents: a, d
        assert_eq!(reverse_closure(&g(), &s), BTreeSet::from(["a".into(),"b".into(),"c".into(),"d".into()]));
    }
    #[test] fn mid_change_pulls_upward_only() {
        let s = BTreeSet::from(["b".to_string()]);
        assert_eq!(reverse_closure(&g(), &s), BTreeSet::from(["a".into(),"b".into(),"d".into()])); // not c
    }
}
```

- [ ] **Step 2: Run** — `cargo test -p vox-cli-ci affected::closure_tests` → 3 PASS. (Note `mid_change_pulls_upward_only` proves we go UP the dep tree, not down — changing `b` does not force rebuilding its dependency `c`.)

- [ ] **Step 3: Commit** — `git commit -am "feat(ci): reverse-dependency closure BFS (TDD)"`

---

## Task 6: `compute_affected` + the subcommand (TDD)

**Files:**
- Modify: `crates/vox-cli-ci/src/affected.rs` (the top-level `compute_affected`)
- Modify: `crates/vox-cli-ci/src/affected_cmd.rs` (CLI wiring + `$GITHUB_OUTPUT`)
- Modify: the `ci` subcommand registry (enum + dispatch)

> *Sonnet note: pre-flight check 1 — "the `ci` subcommand registry" is NOT `vox-cli-ci/src/lib.rs`. It is two files in `crates/vox-cli/src/commands/ci/`: add the variant `#[command(name = "affected-crates")] AffectedCrates(...)` to `CiCmd` in `cmd_enums.rs`, and the arm `CiCmd::AffectedCrates(a) => run_affected_cmd(...)` in `run_body.rs`. Read an existing variant that takes args (not the unit `SsotDrift`) to copy the arg-struct shape. Do not invent the dispatch signature — match an existing one exactly. Also add the crate-name `^[a-zA-Z0-9_-]+$` validation (see Critique) before emitting `affected_p_args`.*

- [ ] **Step 1: Add `compute_affected` + test**

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum Affected {
    Full,                    // a sentinel changed → build everything
    None,                    // no rust crates changed → build nothing
    Crates(BTreeSet<String>),// the affected closure
}

pub fn compute_affected(changed_files: &[String], graph: &BTreeMap<String, Vec<String>>) -> Affected {
    if changed_files.iter().any(|f| is_sentinel(f)) { return Affected::Full; }
    let seeds: BTreeSet<String> = changed_files.iter().filter_map(|f| file_to_crate(f)).map(String::from).collect();
    if seeds.is_empty() { return Affected::None; }
    Affected::Crates(reverse_closure(graph, &seeds))
}

#[cfg(test)]
mod compute_tests {
    use super::*;
    fn g() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([("vox-gui".into(), vec!["vox-config".into()]), ("vox-config".into(), vec![])])
    }
    #[test] fn sentinel_forces_full() {
        assert_eq!(compute_affected(&["Cargo.lock".into()], &g()), Affected::Full);
    }
    #[test] fn docs_only_is_none() {
        assert_eq!(compute_affected(&["docs/x.md".into()], &g()), Affected::None);
    }
    #[test] fn leaf_change_scopes_to_self() {
        assert_eq!(compute_affected(&["crates/vox-gui/src/App.tsx".into()], &g()),
                   Affected::Crates(BTreeSet::from(["vox-gui".to_string()])));
    }
    #[test] fn base_change_pulls_dependents() {
        let a = compute_affected(&["crates/vox-config/src/lib.rs".into()], &g());
        assert_eq!(a, Affected::Crates(BTreeSet::from(["vox-config".into(),"vox-gui".into()])));
    }
}
```

- [ ] **Step 2: Run** — `cargo test -p vox-cli-ci affected::compute_tests` → 4 PASS.

- [ ] **Step 3: CLI: emit GitHub outputs**

In `affected_cmd.rs`:

```rust
/// Args: --changed <file of newline paths> --graph <json> --github-output <path> [--regen --out <p>] [--check]
pub fn run_affected_cmd(args: &[String]) -> i32 {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    if args.iter().any(|a| a == "--regen") {
        let out = get("--out").unwrap_or_else(|| "contracts/ci/crate-graph.v1.json".into());
        return match regen_graph(std::path::Path::new(&out)) { Ok(_) => { eprintln!("wrote {out}"); 0 }, Err(e) => { eprintln!("{e}"); 1 } };
    }
    if args.iter().any(|a| a == "--check") {
        return match check_graph(std::path::Path::new("contracts/ci/crate-graph.v1.json")) { Ok(_) => 0, Err(e) => { eprintln!("::error::{e}"); 1 } };
    }
    let changed_path = get("--changed").expect("--changed required");
    let graph_path = get("--graph").unwrap_or_else(|| "contracts/ci/crate-graph.v1.json".into());
    let changed: Vec<String> = std::fs::read_to_string(&changed_path).unwrap_or_default()
        .lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let graph: CrateGraph = serde_json::from_str(&std::fs::read_to_string(&graph_path).unwrap_or_default()).unwrap_or_default();
    let aff = crate::affected::compute_affected(&changed, &graph.crates);
    let (full, list) = match aff {
        crate::affected::Affected::Full => (true, String::new()),
        crate::affected::Affected::None => (false, String::new()),
        crate::affected::Affected::Crates(set) => (false, set.into_iter().collect::<Vec<_>>().join(" ")),
    };
    // -p args form for cargo: "-p a -p b"
    let p_args = if list.is_empty() { String::new() } else { list.split(' ').map(|c| format!("-p {c}")).collect::<Vec<_>>().join(" ") };
    let out_line = format!("full={full}\naffected_crates={list}\naffected_p_args={p_args}\n");
    if let Some(go) = get("--github-output") {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(go).unwrap();
        write!(f, "{out_line}").unwrap();
    } else { print!("{out_line}"); }
    0
}
```

- [ ] **Step 4: Register the subcommand**

Read a sibling `vox ci` subcommand's registration (enum variant + dispatch arm, e.g. how `gui-visual-review` or `runner-policy-check` is wired) and add `affected-crates` identically, dispatching to `run_affected_cmd`.

- [ ] **Step 5: Build + smoke**

```bash
cargo build -p vox-cli
printf 'crates/vox-gui/src/App.tsx\n' > /tmp/ch.txt
cargo run -p vox-cli -- ci affected-crates --changed /tmp/ch.txt
```
Expected: `full=false` / `affected_crates=vox-gui ...` (vox-gui + any workspace dependents — for a true leaf, just `vox-gui`). Then test a sentinel: `printf 'Cargo.lock\n' > /tmp/ch.txt` → `full=true`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli-ci/src/affected.rs crates/vox-cli-ci/src/affected_cmd.rs crates/vox-cli-ci/src/lib.rs
git commit -m "feat(ci): `vox ci affected-crates` subcommand (sentinel + closure → GH outputs)"
```

---

## Task 7: Generate/refresh the committed graph

> *Sonnet note: `contracts/ci/` already exists (it holds `check-targets.v1.yaml`), so `--regen` only creates the one new JSON file. The `jq` spot-check in Step 2 must show `vox-gui` with a NON-empty deps array and the file must have ~110 keys — if it has far fewer, `cargo metadata` ran with `--no-deps` or in the wrong cwd; STOP and report rather than committing a truncated graph.*

- [ ] **Step 1:** `cargo run -p vox-cli -- ci affected-crates --regen --out contracts/ci/crate-graph.v1.json`
- [ ] **Step 2:** Verify: `cargo run -p vox-cli -- ci affected-crates --check` → exit 0. Spot-check `jq '.crates["vox-gui"]' contracts/ci/crate-graph.v1.json` is non-empty, and the file has ~110 keys.
- [ ] **Step 3: Commit** — `git add contracts/ci/crate-graph.v1.json && git commit -m "chore(ci): commit crate-graph SSOT baseline"`

---

## Task 8: Fix `ci-summary` to accept `skipped` (BLOCKER — do before any job is made skippable)

**Files:** Modify `.github/workflows/ci.yml` (the `ci-summary` job, ~lines 955-976)

> *Sonnet note: the line numbers have drifted. Re-Read `ci.yml` and find the `ci-summary` job by searching for the string `All required jobs succeeded` (real anchor ≈ line 944) and the loop guard `if [ "$r" != "success" ]` (≈ line 954). Edit the guard you actually find; do NOT trust 955-976.*

- [ ] **Step 1: Change the aggregation**

Replace the loop body so a `skipped` upstream job counts as a pass:

```yaml
          for r in \
            "${{ needs.guards-fast.result }}" \
            "${{ needs.lints.result }}" \
            "${{ needs.compiler-gates.result }}" \
            "${{ needs.tests.result }}" \
            "${{ needs.audits.result }}"; do
            if [ "$r" != "success" ] && [ "$r" != "skipped" ]; then fail=1; fi
          done
```

- [ ] **Step 2: Validate** — `python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('ok')"` and confirm `gui-playwright-smoke` etc. are unaffected.

- [ ] **Step 3: Commit** — `git commit -am "ci: ci-summary treats skipped required jobs as pass (enables selective skip)"`

---

## Task 9: `setup` emits `full` + `affected` outputs

**Files:** Modify `.github/workflows/ci.yml` (`setup` job, lines 35-96)

> *Sonnet note: pre-flight check 3 — the `setup` job ALREADY runs `cargo build -p vox-cli --locked --features completion-toestub` (≈ line 90) and uploads `target/debug/vox` as an artifact. Do NOT add the plan's `cargo build -p vox-cli --quiet` — it double-builds the CLI on every PR's critical path. Instead, run the affected computation with the binary that build step already produced: `./target/debug/vox ci affected-crates --changed … --graph … --github-output "$GITHUB_OUTPUT"`, placed AFTER the existing build step. Search for the anchor `cargo build -p vox-cli --locked` to find it.*

- [ ] **Step 1: Build vox-cli early + compute affected**

Add to the `setup` job, after the existing `filter` step, a new step (keep the existing `rust`/`docs` outputs for compatibility):

```yaml
      - name: Build vox-cli + compute affected crates
        id: affected
        shell: bash
        run: |
          if [ "${{ github.event_name }}" = "pull_request" ]; then
            git fetch origin "${{ github.base_ref }}" --depth=256 2>/dev/null || true
            git diff --name-only "origin/${{ github.base_ref }}...HEAD" > /tmp/changed.txt || true
            cargo build -p vox-cli --quiet
            ./target/debug/vox ci affected-crates --changed /tmp/changed.txt \
              --graph contracts/ci/crate-graph.v1.json --github-output "$GITHUB_OUTPUT"
          else
            # push / merge_group → authoritative FULL gate (no base ref; closes feature-unification gap)
            echo "full=true" >> "$GITHUB_OUTPUT"
            echo "affected_crates=" >> "$GITHUB_OUTPUT"
            echo "affected_p_args=" >> "$GITHUB_OUTPUT"
          fi
      - name: Expose affected outputs
        id: out
        run: true   # (the job's `outputs:` map references steps.affected.outputs.*)
```

And in the `setup` job's `outputs:` block, add:
```yaml
    outputs:
      rust_changed: ${{ steps.filter.outputs.rust }}
      docs_changed: ${{ steps.filter.outputs.docs }}
      full: ${{ steps.affected.outputs.full }}
      affected_crates: ${{ steps.affected.outputs.affected_crates }}
      affected_p_args: ${{ steps.affected.outputs.affected_p_args }}
```

> Note: `cargo build -p vox-cli` in `setup` adds ~1–2 min but the heavy jobs `needs: setup` and reuse the sccache-warmed target. On `push`/`merge_group` the build is skipped (full=true short-circuit) — those events already rebuild everything downstream.

- [ ] **Step 2: Validate YAML** + confirm `setup.outputs.full` resolves. Push to a scratch PR and inspect the `setup` job log: for a docs-only PR, `full=false affected_crates=` (empty); for a `vox-gui` change, `affected_crates` lists vox-gui.

- [ ] **Step 3: Commit** — `git commit -am "ci(setup): emit full + affected_crates outputs via vox ci affected-crates"`

---

## Task 10: Shadow mode — compute, log, compare (DO NOT yet change build scope)

**Files:** Modify `.github/workflows/ci.yml` (`tests` job)

> *Sonnet note: this task MUST NOT change the build scope — `tests` still runs full `--workspace`; you are only ADDING a logging step. If you find yourself editing the `nextest` invocation here, STOP — that's Task 11. Also implement the data-driven `shadow-miss` comparator from the Critique section so the Step 2 acceptance gate is mechanical (zero `::warning title=…shadow-miss` over N PRs), not eyeballed.*

> Goal: prove the affected set is correct on real PRs before it gates anything. The `tests` job still runs `--workspace`, but ALSO logs what affected-only *would* have run and whether the workspace result agrees.

- [ ] **Step 1: Add a shadow step to `tests`**

At the start of the `tests` job (still running full `--workspace` as today), add:

```yaml
      - name: Shadow — log affected scope (no behavior change)
        if: ${{ github.event_name == 'pull_request' }}
        run: |
          echo "::notice title=affected-ci shadow::full=${{ needs.setup.outputs.full }} affected=[${{ needs.setup.outputs.affected_crates }}]"
          # Sanity: if not full and affected is empty but rust_changed=true, that's a bug — flag it.
          if [ "${{ needs.setup.outputs.full }}" != "true" ] && [ -z "${{ needs.setup.outputs.affected_crates }}" ] && [ "${{ needs.setup.outputs.rust_changed }}" = "true" ]; then
            echo "::warning title=affected-ci shadow::rust changed but affected set empty — investigate file→crate mapping"
          fi
```

- [ ] **Step 2: Run shadow for a few days / several PRs.** Manually confirm on ≥5 real PRs that: leaf PRs → small affected set; base-crate PRs → large set; docs-only → empty + full=false; sentinel PRs → full=true. **Acceptance gate before Phase 5:** no case where the full `--workspace` test found a failure in a crate that was NOT in the affected set (i.e. affected-only would have missed it). If found, fix the graph/closure (likely a missing edge or a sentinel gap) and extend the test suite.

- [ ] **Step 3: Commit** — `git commit -am "ci(tests): shadow-log affected scope before enabling selective builds"`

---

## Task 11: Flip `tests` to affected-on-PR / full-on-gate

**Files:** Modify `.github/workflows/ci.yml` (`tests` job, lines 657-874)

> *Sonnet note: line numbers will have drifted — find the `tests` job by its `cargo llvm-cov nextest`/`cargo nextest run` invocation, not by line. The `${junit_cfg}` and `--tool-config-file "vox-ci:…"` tokens are existing variables in that job; reuse them verbatim — do NOT invent their values. Confirm the FULL-only doctest line (`cargo test --workspace --doc`) stays inside the `if [ "$FULL" = "true" ]` guard; doctests being skipped on PR is intentional (record it in the Task 13 doc), not a bug to "fix" by running them on every PR.*

- [ ] **Step 1: Scope the nextest invocation**

Replace the workspace nextest commands with affected-aware ones. Define a shell var and branch on `full`:

```yaml
      - name: Tests (affected on PR, full on gate)
        shell: bash
        run: |
          set -euo pipefail
          FULL="${{ needs.setup.outputs.full }}"
          P_ARGS="${{ needs.setup.outputs.affected_p_args }}"
          if [ "$FULL" = "true" ]; then
            SCOPE="--workspace"
          elif [ -z "$P_ARGS" ]; then
            echo "No affected crates — skipping Rust tests."; exit 0
          else
            SCOPE="$P_ARGS"
          fi
          if [ "${{ needs.setup.outputs.rust_changed }}" = "true" ] && [ "$FULL" = "true" ]; then
            cargo llvm-cov nextest $SCOPE --profile ci --tool-config-file "vox-ci:${junit_cfg}"
          else
            cargo nextest run $SCOPE --profile ci --tool-config-file "vox-ci:${junit_cfg}"
          fi
          # doctests: full only on gate (doctest selection by -p is supported but noisy)
          if [ "$FULL" = "true" ]; then cargo test --workspace --doc; fi
```

> Keep llvm-cov coverage only in the FULL (gate) path — coverage of a partial crate set is misleading, and the coverage-gate already runs at merge. Adjust the surrounding coverage-gate steps to run only when `FULL == true`.

- [ ] **Step 2: Guard the coverage-gate steps** with `if: ${{ needs.setup.outputs.full == 'true' }}` so partial-scope PRs don't run/Ë fail coverage thresholds.

- [ ] **Step 3: Verify** on a leaf PR: the `tests` job runs only `cargo nextest run -p vox-gui ...` (seconds, not the full build). On a merge_group run: full `--workspace`. Confirm `ci-summary` stays green when `tests` runs the small scope.

- [ ] **Step 4: Commit** — `git commit -am "ci(tests): affected-crate nextest on PR, full workspace on merge gate"`

---

## Task 12: Flip `lints` + `audits` to affected-on-PR / full-on-gate

**Files:** Modify `.github/workflows/ci.yml` (`lints` lines 463-570, `audits` lines 876-950)

> *Sonnet note: replace the `…` in the clippy line with the REAL allow-list from pre-flight check 4 (the six `-A clippy::…` flags plus `-D warnings`) — copy verbatim, do not abbreviate. In Step 3, the `contains(needs.setup.outputs.affected_crates, 'vox-compiler')` substring check FALSE-MATCHES `vox-compilerd` (a real crate here); prefer the dedicated `affects_compiler` boolean output from the Critique section. Preserve `--exclude vox-gui` on every clippy path — vox-gui breaks `clippy --all-targets`.*

- [ ] **Step 1: `lints` — scope clippy/doc**

```yaml
          FULL="${{ needs.setup.outputs.full }}"; P_ARGS="${{ needs.setup.outputs.affected_p_args }}"
          if [ "$FULL" = "true" ]; then SCOPE="--workspace --exclude vox-gui"; 
          elif [ -z "$P_ARGS" ]; then echo "no affected crates"; exit 0;
          else SCOPE="$(echo "$P_ARGS" | sed 's/-p vox-gui//')"; fi   # keep the vox-gui clippy exclusion
          cargo clippy $SCOPE --all-targets -- -D warnings -A clippy::items_after_test_module …
          if [ "$FULL" = "true" ]; then RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps; fi
```
> Preserve the `--exclude vox-gui` clippy gotcha (memory: vox-gui breaks `clippy --all-targets`). In affected mode, strip any `-p vox-gui` from the args. `cargo doc` full only on gate.

- [ ] **Step 2: `audits` — scope all-features check**

```yaml
          FULL="${{ needs.setup.outputs.full }}"; P_ARGS="${{ needs.setup.outputs.affected_p_args }}"
          if [ "$FULL" = "true" ]; then cargo check --workspace --all-features;
          elif [ -n "$P_ARGS" ]; then cargo check $P_ARGS --all-features;
          else echo "no affected crates"; fi
```
> `cargo check --all-features -p <subset>` is where feature-unification divergence is most likely — that's exactly why the FULL gate re-runs `--workspace --all-features` at merge. The PR-time check is the fast signal.

- [ ] **Step 3: Keep `guards-fast` + `compiler-gates` as-is.** `guards-fast` is light (prebuilt binary). `compiler-gates` is already `-p vox-compiler`/`-p vox-integration-tests` (per-crate). Add a skip-when-not-affected guard to `compiler-gates` only if vox-compiler/vox-integration-tests are not in the affected set:
```yaml
    # at the compiler-gates job level
    if: ${{ needs.setup.outputs.full == 'true' || contains(needs.setup.outputs.affected_crates, 'vox-compiler') || contains(needs.setup.outputs.affected_crates, 'vox-codegen') || github.event_name != 'pull_request' }}
```

- [ ] **Step 4: Verify + commit** — confirm a `vox-gui`-only PR skips `compiler-gates`, runs `lints`/`tests`/`audits` scoped to vox-gui (fast), and `ci-summary` is green. Commit:
```bash
git commit -am "ci(lints,audits): affected-crate scope on PR, full workspace on merge gate"
```

---

## Task 13: SSOT doc + nightly full-build backstop

**Files:**
- Create: `docs/src/ci/affected-crate-selective-ci.md`
- Modify: a nightly schedule (reuse `bench-nightly.yml` pattern or add a tiny `full-workspace-nightly` job)

> *Sonnet note: this doc lives under `docs/src/`, so the YAML frontmatter (`title`/`description`/`category: "CI & Quality"`) is MANDATORY — the pre-push doc-pipeline blocks the push without it. Write it as the first lines of the file, exactly as shown in Step 1; do not omit it. Add the two intentional-gap lines from the Critique (PR-time skips doctests; adding a crate requires regenerating the graph). Verify `bench-nightly.yml` actually exists before claiming to reuse its cron shape — `Grep` for it first.*

- [ ] **Step 1: Write the doc** (canonical `category`):

```markdown
---
title: Affected-Crate Selective CI
description: PR-time CI builds only the crates a change affects; the merge-queue gate and nightly run the full workspace for soundness.
category: "CI & Quality"
---

# Affected-Crate Selective CI

## The rule
- **`pull_request` push:** build/test/clippy only the changed crates + their reverse-dependency closure (`vox ci affected-crates`). Fast feedback.
- **`merge_group` (the gate) + `push:main` + nightly:** full `--workspace`. Authoritative — closes the resolver-v2 feature-unification gap that per-crate builds can miss.

Because the merge queue runs the full build on the batched result before merging, an incorrect affected-only PR result can only delay at the queue — it can never land a break on main.

## Sentinels (force full)
`Cargo.toml` (root), `Cargo.lock`, `.cargo/config.toml`, `rust-toolchain.toml`, `crates/workspace-hack/**`, `.config/hakari.toml`.

## The graph
`contracts/ci/crate-graph.v1.json` (crate → workspace deps), regenerated by `vox ci affected-crates --regen` and drift-gated in `ssot-drift`. Add/remove a crate → regenerate.

## Blast radius (why it helps)
Leaf change (`vox-gui`) → 1 crate. Base change (`vox-config`) → ~62/110. Sentinel → 110.
```

- [ ] **Step 2: Add a nightly full-workspace job** (a `schedule:` trigger running the `tests`+`lints`+`audits` with `full=true` forced) so feature-unification regressions that slip past PR-time affected-only are caught within 24h even on branches that never reach the merge queue. Reuse the `bench-nightly.yml` cron shape.

- [ ] **Step 3: Validate ssot-drift passes for the doc + commit**
```bash
git add docs/src/ci/affected-crate-selective-ci.md .github/workflows/
git commit -m "docs(ci): affected-crate selective CI SSOT + nightly full-build backstop"
```

---

## Self-Review

**Spec coverage:**
- "every GitHub Actions stage runs only against relevant changed files" → Tasks 11–12 scope the 3 heavy Rust jobs; the ~18 non-Rust workflows already have `paths:` filters (audit §5), and #314 just tiered the heaviest non-required ones. ✅ (Note: this plan scopes the **required Rust gate**, the biggest global-rebuild offender; a follow-up could path-filter the remaining light workflows, but they're already cheap.)
- "affected-crate-only Rust build/test" → Tasks 4–7 (computation), 11–12 (wiring). ✅
- "path-filtered job gating" → Task 12 Step 3 (compiler-gates conditional), existing rust/docs. ✅
- "merge-queue batch context" → design decision: merge_group = full (Tasks 8–9, 11–12 branch on `full`). ✅
- "correctness risks (feature unification, proc-macros, build scripts, high-fanout base crates)" → feature-unification handled by full-gate + nightly (Tasks 11–13); proc-macros none (audit §4); build.rs handled by closure (audit §4); high-fanout handled by honest blast-radius (full on base-crate changes is acceptable/expected). ✅
- "39 workflows / required CI / rust-docs filter / sccache" → audited; this plan targets ci.yml (the offender); sccache distributed-backend is a noted out-of-scope follow-up. ✅

**Placeholder scan:** The one intentional ordering note is Task 1 Step 3 (generate the graph) depends on Task 6's registration — flagged inline ("if you implement Task 6 first, run it here"). No TODO/TBD. The clippy arg lists use `…` to mean "keep the existing allow-list verbatim from `ci.yml:508-516`" — the executor copies the real list, not a placeholder feature.

**Type consistency:** `CrateGraph.crates: BTreeMap<String,Vec<String>>` is produced by `graph_from_metadata` (Task 1), consumed by `compute_affected`/`invert`/`reverse_closure` (Tasks 4–6) and the CLI (Task 6). `Affected{Full,None,Crates}` defined Task 6, matched in `run_affected_cmd`. Output keys `full`/`affected_crates`/`affected_p_args` emitted in Task 6, consumed in `setup.outputs` (Task 9) and the jobs (Tasks 11–12) identically.

**Open verification items for the executor** (flagged at point of use): (1) exact host crate/module for `vox ci` subcommands — read a sibling before placing `affected.rs` (Task 6 Step 4); (2) the `ssot-drift` sub-check registration shape (Task 2 Step 4); (3) whether `setup` already builds `vox-cli` or it must be added (Task 9 Step 1); (4) the real clippy allow-list to preserve (Task 12 Step 1).

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-15-affected-crate-selective-ci.md`. Two execution options:

**1. Subagent-Driven (recommended)** — fresh subagent per task, two-stage review. Phases 1–2 (the tested `vox ci affected-crates`) are pure Rust with zero CI risk; Phase 4 (shadow mode) de-risks before Phase 5 flips the gate.

**2. Inline Execution** — execute in this session with checkpoints.

Which approach? (Note: I'd strongly recommend running through **Phase 4 shadow mode** and its acceptance gate before flipping Phase 5 — it's the safeguard against a false-green.)
