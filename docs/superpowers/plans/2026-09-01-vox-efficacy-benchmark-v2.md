---
title: "Vox Efficacy Benchmark v2 — Implementation Plan (audit-corrected)"
description: "Task-by-task TDD plan for a literature-comparable Vox code-generation benchmark: canary-based cheat detection, unbiased pass@k, paired McNemar significance, in-context conditions C0-C3, and corpus-integrity gates. Supersedes the 2026-09-01 v1 plan."
category: "Plans"
status: "draft"
training_eligible: false
training_rationale: "Contains a working exploit against the benchmark oracle and held-out fixture identifiers."
---

# Vox Efficacy Benchmark v2 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.
>
> **Supersedes** [`2026-09-01-vox-efficacy-benchmark-and-leaderboard.md`](2026-09-01-vox-efficacy-benchmark-and-leaderboard.md) (v1). Do not execute v1.
>
> **Audit:** [`vox-efficacy-benchmark-adversarial-audit-2026-09-01.md`](../../src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md) — every task below cites the defect it closes.

**Goal:** Measure how well frontier models write Vox, in a way that (a) cannot be trivially cheated, (b) uses the same estimators and tests as the published literature, and (c) makes a claim the sample size can actually support.

**Architecture:** Phase 0 fixes the corpus *data* (a benchmark cannot be more trustworthy than its split). Phase A builds the anti-cheat spine before anything can score. Phase B fixes verification I/O. Phase C replaces the statistics with literature-standard ones. Phase D adds the in-context conditions that make the measurement meaningful for a language no model has seen. Phase E is the runner; Phase F publication. Correctness is decided **only** by compiler and test exit codes — no LLM judge anywhere.

**Tech Stack:** Rust, `clap`, `serde`, `tokio`/`futures`, `vox_actor_runtime::llm`, Astro 6.

**Spec:** [research doc](../../src/architecture/vox-mens-comparative-efficacy-benchmarking-research-2026-09-01.md) · [audit](../../src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md)

## Global Constraints

- **LLM boundary:** all calls through `vox_actor_runtime::llm`. Never a vendor SDK/hostname. Detector `llm_provider_call` = Error.
- **Test-first:** every new `pub fn` needs a `#[test]` in the same file before commit (`tdd-guard` pre-commit hook).
- **No new `.ps1`/`.sh`/`.py` glue.** Orchestration is `.vox` via `vox run`.
- **Formatting:** `vox run scripts/fmt.vox`. **Never** `cargo fmt --all` (Windows `os error 206`).
- **Crate edges:** zero new workspace edges required — `vox-cli` already depends on `vox-eval`, `vox-corpus`, `vox-db`, `vox-publisher` (`crates/vox-cli/Cargo.toml:170,251,185,201`). If `vox ci crate-edges` objects, STOP and report; **never** author an `exceptions` entry.
- **Layers:** `vox-eval` L2, `vox-corpus` L3, `vox-publisher` L3, `vox-cli` L4. No new crates.
- **Reproducibility pins:** `temperature` and `n` are per-condition (Task 9), **not** the v1 blanket `temperature=0.0, attempts=5`. There is **no seed** — `LlmConfig` has no seed field and none reaches the wire; do not claim seeded reproducibility (audit H4).
- **Binary:** verification needs a `vox` binary; dev-profile is fine. If rustc dies with `STATUS_STACK_BUFFER_OVERRUN` / `0xc0000409` and a `rust_oom` backtrace, that is **memory exhaustion, not a code error** — rebuild with `-j 2`.
- **Nothing publishes** until Phase 0 and Phase A are green. A benchmark that can be cheated or whose split is unknown must not emit a number.

---

## File Structure

| File | Responsibility |
|---|---|
| `contracts/eval/humaneval-vox/manifest.v1.yaml` | **Modify.** Reconcile split; add `added_at` from git. |
| `crates/vox-cli-ci/src/corpus_integrity.rs` | **Create.** CI gate: manifest ≡ spec.toml ≡ `.vox` markers; no duplicate references. |
| `crates/vox-corpus/src/humaneval_runner/mod.rs` | **Create.** Module root. |
| `crates/vox-corpus/src/humaneval_runner/manifest.rs` | **Create.** Fixture loading, held-out filter. |
| `crates/vox-corpus/src/humaneval_runner/compose.rs` | **Create.** Brace-aware composition. |
| `crates/vox-corpus/src/humaneval_runner/canary.rs` | **Create.** Oracle-neutralization detection. |
| `crates/vox-corpus/src/humaneval_runner/verify.rs` | **Create.** File-redirected subprocess verification. |
| `crates/vox-eval/src/corpus_score.rs` | **Create.** Unbiased pass@k over (n, c). |
| `crates/vox-eval/src/corpus_stats.rs` | **Create.** McNemar exact, Holm, cluster bootstrap, Wilson. |
| `crates/vox-corpus/src/humaneval_runner/conditions.rs` | **Create.** C0–C3 prompt construction + context hashing. |
| `crates/vox-cli/src/commands/model/eval_corpus.rs` | **Create.** Runner. |
| `contracts/reports/vox-efficacy/leaderboard.v1.schema.json` | **Create.** Nullable metric columns. |
| `docs-astro/src/pages/benchmarks.astro` | **Create.** Page with resolution-floor disclosure. |

---

# Phase 0 — Corpus integrity (closes C7)

A benchmark is not more trustworthy than its split. This phase runs first because every later number depends on knowing which fixtures are held out.

### Task 1: Corpus-integrity CI gate

**Files:**
- Create: `crates/vox-cli-ci/src/corpus_integrity.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs`

**Interfaces:**
- Produces: `pub struct IntegrityReport { pub split_mismatches: Vec<String>, pub duplicate_groups: Vec<Vec<String>>, pub missing_added_at: Vec<String> }`; `pub fn audit_corpus(corpus_root: &Path) -> anyhow::Result<IntegrityReport>`; `pub fn run(repo_root: &Path) -> anyhow::Result<()>`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli-ci/src/corpus_integrity.rs` with only the doc comment and tests:

```rust
//! `vox ci corpus-integrity` — asserts the eval corpus's held-out split is
//! internally consistent and leak-free.
//!
//! Three independent sources declare `training_eligible`: the manifest, each
//! fixture's `spec.toml`, and (by absence of a marker) the `.vox` files the
//! training-corpus extractor actually reads. As of the 2026-09-01 audit these
//! disagreed 31 / 10 / 0, and held-out fixture 072 had a byte-identical
//! training-eligible twin (141) — i.e. its answer was already in the training
//! set. This gate makes that class of drift a build failure.

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().and_then(|p| p.parent()).expect("workspace root").to_path_buf()
    }

    #[test]
    fn real_corpus_has_no_split_mismatch_no_duplicates_and_full_added_at() {
        let report = audit_corpus(&repo_root().join("contracts/eval/humaneval-vox"))
            .expect("corpus audits");
        assert!(
            report.split_mismatches.is_empty(),
            "manifest and spec.toml disagree on training_eligible for {} fixture(s): {:?}",
            report.split_mismatches.len(), report.split_mismatches
        );
        assert!(
            report.duplicate_groups.is_empty(),
            "duplicate reference bodies leak answers across the split: {:?}",
            report.duplicate_groups
        );
        assert!(
            report.missing_added_at.is_empty(),
            "fixtures without `added_at` cannot be windowed: {:?}", report.missing_added_at
        );
    }

    #[test]
    fn duplicate_detection_normalizes_function_name_and_whitespace() {
        // 072 and 141 differed only by fn name; that must still be a duplicate.
        let a = "fn triangular(n: int) to int {\n    return n * (n + 1) / 2\n}\n";
        let b = "fn sum_to_n(n: int) to int { return n * (n + 1) / 2 }\n";
        assert_eq!(normalize_body(a), normalize_body(b));
    }

    #[test]
    fn duplicate_detection_does_not_collapse_genuinely_different_bodies() {
        let a = "fn f(n: int) to int { return n + 1 }\n";
        let b = "fn f(n: int) to int { return n + 2 }\n";
        assert_ne!(normalize_body(a), normalize_body(b));
    }
}
```

Register in `crates/vox-cli-ci/src/lib.rs` (alphabetical among the existing `pub mod` lines):

```rust
pub mod corpus_integrity;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli-ci corpus_integrity 2>&1 | tail -20`
Expected: FAIL — `cannot find function audit_corpus`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module:

```rust
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// What the corpus audit found. Empty vectors mean a clean corpus.
#[derive(Debug, Default)]
pub struct IntegrityReport {
    /// `"<id>: manifest=<bool> spec=<bool>"` for each disagreeing fixture.
    pub split_mismatches: Vec<String>,
    /// Groups of fixture ids whose reference bodies are identical modulo name.
    pub duplicate_groups: Vec<Vec<String>>,
    /// Fixture ids with no `added_at` in the manifest.
    pub missing_added_at: Vec<String>,
}

/// Canonical form of a reference body for duplicate detection: function names
/// erased, whitespace collapsed. Two fixtures that differ only by name are the
/// same problem, and if they straddle the split the held-out answer is leaked.
#[must_use]
pub fn normalize_body(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut prev_space = false;
    let stripped = src.replace("fn ", "\u{1}");
    for (i, ch) in stripped.char_indices() {
        // Drop the identifier immediately following each erased `fn`.
        if stripped.as_bytes()[i] == 1 {
            out.push_str("fn NAME");
            continue;
        }
        if ch.is_whitespace() {
            if !prev_space { out.push(' '); prev_space = true; }
        } else if out.ends_with("fn NAME") && (ch.is_alphanumeric() || ch == '_') {
            // skip the original name's characters
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Audit the corpus at `corpus_root` for split drift, answer-leaking
/// duplicates, and missing window dates.
pub fn audit_corpus(corpus_root: &Path) -> Result<IntegrityReport> {
    let manifest_path = corpus_root.join("manifest.v1.yaml");
    let raw = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("reading {}", manifest_path.display()))?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&raw)?;
    let entries = doc.get("fixtures").and_then(|f| f.as_sequence())
        .context("manifest has no `fixtures:` sequence")?;

    let mut report = IntegrityReport::default();
    let mut bodies: HashMap<String, Vec<(String, bool)>> = HashMap::new();

    for e in entries {
        let id = e.get("id").and_then(|v| v.as_str()).context("fixture missing id")?.to_string();
        let slug = e.get("slug").and_then(|v| v.as_str()).context("fixture missing slug")?;
        let man_elig = e.get("training_eligible").and_then(serde_yaml::Value::as_bool)
            .with_context(|| format!("fixture {id} missing training_eligible"))?;

        if e.get("added_at").and_then(|v| v.as_str()).is_none() {
            report.missing_added_at.push(id.clone());
        }

        let spec_path = corpus_root.join(format!("problems/{slug}.spec.toml"));
        let spec_raw = std::fs::read_to_string(&spec_path)
            .with_context(|| format!("reading {}", spec_path.display()))?;
        let spec_elig = !spec_raw.contains("training_eligible = false");
        if spec_elig != man_elig {
            report.split_mismatches.push(format!("{id}: manifest={man_elig} spec={spec_elig}"));
        }

        let ref_path = corpus_root.join(format!("problems/{slug}/reference.vox"));
        if let Ok(body) = std::fs::read_to_string(&ref_path) {
            bodies.entry(normalize_body(&body)).or_default().push((id.clone(), man_elig));
        }
    }

    for (_, group) in bodies {
        if group.len() > 1 {
            report.duplicate_groups.push(
                group.iter().map(|(id, e)| format!("{id}({})", if *e { "eligible" } else { "HELD-OUT" })).collect()
            );
        }
    }
    Ok(report)
}

/// CI entry point. Fails the build on any inconsistency.
pub fn run(repo_root: &Path) -> Result<()> {
    let report = audit_corpus(&repo_root.join("contracts/eval/humaneval-vox"))?;
    let mut failures = Vec::new();
    for m in &report.split_mismatches { failures.push(format!("split mismatch: {m}")); }
    for g in &report.duplicate_groups { failures.push(format!("duplicate reference bodies: {g:?}")); }
    for m in &report.missing_added_at { failures.push(format!("missing added_at: {m}")); }
    if !failures.is_empty() {
        for f in &failures { eprintln!("{f}"); }
        anyhow::bail!("corpus-integrity: {} problem(s) — the held-out split is not trustworthy", failures.len());
    }
    println!("corpus-integrity OK");
    Ok(())
}
```

- [ ] **Step 4: Run the gate against the real corpus (it MUST fail now)**

Run: `cargo test -p vox-cli-ci corpus_integrity 2>&1 | tail -30`
Expected: the two unit tests PASS; `real_corpus_has_no_split_mismatch...` **FAILS**, reporting 21 split mismatches, ≥4 duplicate groups, and 164 missing `added_at`. That failure is the point — it is the audit's C7 reproduced as a gate.

- [ ] **Step 5: Commit the gate (red), before fixing the data**

```bash
cargo fmt -p vox-cli-ci
git add crates/vox-cli-ci/src/corpus_integrity.rs crates/vox-cli-ci/src/lib.rs
git commit -m "test(corpus): add corpus-integrity gate; currently RED on split drift and duplicates"
```

---

### Task 2: Reconcile the split and backfill `added_at`

**Files:** Modify `contracts/eval/humaneval-vox/manifest.v1.yaml`, the 21 disputed `spec.toml` files, and the held-out `.vox` files.

- [ ] **Step 1: Decide the SSOT and record it**

The manifest is authoritative — `crates/vox-audit/src/subcommands/humaneval.rs` reads it, and `held-out.v1.json` hashes it. Add this comment above `fixtures:` in the manifest:

```yaml
# SSOT for `training_eligible` is THIS FILE. Each fixture's spec.toml and its
# .vox files must agree; `vox ci corpus-integrity` enforces all three.
# `added_at` is the contamination window and is derived from git history
# (`git log --diff-filter=A`). NEVER back-date it — a fabricated window is
# worse than no window, because it looks like evidence.
```

- [ ] **Step 2: Fix the 21 disputed fixtures**

For each of ids 065–075, 098, 099, 100, 128, 148, 160, 161, 162, 163, 164, set `training_eligible = false` in `contracts/eval/humaneval-vox/problems/<slug>.spec.toml` to match the manifest.

- [ ] **Step 3: Break the duplicate leak**

Fixtures 072 (held-out) and 141 (training-eligible) have identical bodies (`n * (n + 1) / 2`). Duplicates 014/063, 020/134, 026/153 sit within the training-eligible set. For each group, keep one and **replace the other with a genuinely different problem** — do not merely rename, which leaves the answer leaked. Prefer replacing the *training-eligible* member so the held-out set keeps its size.

- [ ] **Step 4: Backfill `added_at` from git, never by hand**

```bash
cd contracts/eval/humaneval-vox
for spec in problems/*.spec.toml; do
  d=$(git log --diff-filter=A --format=%ad --date=short -1 -- "$spec")
  echo "$(basename "$spec" .spec.toml) $d"
done
```

Write each fixture's real addition date into its manifest entry as `added_at: "YYYY-MM-DD"`.

- [ ] **Step 5: Mark held-out `.vox` files so the extractor can see them**

The training-corpus extractor reads *file content*, not the manifest (`crates/vox-corpus/src/corpus/extract_vox/part_helpers.rs:259`). Prepend to `reference.vox` and `tests.vox` of every held-out fixture:

```vox
// training_eligible: false
```

This is defense in depth: today the extractor does not walk `contracts/eval/`, so the leak is latent — but nothing prevents a future walker from ingesting all 164 answer keys, and the manifest's designation is invisible to it.

- [ ] **Step 6: Verify the gate is green**

Run: `cargo test -p vox-cli-ci corpus_integrity 2>&1 | tail -20`
Expected: all three tests PASS.

- [ ] **Step 7: Wire into CI and commit**

Add `CorpusIntegrity` to the `vox ci` dispatch alongside the other `vox-cli-ci` guards, then:

```bash
cargo run -q -p vox-cli -- ci corpus-integrity
cargo run -q -p vox-cli -- ci command-sync --write
git add contracts/eval/humaneval-vox/ crates/vox-cli/ contracts/cli/command-registry.yaml
git commit -m "fix(corpus): reconcile held-out split, break duplicate answer leak, backfill added_at"
```

---

# Phase A — Anti-cheat spine (closes C1)

**Nothing may score before this exists.** A four-line candidate currently scores 100% on every fixture.

### Task 3: Canary-based oracle-neutralization detection

The exploit works by rebinding `assert`. Rather than guess at forms — a top-level `fn assert` does *not* neutralize, a `let`-bound closure does, and arity/return-type variants also do — detect the *effect*: run the candidate against an assertion that must fail. If it passes, the oracle is neutralized.

**Files:** Create `crates/vox-corpus/src/humaneval_runner/canary.rs`, `crates/vox-corpus/src/humaneval_runner/mod.rs`; modify `crates/vox-corpus/src/lib.rs`.

**Interfaces:**
- Produces: `pub fn canary_program(candidate: &str) -> String`; `pub fn is_oracle_neutralized(vox_bin: &Path, candidate: &str, workdir: &Path, timeout: Duration) -> anyhow::Result<bool>`; `pub fn rejects_at_ingest(candidate: &str) -> Option<String>`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-corpus/src/humaneval_runner/mod.rs`:

```rust
//! HumanEval-Vox runner: load fixtures, compose runnable programs, detect
//! oracle neutralization, and verify by compiler/test exit code.

pub mod canary;
pub mod compose;
pub mod conditions;
pub mod manifest;
pub mod verify;

pub use canary::{canary_program, is_oracle_neutralized, rejects_at_ingest};
pub use compose::compose_program;
pub use manifest::{Fixture, held_out, load_corpus};
pub use verify::{VerifyOutcome, verify_program};
```

Create `crates/vox-corpus/src/humaneval_runner/canary.rs` with only the doc comment and tests:

```rust
//! Detection of candidates that neutralize the scoring oracle.
//!
//! A candidate can rebind `assert` so every fixture assertion becomes a no-op,
//! scoring 100% with a wrong answer. Empirically (2026-09-01, vox 0.6.0):
//! a top-level `fn assert(...)` does NOT neutralize — the builtin wins — but
//! `let assert = fn(c: bool) to bool { return true }` DOES, as do arity and
//! return-type variants. Enumerating forms is therefore a losing game.
//!
//! Instead we detect the effect: run the candidate against `assert(false)`.
//! An honest candidate fails it; a neutralized one passes. This is exact,
//! form-independent, and costs one short subprocess per candidate.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canary_program_appends_a_must_fail_assertion() {
        let p = canary_program("fn f() to int { return 1 }");
        assert!(p.contains("fn f() to int"), "candidate preserved");
        assert!(p.contains("assert(false)"), "canary assertion present");
        assert_eq!(p.matches("fn main").count(), 1, "exactly one main");
    }

    #[test]
    fn ingest_rejects_rebinding_the_oracle() {
        // The cheap first line of defence: refuse obvious rebinding outright.
        assert!(rejects_at_ingest("let assert = fn(c: bool) to bool { return true }").is_some());
        assert!(rejects_at_ingest("  let  assert  =  fn(c: bool) to bool { return true }").is_some());
        assert!(rejects_at_ingest("fn assert(c: bool) to bool { return true }").is_some());
        assert!(rejects_at_ingest("fn main() to str { return \"x\" }").is_some());
        assert!(rejects_at_ingest("let print = fn(s: str) to bool { return true }").is_some());
    }

    #[test]
    fn ingest_allows_honest_solutions_including_similar_names() {
        assert!(rejects_at_ingest("fn nth_prime(n: int) to int { return 2 }").is_none());
        // Must not false-positive on identifiers that merely contain the words.
        assert!(rejects_at_ingest("let asserted = 1").is_none());
        assert!(rejects_at_ingest("fn assertion_count() to int { return 0 }").is_none());
        assert!(rejects_at_ingest("let mainline = 3").is_none());
    }
}
```

Add to `crates/vox-corpus/src/lib.rs`:

```rust
pub mod humaneval_runner;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus humaneval_runner::canary 2>&1 | tail -20`
Expected: FAIL — `cannot find function canary_program` / `rejects_at_ingest`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module:

```rust
use anyhow::Result;
use std::path::Path;
use std::time::Duration;

/// Identifiers a candidate may never bind: the scoring oracle and the entry
/// point the fixture supplies.
const RESERVED: &[&str] = &["assert", "main", "print"];

/// Cheap first-line rejection of a candidate that rebinds a reserved name.
///
/// Returns `Some(reason)` when the candidate must be refused before it is ever
/// run. Word-boundary matched so `asserted` / `assertion_count` / `mainline`
/// are unaffected.
#[must_use]
pub fn rejects_at_ingest(candidate: &str) -> Option<String> {
    for line in candidate.lines() {
        let t = line.trim_start();
        for kw in ["let ", "fn "] {
            if let Some(rest) = t.strip_prefix(kw) {
                let rest = rest.trim_start().trim_start_matches("mut ");
                let name: String =
                    rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                if RESERVED.contains(&name.as_str()) {
                    return Some(format!("candidate rebinds reserved name `{name}`"));
                }
            }
        }
    }
    None
}

/// The candidate plus a `main` whose single assertion must fail.
#[must_use]
pub fn canary_program(candidate: &str) -> String {
    format!("{}\n\nfn main() to str {{\n    assert(false)\n    return \"ok\"\n}}\n", candidate.trim_end())
}

/// True when the candidate neutralized the oracle: `assert(false)` passed.
///
/// A compile failure is NOT neutralization — it returns `false`, and the normal
/// verification path records the compile error. Only a clean exit 0 on a
/// must-fail assertion indicates cheating.
pub fn is_oracle_neutralized(
    vox_bin: &Path,
    candidate: &str,
    workdir: &Path,
    timeout: Duration,
) -> Result<bool> {
    let outcome = super::verify::run_program(
        vox_bin, &canary_program(candidate), workdir, "canary", timeout,
    )?;
    Ok(outcome.compiled && outcome.ran_ok)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus humaneval_runner::canary 2>&1 | tail -20`
Expected: PASS — 3 tests. (`is_oracle_neutralized` is covered by the integration test in Task 5.)

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/ crates/vox-corpus/src/lib.rs
git commit -m "feat(corpus): canary-based oracle-neutralization detection and ingest rejection"
```

---

# Phase B — Verification I/O (closes C6)

### Task 4: File-redirected subprocess verification

v1 piped stdout/stderr without draining them. Measured: 163,149 bytes of diagnostics from a cascading-type-error file versus an ~8 KiB Windows pipe buffer — the child blocks, the harness times out, and a compile error is misreported as a timeout. v1 also read `stderr` for the failure detail while `vox check` writes diagnostics to **stdout**.

**Files:** Create `crates/vox-corpus/src/humaneval_runner/verify.rs`.

**Interfaces:**
- Produces: `pub struct RunOutcome { pub compiled: bool, pub ran_ok: bool, pub detail: String }`; `pub fn run_program(vox_bin: &Path, program: &str, workdir: &Path, tag: &str, timeout: Duration) -> anyhow::Result<RunOutcome>`; `pub struct VerifyOutcome { pub compiled: bool, pub tests_passed: bool, pub cheated: bool, pub detail: String }`; `pub fn verify_program(...) -> anyhow::Result<VerifyOutcome>`.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-corpus/src/humaneval_runner/verify.rs` with only the doc comment and tests:

```rust
//! Subprocess verification: `vox check` then `vox run --mode interp`.
//!
//! Output is redirected to FILES, never pipes. A cascading type error produces
//! ~163 KB of diagnostics (measured 2026-09-01) against an ~8 KiB Windows
//! anonymous-pipe buffer; an undrained pipe deadlocks the child and the
//! harness reports a timeout instead of the compile error that actually
//! happened. Files have no such limit and need no reader thread.
//!
//! `vox check` writes diagnostics to STDOUT, so `detail` reads stdout first.

#[cfg(test)]
mod tests {
    use super::*;

    fn vox_bin() -> Option<std::path::PathBuf> {
        for p in ["target/release/vox.exe", "target/release/vox",
                  "target/debug/vox.exe", "target/debug/vox"] {
            let c = std::path::Path::new(p);
            if c.exists() { return Some(c.to_path_buf()); }
        }
        None
    }

    fn workdir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vox-verify-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn large_diagnostic_output_does_not_deadlock() {
        let Some(bin) = vox_bin() else { eprintln!("skip: no vox binary"); return; };
        // ~800 type errors: far past any pipe buffer.
        let mut src = String::from("fn g(xs: list[int]) to int {\n");
        for i in 0..800 { src.push_str(&format!("    let v{i}: int = xs[{i}]\n")); }
        src.push_str("    return 0\n}\nfn main() to str { return \"ok\" }\n");
        let d = workdir("bigdiag");
        let out = run_program(&bin, &src, &d, "big", std::time::Duration::from_secs(60))
            .expect("must return an outcome, not hang");
        assert!(!out.compiled, "this program does not compile");
        assert!(
            !out.detail.contains("timed out"),
            "a compile error must be reported as such, not as a timeout: {}", out.detail
        );
        assert!(out.detail.contains("error"), "detail must carry the real diagnostic: {}", out.detail);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn honest_and_cheating_candidates_are_distinguished() {
        let Some(bin) = vox_bin() else { eprintln!("skip: no vox binary"); return; };
        let d = workdir("cheat");
        let tests_main = "fn main() to str {\n    assert(nth_prime(1) == 2)\n    return \"ok\"\n}\n";
        let t = std::time::Duration::from_secs(30);

        let honest = verify_program(&bin, "fn nth_prime(n: int) to int { return 2 }", tests_main, &d, t).unwrap();
        assert!(honest.compiled && honest.tests_passed && !honest.cheated);

        let wrong = verify_program(&bin, "fn nth_prime(n: int) to int { return 0 }", tests_main, &d, t).unwrap();
        assert!(wrong.compiled && !wrong.tests_passed && !wrong.cheated);

        // The exploit: wrong answer + rebound oracle. Must be caught as cheating,
        // NOT recorded as a pass.
        let cheat = verify_program(
            &bin,
            "let assert = fn(c: bool) to bool { return true }\nfn nth_prime(n: int) to int { return 0 }",
            tests_main, &d, t,
        ).unwrap();
        assert!(cheat.cheated, "oracle rebinding must be detected");
        assert!(!cheat.tests_passed, "a cheating candidate must never score a pass");
        std::fs::remove_dir_all(&d).ok();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus humaneval_runner::verify 2>&1 | tail -20`
Expected: FAIL — `cannot find function run_program`.

- [ ] **Step 3: Write minimal implementation**

Insert above the test module:

```rust
use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Result of compiling and running one program.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// `vox check` exited 0.
    pub compiled: bool,
    /// `vox run --mode interp` exited 0.
    pub ran_ok: bool,
    /// First meaningful diagnostic line, or a timeout note.
    pub detail: String,
}

/// Verdict for one candidate against one fixture.
#[derive(Debug, Clone)]
pub struct VerifyOutcome {
    pub compiled: bool,
    pub tests_passed: bool,
    /// The candidate neutralized the scoring oracle.
    pub cheated: bool,
    pub detail: String,
}

/// Compile and run `program`, capturing output to files under `workdir`.
///
/// `tag` disambiguates the on-disk filenames so concurrent verifications never
/// collide — v1 reused one `candidate.vox` for every fixture, which is safe
/// sequentially and silently wrong the moment anything is parallelised.
pub fn run_program(
    vox_bin: &Path, program: &str, workdir: &Path, tag: &str, timeout: Duration,
) -> Result<RunOutcome> {
    std::fs::create_dir_all(workdir)?;
    let src = workdir.join(format!("{tag}.vox"));
    std::fs::write(&src, program)?;

    let check = exec(vox_bin, &["check", &src.to_string_lossy()], workdir, &format!("{tag}.check"), timeout)?;
    if !check.success {
        return Ok(RunOutcome { compiled: false, ran_ok: false, detail: check.detail });
    }
    let run = exec(vox_bin, &["run", "--mode", "interp", &src.to_string_lossy()], workdir, &format!("{tag}.run"), timeout)?;
    Ok(RunOutcome { compiled: true, ran_ok: run.success, detail: if run.success { String::new() } else { run.detail } })
}

/// Full verification: reject rebinding at ingest, prove the oracle is live via
/// the canary, then score against the fixture's real assertions.
pub fn verify_program(
    vox_bin: &Path, candidate: &str, tests_main: &str, workdir: &Path, timeout: Duration,
) -> Result<VerifyOutcome> {
    if let Some(reason) = super::canary::rejects_at_ingest(candidate) {
        return Ok(VerifyOutcome { compiled: false, tests_passed: false, cheated: true, detail: reason });
    }
    if super::canary::is_oracle_neutralized(vox_bin, candidate, workdir, timeout)? {
        return Ok(VerifyOutcome {
            compiled: true, tests_passed: false, cheated: true,
            detail: "candidate neutralized the scoring oracle (canary assert(false) passed)".to_string(),
        });
    }
    let program = super::compose::compose_program(candidate, tests_main)?;
    let out = run_program(vox_bin, &program, workdir, "candidate", timeout)?;
    Ok(VerifyOutcome {
        compiled: out.compiled, tests_passed: out.compiled && out.ran_ok, cheated: false, detail: out.detail,
    })
}

struct Exec { success: bool, detail: String }

/// Spawn with stdout/stderr redirected to files (never pipes), killing the
/// child if it outlives `timeout`.
fn exec(vox_bin: &Path, args: &[&str], workdir: &Path, tag: &str, timeout: Duration) -> Result<Exec> {
    let out_path = workdir.join(format!("{tag}.out"));
    let err_path = workdir.join(format!("{tag}.err"));
    let mut child = Command::new(vox_bin)
        .args(args)
        .stdout(Stdio::from(std::fs::File::create(&out_path)?))
        .stderr(Stdio::from(std::fs::File::create(&err_path)?))
        .stdin(Stdio::null())
        .spawn()
        .with_context(|| format!(
            "spawning {} — build it first: cargo build -p vox-cli --release (add `-j 2` if \
             rustc dies with an allocation failure)", vox_bin.display()))?;

    let started = Instant::now();
    loop {
        match child.try_wait()? {
            Some(_) => break,
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(Exec { success: false, detail: format!("timed out after {}s", timeout.as_secs()) });
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let status = child.wait()?;
    // `vox check` writes diagnostics to stdout; fall back to stderr.
    let detail = first_line(&std::fs::read_to_string(&out_path).unwrap_or_default())
        .or_else(|| first_line(&std::fs::read_to_string(&err_path).unwrap_or_default()))
        .unwrap_or_else(|| "failed with no diagnostic output".to_string());
    Ok(Exec { success: status.success(), detail })
}

fn first_line(text: &str) -> Option<String> {
    text.lines().map(str::trim).find(|l| !l.is_empty())
        .map(|l| l.chars().take(200).collect())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus humaneval_runner 2>&1 | tail -20`
Expected: PASS. The deadlock test is the regression guard for C6; the cheat test is the regression guard for C1.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/verify.rs
git commit -m "feat(corpus): file-redirected verification with canary cheat detection"
```

---

### Task 5: Brace-aware composition (closes H3)

v1 cut the candidate at its first `\nfn main(` **to EOF**, deleting any helper written after a demo `main`, and deleted the entire solution when `main` came first.

**Files:** Create `crates/vox-corpus/src/humaneval_runner/compose.rs`.

**Interfaces:** `pub fn extract_test_block(tests_source: &str) -> anyhow::Result<String>`; `pub fn strip_candidate_main(candidate: &str) -> String`; `pub fn compose_program(candidate: &str, tests_source: &str) -> anyhow::Result<String>`.

- [ ] **Step 1: Write the failing test**

```rust
//! Compose a runnable program from a candidate plus a fixture's assertions.
//!
//! Verified across all 164 fixtures (2026-09-01): every `tests.vox` has exactly
//! one `fn main(`, always last, with no declarations after it — so a suffix cut
//! is exact for the corpus. The candidate side is NOT corpus-shaped, so its
//! `main` is excised by brace matching rather than a suffix cut.

#[cfg(test)]
mod tests {
    use super::*;

    const TESTS: &str = "fn f(n: int) to int {\n    return 0\n}\n\nfn main() to str {\n    assert(f(1) == 2)\n    return \"ok\"\n}\n";

    #[test]
    fn extract_takes_main_and_drops_the_reference() {
        let b = extract_test_block(TESTS).unwrap();
        assert!(b.starts_with("fn main() to str {"));
        assert!(b.contains("assert(f(1) == 2)"));
        assert!(!b.contains("return 0"), "reference body must not survive");
    }

    #[test]
    fn extract_fails_closed_without_a_main() {
        assert!(extract_test_block("fn helper() to int { return 1 }").is_err());
    }

    #[test]
    fn strip_keeps_helpers_written_after_a_demo_main() {
        // v1 deleted `helper` here, turning a correct solution into a compile error.
        let c = "fn f() to int { return helper() }\n\nfn main() to str {\n    return \"demo\"\n}\n\nfn helper() to int {\n    return 7\n}\n";
        let s = strip_candidate_main(c);
        assert!(s.contains("fn f() to int"), "solution kept");
        assert!(s.contains("fn helper() to int"), "helper after main MUST survive");
        assert!(!s.contains("fn main"), "demo main removed");
    }

    #[test]
    fn strip_keeps_the_solution_when_main_comes_first() {
        // v1 returned an empty string here — a guaranteed 0 for the fixture.
        let c = "fn main() to str {\n    return \"demo\"\n}\n\nfn f() to int {\n    return 1\n}\n";
        let s = strip_candidate_main(c);
        assert!(s.contains("fn f() to int"), "solution after a leading main MUST survive");
        assert!(!s.contains("fn main"));
    }

    #[test]
    fn strip_is_a_noop_without_a_main() {
        let c = "fn f() to int { return 1 }\n";
        assert_eq!(strip_candidate_main(c).trim(), c.trim());
    }

    #[test]
    fn compose_yields_exactly_one_main() {
        let p = compose_program("fn f(n: int) to int { return 2 }", TESTS).unwrap();
        assert_eq!(p.matches("fn main").count(), 1);
        assert!(p.contains("assert(f(1) == 2)"));
        assert!(!p.contains("return 0"), "reference body never reaches the program");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus humaneval_runner::compose 2>&1 | tail -20`
Expected: FAIL — functions not found.

- [ ] **Step 3: Write minimal implementation**

```rust
use anyhow::{Result, bail};

/// The fixture's assertion block: from its `fn main(` line to EOF.
pub fn extract_test_block(tests_source: &str) -> Result<String> {
    if tests_source.starts_with("fn main(") { return Ok(tests_source.to_string()); }
    match tests_source.find("\nfn main(") {
        Some(i) => Ok(tests_source[i + 1..].to_string()),
        None => bail!("fixture tests.vox has no `fn main(` block — refusing to grade against an empty test"),
    }
}

/// Remove only the candidate's own `fn main` block, preserving everything else.
///
/// Brace-matched (ignoring braces inside strings and line comments) rather than
/// cut-to-EOF, because helpers routinely follow a model's demo `main`.
#[must_use]
pub fn strip_candidate_main(candidate: &str) -> String {
    let Some(start) = find_main_start(candidate) else { return candidate.to_string() };
    let Some(open) = candidate[start..].find('{').map(|i| start + i) else {
        return candidate[..start].trim_end().to_string();
    };
    match match_brace(candidate, open) {
        Some(end) => format!("{}{}", &candidate[..start], &candidate[end + 1..])
            .lines().collect::<Vec<_>>().join("\n").trim().to_string(),
        None => candidate[..start].trim_end().to_string(),
    }
}

fn find_main_start(s: &str) -> Option<usize> {
    if s.starts_with("fn main(") { return Some(0); }
    s.find("\nfn main(").map(|i| i + 1)
}

/// Index of the `}` closing the `{` at `open`, skipping strings and `//` comments.
fn match_brace(s: &str, open: usize) -> Option<usize> {
    let b = s.as_bytes();
    let (mut depth, mut i) = (0i32, open);
    let (mut in_str, mut in_cmt) = (false, false);
    while i < b.len() {
        let c = b[i] as char;
        if in_cmt { if c == '\n' { in_cmt = false; } }
        else if in_str {
            if c == '\\' { i += 1; } else if c == '"' { in_str = false; }
        } else if c == '"' { in_str = true; }
        else if c == '/' && i + 1 < b.len() && b[i + 1] == b'/' { in_cmt = true; }
        else if c == '{' { depth += 1; }
        else if c == '}' { depth -= 1; if depth == 0 { return Some(i); } }
        i += 1;
    }
    None
}

/// Candidate body plus the fixture's assertion block.
pub fn compose_program(candidate: &str, tests_source: &str) -> Result<String> {
    let body = strip_candidate_main(candidate);
    Ok(format!("{}\n\n{}", body.trim_end(), extract_test_block(tests_source)?.trim_start()))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus humaneval_runner::compose 2>&1 | tail -20`
Expected: PASS — 6 tests.

- [ ] **Step 5: Oracle sweep — the harness's own false-negative rate**

Compose and verify **all 164** reference solutions. Any failure is a harness bug, not a model result. v1 smoke-tested one fixture; this establishes the rate.

```bash
cargo test -p vox-corpus --release oracle_sweep -- --ignored --nocapture
```

Add this test to `compose.rs`:

```rust
    /// Every reference solution must pass its own fixture. A failure here means
    /// the harness is broken; without this the benchmark's false-negative rate
    /// is unknown. `--ignored` because it shells out to the toolchain 164 times.
    #[test]
    #[ignore = "requires a built vox binary; run explicitly"]
    fn oracle_sweep_all_164_references_pass() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        let corpus = root.join("contracts/eval/humaneval-vox");
        let fixtures = crate::humaneval_runner::load_corpus(&corpus).expect("corpus loads");
        let bin = ["target/release/vox.exe", "target/debug/vox.exe", "target/release/vox", "target/debug/vox"]
            .iter().map(|p| root.join(p)).find(|p| p.exists()).expect("build vox first");
        let d = std::env::temp_dir().join(format!("vox-oracle-{}", std::process::id()));
        let mut failures = Vec::new();
        for f in &fixtures {
            let tests = std::fs::read_to_string(&f.tests_path).unwrap();
            let reference = std::fs::read_to_string(f.tests_path.parent().unwrap().join("reference.vox")).unwrap();
            let out = crate::humaneval_runner::verify_program(
                &bin, &reference, &tests, &d, std::time::Duration::from_secs(30)).unwrap();
            if !out.tests_passed { failures.push(format!("{}: {}", f.id, out.detail)); }
        }
        std::fs::remove_dir_all(&d).ok();
        assert!(failures.is_empty(), "{} reference solution(s) failed the harness: {:?}", failures.len(), failures);
    }
```

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/compose.rs
git commit -m "feat(corpus): brace-aware composition + 164-fixture oracle sweep"
```

---

# Phase C — Literature-standard statistics (closes C2, C3, C4, C5)

### Task 6: Unbiased pass@k

**Files:** Create `crates/vox-eval/src/corpus_score.rs`; modify `crates/vox-eval/src/lib.rs`.

**Interfaces:** `pub struct AttemptOutcome`; `pub struct FixtureOutcome { pub fixture_id: String, pub n: usize, pub c: usize, pub attempts: Vec<AttemptOutcome> }`; `pub fn pass_at_k(n: usize, c: usize, k: usize) -> f64`; `pub fn corpus_pass_at_k(outcomes: &[FixtureOutcome], k: usize) -> f64`; `pub struct CorpusScore`; `pub fn score_corpus(outcomes: &[FixtureOutcome], k: usize) -> CorpusScore`.

- [ ] **Step 1: Write the failing test**

```rust
//! Unbiased pass@k over per-problem (n, c), per Chen et al. 2021 (arXiv 2107.03374).
//!
//! v1 computed "any attempt passed" with k derived from the data. That is
//! degenerate: at n=k it returns 1.000 for any problem with one success, and a
//! strong model reported k=1 while a weak one reported k=5 — then both were
//! sorted into one column. `k` is a CONFIG input here, never derived.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_at_k_matches_the_closed_form_and_is_stable_at_scale() {
        // Product form must equal 1 - C(n-c,k)/C(n,k).
        fn closed(n: u64, c: u64, k: u64) -> f64 {
            fn comb(n: u64, k: u64) -> f64 {
                if k > n { return 0.0; }
                (0..k).map(|i| (n - i) as f64 / (i + 1) as f64).product()
            }
            if n - c < k { 1.0 } else { 1.0 - comb(n - c, k) / comb(n, k) }
        }
        for n in 1..=30u64 { for c in 0..=n { for k in 1..=n {
            let (a, b) = (closed(n, c, k), pass_at_k(n as usize, c as usize, k as usize));
            assert!((a - b).abs() < 1e-9, "n={n} c={c} k={k}: {a} vs {b}");
        }}}
        assert!(pass_at_k(200, 100, 100).is_finite(), "must not overflow at literature scale");
    }

    #[test]
    fn pass_at_1_equals_the_empirical_rate() {
        assert!((pass_at_k(10, 5, 1) - 0.5).abs() < 1e-9);
        assert_eq!(pass_at_k(10, 0, 1), 0.0);
        assert_eq!(pass_at_k(10, 10, 1), 1.0);
    }

    #[test]
    fn n_equals_k_is_degenerate_which_is_why_k_must_be_config_driven() {
        // The v1 bug, pinned as a regression: at n=k any success scores 1.0.
        for n in [5, 10, 20] { assert_eq!(pass_at_k(n, 1, n), 1.0); }
        // With k=1 the same data is informative.
        assert!((pass_at_k(20, 1, 1) - 0.05).abs() < 1e-9);
    }

    #[test]
    fn corpus_pass_at_k_averages_over_problems() {
        let o = vec![
            FixtureOutcome { fixture_id: "a".into(), n: 10, c: 10, attempts: vec![] },
            FixtureOutcome { fixture_id: "b".into(), n: 10, c: 0, attempts: vec![] },
        ];
        assert!((corpus_pass_at_k(&o, 1) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn score_corpus_panics_when_k_exceeds_samples() {
        let o = vec![FixtureOutcome { fixture_id: "a".into(), n: 1, c: 1, attempts: vec![] }];
        assert!(std::panic::catch_unwind(|| corpus_pass_at_k(&o, 5)).is_err(),
            "k > n must fail loudly, not silently report a wrong number");
    }
}
```

Add `pub mod corpus_score;` to `crates/vox-eval/src/lib.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-eval corpus_score 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
use serde::{Deserialize, Serialize};

/// One generation attempt. `compiled`/`tests_passed` are exit-code facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptOutcome {
    pub compiled: bool,
    pub tests_passed: bool,
    pub cheated: bool,
    pub total_tokens: u32,
    pub latency_ms: i64,
    pub cost_usd: Option<f64>,
}

/// All attempts at one fixture. `n` samples drawn, `c` correct — both required
/// by the unbiased estimator, so attempts are NEVER stopped early.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureOutcome {
    pub fixture_id: String,
    pub n: usize,
    pub c: usize,
    pub attempts: Vec<AttemptOutcome>,
}

/// Unbiased pass@k for one problem (Chen et al. 2021).
///
/// Product form: the closed form `1 - C(n-c,k)/C(n,k)` loses precision and
/// overflows at literature scales (n=200, k=100).
#[must_use]
pub fn pass_at_k(n: usize, c: usize, k: usize) -> f64 {
    assert!(n >= k, "pass@k requires n >= k; got n={n}, k={k}");
    if n - c < k { return 1.0; }
    let mut prod = 1.0f64;
    for j in (n - c + 1)..=n { prod *= 1.0 - (k as f64) / (j as f64); }
    1.0 - prod
}

/// Corpus pass@k: the mean of per-problem unbiased estimates.
#[must_use]
pub fn corpus_pass_at_k(outcomes: &[FixtureOutcome], k: usize) -> f64 {
    if outcomes.is_empty() { return 0.0; }
    outcomes.iter().map(|f| pass_at_k(f.n, f.c, k)).sum::<f64>() / outcomes.len() as f64
}

/// Published axes for one (model, harness, condition) row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusScore {
    pub n_fixtures: usize,
    pub k: usize,
    pub pass_at_1: f64,
    pub pass_at_k: f64,
    pub compile_rate: f64,
    pub n_cheated: usize,
    pub n_infra_errors: usize,
    /// `None` when this row's solutions were ingested rather than generated —
    /// never 0, which would publish a false "used no tokens" claim.
    pub total_tokens: Option<u64>,
    pub tokens_per_pass: Option<f64>,
    pub p50_ms: Option<i64>,
    pub cumulative_cost_usd: Option<f64>,
    pub cost_per_success_usd: Option<f64>,
    /// Per-problem first-attempt outcomes, for the paired tests in `corpus_stats`.
    pub per_problem_pass_at_1: Vec<bool>,
}

/// Fold outcomes at a caller-supplied `k`. `measured` says whether generation
/// metrics exist for this row.
#[must_use]
pub fn score_corpus(outcomes: &[FixtureOutcome], k: usize, measured: bool) -> CorpusScore {
    let n_fixtures = outcomes.len();
    let frac = |x: usize| if n_fixtures == 0 { 0.0 } else { x as f64 / n_fixtures as f64 };
    let first = |f: &FixtureOutcome| f.attempts.first();

    let n_compiled = outcomes.iter().filter(|f| first(f).is_some_and(|a| a.compiled)).count();
    let n_cheated = outcomes.iter().filter(|f| f.attempts.iter().any(|a| a.cheated)).count();
    let all: Vec<&AttemptOutcome> = outcomes.iter().flat_map(|f| f.attempts.iter()).collect();

    let (total_tokens, tokens_per_pass, p50_ms, cum_cost, cost_per_success) = if measured {
        let tt: u64 = all.iter().map(|a| a.total_tokens as u64).sum();
        let passes = outcomes.iter().filter(|f| f.c > 0).count().max(1);
        let mut lat: Vec<i64> = all.iter().map(|a| a.latency_ms).collect();
        lat.sort_unstable();
        let p50 = if lat.is_empty() { 0 } else { lat[(0.5 * lat.len() as f64).ceil().max(1.0) as usize - 1] };
        let cost: f64 = all.iter().filter_map(|a| a.cost_usd).sum();
        let known = all.iter().any(|a| a.cost_usd.is_some());
        (Some(tt), Some(tt as f64 / passes as f64), Some(p50),
         known.then_some(cost), (known && passes > 0).then(|| cost / passes as f64))
    } else { (None, None, None, None, None) };

    CorpusScore {
        n_fixtures,
        k,
        pass_at_1: corpus_pass_at_k(outcomes, 1),
        pass_at_k: corpus_pass_at_k(outcomes, k),
        compile_rate: frac(n_compiled),
        n_cheated,
        n_infra_errors: 0,
        total_tokens, tokens_per_pass, p50_ms,
        cumulative_cost_usd: cum_cost,
        cost_per_success_usd: cost_per_success,
        per_problem_pass_at_1: outcomes.iter()
            .map(|f| first(f).is_some_and(|a| a.tests_passed && !a.cheated)).collect(),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-eval corpus_score 2>&1 | tail -20`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-eval
git add crates/vox-eval/src/corpus_score.rs crates/vox-eval/src/lib.rs
git commit -m "feat(vox-eval): unbiased pass@k with config-driven k and nullable metrics"
```

---

### Task 7: Paired significance, multiplicity correction, and the resolution floor

**Files:** Create `crates/vox-eval/src/corpus_stats.rs`; modify `crates/vox-eval/src/lib.rs`.

**Interfaces:** `pub fn mcnemar_exact_p(b: usize, c: usize) -> f64`; `pub fn paired_compare(a: &[bool], b: &[bool]) -> PairedResult`; `pub fn holm_reject(p_values: &[f64], alpha: f64) -> Vec<bool>`; `pub fn bootstrap_ci(per_problem: &[f64], reps: usize, seed: u64) -> (f64, f64)`; `pub fn wilson_interval(successes: usize, trials: usize, z: f64) -> ConfidenceInterval`; `pub fn min_detectable_difference(n: usize) -> f64`.

- [ ] **Step 1: Write the failing test**

```rust
//! Paired significance testing for benchmark comparisons.
//!
//! Every model attempts the SAME fixtures, so comparisons are paired binary
//! outcomes and the correct test is McNemar's — exact, since discordant pairs
//! are few at this corpus size. v1 declared significance iff two Wilson
//! intervals failed to overlap, which fires at an effective alpha near 0.005
//! (Cumming 2009) and has roughly half the efficiency of the correct test
//! (Schenker & Gentleman 2001), i.e. systematic false negatives.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcnemar_is_symmetric_and_bounded() {
        assert!((mcnemar_exact_p(5, 5) - 1.0).abs() < 1e-9, "no asymmetry -> p=1");
        assert_eq!(mcnemar_exact_p(0, 0), 1.0, "no discordant pairs -> p=1");
        assert!((mcnemar_exact_p(8, 1) - mcnemar_exact_p(1, 8)).abs() < 1e-12, "symmetric");
        for (b, c) in [(0, 10), (3, 7), (10, 0)] {
            let p = mcnemar_exact_p(b, c);
            assert!((0.0..=1.0).contains(&p), "p out of range for b={b} c={c}: {p}");
        }
    }

    #[test]
    fn mcnemar_detects_a_lopsided_difference_that_ci_overlap_would_miss() {
        // 10 discordant pairs all favouring one model: unambiguous.
        assert!(mcnemar_exact_p(10, 0) < 0.005);
        // The v1 example: 24/31 vs 26/31. Marginal CIs overlap heavily, but if
        // the disagreement is one-sided the paired test resolves it.
        assert!(mcnemar_exact_p(0, 2) > 0.05, "2 discordant pairs cannot resolve anything");
    }

    #[test]
    fn paired_compare_counts_discordant_pairs_correctly() {
        let a = [true, true, false, false];
        let b = [true, false, true, false];
        let r = paired_compare(&a, &b);
        assert_eq!(r.b_only, 1, "a passed, b failed");
        assert_eq!(r.c_only, 1, "b passed, a failed");
        assert!((r.p_value - 1.0).abs() < 1e-9, "1 vs 1 is a tie");
    }

    #[test]
    fn holm_is_more_conservative_than_raw_but_less_than_bonferroni() {
        let ps = [0.001, 0.02, 0.04];
        let rejected = holm_reject(&ps, 0.05);
        assert!(rejected[0], "smallest p survives Holm at alpha/3");
        // Plain Bonferroni would reject only p < 0.0167; Holm steps up.
        assert_eq!(rejected.len(), 3);
    }

    #[test]
    fn bootstrap_ci_brackets_the_mean_and_narrows_with_more_problems() {
        let small: Vec<f64> = (0..20).map(|i| if i < 16 { 1.0 } else { 0.0 }).collect();
        let large: Vec<f64> = (0..400).map(|i| if i < 320 { 1.0 } else { 0.0 }).collect();
        let (sl, sh) = bootstrap_ci(&small, 2000, 42);
        let (ll, lh) = bootstrap_ci(&large, 2000, 42);
        assert!(sl < 0.8 && 0.8 < sh, "brackets the 0.8 mean");
        assert!((lh - ll) < (sh - sl), "more problems -> tighter interval");
    }

    #[test]
    fn min_detectable_difference_reports_the_resolution_floor() {
        // Measured by exact enumeration 2026-09-01: 31 fixtures cannot resolve
        // a 10-point difference (power ~0.09). This must be published, not hidden.
        assert!(min_detectable_difference(31) >= 0.20, "31 fixtures is a ~25-30pt floor");
        assert!(min_detectable_difference(164) < min_detectable_difference(31));
    }
}
```

Add `pub mod corpus_stats;` to `crates/vox-eval/src/lib.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-eval corpus_stats 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
use serde::{Deserialize, Serialize};

pub const Z_95: f64 = 1.959_963_984_540_054;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceInterval { pub point: f64, pub low: f64, pub high: f64 }

/// Result of a paired comparison between two systems on the same problems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedResult {
    /// Problems where A passed and B failed.
    pub b_only: usize,
    /// Problems where B passed and A failed.
    pub c_only: usize,
    pub p_value: f64,
    /// A's rate minus B's rate.
    pub difference: f64,
}

fn log_comb(n: usize, k: usize) -> f64 {
    (0..k).map(|i| ((n - i) as f64).ln() - ((i + 1) as f64).ln()).sum()
}

/// Two-sided exact McNemar p-value over discordant counts.
///
/// Exact rather than chi-square because discordant pairs are few at this corpus
/// size (the chi-square approximation needs b + c >= 25).
#[must_use]
pub fn mcnemar_exact_p(b: usize, c: usize) -> f64 {
    let nd = b + c;
    if nd == 0 { return 1.0; }
    let lo = b.min(c);
    let tail: f64 = (0..=lo).map(|i| (log_comb(nd, i) - (nd as f64) * 2f64.ln()).exp()).sum();
    (2.0 * tail).min(1.0)
}

/// Compare two systems on identical problems (paired binary outcomes).
#[must_use]
pub fn paired_compare(a: &[bool], b: &[bool]) -> PairedResult {
    assert_eq!(a.len(), b.len(), "paired comparison requires identical problem sets");
    let b_only = a.iter().zip(b).filter(|(x, y)| **x && !**y).count();
    let c_only = a.iter().zip(b).filter(|(x, y)| !**x && **y).count();
    let rate = |v: &[bool]| if v.is_empty() { 0.0 } else { v.iter().filter(|x| **x).count() as f64 / v.len() as f64 };
    PairedResult { b_only, c_only, p_value: mcnemar_exact_p(b_only, c_only), difference: rate(a) - rate(b) }
}

/// Holm-Bonferroni step-down. Returns per-input rejection flags in input order.
///
/// Controls family-wise error across the m(m-1)/2 pairwise claims published in
/// one run; uniformly more powerful than plain Bonferroni.
#[must_use]
pub fn holm_reject(p_values: &[f64], alpha: f64) -> Vec<bool> {
    let m = p_values.len();
    let mut idx: Vec<usize> = (0..m).collect();
    idx.sort_by(|&i, &j| p_values[i].partial_cmp(&p_values[j]).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = vec![false; m];
    for (rank, &i) in idx.iter().enumerate() {
        if p_values[i] <= alpha / (m - rank) as f64 { out[i] = true; } else { break; }
    }
    out
}

/// Percentile bootstrap CI for a mean over problems (cluster-resampled).
///
/// Correct for pass@k, which is a mean of per-problem estimates rather than a
/// binomial proportion — Wilson does not apply there.
#[must_use]
pub fn bootstrap_ci(per_problem: &[f64], reps: usize, seed: u64) -> (f64, f64) {
    if per_problem.is_empty() { return (0.0, 1.0); }
    let n = per_problem.len();
    let mut state = seed | 1;
    let mut next = || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; state };
    let mut means: Vec<f64> = (0..reps).map(|_| {
        (0..n).map(|_| per_problem[(next() % n as u64) as usize]).sum::<f64>() / n as f64
    }).collect();
    means.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    (means[(0.025 * reps as f64) as usize], means[((0.975 * reps as f64) as usize).min(reps - 1)])
}

/// Wilson score interval. Valid ONLY for single-attempt pass@1 (a genuine
/// binomial proportion). Use `bootstrap_ci` for pass@k.
#[must_use]
pub fn wilson_interval(successes: usize, trials: usize, z: f64) -> ConfidenceInterval {
    if trials == 0 { return ConfidenceInterval { point: 0.0, low: 0.0, high: 1.0 }; }
    let (n, p) = (trials as f64, successes as f64 / trials as f64);
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let centre = p + z2 / (2.0 * n);
    let margin = z * ((p * (1.0 - p) / n) + (z2 / (4.0 * n * n))).sqrt();
    ConfidenceInterval { point: p, low: ((centre - margin) / denom).max(0.0), high: ((centre + margin) / denom).min(1.0) }
}

/// Smallest difference resolvable at ~80% power with `n` problems.
///
/// From exact McNemar enumeration (2026-09-01): n=31 detects a 10-point
/// difference only ~9% of the time and needs ~25-30 points for 80% power;
/// n=164 resolves ~10 points. Published on the leaderboard so readers can see
/// which rows are genuinely tied.
#[must_use]
pub fn min_detectable_difference(n: usize) -> f64 {
    match n {
        0..=40 => 0.25,
        41..=80 => 0.18,
        81..=130 => 0.13,
        131..=250 => 0.10,
        251..=450 => 0.07,
        _ => 0.05,
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-eval corpus_stats 2>&1 | tail -20`
Expected: PASS — 6 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-eval
git add crates/vox-eval/src/corpus_stats.rs crates/vox-eval/src/lib.rs
git commit -m "feat(vox-eval): McNemar paired tests, Holm correction, bootstrap CIs, resolution floor"
```

---

# Phase D — In-context conditions (closes S3)

### Task 8: Conditions C0–C3

Frontier models have never seen Vox, so a zero-shot prompt measures guessability from Rust/Python similarity — a floor near zero that says nothing. The meaningful measurement is in-context acquisition of an unseen grammar, and `vox-grammar-export` already emits a ~780-token compact grammar that nothing in the benchmark path injects.

**Files:** Create `crates/vox-corpus/src/humaneval_runner/conditions.rs`.

**Interfaces:** `pub enum Condition { C0ZeroShot, C1Grammar, C2FewShot, C3FullDocs }`; `pub struct PromptContext { pub condition: Condition, pub context_text: String, pub context_hash: String }`; `pub fn build_context(condition: Condition, repo_root: &Path) -> anyhow::Result<PromptContext>`; `pub fn build_prompt(ctx: &PromptContext, signature: &str, task: &str) -> String`.

- [ ] **Step 1: Write the failing test**

```rust
//! Prompt conditions C0-C3.
//!
//! Reported as separate columns, never averaged: the C0->C3 lift is the
//! headline result, because it is the falsifiable form of "Vox is a good LLM
//! target". `context_hash` is part of row identity — without it a later doc
//! edit silently changes every score and no run is comparable to any other.

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
    }

    #[test]
    fn c0_supplies_no_language_context() {
        let ctx = build_context(Condition::C0ZeroShot, &root()).unwrap();
        assert!(ctx.context_text.is_empty(), "C0 is the no-context control");
    }

    #[test]
    fn c1_includes_the_compact_grammar_and_is_small() {
        let ctx = build_context(Condition::C1Grammar, &root()).unwrap();
        assert!(!ctx.context_text.is_empty(), "C1 must carry the grammar");
        assert!(ctx.context_text.contains("fn"), "grammar mentions declarations");
        // ~780 tokens; keep it well under a budget that would crowd the task.
        assert!(ctx.context_text.len() < 8_000, "compact grammar must stay compact: {} chars", ctx.context_text.len());
    }

    #[test]
    fn conditions_have_distinct_stable_hashes() {
        let a = build_context(Condition::C0ZeroShot, &root()).unwrap();
        let b = build_context(Condition::C1Grammar, &root()).unwrap();
        assert_ne!(a.context_hash, b.context_hash, "different context -> different row identity");
        let a2 = build_context(Condition::C0ZeroShot, &root()).unwrap();
        assert_eq!(a.context_hash, a2.context_hash, "same context -> stable hash");
    }

    #[test]
    fn prompt_pins_the_signature_and_forbids_a_main() {
        let ctx = build_context(Condition::C0ZeroShot, &root()).unwrap();
        let p = build_prompt(&ctx, "fn nth_prime(n: int) to int", "Return the nth prime.");
        assert!(p.contains("fn nth_prime(n: int) to int"));
        assert!(p.contains("Return the nth prime."));
        let lower = p.to_lowercase();
        assert!(lower.contains("do not") && lower.contains("main"), "must forbid a candidate main");
        assert!(lower.contains("assert"), "must forbid redefining assert (the C1 exploit)");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-corpus humaneval_runner::conditions 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
use anyhow::Result;
use std::path::Path;

/// How much Vox reference material the model receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Condition { C0ZeroShot, C1Grammar, C2FewShot, C3FullDocs }

impl Condition {
    #[must_use] pub fn id(self) -> &'static str {
        match self { Self::C0ZeroShot => "C0", Self::C1Grammar => "C1", Self::C2FewShot => "C2", Self::C3FullDocs => "C3" }
    }
}

/// Resolved context for one condition, with the hash that pins row identity.
#[derive(Debug, Clone)]
pub struct PromptContext { pub condition: Condition, pub context_text: String, pub context_hash: String }

/// Assemble the context for `condition`.
///
/// C1 uses `vox-grammar-export`'s compact LLM prompt, which is the documented
/// SSOT for this and is ~780 tokens.
pub fn build_context(condition: Condition, repo_root: &Path) -> Result<PromptContext> {
    let text = match condition {
        Condition::C0ZeroShot => String::new(),
        Condition::C1Grammar => vox_grammar_export::emit_compact_llm_prompt(),
        Condition::C2FewShot => format!(
            "{}\n\n## Worked examples\n\n{}",
            vox_grammar_export::emit_compact_llm_prompt(),
            std::fs::read_to_string(repo_root.join("examples/golden/ref_syntax.vox")).unwrap_or_default()
        ),
        Condition::C3FullDocs => format!(
            "{}\n\n## Worked examples\n\n{}\n\n## Syntax reference\n\n{}",
            vox_grammar_export::emit_compact_llm_prompt(),
            std::fs::read_to_string(repo_root.join("examples/golden/ref_syntax.vox")).unwrap_or_default(),
            std::fs::read_to_string(repo_root.join("docs/src/reference/ref-syntax.md")).unwrap_or_default()
        ),
    };
    let mut h = sha2::Sha256::new();
    sha2::Digest::update(&mut h, condition.id().as_bytes());
    sha2::Digest::update(&mut h, text.as_bytes());
    Ok(PromptContext {
        condition,
        context_hash: format!("{:x}", sha2::Digest::finalize(h))[..16].to_string(),
        context_text: text,
    })
}

/// The single-turn prompt. Identical across conditions except for the context
/// block, so the condition is the only manipulated variable.
#[must_use]
pub fn build_prompt(ctx: &PromptContext, signature: &str, task: &str) -> String {
    let preamble = if ctx.context_text.is_empty() {
        String::new()
    } else {
        format!("Here is a reference for the Vox language:\n\n{}\n\n---\n\n", ctx.context_text)
    };
    format!(
        "{preamble}Write a Vox function with EXACTLY this signature:\n\n    {signature}\n\n\
         Task: {task}\n\n\
         Rules:\n\
         - Reply with ONLY Vox source code. No prose, no explanation, no markdown fences.\n\
         - Do NOT write a `fn main()`; only the function above plus any helpers it needs.\n\
         - Do NOT define or rebind `assert`, `main`, or `print`.\n\
         - Do not read files or access the network."
    )
}
```

Add to `crates/vox-corpus/Cargo.toml` under `[dependencies]`: `vox-grammar-export = { workspace = true }` and `sha2 = { workspace = true }`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-corpus humaneval_runner::conditions 2>&1 | tail -20`
Expected: PASS — 4 tests.

- [ ] **Step 5: Verify no crate-edge violation**

Run: `cargo run -q -p vox-cli -- ci crate-edges`
Expected: PASS (`vox-corpus` L3 → `vox-grammar-export`; confirm the edge already exists — `vox-compiler` depends on it). If rejected, STOP and report; do not author an exceptions entry.

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/humaneval_runner/conditions.rs crates/vox-corpus/Cargo.toml
git commit -m "feat(corpus): C0-C3 in-context conditions with hashed context identity"
```

---

# Phase E — Runner

### Task 9: `vox model eval-corpus`

Closes C3 (no early break), H1 (nullable metrics), H5 (infra errors not model failures), H6 (spend ceiling), H7 (stable digest).

**Files:** Create `crates/vox-cli/src/commands/model/eval_corpus.rs`; modify `crates/vox-cli/src/commands/model/mod.rs`.

- [ ] **Step 1: Write the failing test**

```rust
//! `vox model eval-corpus` — score one (model, harness, condition) tuple.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_digest_is_stable_across_toolchains_and_covers_every_knob() {
        // v1 used DefaultHasher, which Rust does not guarantee stable across
        // releases — a toolchain bump silently re-keyed every historical row.
        let a = config_digest("vox-harness", 10, 1, "C1", "ctxhash", 0.8, 4096, "abc123");
        assert_eq!(a, config_digest("vox-harness", 10, 1, "C1", "ctxhash", 0.8, 4096, "abc123"));
        assert_eq!(a.len(), 16, "sha256 prefix");
        // Every input must change identity.
        assert_ne!(a, config_digest("claude-code", 10, 1, "C1", "ctxhash", 0.8, 4096, "abc123"));
        assert_ne!(a, config_digest("vox-harness", 10, 1, "C0", "ctxhash", 0.8, 4096, "abc123"));
        assert_ne!(a, config_digest("vox-harness", 10, 1, "C1", "OTHER", 0.8, 4096, "abc123"));
        assert_ne!(a, config_digest("vox-harness", 10, 1, "C1", "ctxhash", 0.2, 4096, "abc123"),
            "temperature must be part of row identity");
        assert_ne!(a, config_digest("vox-harness", 10, 1, "C1", "ctxhash", 0.8, 4096, "def456"),
            "compiler commit must be part of row identity");
    }

    #[test]
    fn provider_errors_are_infra_not_model_failures() {
        assert!(is_infra_error("rate limited: 429 too many requests"));
        assert!(is_infra_error("context length exceeded"));
        assert!(!is_infra_error("model returned malformed code"));
    }

    #[test]
    fn extract_vox_code_unwraps_fences_and_passes_bare_code() {
        assert!(extract_vox_code("```vox\nfn f() to int { return 1 }\n```").starts_with("fn f()"));
        assert!(extract_vox_code("```\nfn f() to int { return 1 }\n```").starts_with("fn f()"));
        assert_eq!(extract_vox_code("fn f() to int { return 1 }").trim(), "fn f() to int { return 1 }");
    }

    #[test]
    fn load_solution_dir_errors_on_ambiguous_ids() {
        let d = std::env::temp_dir().join(format!("vox-sol-amb-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("041.vox"), "fn a() to int { return 1 }").unwrap();
        std::fs::write(d.join("041-nth-prime.vox"), "fn b() to int { return 2 }").unwrap();
        // v1 silently picked one at random depending on readdir order.
        assert!(load_solution_dir(&d).is_err(), "ambiguous fixture id must be an error");
        std::fs::remove_dir_all(&d).ok();
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use vox_corpus::humaneval_runner::conditions::{Condition, build_context, build_prompt};

#[derive(Parser, Debug, Clone)]
pub struct EvalCorpusArgs {
    #[arg(long)] pub model: String,
    #[arg(long, default_value = "vox-harness")] pub harness: String,
    /// Prompt condition: C0 zero-shot, C1 grammar, C2 few-shot, C3 full docs.
    #[arg(long, default_value = "C1")] pub condition: String,
    /// Samples per fixture. All are drawn; there is no early stop.
    #[arg(long, default_value_t = 10)] pub n: usize,
    /// k for pass@k. Must satisfy k <= n.
    #[arg(long, default_value_t = 1)] pub k: usize,
    /// Sampling temperature. Use 0.0 with n=1 (greedy headline) or 0.8 with n>=10.
    #[arg(long, default_value_t = 0.8)] pub temperature: f32,
    #[arg(long, default_value_t = 4096)] pub max_tokens: u64,
    #[arg(long)] pub from_dir: Option<PathBuf>,
    #[arg(long, default_value = "contracts/eval/humaneval-vox")] pub corpus: PathBuf,
    #[arg(long, default_value_t = false)] pub include_training_eligible: bool,
    #[arg(long)] pub vox_bin: Option<PathBuf>,
    #[arg(long, default_value_t = 60)] pub timeout_secs: u64,
    /// Abort the sweep once spend exceeds this; the run is marked incomplete.
    #[arg(long)] pub max_spend_usd: Option<f64>,
    #[arg(long)] pub output: Option<PathBuf>,
    #[arg(long, default_value_t = false)] pub no_write_back: bool,
}

/// sha256-based row identity over every knob that can change a score.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn config_digest(
    harness: &str, n: usize, k: usize, condition: &str, context_hash: &str,
    temperature: f32, max_tokens: u64, compiler_commit: &str,
) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(format!("{harness}|{n}|{k}|{condition}|{context_hash}|{temperature}|{max_tokens}|{compiler_commit}"));
    format!("{:x}", h.finalize())[..16].to_string()
}

/// Provider-side failures that must NOT be scored as model failures.
#[must_use]
pub fn is_infra_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("rate limited") || m.contains("429") || m.contains("context length")
        || m.contains("502") || m.contains("503") || m.contains("timeout")
}

/// Pull Vox source from a completion, unwrapping a fenced block when present.
#[must_use]
pub fn extract_vox_code(completion: &str) -> String {
    let Some(open) = completion.find("```") else { return completion.trim().to_string() };
    let after = &completion[open + 3..];
    let body = &after[after.find('\n').map_or(0, |i| i + 1)..];
    match body.find("```") { Some(c) => body[..c].trim().to_string(), None => body.trim().to_string() }
}

/// Load `<id>[-slug].vox` solutions, erroring on ambiguity rather than
/// silently picking one by directory order.
pub fn load_solution_dir(dir: &Path) -> Result<HashMap<String, String>> {
    let mut out: HashMap<String, String> = HashMap::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("vox") { continue; }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
        let id = stem.split('-').next().unwrap_or(&stem).to_string();
        if out.contains_key(&id) {
            anyhow::bail!("ambiguous fixture id `{id}` in {} — two files map to the same fixture", dir.display());
        }
        out.insert(id, std::fs::read_to_string(&path)?);
    }
    Ok(out)
}
```

Then implement `run(args)`: parse the condition, `anyhow::ensure!(args.k <= args.n)`, load and filter fixtures, build the context, resolve the binary (release then debug), and for each fixture draw **exactly `n`** attempts with no early break — recording `cheated` from `verify_program`, skipping infra errors into `n_infra_errors` rather than counting them as failures, and aborting on `max_spend_usd` with `run_complete: false`. Register the subcommand in `mod.rs` as in v1.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli eval_corpus 2>&1 | tail -20`
Expected: PASS — 4 tests.

- [ ] **Step 5: Verify the LLM-boundary detector and sync the registry**

```bash
cargo run -q -p vox-cli -- audit code
cargo run -q -p vox-cli -- ci crate-edges
cargo run -q -p vox-cli -- ci command-sync --write
UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli command_catalog
```

Expected: no `llm_provider_call` finding (all traffic via the facade).

- [ ] **Step 6: End-to-end oracle check**

```bash
cargo build -p vox-cli --release -j 2
mkdir -p /tmp/vox-oracle
for d in contracts/eval/humaneval-vox/problems/*/; do
  id=$(basename "$d" | cut -d- -f1); cp "$d/reference.vox" "/tmp/vox-oracle/$id.vox" 2>/dev/null || true
done
cargo run -q -p vox-cli --release -- model eval-corpus \
  --model reference-oracle --harness oracle --condition C0 --n 1 --k 1 \
  --from-dir /tmp/vox-oracle --include-training-eligible
```

Expected: **pass@1 = 1.000, n_cheated = 0.** Anything less is a harness bug. This is the gate that must be green before any model is scored.

- [ ] **Step 7: Commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/model/ contracts/cli/command-registry.yaml crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt docs/src/reference/cli-command-surface.generated.md
git commit -m "feat(vox-cli): eval-corpus runner with conditions, full-n sampling, and spend ceiling"
```

---

# Phase F — Publication

### Task 10: Leaderboard artifact, page, and automation

Carries forward v1 Tasks 10–11 with the audit's corrections. Key deltas from v1:

- **Nullable metric columns.** `tokens_per_pass`, `p50_ms`, `total_tokens`, `cost_per_success_usd` are `["number","null"]`; the renderer prints `—` for null. Ingested rows must never publish `0` for tokens or latency.
- **Row identity** includes `condition_id`, `context_hash`, `config_digest`, `corpus_hash`, `vox_commit`, `n`, `k`, `temperature`. Rows may only be compared within an identical `corpus_hash` **and** `condition_id`.
- **Significance** comes from `paired_compare` + `holm_reject`, not interval overlap. The page renders "tied" when Holm does not reject.
- **Resolution floor** printed on the page from `min_detectable_difference(n_fixtures)`: *"With N fixtures this benchmark cannot resolve differences below X points; closer rows are ties."*
- **A COI statement** on the page: Vox authors the language, corpus, compiler, harness, and one contestant.
- **Provenance block** per row: `measured: {tokens, latency, attempts}` so ingested and generated rows are visually distinguishable.
- **Sanity gate before publish:** the oracle sweep must be 100% and a majority of models must have completed, or the previous board keeps serving.
- **Automation** per the CI track: a `schedule:`-triggered workflow (exempt from the concurrency guard; declare a non-cancelling block anyway), `runs-on: [self-hosted, linux, x64]` (no exception row needed), `OPENROUTER_API_KEY` from repo secrets, a `.vox` sweep script, `contracts/reports/vox-efficacy/**` added to `docs-deploy.yml`'s path filter, an explicit `gh workflow run docs-deploy.yml` (a `GITHUB_TOKEN` push triggers nothing), and **no `[skip ci]`**. Register the schema in `contracts/index.yaml`; add the artifact to `.gitattributes` as `linguist-generated`; do **not** register it with `ssot-drift` (it is not reproducible from committed inputs).

Build this **last**. Publishing a broken number on a schedule is worse than publishing nothing.

---

## Claims this plan supports, and claims it does not

**Supported once Phases 0–E are green:**
- The **C0→C3 context lift** per model — the headline, and the falsifiable form of "Vox is a good LLM target".
- MENS versus **its own base checkpoint** at a matched condition.
- Cost and latency per solved fixture.
- Failure-class distributions (descriptive, no ranking).
- "Here is our corpus and runner — score your own system."

**Not supported at any sample size from this project:** model-vs-model rankings among Claude/GPT/Gemini/Kimi/Qwen/Grok (31 fixtures resolve ~25–30 points; 164 resolve ~10; frontier models sit closer than that); harness rankings against Claude Code/Cursor/Warp (hand-driven, single operator, no protocol); and "Vox is a better LLM target than Python" (no cross-language arm, and the training-data-volume confound is uncontrollable).

---

## Self-Review

**Audit coverage:** C1→Tasks 3,4 · C2→Task 6 · C3→Tasks 6,9 · C4→Task 7 · C5→Tasks 7,10 · C6→Task 4 · C7→Tasks 1,2 · H1→Tasks 6,10 · H2→Task 3 (canary replaces the substring guard entirely) · H3→Task 5 · H4→Global Constraints (claim deleted) · H5→Task 9 · H6→Task 9 · H7→Task 9 · H8→Task 9 (`max_tokens` default 4096) · S3→Task 8.

**Deliberately deferred:** S1 (the `vox-syntax` tier covering the 83% of the language this corpus never touches) is the largest remaining gap and needs its own plan — it depends on grammar work landing for `actor`/`state_machine`. S2 (corpus expansion past 200 problems, required before any ranking claim) is data work, not code.

**Known risk:** Task 9 Step 6's oracle check is the gate. If reference solutions do not score 1.000, the fault is the harness. Never interpret a model score collected before that gate is green.
