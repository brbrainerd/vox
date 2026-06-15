# Commit-Audit Notes — Coverage Remediation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Audience:** Claude Sonnet 4.6. Every step gives exact paths, exact code, and a test. Do not improvise schema or thresholds — they are derived from the audit numbers in §1.

**Goal:** Close the *coverage* gap in the `refs/notes/commit-audit` overlay so the notes give a genuinely honest account of what each commit changed — including an explicit, machine-checkable statement of *how much of the diff was actually read* — and re-synthesize the high-value commits whose notes are currently rollup-only because generated/vendored noise crowded out the real source.

**Architecture:** All work is local Python tooling under `graphify-out/` (gitignored) operating on a git-notes ref. Non-destructive: we only add/replace notes on `refs/notes/commit-audit`, never touch branch history. The keystone is fixing how `bounded_diff.py` separates *authored source* from *generated/vendored churn*, then re-deriving an honest per-commit `coverage_pct`, then re-synthesizing only the commits that both (a) were under-covered and (b) contain real authored source.

**Tech Stack:** Python 3 (interpreter pinned at `graphify-out/.graphify_python`), `git`, OpenRouter (`google/gemini-2.5-flash`, key in `$OPENROUTER_API_KEY`). Tests with `pytest` (already installed via `uv tool install --with pytest graphifyy`).

---

## 0. Status — what is already done and SOLID (do not redo)

The first two phases of this project are **complete and pushed**:

- **All 2,571 non-merge commits have a note** on `refs/notes/commit-audit` (live on origin).
- **Scale tag** (`scale: ~N lines across M files [XS/S/M/L/XL]`) on every note — idempotent, correct.
- **DIVERGENCE callouts** on the 25 stat-only mega-commits.
- **Truncation residual** is 6/2571 (4 confirmed false positives + 2 real) = 0.23%.
- **1,870 commits (the ≤300-line `full` tier) are genuinely, completely covered** — the LLM saw the entire diff. This is the bulk of real authored history and needs **no further work**.

This plan does **not** re-run synthesis on those 1,870 commits. It targets a specific, measured coverage gap described below.

---

## 1. The evidence (why this plan exists)

Measured from `graphify-out/commit_audit.json` + `graphify-out/rewrite_journal.jsonl` on 2026-06-15. Total changed lines across all 2,571 commits = **8,513,487**.

### 1a. Coverage is tiered by *line count*, not by what the LLM saw

| Tier | Commits | % of commits | Changed lines | % of volume |
|------|--------:|-------------:|--------------:|------------:|
| `full` (≤12k lines) | 2,508 | 97.55% | 1,103,275 | 12.96% |
| `sampled` (12k–50k) | 38 | 1.48% | 821,497 | 9.65% |
| `stat-only` (>50k) | 25 | 0.97% | 6,588,715 | **77.39%** |

**87% of the line-volume lives in 63 commits whose notes were built from file *stats*, not from reading the diff.**

### 1b. But that 87% is heavily inflated by generated/recommitted noise

The stat-only volume is dominated by content that is **not hand-authored**:
- `crates/vox-populi/src/mens/kernels/*.ptx` — compiled CUDA kernels. `quantized.ptx` (198,428 lines) alone was **recommitted across 5 commits**; the ~407k-line `.ptx` block appears identically in `6b5f71dd4`, `a9d9641de`, and `933a6f69b`.
- `patches/*/src/bindings.rs` — vendored FFI bindings (e.g. `webview2-com-sys` 43,991 lines, generated).
- `mens/runs/**/checkpoint_state.json`, `docs/agents/doc-inventory.json`, `contracts/reports/**/findings-*.json` — generated ML/report state.

The current `ELIDE_RE` in `graphify-out/bounded_diff.py` **does not match any of these**, so they counted as "source."

### 1c. The `full` label overstates coverage for line-dense commits — a real bug

In `bounded_diff.py`, the constant `budget = 12000` is used for **two different units**:
- as a **line-count** threshold for the tier label: `"full" if total_lines <= budget else "sampled"`
- as a **character** budget for diff accumulation: `if used >= budget` / `used += len(take)` (chars)

A commit with 4,806 changed lines is labeled `full`, but its diff *text* is far larger than 12,000 chars, so the LLM only saw the first ~12 KB. Direct measurement confirms: `a151c0de8` (4,806 L) and `40c71362e` (4,775 L) both produced a `diff_block` capped at exactly 12,050 chars — **<10% of the real diff** — yet are labeled `full`.

Within the `full` tier: **1,870 commits ≤300 lines are truly complete; 410 are 301–1000 lines (likely partial); 228 are >1000 lines (certainly text-truncated despite the `full` label).**

### 1d. After stripping the real noise, the non-full commits split into three classes

Re-measuring the 63 sampled+stat-only commits with a corrected elide set (adds `.ptx`, `.cu/.cuh`, `patches/`, `mens/runs/`, `checkpoint_state.json`, `doc-inventory.json`, report `findings*.json`, `.safetensors`, `.gguf`):

- **Class A — pure-noise / mechanical** (6 commits: `e9af28921`, `1b2d6fa5c`, `e502af751`, `5f4c667c2`*, `79b113a57`, `39ed9bf60`): authored source ≤12k or near-zero. Examples: "untrack 124k files from target" (35 authored lines), "unify target dir" (545). **Their rollup + DIVERGENCE note is already honest and final. No re-synthesis.**
- **Class B — tree-wide re-adds** (e.g. `6b5f71dd4` "trusted-signing v2" 1.3M authored, `a9d9641de` "eval sandbox", `1d1c33aaf` "Rebuild main tip after local object corruption", `933a6f69b` "native installers"): the diff is "the whole tree was re-touched." A per-change "what & why" is impossible because the change *is* "re-add everything." **Action: a corrected note that states this explicitly + the true semantic delta, not a feature narrative.**
- **Class C — genuine large features** (~21 fit ≤12k after strip + ~20 real `feat`/`refactor` in the 13k–142k authored range: `9d83aca24` vox-scaling-policy 142k, `2ea53f56c` MCP tool system 71k, `ae726dbfb` core orchestrator 58k, `e828828a9` plugin redesign 43k, …): real engineering that currently has only a rollup note. **These are the high-value re-synthesis targets.**

---

## 2. File structure

| File | Responsibility | Action |
|------|----------------|--------|
| `graphify-out/bounded_diff.py` | per-commit context extraction | **modify**: corrected elide set; split line-threshold from char-budget; emit `coverage_pct`, `authored_lines`, `elided_lines` |
| `graphify-out/coverage_audit.py` | **new**: classify all 2,571 commits into Class A/B/C and write `coverage_plan.json` | create |
| `graphify-out/subsynth_module.py` | **new**: per-module sub-synthesis for Class C commits over budget | create |
| `graphify-out/reclassify_megacommits.py` | **new**: Class B "tree-wide re-add" honest-note builder | create |
| `graphify-out/synth_message.py` | message synthesis + prompt | **modify**: thread `coverage_pct` into the audit footer |
| `graphify-out/verify_coverage.py` | gate | **modify**: add a `coverage_pct` gate alongside the truncation gate |
| `graphify-out/tests/test_bounded_diff_elide.py` | TDD for corrected elide + coverage_pct | create |
| `graphify-out/tests/test_coverage_audit.py` | TDD for classification | create |

> **Reminder:** everything under `graphify-out/` is gitignored. Do **not** `git add` any of it. The only durable output is the updated notes ref.

---

## 3. Tasks

### Task 1: Corrected elide set + honest coverage metrics in `bounded_diff.py`

**Files:**
- Modify: `graphify-out/bounded_diff.py`
- Test: `graphify-out/tests/test_bounded_diff_elide.py`

- [ ] **Step 1: Write the failing test**

```python
# graphify-out/tests/test_bounded_diff_elide.py
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import bounded_diff as bd

def test_elide_matches_generated_and_vendored():
    for p in [
        "crates/vox-populi/src/mens/kernels/quantized.ptx",
        "patches/webview2-com-sys-0.38.2/src/bindings.rs",
        "mens/runs/v1/checkpoint_state.json",
        "docs/agents/doc-inventory.json",
        "contracts/reports/scaling-audit/findings-latest.json",
        "crates/x/src/foo.cu",
        "weights/model.safetensors",
        "weights/model.gguf",
    ]:
        assert bd.is_elided(p), f"should elide: {p}"

def test_elide_keeps_real_source():
    for p in [
        "crates/vox-cli/src/commands/ci/config_hygiene.rs",
        "docs/src/architecture/layers.toml",
        "scripts/coverage-graph/os_compat.py",
    ]:
        assert not bd.is_elided(p), f"should NOT elide: {p}"

def test_context_emits_coverage_fields():
    # smallest real commit in the repo's first 50 — use HEAD which is small
    ctx = bd.commit_context("HEAD")
    assert "coverage_pct" in ctx
    assert "authored_lines" in ctx
    assert "elided_lines" in ctx
    assert 0.0 <= ctx["coverage_pct"] <= 1.0
```

- [ ] **Step 2: Run it, verify it fails**

Run: `$(cat graphify-out/.graphify_python) -m pytest graphify-out/tests/test_bounded_diff_elide.py -q`
Expected: FAIL — `is_elided` does not exist yet; `coverage_pct` not in ctx.

- [ ] **Step 3: Implement in `bounded_diff.py`**

Replace the module-level `ELIDE_RE` block with the corrected set, expose an `is_elided` helper, and rewrite the tail of `commit_context` to compute honest metrics. Concretely:

```python
# --- replace the existing ELIDE_RE definition with this expanded set ---
ELIDE_RE = re.compile(
    r"(\.lock$|(^|/)Cargo\.lock$|(^|/)pnpm-lock\.yaml$|(^|/)package-lock\.json$"
    r"|\.generated\.|(^|/)SUMMARY\.md$|(^|/)feed\.xml$|architecture-index"
    r"|\.min\.(js|css)$|\.(png|jpg|jpeg|gif|webp|ico|pdf|woff2?|ttf|wasm|bin|zip)$"
    r"|(^|/)node_modules/|(^|/)vendor/|(^|/)target/"
    # --- additions (see plan §1b/§1d) ---
    r"|\.ptx$|\.(cu|cuh)$"                       # compiled / CUDA kernels
    r"|(^|/)patches/"                            # vendored third-party crates
    r"|(^|/)mens/runs/"                          # ML run artifacts
    r"|checkpoint_state\.json$"                  # ML training state
    r"|(^|/)docs/agents/doc-inventory\.json$"    # generated doc inventory
    r"|(^|/)contracts/reports/.*findings.*\.json$"  # generated audit reports
    r"|\.safetensors$|\.gguf$)",                 # model weights
    re.I,
)

def is_elided(path: str) -> bool:
    """True if PATH is generated/vendored/binary and should be summarised as
    stats only, never fed as raw diff to the LLM."""
    return bool(ELIDE_RE.search(path))
```

Then, inside `commit_context`, after the `files` list and `total_lines` are built but BEFORE the mega-commit early return, compute the authored/elided split and use **authored_lines** (not raw total) for the budget decision:

```python
    elided_lines = sum(a + d for (p, a, d) in files if is_elided(p))
    authored_lines = total_lines - elided_lines

    # mega-commit decision now keys off AUTHORED source, not raw churn.
    # A 2M-line commit that is 99% generated is NOT stat-only if its authored
    # remainder fits; conversely keep the hard ceiling for genuine giants.
    if authored_lines > MEGA_LINE_THRESHOLD:
        return {
            "hash": full_hash, "author": author, "date": date, "subject": subject,
            "body": body, "stat_block": stat_block, "diff_block": "",
            "derived": "stat-only", "total_lines": total_lines,
            "authored_lines": authored_lines, "elided_lines": elided_lines,
            "n_files": len(files), "coverage_pct": 0.0,
        }
```

Finally, in the non-mega return, track how many chars of authored diff existed vs. how many were fed, and emit `coverage_pct`. Add a running `authored_diff_chars` accumulator alongside `used`, and compute:

```python
    # derived label keyed on AUTHORED lines vs a dedicated line threshold.
    # NOTE: FULL_LINE_LIMIT (lines) is separate from CHAR_BUDGET (chars for diff text).
    # Keep the existing `budget` variable (or rename it CHAR_BUDGET = 120_000) as the
    # char accumulation cap for the diff loop — do NOT reuse FULL_LINE_LIMIT there.
    FULL_LINE_LIMIT = 12000
    CHAR_BUDGET = 120_000  # char limit for diff_parts accumulation (10x the old 12k)
    derived = "full" if authored_lines <= FULL_LINE_LIMIT else "sampled"
    # coverage_pct = fraction of authored diff text actually placed in diff_block
    coverage_pct = 1.0 if authored_diff_chars == 0 else min(1.0, used / authored_diff_chars)
    return {
        "hash": full_hash, "author": author, "date": date, "subject": subject,
        "body": body, "stat_block": stat_block, "diff_block": "\n".join(diff_parts),
        "derived": derived, "total_lines": total_lines,
        "authored_lines": authored_lines, "elided_lines": elided_lines,
        "n_files": len(files), "coverage_pct": round(coverage_pct, 3),
    }
```

To populate `authored_diff_chars`, in the file loop, before truncating each non-elided file's `raw`, add its full length: `authored_diff_chars += len(raw)` (count the real hunk size, even when you only `take` part of it). Initialise `authored_diff_chars = 0` next to `used = 0`.

- [ ] **Step 4: Run the test, verify it passes**

Run: `$(cat graphify-out/.graphify_python) -m pytest graphify-out/tests/test_bounded_diff_elide.py -q`
Expected: PASS (3 tests).

- [ ] **Step 5: Sanity-check the headline number**

Run:
```bash
$(cat graphify-out/.graphify_python) -c "
import sys; sys.path.insert(0,'graphify-out')
from bounded_diff import commit_context
c = commit_context('6b5f71dd4')
print('authored:', c['authored_lines'], 'elided:', c['elided_lines'], 'coverage:', c['coverage_pct'])
"
```
Expected: `elided` is now in the hundreds-of-thousands (the `.ptx`/`patches/` blocks are caught); `authored` is much lower than the old 1.9M. (It will still be large for this tree-wide re-add — that is Class B, handled in Task 3.)

- [ ] **Step 6: Commit** — *(journal/tooling only; nothing tracked changes — skip `git add`. This "commit" step is a checkpoint marker, not a git commit, because `graphify-out/` is gitignored.)*

---

### Task 2: Classify all commits → `coverage_plan.json` (`coverage_audit.py`)

**Files:**
- Create: `graphify-out/coverage_audit.py`
- Test: `graphify-out/tests/test_coverage_audit.py`

- [ ] **Step 1: Write the failing test**

```python
# graphify-out/tests/test_coverage_audit.py
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import coverage_audit as ca

def test_classify_rules():
    # Class A: trivial authored remainder -> note already final
    assert ca.classify(authored=35, total=88036, subject="fix(gitignore): untrack 124k") == "A"
    # Class B: tree-wide re-add signalled by subject keywords + huge authored
    assert ca.classify(authored=1_316_394, total=1_968_295,
                       subject="fix(ci): migrate trusted-signing-action") == "B"
    assert ca.classify(authored=110_846, total=254_146,
                       subject="Rebuild main tip after local object corruption") == "B"
    # Class C: genuine large feature
    assert ca.classify(authored=141_885, total=242_005,
                       subject="feat(scaling): introduce vox-scaling-policy") == "C"
    # small authored, normal feature -> not a remediation target at all
    assert ca.classify(authored=120, total=200, subject="feat: small thing") == "ok"
```

- [ ] **Step 2: Run it, verify it fails** — `... -m pytest graphify-out/tests/test_coverage_audit.py -q` → FAIL (no module).

- [ ] **Step 3: Implement `coverage_audit.py`**

```python
"""Classify every non-merge commit by remediation class (plan §1d) and write
graphify-out/coverage_plan.json. Pure-deterministic; no LLM.

Classes:
  A  pure-noise/mechanical — authored remainder trivial; existing rollup is final.
  B  tree-wide re-add/cleanup — huge authored churn from re-touching the whole
     tree; per-change narrative impossible. Reclassify with an honest note.
  C  genuine large feature/refactor under-covered — re-synthesize.
  ok well-covered already (full tier, complete diff) — no action.
"""
import json, re, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from bounded_diff import commit_context

OUT = Path(__file__).resolve().parent
A_MAX_AUTHORED = 12000           # ≤ this authored => Class A if also under-covered
B_SUBJECT_RE = re.compile(
    r"\b(rebuild|re-?add|untrack|unify target|full repo cleanup|migrate to native"
    r"|object corruption|purge|remove .*tracking|restore .*backlog)\b", re.I)
B_MIN_AUTHORED = 200000          # tree-wide re-adds are enormous even after strip

def classify(authored: int, total: int, subject: str) -> str:
    if authored <= 300 and total <= 1000:
        return "ok"
    if authored <= A_MAX_AUTHORED and total > 50000:
        return "A"                       # mega by churn, trivial by authored source
    if authored >= B_MIN_AUTHORED or (B_SUBJECT_RE.search(subject) and authored >= 50000):
        return "B"
    if authored > A_MAX_AUTHORED:
        return "C"
    return "ok"

def main():
    audit = json.loads((OUT / "commit_audit.json").read_text(encoding="utf-8"))
    recs = audit["records"]
    rows = []
    counts = {"ok": 0, "A": 0, "B": 0, "C": 0}
    for i, r in enumerate(recs):
        ctx = commit_context(r["hash"])
        cls = classify(ctx["authored_lines"], ctx["total_lines"], r["subject"])
        counts[cls] += 1
        rows.append({
            "hash": r["hash"], "short": r["short"], "subject": r["subject"],
            "total_lines": ctx["total_lines"], "authored_lines": ctx["authored_lines"],
            "elided_lines": ctx["elided_lines"], "coverage_pct": ctx["coverage_pct"],
            "class": cls,
        })
        if (i + 1) % 200 == 0:
            print(f"  classified {i+1}/{len(recs)}", file=sys.stderr)
    (OUT / "coverage_plan.json").write_text(
        json.dumps({"counts": counts, "rows": rows}, indent=2), encoding="utf-8")
    print("counts:", counts)

if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run the test, verify it passes** — `... -m pytest graphify-out/tests/test_coverage_audit.py -q` → PASS.

- [ ] **Step 5: Generate the plan**

Run: `$(cat graphify-out/.graphify_python) graphify-out/coverage_audit.py`
Expected: prints `counts: {...}`; writes `graphify-out/coverage_plan.json`. Class C should be on the order of ~40 commits (the re-synthesis worklist). Record the exact counts — they drive Tasks 3–4.

---

### Task 3: Class B honest re-notes (`reclassify_megacommits.py`)

For tree-wide re-adds, do not invent a feature narrative. Emit a note that states what the commit mechanically is and the **semantic delta** (files whose content actually changed vs. pure re-add).

**Files:** Create `graphify-out/reclassify_megacommits.py`

- [ ] **Step 1: Implement** — for each Class B hash, compute the semantic delta with `git show --numstat` already in hand, plus a one-line statement; build the note body as:

```python
"""Build honest notes for Class B (tree-wide re-add) commits.

A Class B commit re-touches most of the tree (rebase recovery, target untrack,
native-installer migration). We do NOT ask the LLM for a feature story — we
state the mechanical reality + directory rollup + the existing scale/DIVERGENCE
lines, and set coverage_pct=stat-only honestly.
"""
import json, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from bounded_diff import commit_context, _run
from add_difficulty import insert_scale_line

OUT = Path(__file__).resolve().parent

def build_note(h: str) -> str:
    ctx = commit_context(h)
    # directory rollup from numstat (top 25 dirs by lines)
    import collections, re
    dirs = collections.Counter()
    for ln in _run(["show", "--numstat", "--format=", h]).strip().splitlines():
        m = re.match(r"^(\S+)\t(\S+)\t(.+)$", ln)
        if not m:
            continue
        a = int(m.group(1)) if m.group(1).isdigit() else 0
        d = int(m.group(2)) if m.group(2).isdigit() else 0
        p = m.group(3).split("/")[0]
        dirs[p] += a + d
    roll = "\n".join(f"- {k}: {v:,} lines" for k, v in dirs.most_common(25))
    subj = ctx["subject"]
    body = (
        f"{subj}\n\n"
        f"MECHANICAL COMMIT (tree-wide re-add / cleanup — not a single feature). "
        f"This commit re-touched {ctx['n_files']:,} files "
        f"({ctx['total_lines']:,} lines changed, of which {ctx['elided_lines']:,} "
        f"are generated/vendored). A per-change rationale is not meaningful; the "
        f"directory rollup below is the honest summary.\n\n"
        f"Directory rollup:\n{roll}\n\n"
        f"--- audit: class=B (tree-wide re-add) | coverage=stat-only | "
        f"authored~{ctx['authored_lines']:,}L | original='{subj}' ---"
    )
    return insert_scale_line(body, ctx["total_lines"], ctx["n_files"])

def main():
    plan = json.loads((OUT / "coverage_plan.json").read_text(encoding="utf-8"))
    bdir = OUT / "proposed_messages"
    for row in plan["rows"]:
        if row["class"] != "B":
            continue
        (bdir / f"{row['hash']}.txt").write_text(build_note(row["hash"]), encoding="utf-8")
        print("reclassified B:", row["short"], row["subject"][:50])

if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run it** — `$(cat graphify-out/.graphify_python) graphify-out/reclassify_megacommits.py`. Spot-check one output file (e.g. `6b5f71dd4...txt`) — it must NOT claim to be a feature; it must state "tree-wide re-add."

---

### Task 4: Class C re-synthesis with per-module sub-synthesis (`subsynth_module.py`)

Class C commits are real features. Those whose authored source ≤12k re-synthesize directly via the existing `synth_one` (the corrected `bounded_diff` now strips the noise so the real diff fits). Those still >12k get per-module sub-synthesis: summarize each top-level module's authored diff separately, then compose one message.

**Files:** Create `graphify-out/subsynth_module.py`

- [ ] **Step 1: PRE-CHECK — confirm `_post` exists in `synth_message.py`**

Before writing any code, open `graphify-out/synth_message.py` and find the function that makes the raw OpenRouter HTTP call (it builds a `payload` with `model`, `messages`, `max_tokens` and posts to the API). If it is inlined inside `synth_one` rather than a standalone function, **refactor it out first** as:
```python
def _post(prompt: str, max_tokens: int = 1200, system: str = SYSTEM) -> str:
    """Returns the message content string from OpenRouter."""
    ...  # move the existing payload-build + requests.post + retry block here
```
Then re-run any existing `synth_message` tests to confirm nothing broke. Only after `_post` is confirmed importable should you proceed.

Similarly, confirm `_run` is a module-level function in `bounded_diff.py` (it is used in Task 3's `reclassify_megacommits.py`). If it's nested inside another function or named differently, expose it at module level before proceeding.

- [ ] **Step 2: Implement `subsynth_module.py`**

```python
"""Class C re-synthesis. For each Class C commit:
  - if authored_lines <= 12000: re-run synth_one (corrected bounded_diff now
    feeds the real source because noise is elided).
  - else: per-module sub-synthesis — group authored files by top-level dir,
    summarise each group's bounded diff with one LLM call, then a final compose
    call turns the per-module bullets into a single Conventional-Commits message.
Writes results to proposed_messages/<hash>.txt and appends to rewrite_journal.jsonl.
"""
import json, re, sys, collections
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from bounded_diff import commit_context, is_elided, _run
from synth_message import synth_one, _post   # _post = the raw OpenRouter call helper
from add_difficulty import insert_scale_line

OUT = Path(__file__).resolve().parent
MODULE_BUDGET = 10000

def _authored_files(h):
    files = []
    for ln in _run(["show", "--numstat", "--format=", h]).strip().splitlines():
        m = re.match(r"^(\S+)\t(\S+)\t(.+)$", ln)
        if not m:
            continue
        a = int(m.group(1)) if m.group(1).isdigit() else 0
        d = int(m.group(2)) if m.group(2).isdigit() else 0
        p = m.group(3)
        if " => " in p:
            p = re.sub(r"\{[^}]*=> ([^}]*)\}", r"\1", p).split(" => ")[-1].strip()
        if is_elided(p):
            continue
        files.append((p, a + d))
    return files

def _module_summary(h, module, paths):
    diff = ""
    for p in paths:
        raw = _run(["show", "--format=", "--unified=2", h, "--", p])
        diff += raw[:2000] + "\n"
        if len(diff) > MODULE_BUDGET:
            diff = diff[:MODULE_BUDGET] + "\n... (module diff truncated)"
            break
    prompt = (f"Module `{module}` of commit {h[:9]}. Summarise what changed here "
              f"and why, in 1-3 '- ' bullets. Authored diff:\n{diff}\n"
              f"Return only the bullets.")
    return _post(prompt, max_tokens=400)

def resynth(h):
    ctx = commit_context(h)
    if ctx["authored_lines"] <= 12000:
        res = synth_one(h, concise=True, max_tokens=2500)
        return res  # synth_one already wrote the file + journal line
    # per-module path
    groups = collections.defaultdict(list)
    for p, n in _authored_files(h):
        groups[p.split("/")[0] + ("/" + p.split("/")[1] if "/" in p else "")].append(p)
    bullets = []
    for module, paths in sorted(groups.items(), key=lambda kv: -sum(1 for _ in kv[1])):
        bullets.append(_module_summary(h, module, paths))
    compose = (f"Commit {h[:9]} ({ctx['subject']}). Below are per-module change "
               f"summaries. Write ONE Conventional-Commits message: subject line "
               f"<=72 chars, blank line, then the consolidated bullets (dedup, keep "
               f"every module). Per-module summaries:\n\n" + "\n".join(bullets) +
               "\n\nReturn only the commit message.")
    msg = _post(compose, max_tokens=3000)
    msg = insert_scale_line(msg, ctx["total_lines"], ctx["n_files"])
    msg += (f"\n\n--- audit: class=C (per-module sub-synthesis) | "
            f"coverage~module-level | authored~{ctx['authored_lines']:,}L ---")
    (OUT / "proposed_messages" / f"{h}.txt").write_text(msg, encoding="utf-8")
    with (OUT / "rewrite_journal.jsonl").open("a", encoding="utf-8") as f:
        f.write(json.dumps({"hash": h, "ok": True, "derived": "submodule",
                            "repair": True}) + "\n")
    return {"hash": h, "ok": True}

def main():
    plan = json.loads((OUT / "coverage_plan.json").read_text(encoding="utf-8"))
    todo = [r["hash"] for r in plan["rows"] if r["class"] == "C"]
    print(f"Class C re-synthesis: {len(todo)} commits")
    for i, h in enumerate(todo):
        resynth(h)
        print(f"  {i+1}/{len(todo)} {h[:9]} done")

if __name__ == "__main__":
    main()
```

> **NOTE for Sonnet:** `synth_message.py` may not currently expose a `_post(prompt, max_tokens)` helper. Before Step 1, open `synth_message.py` and confirm the name of the function that performs the raw OpenRouter HTTP call (it builds the `payload` with `model`, `messages`, `max_tokens`). If it is inlined inside `synth_one`, **first refactor it out** into a module-level `def _post(prompt: str, max_tokens: int = 1200, system: str = SYSTEM) -> str:` that returns the message content string, and re-run `graphify-out/tests/test_synth_message.py` to confirm nothing broke. Only then write `subsynth_module.py`. Do not duplicate the HTTP/retry logic.

- [ ] **Step 3: Dry-run on ONE Class C commit first** — temporarily set `todo = todo[:1]` (or call `resynth("9d83aca24...")` directly), inspect the output file. Confirm it reads as a real per-module account, not a rollup. Then run the full set.

- [ ] **Step 4: Run the full Class C set** — `$(cat graphify-out/.graphify_python) graphify-out/subsynth_module.py`. This makes real OpenRouter calls; expect a few minutes and a few cents.

---

### Task 5: Coverage gate + re-attach + push

**Files:** Modify `graphify-out/verify_coverage.py`; reuse `graphify-out/attach_notes.py`.

- [ ] **Step 1: Add a coverage gate to `verify_coverage.py`** — after the existing truncation gate, load `coverage_plan.json` and build a set of Class C hashes that have NOT yet been re-synthesized. Add to `gate()`:

```python
    import collections as _col
    # latest-per-hash from the append-only journal
    journal_path = OUT / "rewrite_journal.jsonl"
    latest = {}
    for ln in journal_path.read_text(encoding="utf-8").splitlines():
        if not ln.strip():
            continue
        rec = json.loads(ln)
        latest[rec["hash"]] = rec

    plan_path = OUT / "coverage_plan.json"
    if plan_path.exists():
        plan = json.loads(plan_path.read_text(encoding="utf-8"))
        class_c_unremediated = [
            r["hash"] for r in plan["rows"]
            if r["class"] == "C"
            and not (latest.get(r["hash"], {}).get("repair") is True)
        ]
        if class_c_unremediated:
            msgs.append(
                f"{len(class_c_unremediated)} Class C commits still rollup-only: "
                + ", ".join(h[:9] for h in class_c_unremediated[:5])
                + ("..." if len(class_c_unremediated) > 5 else "")
            )
```

- [ ] **Step 2: Re-attach all notes** — `$(cat graphify-out/.graphify_python) graphify-out/attach_notes.py` (idempotent; overwrites notes for the regenerated hashes only).

- [ ] **Step 3: Run the gate** — `$(cat graphify-out/.graphify_python) graphify-out/verify_coverage.py`. Expected: `PASS`. If it lists Class C stragglers, re-run Task 4 for those hashes.

- [ ] **Step 4: Regenerate the divergence report** — `$(cat graphify-out/.graphify_python) graphify-out/divergence_report.py` (now reflects honest coverage_pct).

- [ ] **Step 5: Push the notes update**

```bash
git push origin refs/notes/commit-audit
```

> **Pre-push gotcha (known):** the pre-push hook runs `vox ci ssot-drift`, which requires the installed `vox` binary to (a) embed the current `contracts/cli/command-registry.yaml` and (b) be built at the current HEAD commit. If the push fails with "command-registry … does not match" or "installed vox is stale", run `cargo build -p vox-cli`, then copy `target/debug/vox.exe` over `~/.cargo/bin/vox.exe` (stop any running `vox` process first), then retry. If `gui-surface-coverage drift` appears, run `vox ci gui-surface-coverage --write` and commit the result on the branch (it is tracked, unlike `graphify-out/`).

---

## 4. Self-review checklist (run before declaring done)

- [ ] `coverage_plan.json` counts sum to 2,571 and Class C count matches the number of commits re-synthesized in Task 4.
- [ ] No Class A commit was re-synthesized (their rollup is final — re-running wastes tokens and can re-introduce truncation).
- [ ] At least one Class B output file manually confirmed to say "tree-wide re-add," not a feature story.
- [ ] At least one Class C output file manually confirmed to contain real per-module substance absent from the old rollup.
- [ ] `verify_coverage.py` passes both the truncation gate AND the new coverage gate.
- [ ] Nothing under `graphify-out/` was `git add`-ed.
- [ ] Notes pushed; `git log --notes=refs/notes/commit-audit -1 <a Class C hash>` shows the new note on origin after `git fetch origin "refs/notes/*:refs/notes/*"`.

## 5. Reversal / safety

- Notes are non-destructive and reversible: `git update-ref -d refs/notes/commit-audit` (local) + `git push origin :refs/notes/commit-audit` (remote).
- Never force-push, never rewrite branch history, never touch branch protection.
- Do not raise `MEGA_LINE_THRESHOLD` or the per-module budget without re-checking that Class C outputs stay non-truncated (re-run `detect_truncated.flagged_hashes()`).
- The original synthesis journal (`rewrite_journal.jsonl`) is append-only; Task 4 appends `repair`/`submodule` lines, so the latest-per-hash logic in `verify_coverage.py` picks up the new state automatically.

---

## Appendix — exact remediation worklist (from §1d measurement, 2026-06-15)

Class B (tree-wide re-add, honest re-note — Task 3): `6b5f71dd4`, `a9d9641de`, `1d1c33aaf`, `933a6f69b`, plus any others the classifier tags B (e.g. very large `purge`/`restore backlog` commits). The classifier is authoritative; this list is a sanity anchor.

Class C high-value re-synthesis (Task 4), confirmed real features currently rollup-only:
`9d83aca24` (vox-scaling-policy, 142k authored), `2ea53f56c` (MCP tool system, 71k), `ae726dbfb` (core orchestrator, 58k), `e828828a9` (plugin redesign, 43k), `e7efc46bd` (MCP toolset), `b670b4a62` (gui dock nav), `23e872e2a` (scientia self-publication), `0959122fa` (autonomous orchestrator), `7feb7886c` (autonomic + LLM target), `7a05b8102` (gui coverage gate), and the ~20 other sampled `feat`/`refactor` commits in the 13k–142k authored band. Use `coverage_plan.json` rows where `class == "C"` as the definitive set.
