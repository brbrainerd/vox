# Semantic Coverage to the Efficient Ceiling (~95%) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the 5,793 *reached-but-unproven* symbols (executed by tests, but with zero asserted behavior) into *proven* symbols using real behavioral assertions, gated by an automated anti-bogus-test check, and lock the gain behind a CI ratchet — without writing tests that "just touch code."

**Architecture:** Three layers. (1) A **CI ratchet** freezes the measurement so progress is enforceable and can't regress. (2) A **test-quality gate** (a `vox-code-audit` detector) plus a **worklist classifier** define the *efficient frontier* — the behaviorally-meaningful subset worth proving — and reject touch-tests. (3) **Leverage waves** grind the top-6 crates' worklists with TDD behavioral assertions. Everything is measured against `contracts/reports/semantic-coverage.v1.json`.

**Tech Stack:** Rust (workspace crates + `vox-code-audit` detector framework), `cargo llvm-cov` + `llvm-profdata`, the existing `scripts/coverage-graph/` Python toolchain (`ingest_reaches.py`, `export_lcov_chunked.py`), GitHub Actions (`.github/workflows/ci.yml`), `.vox` automation where new glue is needed.

---

## What "95% efficiently, without bogus tests" actually means

Read this before starting — it sets every target below and prevents the plan from chasing a fantasy number.

**The honest denominators (verified, reproducible 2026-06-15):**

| Bucket | Count | What it is | Cost to prove |
|---|---:|---|---|
| Proven | 3,088 (15.9%) | assertion-backed behavior | — done |
| **Reached-but-unproven** | **5,793** | **executed by a test, asserts nothing** | **CHEAP — the test already runs the code; just add a real assertion** |
| Neither reached nor proven | ~8,400 | not executed by any test | EXPENSIVE — needs a brand-new test that *executes* the code first |

**"Efficient 95%" = prove ~95% of the behaviorally-meaningful slice of the 5,793**, NOT 95% `proven_pct`. The 5,793 are the cheap frontier: the code is already exercised, so proving it is "add one honest assertion to a test that already runs." The ~8,400 unreached symbols are the expensive frontier (you must first write a test that executes them) and are explicitly **out of scope** for the efficient target — chasing them is what produces touch-tests.

Of the 5,793, a chunk are *trivial* (derives, `Display`, getters, `new`/`Default`, builders) where a behavioral assertion adds no signal — proving them is itself the "useless touch." The worklist classifier (Phase 1) removes these. The realistic efficient target:

- **Frontier (worth proving):** ~4,000–4,500 of the 5,793 (after trivial removal).
- **Efficient-95% target:** prove ≥ 90% of the frontier → `proven` rises from 3,088 to ~6,800–7,200, i.e. **`proven_pct` 15.9% → ~36%**, and the dangerous "executed but unproven" illusion shrinks by ~95%.
- **Stopping criterion:** when the per-crate worklist (frontier symbols) is drained to ≤ 5% remaining, or two consecutive waves add < 10 newly-proven frontier symbols each (diminishing returns).

If a future stakeholder wants `proven_pct` ≥ 90%, that is a *different, expensive* project against the 8,400 unreached symbols and must be scoped separately. This plan does not promise it and will say so in the final doc.

---

## Code review: follow-ups and everything we did NOT fix

State as of HEAD (`0e16e9d592`). Structural patterns #1,#3,#4,#5,#6,#7 are covered; the items below are open.

| # | Item | Type | Severity | Where it's handled in this plan |
|---|------|------|----------|----------------------------------|
| R1 | CI reach-ratchet not wired (reach is reproducible locally only) | Infra / decision-gated | High | **Phase 0** |
| R2 | `reached_not_proven` baseline not stored as a comparable number | Infra | High | **Phase 0 Task 0.2** |
| R3 | No automated guard against bogus/touch-tests | Quality | High | **Phase 1** |
| R4 | `@traced` is a *uniformly dead* decorator: `set_decorators` sets `FnDecl.is_traced=true` but no `HirFn` field consumes it — silently dropped for all fn-shaped decls (only `HirAgentHandler` has the field, never set true) | Real defect | Medium | **Phase 2 Task 2.3** |
| R5 | Decorator-order asymmetry: `@pure` *before* `@example`/`@test` is a hard parse error, but *after* parses fine — same two decorators, order-dependent acceptance | Real defect (parser) | Medium | **Phase 2 Task 2.4** |
| R6 | Pattern #2 `@deprecated("reason")`: documented (`docs/.../ref-decorators.md`) but does not parse (`Expected fn, found (`) — the arg is dropped at grammar | Doc-vs-impl drift | Low | **Phase 2 Task 2.5** |
| R7 | Weak-test tail: `assert_ne!` on derived enum discriminants; self-constructed-literal re-asserts; overstated `// Catches:` on no-panic tests | Quality | Medium | **Phase 1 (gate)** + **Phase 2 Task 2.1** |
| R8 | Crypto hash correctness under-pinned: only SHA3 has a known-answer vector; `secure_hash` (BLAKE3) / `fast_hash` (xxh3) lean on inequality — a wrong-but-deterministic algo would pass | Quality / security | Medium | **Phase 2 Task 2.2** |
| R9 | `catch_all_swallow` + `cross_crate_dup` detectors are `Severity::Info` (advisory), not wired into a blocking gate | Infra | Low | **Phase 2 Task 2.6** |
| R10 | Workflow/activity `uses` doc-drift: `parse_workflow_decl` doc claims a `uses` clause never parsed | Doc-vs-impl drift | Low | **Phase 2 Task 2.5 (same sweep)** |
| R11 | The 5,793 reached-but-unproven set itself | The actual work | — | **Phase 3 + Phase 4** |

---

## File structure

**New files:**
- `scripts/coverage-graph/prune_graph_snapshot.py` — strips `graph.json` to the fields the ratchet needs; gzips it.
- `contracts/reports/semantic-coverage-graph.snapshot.json.gz` — the committed frozen graph snapshot (~5–6 MB).
- `scripts/coverage-graph/ratchet_check.py` — compares current `reached_not_proven` vs the committed baseline; non-zero exit on regression.
- `scripts/coverage-graph/emit_unproven_worklist.py` — emits per-crate ranked frontier worklists from `graph.json`.
- `crates/vox-code-audit/src/detectors/weak_test.rs` — detector flagging touch-test anti-patterns.
- `crates/vox-code-audit/src/detectors/semcov_wave_weaktest_tests.rs` — its tests.
- Per-crate behavioral test modules under each target crate (Phase 3), e.g. `crates/vox-codegen/src/semcov_behavior_tests.rs`.

**Modified files:**
- `contracts/reports/semantic-coverage.v1.json` — add `reached_not_proven` baseline + per-crate frontier counts.
- `.github/workflows/ci.yml` — add the ratchet step to the existing Linux `tests` (llvm-cov) job.
- `crates/vox-code-audit/src/detectors/mod.rs` — register `weak_test`; bump `rule_count()`.
- `crates/vox-crypto/src/facades.rs` — add BLAKE3 / xxh3 known-answer vectors (R8).
- `crates/vox-compiler/src/hir/lower/decl.rs` + `crates/vox-compiler/src/hir/nodes/decl.rs` — `@traced` decision (R4).
- `docs/src/architecture/semantic-coverage-status-2026-06-15.md` — keep the SSOT current as phases land.

---

## Phase 0 — Lock the ruler (CI ratchet)

Without an enforced baseline, "95%" is unmeasurable and silently regresses. This phase makes `reached_not_proven` a committed number that CI fails on if it rises.

> **Decision gate (confirm before Task 0.1):** this commits a ~5–6 MB gzipped graph snapshot to git history and edits `.github/workflows/ci.yml`. Both are repo-affecting. If the snapshot size is unacceptable, switch to the CI-artifact variant noted in Task 0.1.

### Task 0.1: Prune + commit the frozen graph snapshot

**Files:**
- Create: `scripts/coverage-graph/prune_graph_snapshot.py`
- Create (committed, force-add): `contracts/reports/semantic-coverage-graph.snapshot.json.gz`

- [ ] **Step 1: Write the pruner**

```python
# scripts/coverage-graph/prune_graph_snapshot.py
"""Strip graphify-out/graph.json to ONLY the fields ingest_reaches.py consumes, and
gzip it, so a small frozen snapshot can be committed for the CI ratchet. The full
graph (~109 MB, gitignored, LLM-derived) is not reproducible in CI; this snapshot is.

Usage:
  python prune_graph_snapshot.py --graph graphify-out/graph.json \
      --out contracts/reports/semantic-coverage-graph.snapshot.json.gz
"""
import argparse, gzip, json
from pathlib import Path

NODE_FIELDS = ("id", "label", "source_file", "source_location", "file_type", "_origin")

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--graph", default="graphify-out/graph.json")
    ap.add_argument("--out", default="contracts/reports/semantic-coverage-graph.snapshot.json.gz")
    args = ap.parse_args()
    g = json.loads(Path(args.graph).read_text(encoding="utf-8"))
    pruned = {
        "nodes": [{k: n.get(k) for k in NODE_FIELDS} for n in g["nodes"]],
        "links": [l for l in g["links"] if l.get("relation") == "proves"],
    }
    blob = json.dumps(pruned, separators=(",", ":")).encode("utf-8")
    Path(args.out).write_bytes(gzip.compress(blob, compresslevel=9))
    print(f"snapshot: {len(pruned['nodes'])} nodes, "
          f"{len(pruned['links'])} proves-links, "
          f"{Path(args.out).stat().st_size/1e6:.1f} MB gz")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Generate the snapshot**

Run: `python scripts/coverage-graph/prune_graph_snapshot.py`
Expected: prints `snapshot: 56754 nodes, 54613 proves-links, ~5-6 MB gz`

- [ ] **Step 3: Make `ingest_reaches.py` accept a gzipped snapshot**

In `scripts/coverage-graph/ingest_reaches.py`, replace the graph read (`g = json.loads(Path(args.graph).read_text(...))`) with a helper that transparently handles `.gz`:

```python
def _load_graph(path: str):
    import gzip
    p = Path(path)
    raw = gzip.decompress(p.read_bytes()) if p.suffix == ".gz" else p.read_bytes()
    return json.loads(raw)
# ...
g = _load_graph(args.graph)
```

- [ ] **Step 4: Verify ingest against the snapshot reproduces 5,793**

Run: `python scripts/coverage-graph/ingest_reaches.py --lcov target/llvm-cov-lcov.info --graph contracts/reports/semantic-coverage-graph.snapshot.json.gz --out /tmp/g.json --report /tmp/r.md`
Expected: stdout `annotated=... reached_not_proven=5793` (matches the committed baseline; the snapshot carries the same nodes + proves edges as the full graph).

- [ ] **Step 5: Commit (force-add past gitignore)**

```bash
git add -f contracts/reports/semantic-coverage-graph.snapshot.json.gz
git add scripts/coverage-graph/prune_graph_snapshot.py scripts/coverage-graph/ingest_reaches.py
git commit -m "feat(coverage): commit frozen pruned graph snapshot for CI ratchet"
```

> **CI-artifact variant (if the blob is rejected):** skip the `git add -f`; instead have the CI job upload the snapshot as a workflow artifact from a manually-triggered `prune` job and download it in the ratchet step. Weaker provenance; documented in the status doc.

### Task 0.2: Store the baseline number

**Files:**
- Modify: `contracts/reports/semantic-coverage.v1.json` (top-level `totals.reached_not_proven` already added this session = 5793; add an explicit ratchet baseline block).

- [ ] **Step 1: Add the ratchet baseline**

Add under `totals`:

```json
"ratchet": {
  "reached_not_proven_baseline": 5793,
  "snapshot": "contracts/reports/semantic-coverage-graph.snapshot.json.gz",
  "updated": "2026-06-15",
  "policy": "CI fails if current reached_not_proven > baseline. Lower the baseline (never raise) in the same PR that adds the proving tests, and regenerate the snapshot if the proven map changed."
}
```

- [ ] **Step 2: Validate + commit**

Run: `python -c "import json; json.load(open('contracts/reports/semantic-coverage.v1.json')); print('ok')"`
Expected: `ok`

```bash
git add contracts/reports/semantic-coverage.v1.json
git commit -m "chore(coverage): record reached_not_proven ratchet baseline (5793)"
```

### Task 0.3: The ratchet comparator

**Files:**
- Create: `scripts/coverage-graph/ratchet_check.py`

- [ ] **Step 1: Write the comparator**

```python
# scripts/coverage-graph/ratchet_check.py
"""Fail (exit 1) if current reached_not_proven exceeds the committed baseline.
Run AFTER ingest_reaches.py has produced its report.

Usage:
  python ratchet_check.py --report graphify-out/REACHED_VS_PROVEN.md \
      --baseline contracts/reports/semantic-coverage.v1.json
"""
import argparse, json, re, sys
from pathlib import Path

def current_rnp(report: str) -> int:
    m = re.search(r"Total reached-but-unproven symbols:\s*(\d+)", Path(report).read_text(encoding="utf-8"))
    if not m:
        print("ratchet: could not parse report", file=sys.stderr); sys.exit(2)
    return int(m.group(1))

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--report", required=True)
    ap.add_argument("--baseline", required=True)
    args = ap.parse_args()
    cur = current_rnp(args.report)
    base = json.loads(Path(args.baseline).read_text(encoding="utf-8"))["totals"]["ratchet"]["reached_not_proven_baseline"]
    if cur > base:
        print(f"::error::reached-but-unproven ROSE {base} -> {cur} (+{cur-base}). Add behavioral assertions or justify.")
        return 1
    if cur < base:
        print(f"::notice::reached-but-unproven improved {base} -> {cur} (-{base-cur}). Lower the baseline in this PR.")
    print(f"ratchet OK: {cur} <= {base}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Test it locally (pass + fail)**

Run: `python scripts/coverage-graph/ratchet_check.py --report graphify-out/REACHED_VS_PROVEN.md --baseline contracts/reports/semantic-coverage.v1.json`
Expected: `ratchet OK: 5793 <= 5793`

- [ ] **Step 3: Commit**

```bash
git add scripts/coverage-graph/ratchet_check.py
git commit -m "feat(coverage): reach-ratchet comparator"
```

### Task 0.4: Wire into CI (Linux llvm-cov job)

**Files:**
- Modify: `.github/workflows/ci.yml` (the existing Linux `tests` job already runs `cargo llvm-cov report --lcov --output-path target/llvm-cov-lcov.info`).

- [ ] **Step 1: Add the ratchet steps after the existing lcov export**

```yaml
      - name: Semantic-coverage reach ingest
        run: |
          python scripts/coverage-graph/ingest_reaches.py \
            --lcov target/llvm-cov-lcov.info \
            --graph contracts/reports/semantic-coverage-graph.snapshot.json.gz \
            --out /tmp/graph.reached.json \
            --report /tmp/REACHED_VS_PROVEN.md
      - name: Semantic-coverage ratchet
        run: |
          python scripts/coverage-graph/ratchet_check.py \
            --report /tmp/REACHED_VS_PROVEN.md \
            --baseline contracts/reports/semantic-coverage.v1.json
```

(No chunked export on Linux — `llvm-cov` exports lcov directly; the Windows arg-limit workaround is local-only.)

- [ ] **Step 2: Verify YAML + push to a branch; confirm the job runs green**

Run: `python -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('yaml ok')"`
Expected: `yaml ok`. Then push and confirm the `tests` job's ratchet step prints `ratchet OK`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(coverage): enforce reached-but-unproven ratchet on Linux llvm-cov job"
```

---

## Phase 1 — The anti-bogus-test engine

This is the heart of "without bogus tests." Two pieces: a **detector** that fails CI on touch-test anti-patterns, and a **worklist classifier** that hands waves only behaviorally-meaningful targets.

### Task 1.1: `weak_test` detector

**Files:**
- Create: `crates/vox-code-audit/src/detectors/weak_test.rs`
- Create: `crates/vox-code-audit/src/detectors/semcov_wave_weaktest_tests.rs`
- Modify: `crates/vox-code-audit/src/detectors/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/vox-code-audit/src/detectors/semcov_wave_weaktest_tests.rs
#[cfg(test)]
mod weak_test_detector_tests {
    use crate::detectors::weak_test::WeakTestDetector;
    use crate::rules::{DetectionRule, SourceFile};
    use std::path::PathBuf;

    fn findings(src: &str) -> Vec<crate::rules::Finding> {
        let f = SourceFile::new(PathBuf::from("t.rs"), src.to_string());
        WeakTestDetector::new().detect(&f, None)
    }

    #[test]
    fn flags_test_with_no_assertion() {
        let src = "#[test]\nfn t() {\n    let _ = compute(1);\n}\n";
        assert!(!findings(src).is_empty(), "a #[test] with no assert must be flagged");
    }

    #[test]
    fn flags_self_compare_literal() {
        let src = "#[test]\nfn t() {\n    assert_eq!(3, 3);\n}\n";
        assert!(findings(src).iter().any(|f| f.message.contains("tautolog")));
    }

    #[test]
    fn flags_is_ok_only_assertion() {
        let src = "#[test]\nfn t() {\n    assert!(run().is_ok());\n}\n";
        assert!(findings(src).iter().any(|f| f.message.contains("shallow")));
    }

    #[test]
    fn does_not_flag_real_behavioral_assertion() {
        let src = "#[test]\nfn t() {\n    assert_eq!(compute(2), 4);\n}\n";
        assert!(findings(src).is_empty(), "a value-pinning assert must NOT be flagged");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-code-audit --lib weak_test_detector_tests`
Expected: FAIL — `WeakTestDetector` not found.

- [ ] **Step 3: Implement the detector**

```rust
// crates/vox-code-audit/src/detectors/weak_test.rs
//! Flags touch-test anti-patterns inside `#[test]` functions: no assertion at all,
//! tautological self-compares (`assert_eq!(x, x)` with identical literal sides), and
//! shallow-only assertions (`.is_ok()`/`.is_some()`/`!is_empty()` with nothing else).
//! Info+Warning severity — the gate gate-blocks on Warning in `tests/` paths.
use crate::rules::{DetectionRule, Finding, Language, Severity, SourceFile};

pub struct WeakTestDetector;
impl WeakTestDetector { pub fn new() -> Self { Self } }
impl Default for WeakTestDetector { fn default() -> Self { Self::new() } }

const SHALLOW: &[&str] = &[".is_ok()", ".is_some()", ".is_err()", ".is_none()", "!.is_empty()", ".is_empty()"];

impl DetectionRule for WeakTestDetector {
    fn id(&self) -> &'static str { "weak_test" }
    fn name(&self) -> &'static str { "Weak / touch test" }
    fn languages(&self) -> &'static [Language] { &[Language::Rust] }

    fn detect(&self, file: &SourceFile, _schema: Option<&serde_json::Value>) -> Vec<Finding> {
        let mut out = Vec::new();
        let lines: Vec<&str> = file.content.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            if lines[i].trim_start().starts_with("#[test]") || lines[i].contains("#[tokio::test]") {
                // collect the body until the matching closing brace at fn indent
                let (start, end) = fn_body_range(&lines, i);
                let body = lines[start..end].join("\n");
                let asserts: Vec<&str> = (start..end)
                    .map(|k| lines[k].trim())
                    .filter(|l| l.contains("assert") || l.contains("panic!"))
                    .collect();
                let fn_line = lines[start].trim().to_string();
                if asserts.is_empty() {
                    out.push(mk(file, start, Severity::Warning,
                        format!("test has NO assertion (touch test): {fn_line}")));
                } else {
                    for a in &asserts {
                        if is_self_compare(a) {
                            out.push(mk(file, start, Severity::Warning,
                                format!("tautological self-compare assertion: {a}")));
                        }
                    }
                    let only_shallow = asserts.iter().all(|a|
                        SHALLOW.iter().any(|s| a.contains(s)) && !a.contains("assert_eq!"));
                    if only_shallow {
                        out.push(mk(file, start, Severity::Info,
                            format!("only shallow .is_ok()/.is_some() assertions — pin a value/variant: {fn_line}")));
                    }
                    let _ = body;
                }
                i = end;
                continue;
            }
            i += 1;
        }
        out
    }
}

fn fn_body_range(lines: &[&str], test_attr: usize) -> (usize, usize) {
    // find `fn` after the attribute, then brace-match to the end
    let mut j = test_attr;
    while j < lines.len() && !lines[j].contains("fn ") { j += 1; }
    let start = j.min(lines.len().saturating_sub(1));
    let mut depth = 0i32; let mut seen = false; let mut k = start;
    while k < lines.len() {
        depth += lines[k].matches('{').count() as i32;
        if depth > 0 { seen = true; }
        depth -= lines[k].matches('}').count() as i32;
        if seen && depth <= 0 { return (start, (k + 1).min(lines.len())); }
        k += 1;
    }
    (start, lines.len())
}

fn is_self_compare(a: &str) -> bool {
    // assert_eq!(X, X) with byte-identical, literal-ish sides
    if let Some(inner) = a.split_once("assert_eq!(").map(|(_, r)| r) {
        let inner = inner.trim_end_matches(");").trim_end_matches(')');
        if let Some((l, r)) = split_top_comma(inner) {
            return l.trim() == r.trim();
        }
    }
    false
}

fn split_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (idx, c) in s.char_indices() {
        match c { '(' | '[' | '{' => depth += 1, ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return Some((&s[..idx], &s[idx + 1..])), _ => {} }
    }
    None
}

fn mk(file: &SourceFile, line: usize, severity: Severity, message: String) -> Finding {
    Finding {
        rule_id: "weak_test".to_string(),
        message,
        file: file.path.clone(),
        line: line + 1,
        column: 1,
        severity,
    }
}
```

> NOTE: match `Finding`'s actual constructor/fields to the ones used in a sibling detector (e.g. `crates/vox-code-audit/src/detectors/empty_body.rs`) — copy that crate's exact `Finding { .. }` shape and `Severity` import path rather than the illustrative shape above.

- [ ] **Step 4: Register the detector**

In `crates/vox-code-audit/src/detectors/mod.rs`: add `pub mod weak_test;`, add `#[cfg(test)] mod semcov_wave_weaktest_tests;`, push `Box::new(weak_test::WeakTestDetector::new())` into `all_rules(...)`, and bump `rule_count()` from 52 to 53 (update the `all_rules_instantiate` count test).

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p vox-code-audit --lib weak_test`
Expected: PASS (all 4 detector tests + `all_rules_instantiate`).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-code-audit/src/detectors/weak_test.rs crates/vox-code-audit/src/detectors/semcov_wave_weaktest_tests.rs crates/vox-code-audit/src/detectors/mod.rs
git commit -m "feat(code-audit): weak_test detector flags touch-test anti-patterns"
```

### Task 1.2: Run the gate over existing semcov tests; triage

- [ ] **Step 1: Scan the repo's test files**

Run: `cargo run -p vox-cli -- audit --gate all 2>&1 | grep weak_test | tee /tmp/weak.txt; wc -l /tmp/weak.txt`
Expected: a list of existing weak tests (the review predicted a tail: shallow `is_ok`/`is_some`, the `assert_ne`-on-derived family, no-assert tests).

- [ ] **Step 2: Record the count as a second ratchet**

Add `totals.ratchet.weak_test_baseline` to `semantic-coverage.v1.json` = the current count, with policy "must not rise; lower as the tail is fixed (Phase 2 Task 2.1)". Commit.

### Task 1.3: Worklist classifier (the frontier)

**Files:**
- Create: `scripts/coverage-graph/emit_unproven_worklist.py`

- [ ] **Step 1: Write the classifier**

```python
# scripts/coverage-graph/emit_unproven_worklist.py
"""Emit per-crate, leverage-ranked worklists of FRONTIER symbols: reached-but-unproven
AND non-trivial (skip getters/derives/Display/new/Default/builders). These are the
symbols worth a behavioral assertion — the input to Phase 3 waves.

Usage:
  python emit_unproven_worklist.py --graph contracts/reports/semantic-coverage-graph.snapshot.json.gz \
      --lcov target/llvm-cov-lcov.info --out-dir graphify-out/worklists
Requires the same lcov used for ingest (so `reached` matches).
"""
import argparse, gzip, json, re
from collections import defaultdict
from pathlib import Path
import importlib.util

# reuse ingest's lcov parser + norm
spec = importlib.util.spec_from_file_location("ingest", Path(__file__).with_name("ingest_reaches.py"))
ingest = importlib.util.module_from_spec(spec); spec.loader.exec_module(ingest)

TRIVIAL = re.compile(r"^(new|default|from|from_str|fmt|clone|eq|hash|builder|with_|get_|is_|as_|to_|into_|len|size)\b", re.I)

def crate_of(fp: str) -> str:
    p = (fp or "").replace("\\", "/")
    return p.split("crates/")[1].split("/")[0] if "crates/" in p else "?"

def load_graph(path):
    raw = gzip.decompress(Path(path).read_bytes()) if path.endswith(".gz") else Path(path).read_bytes()
    return json.loads(raw)

def is_trivial(label: str) -> bool:
    base = ingest.norm(label or "")
    return bool(TRIVIAL.match(base)) or len(base) <= 2

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--graph", required=True)
    ap.add_argument("--lcov", required=True)
    ap.add_argument("--out-dir", default="graphify-out/worklists")
    args = ap.parse_args()
    by_fn, hit_lines = ingest.parse_lcov(args.lcov)
    g = load_graph(args.graph)
    proven = {l["target"] for l in g["links"] if l.get("relation") == "proves"}
    test_keys = {((n.get("source_file") or ""), ingest.norm(n.get("label", "")))
                 for n in g["nodes"] if n.get("_origin") == "test"}
    rows = defaultdict(list)
    for n in g["nodes"]:
        if n.get("file_type") != "code":
            continue
        sf = (n.get("source_file") or "").replace("\\", "/")
        nm = ingest.norm(n.get("label", ""))
        if "/tests/" in sf or (sf, nm) in test_keys:
            continue
        loc = (n.get("source_location") or "").lstrip("L")
        line_no = int(loc) if loc.isdigit() else None
        reached = (line_no in hit_lines.get(sf, set())) if line_no is not None else False
        reached = reached or by_fn.get(sf, {}).get(nm, False)
        if reached and n["id"] not in proven and not is_trivial(n.get("label", "")):
            rows[crate_of(sf)].append((n.get("label", ""), sf, n.get("source_location", "")))
    Path(args.out_dir).mkdir(parents=True, exist_ok=True)
    summary = sorted(((c, len(v)) for c, v in rows.items()), key=lambda kv: -kv[1])
    for c, items in rows.items():
        lines = ["label\tsource_file\tline"] + [f"{l}\t{f}\t{loc}" for (l, f, loc) in sorted(items)]
        (Path(args.out_dir) / f"{c}.tsv").write_text("\n".join(lines), encoding="utf-8")
    (Path(args.out_dir) / "_summary.tsv").write_text(
        "\n".join(f"{c}\t{n}" for c, n in summary), encoding="utf-8")
    print(f"frontier total = {sum(n for _, n in summary)} across {len(summary)} crates")
    for c, n in summary[:8]:
        print(f"  {c}\t{n}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Generate the worklists**

Run: `python scripts/coverage-graph/emit_unproven_worklist.py --graph contracts/reports/semantic-coverage-graph.snapshot.json.gz --lcov target/llvm-cov-lcov.info`
Expected: prints `frontier total = ~4000-4500 across ~60 crates` and the top-8 crates; writes `graphify-out/worklists/<crate>.tsv`.

- [ ] **Step 3: Commit the generator (worklists are gitignored output)**

```bash
git add scripts/coverage-graph/emit_unproven_worklist.py
git commit -m "feat(coverage): frontier worklist generator (non-trivial reached-but-unproven)"
```

---

## Phase 2 — Clean the known defects (R4–R10)

Small, high-signal, mostly TDD. Each is independent; do in any order.

### Task 2.1: Fix the weak-test tail flagged by the new detector (R7)

**Files:** the files listed in `/tmp/weak.txt` from Task 1.2 (e.g. `crates/vox-secrets/src/semcov_wave45_tests.rs`, `crates/vox-vcs/src/semcov_wave44_tests.rs`).

- [ ] **Step 1:** For each flagged test, replace the shallow/tautological assertion with a value/variant-pinning one. Example — `vox-vcs` `resolve_strategy_variants_are_distinct` (`assert_ne!` on derived enum) → assert each variant's *behavior* (e.g. its `Display` string or its effect), not discriminant inequality. If a flagged test genuinely cannot be strengthened, delete it (a touch test is negative value).
- [ ] **Step 2:** Run `cargo test -p <crate>` for each touched crate; all green.
- [ ] **Step 3:** Re-run the gate; confirm `weak_test` count dropped. Lower `weak_test_baseline` in the JSON.
- [ ] **Step 4: Commit** per crate: `git commit -m "test(<crate>): strengthen weak assertions flagged by weak_test"`.

### Task 2.2: Crypto known-answer vectors (R8)

**Files:** Modify `crates/vox-crypto/src/facades.rs` (test module).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn secure_hash_blake3_known_answer() {
    // BLAKE3("") known vector.
    let got = hex::encode(secure_hash(&[]));
    assert_eq!(got, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
}
#[test]
fn fast_hash_xxh3_known_answer() {
    // xxh3_64("") — pin the exact 64-bit value for the configured seed.
    assert_eq!(fast_hash(b""), 0x2D06800538D394C2);
}
```

- [ ] **Step 2: Run to verify** (`cargo test -p vox-crypto secure_hash_blake3_known_answer fast_hash_xxh3_known_answer`). If a value mismatches, the test prints the actual — **paste the printed value** (the test is the source of truth for the configured algo/seed), then confirm it matches the upstream BLAKE3/xxh3 reference for `""`. If they differ, that is a real wiring bug — investigate before pinning.
- [ ] **Step 3: Commit** `test(crypto): known-answer vectors for BLAKE3 secure_hash + xxh3 fast_hash`.

### Task 2.3: `@traced` dead decorator (R4)

**Files:** `crates/vox-compiler/src/hir/nodes/decl.rs`, `crates/vox-compiler/src/hir/lower/decl.rs`, `crates/vox-ast/src/decl/callable.rs`.

- [ ] **Step 1: Decide (escalate if unclear):** either (a) **wire it** — add `is_traced: bool` to `HirFn`, set it in `lower_fn` from `FnDecl.is_traced`, and add a regression test that `@traced fn` → `HirFn.is_traced == true`; or (b) **remove the dead path** — delete the `is_traced` threading in `set_decorators` for fn-shaped decls and document `@traced` as agent-handler-only. Prefer (a) if any consumer (codegen/telemetry) wants it; (b) if not. Grep `is_traced` consumers first: `cargo run -p vox-cli -- ... ` / `git grep is_traced`.
- [ ] **Step 2: Write the failing test** for the chosen direction (e.g. `@traced\nfn f() to int {1}` → `hir.functions[0].is_traced`).
- [ ] **Step 3: Implement; run; green.**
- [ ] **Step 4: Commit** `fix(compiler): wire (or retire) the @traced decorator — was silently dropped`.

### Task 2.4: Decorator-order asymmetry (R5)

**Files:** `crates/vox-compiler/src/parser/descent/decl/head*.rs`.

- [ ] **Step 1: Write the failing test** asserting `@pure\n@example\nfn f() to int {1}` parses (currently errors).
- [ ] **Step 2: Run — confirm it fails** (`Expected fn, found @example`).
- [ ] **Step 3: Fix the parser** so decorator order is commutative for `@pure` + `@example`/`@test` (collect all leading decorators before dispatching on the head keyword), OR — if intentional — change the test to assert a *clear diagnostic* and document the ordering rule. Escalate if the grammar change is broad.
- [ ] **Step 4: Run; green. Commit** `fix(parser): accept @pure before @example/@test (decorator-order parity)`.

### Task 2.5: Doc-vs-impl drift sweep (R6 + R10)

**Files:** `docs/src/reference/ref-decorators.md`, `crates/vox-compiler/src/parser/descent/decl/mid.rs` doc comments.

- [ ] **Step 1:** For `@deprecated("reason")`: either implement arg parsing (thread `deprecated_reason: Option<String>` through `ConstDecl`/`FnDecl` → HIR) **or** correct the doc to the supported bare `@deprecated`. Default to the doc fix (lower cost; the value is small). Add a test pinning the chosen reality.
- [ ] **Step 2:** Fix the `parse_workflow_decl` doc comment that claims a `uses` clause never parsed.
- [ ] **Step 3: Commit** `docs(decorators): correct @deprecated arg + workflow uses drift`.

### Task 2.6: Gate the structural detectors (R9)

**Files:** wherever gate→detector severity policy lives (find via `git grep "Severity::Error" crates/vox-code-audit crates/vox-cli`).

- [ ] **Step 1:** Add fixtures: a `.rs`/`.vox` file that SHOULD trip `catch_all_swallow` and one for `cross_crate_dup`, plus a clean control. Assert the detectors fire on the bad fixture and stay silent on the control (TDD).
- [ ] **Step 2:** Promote both detectors from advisory `Info` to a gated tier (or add them to the `vox audit --gate` set that blocks), keeping them non-blocking on pre-existing findings via an allowlist baseline if needed.
- [ ] **Step 3: Commit** `feat(code-audit): gate catch_all_swallow + cross_crate_dup with fixtures`.

---

## Phase 3 — Leverage waves (the grind, made repeatable)

This is the engine for R11. **Do NOT enumerate 4,000 tasks.** Instead apply one repeatable method per crate, in leverage order, driven by the worklist, gated by `weak_test`.

**Leverage order (from the reproducible report):** `vox-codegen` (358, worst proven-ratio) → `vox-compiler` (699) → `vox-orchestrator` (666) → `vox-code-audit` (571) → `vox-publisher` (419) → `vox-populi` (330). These 6 = ~3,043 of the frontier (~53%).

### The wave method (apply to each crate)

For crate `C` with worklist `graphify-out/worklists/C.tsv`:

- [ ] **Step 1: Pull the worklist slice.** Take the next ~20–40 frontier symbols (sorted by file, so related symbols cluster). Read each symbol's source.
- [ ] **Step 2: For each symbol, find the test that ALREADY reaches it** (it's reached-but-unproven, so some test executes it). Prefer strengthening that test's assertions over writing a new one — cheaper and avoids duplicate execution.
- [ ] **Step 3: Add ONE behavioral assertion** that pins a specific output/variant/boundary/invariant of the symbol, with a `// Catches: <specific plausible bug>` comment. Forbidden: `.is_ok()`-only, `assert_ne!` on derived discriminants, re-asserting a self-constructed literal, `assert!(true)`. (The `weak_test` gate enforces this.)
- [ ] **Step 4: Run the crate's tests.** `cargo test -p C`. Green.
- [ ] **Step 5: Re-measure.** Regenerate lcov for `C` (or the workspace), re-run ingest, confirm `reached_not_proven` dropped by ~the number of symbols you proved. If it didn't drop, the assertion isn't being credited as a `proves` edge — check the overlay (the test must reference the symbol such that Phase-1 overlay records `proves`).
- [ ] **Step 6: Commit** `test(C): prove N frontier symbols (wave K)`.
- [ ] **Step 7: Lower the ratchet baseline** by the proven count in the same PR; regenerate the snapshot only if new symbols/edges appeared.
- [ ] **Step 8: Loop** Steps 1–7 until `C.tsv` is ≤ 5% remaining OR two consecutive waves prove < 10 symbols each.

### Worked first wave — `vox-codegen` (concrete template)

- [ ] **Step 1:** `head -40 graphify-out/worklists/vox-codegen.tsv` → e.g. an emit function `emit_binary_op`, a `route_manifest` builder, a `slice_list` helper.
- [ ] **Step 2:** Find the existing codegen test that runs `emit_binary_op` (e.g. in `crates/vox-codegen/src/...` test module or `crates/vox-codegen-ts/tests/`).
- [ ] **Step 3:** Add a behavioral assertion — e.g. assert the emitted TS for `a + b` is exactly `a + b` (string-equality on the emitted fragment), not just "emit returns Ok". `// Catches: binary-op emitter swapping operands or dropping the operator`.
- [ ] **Step 4:** `cargo test -p vox-codegen` → green.
- [ ] **Step 5:** Re-measure; confirm the 3 symbols moved proven.
- [ ] **Step 6:** Commit `test(vox-codegen): prove emit_binary_op/route_manifest/slice_list (wave 1)`.

> Use a **workflow** (parallel read-only design agents) to pre-draft each wave's assertions: feed one agent the worklist slice + the symbols' source, have it return exact `// Catches:` + assertion code per symbol; you implement + run + commit in the main session (subagents are read-only here). This is the same loop that delivered structural patterns #2–#7 efficiently.

---

## Phase 4 — Ratchet to the efficient ceiling, then stop honestly

- [ ] **Task 4.1:** After the top-6 crates, run `emit_unproven_worklist.py` again; sweep the long tail of smaller crates in descending frontier size, same method, until the workspace frontier is ≤ 5% of its Phase-1 value OR diminishing returns trip (two waves < 10 each).
- [ ] **Task 4.2:** Update `docs/src/architecture/semantic-coverage-status-2026-06-15.md`: final `proven_pct` (expected ~35–40%), frontier drained %, and an explicit **"efficient ceiling reached"** statement that the residual ~8,400 *neither-reached* symbols are a separate, expensive project (write-new-execution-tests) and are deliberately out of scope. No touch-tests were added to inflate the number — the `weak_test` gate proves it.
- [ ] **Task 4.3:** Flip the `weak_test` detector to fully blocking (no allowlist) once the tail is zero, so the anti-bogus-test invariant holds permanently.
- [ ] **Task 4.4: Commit + PR** the whole initiative; final reviewer pass via superpowers:requesting-code-review.

---

## Self-review

**Spec coverage:** R1→Phase 0; R2→0.2; R3→Phase 1; R4→2.3; R5→2.4; R6→2.5; R7→1.1/2.1; R8→2.2; R9→2.6; R10→2.5; R11→Phase 3/4. The "efficient 95%" definition is pinned in the framing section and enforced by the `weak_test` gate + worklist classifier. All follow-ups from the session are mapped.

**Placeholder scan:** Phases 0–2 contain real code/commands. Phase 3 is deliberately a *method + one worked wave* rather than 4,000 enumerated tasks (enumerating them would be the very touch-test fantasy the user warned against); the method is concrete (worklist → strengthen-existing-test → behavioral assert → re-measure → ratchet). Two code blocks carry explicit "match the sibling's exact `Finding`/`Severity` shape" notes because those types live in the crate and must be copied, not guessed.

**Type consistency:** `reached_not_proven` (JSON `totals`) is the single metric used by `ratchet_check.py` and the baseline. `weak_test` is the detector id used in the detector, mod registration, gate scan, and second ratchet. Worklist TSV schema (`label\tsource_file\tline`) is produced by `emit_unproven_worklist.py` and consumed by Phase 3 Step 1.

**Known risks called out inline:** the snapshot-commit decision gate (Phase 0), the `proves`-edge credit check (Phase 3 Step 5 — if assertions don't lower the number, the overlay isn't recording them), and the honest ceiling (Phase 4.2 — this does not reach 90% `proven_pct`).
