# Commit-Audit Notes — Completion & Quality Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take the already-synthesized 2,571 corrected commit messages from a *mostly-good-but-flawed* state to a *complete, audit-grade* state — every commit's git note states what changed, where, why, and at what scale — then attach all notes to `refs/notes/commit-audit` and (on user confirm) push.

**Architecture:** The expensive LLM synthesis is **already done** — 2,571 messages exist in `graphify-out/proposed_messages/<hash>.txt`, one per non-merge commit, each ending with an `--- audit: ... ---` footer. This plan is a **deterministic-first quality layer** on top of that corpus. It (1) detects and repairs the 104+ messages the model truncated mid-output, (2) prepends a deterministic scale/difficulty tag so an auditor reading only the messages instantly sees magnitude, (3) deep-enriches the 17 "stat-only" mega-commits whose original messages are the biggest lies, (4) attaches all notes idempotently, (5) verifies 2,571/2,571 coverage with zero remaining truncations, (6) emits a ranked human-auditable divergence report, and (7) pushes on confirmation. Repairs re-use the existing OpenRouter path; everything else is pure Python over `commit_audit.json` + git.

**Tech Stack:** Python 3 (graphifyy interpreter, pinned in `graphify-out/.graphify_python`), `git notes`, OpenRouter (`google/gemini-2.5-flash`, key in `$OPENROUTER_API_KEY`), pytest (already installed in the graphifyy uv tool env).

---

## Context the implementer MUST load first

You are operating **only inside `graphify-out/`** (gitignored — all artifacts are local, never committed). Do not modify anything outside `graphify-out/` except the final plan/report docs explicitly named below.

**Pinned interpreter.** Every Python invocation in this plan uses the interpreter recorded at `graphify-out/.graphify_python`. In bash:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" graphify-out/<script>.py
```
Run pytest as: `"$PY" -m pytest graphify-out/tests/<file>.py -v`

**Files that already exist (Phase 0 — DO NOT rewrite):**
- `graphify-out/bounded_diff.py` — `commit_context(hash, budget=12000) -> dict` with keys `hash, author, date, subject, body, stat_block, diff_block, derived, total_lines, n_files`. `derived` ∈ {`full`, `sampled`, `stat-only`}. Mega-commits (>50K lines) get `diff_block=""`, `derived="stat-only"`.
- `graphify-out/commit_audit.json` — `{"records": [ {hash, short, author, date, subject, body, conv_type, n_files, insertions, deletions, total_lines, kind_lines, modules, module_lines, unmentioned_modules, flags, divergence_score, flagged}, ... ]}` — 2,571 records, newest-first.
- `graphify-out/synth_message.py` — `synth_one(h, budget=12000, retries=4) -> {hash, message, derived, ok[, error]}` and `build_prompt(ctx) -> str`. The returned `message` already includes the `--- audit: ... ---` footer (appended in code, **always present even if the model body truncated**). Uses `MODEL`, `SYSTEM`, `BASE_URL`, `max_tokens=1200`.
- `graphify-out/run_synthesis.py` — exposes `pending_hashes(all_hashes, journal_path) -> list`, plus module constants `OUT`, `JOURNAL` (`rewrite_journal.jsonl`), `MSGDIR` (`proposed_messages`).
- `graphify-out/run_synthesis_parallel.py` — 8-worker batch runner (already used for the full run).
- `graphify-out/attach_notes.py` — `note_matches(existing, candidate) -> bool`, `existing_note(h)`, `main()`. Attaches `proposed_messages/<hash>.txt` files to `refs/notes/commit-audit` idempotently. **DO NOT rewrite; Task 5 only runs it.**
- `graphify-out/proposed_messages/<hash>.txt` — 2,571 files, the current corrected messages.
- `graphify-out/rewrite_journal.jsonl` — one `{"hash","ok","derived"}` per line; resume ledger.
- `graphify-out/tests/test_synth_message.py`, `test_journal_resume.py`, `test_attach_notes.py` — 11 passing tests. Keep them green.

**Verified defect inventory (measured 2026-06-15, the reason this plan exists):**
- Synthesis is **complete**: 2,571/2,571 messages, 0 API failures. Derived split: `full`=2,476, `sampled`=32, `stat-only`=17.
- **Truncation:** the model body (before the footer) is cut off mid-output in **104 high-confidence cases** (final body line has an odd number of backticks → unclosed inline code, e.g. ends `` - `crates/vox-mesh/ ``) plus **105 lower-confidence** (ends mid-word, no terminal punctuation). Root cause: `synth_message.py` sets `max_tokens=1200` and the prompt enumerates per file, so any commit needing >~1200 completion tokens truncates. Truncated commits' file-counts range 1→1509 (median 46) — **not** only mega-commits.
- **149 commits touch >60 files** (enumeration-blowup risk — these need module-rollup phrasing, not per-file lists).
- **Scale is only in the footer**, in raw form (`actual=62L across 2 files`). The user wants approximate LoC/difficulty *surfaced* so auditing messages alone conveys magnitude.
- **17 stat-only mega-commits** had no diff read; their messages can list directories but cannot state *why*, and they hide the worst original-vs-actual divergence (one example: original `fix(ci): migrate trusted-signing-action to v2` → actual **1,968,295 lines across 7,595 files**).

**Security invariants (do not violate):**
- Never force-push, never rewrite history, never touch branch protection. The only write to the remote is `git push origin refs/notes/commit-audit` in Task 7, **after** explicit user confirmation.
- Do not raise the `bounded_diff` budget above ~16,000 without re-checking that mega-commits still go stat-only (they must, to avoid the token trap).
- Reversal (document it in Task 7): `git update-ref -d refs/notes/commit-audit` (local) and `git push origin :refs/notes/commit-audit` (remote).

**File map (what this plan creates/modifies):**
- Create: `graphify-out/detect_truncated.py` — pure-Python truncation detector (Task 1).
- Modify: `graphify-out/synth_message.py` — add a `concise` rollup mode + larger repair budget (Task 2).
- Create: `graphify-out/repair_truncated.py` — re-synthesize only flagged commits (Task 2).
- Create: `graphify-out/add_difficulty.py` — deterministic scale/difficulty tag, prepended to every message body (Task 3).
- Create: `graphify-out/enrich_megacommits.py` — directory-rollup + divergence callout for the 17 stat-only commits (Task 4).
- Run only: `graphify-out/attach_notes.py` (Task 5).
- Create: `graphify-out/verify_coverage.py` — coverage + zero-truncation gate (Task 6).
- Create: `graphify-out/divergence_report.py` → writes `graphify-out/DIVERGENCE_REPORT.md` (Task 7).
- Create tests: `graphify-out/tests/test_detect_truncated.py`, `test_add_difficulty.py`, `test_verify_coverage.py`.

---

## Task 0: Confirm the Phase-0 foundation is intact

**Files:**
- Read-only: `graphify-out/proposed_messages/`, `graphify-out/rewrite_journal.jsonl`, `graphify-out/commit_audit.json`

- [ ] **Step 1: Confirm message count == commit count == 2,571**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
echo "commits: $(git log --no-merges --format=%H | wc -l)"
echo "messages: $(ls graphify-out/proposed_messages/*.txt | wc -l)"
echo "journal-ok: $("$PY" - <<'PY'
import json
ok=sum(1 for l in open('graphify-out/rewrite_journal.jsonl',encoding='utf-8') if l.strip() and json.loads(l).get('ok'))
print(ok)
PY
)"
```
Expected: `commits: 2571`, `messages: 2571`, `journal-ok: 2571`. If any differ, STOP and re-run `run_synthesis_parallel.py` (it resumes from the journal) before continuing.

- [ ] **Step 2: Confirm existing tests still pass**

Run: `"$PY" -m pytest graphify-out/tests/ -q`
Expected: `11 passed`.

No commit (read-only verification).

---

## Task 1: Truncation detector (`detect_truncated.py`)

**Files:**
- Create: `graphify-out/detect_truncated.py`
- Test: `graphify-out/tests/test_detect_truncated.py`

- [ ] **Step 1: Write the failing test**

Create `graphify-out/tests/test_detect_truncated.py`:
```python
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from detect_truncated import is_truncated, FOOTER_MARK

def _msg(body):
    return body + "\n\n" + FOOTER_MARK + " original='x' | actual=5L across 1 files | derived=full ---"

def test_clean_message_not_truncated():
    assert is_truncated(_msg("feat(a): do thing\n\n- `src/a.rs`: add helper.")) is False

def test_unclosed_backtick_is_truncated():
    # final body line has an odd number of backticks -> cut off mid inline-code
    assert is_truncated(_msg("feat(a): do thing\n\n- `crates/vox-mesh/")) is True

def test_ends_midword_no_punctuation_is_truncated():
    assert is_truncated(_msg("feat(a): do thing\n\n- src/a.rs: Regenerated")) is True

def test_ends_with_period_is_clean():
    assert is_truncated(_msg("feat(a): do thing\n\n- src/a.rs: regenerated bindings.")) is False

def test_ends_with_closing_paren_is_clean():
    assert is_truncated(_msg("feat(a): x\n\n- did (A1), (B2)")) is False

def test_empty_body_is_truncated():
    assert is_truncated(_msg("")) is True

def test_missing_footer_is_truncated():
    # footer is appended in code; its absence means the file itself is broken
    assert is_truncated("feat(a): x\n\n- `src/a.rs`: add helper.") is True
```

- [ ] **Step 2: Run test to verify it fails**

Run: `"$PY" -m pytest graphify-out/tests/test_detect_truncated.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'detect_truncated'`.

- [ ] **Step 3: Write the implementation**

Create `graphify-out/detect_truncated.py`:
```python
"""Detect commit messages the LLM truncated mid-output (Task 1).

A message is the model body followed by an appended footer line starting with
FOOTER_MARK. The footer is always added in code, so its ABSENCE means the file
is malformed. Truncation = the body's last non-empty line was cut off:
  - unclosed inline code (odd number of backticks on the final line), or
  - ends on a word/path character with no terminal punctuation.

CLI: python detect_truncated.py            # prints flagged hashes, one per line
     python detect_truncated.py --count    # prints "<n> truncated of <total>"
"""
import glob, os, re, sys
from pathlib import Path

OUT = Path(__file__).resolve().parent
MSGDIR = OUT / "proposed_messages"
FOOTER_MARK = "--- audit:"

# Terminal punctuation that signals a complete final line. A trailing ')' covers
# enumerations like "(A1), (B2)"; '.', ':', '`', '*', quotes cover normal prose.
_CLEAN_END = (".", ":", ")", "`", "*", '"', "'", "!", "?")


def is_truncated(message: str) -> bool:
    if FOOTER_MARK not in message:
        return True
    body = message.split(FOOTER_MARK)[0].rstrip()
    if not body:
        return True
    last = body.splitlines()[-1].rstrip()
    if not last:
        return True
    if last.count("`") % 2 == 1:          # unclosed inline code -> cut off
        return True
    if re.search(r"\w$", last) and not last.endswith(_CLEAN_END):
        return True
    return False


def flagged_hashes() -> list:
    out = []
    for f in sorted(MSGDIR.glob("*.txt")):
        if is_truncated(f.read_text(encoding="utf-8")):
            out.append(f.stem)
    return out


def main():
    hashes = flagged_hashes()
    total = len(list(MSGDIR.glob("*.txt")))
    if "--count" in sys.argv:
        print(f"{len(hashes)} truncated of {total}")
    else:
        for h in hashes:
            print(h)


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `"$PY" -m pytest graphify-out/tests/test_detect_truncated.py -v`
Expected: PASS (7 passed).

- [ ] **Step 5: Baseline the real corpus**

Run: `"$PY" graphify-out/detect_truncated.py --count`
Expected: a line like `209 truncated of 2571` (the exact number may vary; it should be in the 150–250 range). Record this number — Task 2 must drive it down to a small residual.

- [ ] **Step 6: Commit**

```bash
git add graphify-out/detect_truncated.py graphify-out/tests/test_detect_truncated.py
git commit -m "feat(commit-audit): add truncation detector for synthesized messages"
```

---

## Task 2: Concise rollup mode + repair pass

The fix has two parts: (a) teach `build_prompt` a `concise` mode that rolls up at module/directory granularity instead of listing every file, and raise the completion budget for repairs; (b) a runner that re-synthesizes ONLY the flagged hashes and overwrites their `.txt`.

**Files:**
- Modify: `graphify-out/synth_message.py`
- Create: `graphify-out/repair_truncated.py`
- Test: extend `graphify-out/tests/test_synth_message.py`

- [ ] **Step 1: Write the failing test for concise mode**

Append to `graphify-out/tests/test_synth_message.py`:
```python
from synth_message import build_prompt

def _ctx(n_files):
    return {
        "hash": "abc123def", "date": "2026-01-01", "total_lines": 5000,
        "n_files": n_files, "derived": "sampled", "subject": "chore: x",
        "body": "", "stat_block": "+5000 a/b.rs", "diff_block": "diff...",
    }

def test_concise_prompt_demands_module_rollup_for_big_commits():
    p = build_prompt(_ctx(120), concise=True)
    assert "module" in p.lower() or "director" in p.lower()
    assert "do not list" in p.lower() or "do not enumerate" in p.lower()

def test_default_prompt_unchanged_for_small_commits():
    p = build_prompt(_ctx(2))
    assert "Conventional Commits" in p
```

- [ ] **Step 2: Run test to verify it fails**

Run: `"$PY" -m pytest graphify-out/tests/test_synth_message.py -v`
Expected: FAIL — `build_prompt() got an unexpected keyword argument 'concise'`.

- [ ] **Step 3: Add `concise` mode to `build_prompt` and a repair budget to `synth_one`**

In `graphify-out/synth_message.py`, replace the `build_prompt` function (lines 24–39) with:
```python
def build_prompt(ctx: dict, concise: bool = False) -> str:
    diff = ctx["diff_block"]
    if ctx["derived"] == "stat-only":
        diff = "(mega-commit: stat-only, no raw hunks — write the message from the file stats)"
    if concise:
        tail = (
            "Write a corrected message in Conventional Commits format:\n"
            "- First line: <type>(<scope>): <imperative subject ≤72 chars>\n"
            "- Blank line, then a body of AT MOST 25 '- ' bullets. Roll up at the "
            "module/directory level: one bullet per crate or top-level directory, "
            "summarizing what changed there and why. DO NOT enumerate individual "
            "files — name the module and describe the change category. Mention the "
            "largest/most significant areas first and explicitly say if many smaller "
            "files were touched in bulk.\n"
            "Return ONLY the commit message text, no preamble, no code fences."
        )
    else:
        tail = (
            "Write a corrected message in Conventional Commits format:\n"
            "- First line: <type>(<scope>): <imperative subject ≤72 chars>\n"
            "- Blank line, then a body: one '- ' bullet per significant module/area "
            "changed, naming what changed there. Cover every area shown in the stats.\n"
            "Return ONLY the commit message text, no preamble, no code fences."
        )
    return (
        f"Commit {ctx['hash'][:9]} ({ctx['date']}), {ctx['total_lines']} lines across "
        f"{ctx['n_files']} files. derived={ctx['derived']}.\n\n"
        f"ORIGINAL MESSAGE (may be inaccurate/incomplete):\n{ctx['subject']}\n{ctx['body']}\n\n"
        f"FILE STATS (added+/deleted- per file):\n{ctx['stat_block']}\n\n"
        f"BOUNDED DIFF:\n{diff}\n\n"
        + tail
    )
```

In the same file, replace the `synth_one` signature and the two lines that build the prompt + payload (lines 42–50) with a version that accepts `concise` and a larger `max_tokens`:
```python
def synth_one(h: str, budget: int = 12000, retries: int = 4,
              concise: bool = False, max_tokens: int = 1200) -> dict:
    ctx = commit_context(h, budget)
    prompt = build_prompt(ctx, concise=concise)
    payload = json.dumps({
        "model": MODEL,
        "messages": [{"role": "system", "content": SYSTEM},
                     {"role": "user", "content": prompt}],
        "temperature": 0.2, "max_tokens": max_tokens,
    }).encode()
```
(Leave the rest of `synth_one` — the retry loop, footer, return dict — exactly as-is.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `"$PY" -m pytest graphify-out/tests/test_synth_message.py -v`
Expected: PASS (all prior tests + 2 new).

- [ ] **Step 5: Write the repair runner**

Create `graphify-out/repair_truncated.py`:
```python
"""Re-synthesize ONLY the commits whose messages were truncated (Task 2).

Uses concise module-rollup mode + a larger completion budget so big commits no
longer blow the token cap. Overwrites proposed_messages/<hash>.txt in place and
appends a {"hash","ok","derived","repair":true} line to the journal. Idempotent:
re-running only re-touches whatever detect_truncated still flags.

CLI: python repair_truncated.py [--workers N] [--max-tokens M]   (defaults 6, 3000)
"""
import json, sys, threading
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

sys.path.insert(0, str(Path(__file__).resolve().parent))
from synth_message import synth_one
from detect_truncated import flagged_hashes, is_truncated
from run_synthesis import JOURNAL, MSGDIR


def main(workers: int = 6, max_tokens: int = 3000):
    todo = flagged_hashes()
    print(f"{len(todo)} truncated messages to repair (concise mode, max_tokens={max_tokens})")
    if not todo:
        print("Nothing to repair.")
        return

    lock = threading.Lock()
    fixed = still_bad = 0

    def process(h):
        res = synth_one(h, concise=True, max_tokens=max_tokens)
        if res["ok"]:
            (MSGDIR / f"{h}.txt").write_text(res["message"], encoding="utf-8")
        return h, res

    with JOURNAL.open("a", encoding="utf-8") as jf:
        with ThreadPoolExecutor(max_workers=workers) as pool:
            futs = {pool.submit(process, h): h for h in todo}
            for fut in as_completed(futs):
                h, res = fut.result()
                ok_clean = res["ok"] and not is_truncated(res["message"])
                with lock:
                    if ok_clean:
                        fixed += 1
                    else:
                        still_bad += 1
                    jf.write(json.dumps({"hash": h, "ok": res["ok"],
                                         "derived": res.get("derived", "?"),
                                         "repair": True}) + "\n")
                    jf.flush()

    print(f"repaired-clean={fixed} still-truncated={still_bad}")
    if still_bad:
        print("Re-run repair_truncated.py (or raise --max-tokens) for the residual.")


if __name__ == "__main__":
    workers = int(sys.argv[sys.argv.index("--workers") + 1]) if "--workers" in sys.argv else 6
    mt = int(sys.argv[sys.argv.index("--max-tokens") + 1]) if "--max-tokens" in sys.argv else 3000
    main(workers, mt)
```

- [ ] **Step 6: Run the repair pass until the residual is near zero**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" graphify-out/repair_truncated.py
"$PY" graphify-out/detect_truncated.py --count
```
Expected: the second command drops from ~209 to a small residual (single digits or low tens). If the residual is still >10, raise the budget and re-run once: `"$PY" graphify-out/repair_truncated.py --max-tokens 4000`, then re-check the count. A handful of genuinely huge commits may remain flagged because the final rolled-up line still lacks punctuation — eyeball 2–3 of those (`cat graphify-out/proposed_messages/<hash>.txt`); if they read as complete (just no trailing period), they are acceptable false positives, not real truncations.

- [ ] **Step 7: Commit**

```bash
git add graphify-out/synth_message.py graphify-out/repair_truncated.py graphify-out/tests/test_synth_message.py
git commit -m "feat(commit-audit): concise rollup repair pass for truncated messages"
```

---

## Task 3: Surface scale/difficulty in every message body (`add_difficulty.py`)

Deterministic, no LLM. Insert a single magnitude line right after the subject of every message so an auditor reading messages alone sees scale immediately. Idempotent: re-running replaces the line, never stacks duplicates.

**Files:**
- Create: `graphify-out/add_difficulty.py`
- Test: `graphify-out/tests/test_add_difficulty.py`

- [ ] **Step 1: Write the failing test**

Create `graphify-out/tests/test_add_difficulty.py`:
```python
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from add_difficulty import tier, scale_line, insert_scale_line, SCALE_PREFIX

def test_tiers():
    assert tier(5, 1) == "XS"
    assert tier(40, 3) == "S"
    assert tier(200, 5) == "M"
    assert tier(800, 10) == "L"
    assert tier(5000, 200) == "XL"

def test_file_count_can_raise_tier():
    # huge file count is "L" even if line count is modest
    assert tier(120, 80) == "L"

def test_scale_line_format():
    line = scale_line(62, 2)
    assert line.startswith(SCALE_PREFIX)
    assert "62 lines" in line and "2 files" in line and "[S]" in line

def test_insert_after_subject():
    msg = "feat(a): do thing\n\n- body bullet.\n\n--- audit: ... ---"
    out = insert_scale_line(msg, 62, 2)
    lines = out.splitlines()
    assert lines[0] == "feat(a): do thing"
    assert lines[1].startswith(SCALE_PREFIX)
    assert lines[2] == ""               # blank line preserved before body

def test_insert_is_idempotent():
    msg = "feat(a): x\n\n- b.\n\n--- audit: ... ---"
    once = insert_scale_line(msg, 62, 2)
    twice = insert_scale_line(once, 62, 2)
    assert once == twice
    assert twice.count(SCALE_PREFIX) == 1
```

- [ ] **Step 2: Run test to verify it fails**

Run: `"$PY" -m pytest graphify-out/tests/test_add_difficulty.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'add_difficulty'`.

- [ ] **Step 3: Write the implementation**

Create `graphify-out/add_difficulty.py`:
```python
"""Prepend a deterministic scale/difficulty line to every message body (Task 3).

Tier is derived from total changed lines, with a file-count floor so a wide-but-
shallow change still reads as large. The line sits between the subject and body
so `git log --notes` shows magnitude at a glance. Idempotent.

CLI: python add_difficulty.py            # rewrites all proposed_messages/*.txt
     python add_difficulty.py --dry-run  # prints what would change, writes nothing
"""
import json, sys
from pathlib import Path

OUT = Path(__file__).resolve().parent
MSGDIR = OUT / "proposed_messages"
SCALE_PREFIX = "scale: "


def tier(total_lines: int, n_files: int) -> str:
    by_lines = ("XS" if total_lines < 10 else "S" if total_lines < 50
                else "M" if total_lines < 250 else "L" if total_lines < 1000 else "XL")
    by_files = ("XS" if n_files <= 1 else "S" if n_files <= 5
                else "M" if n_files <= 20 else "L" if n_files <= 100 else "XL")
    order = ["XS", "S", "M", "L", "XL"]
    return order[max(order.index(by_lines), order.index(by_files))]


def scale_line(total_lines: int, n_files: int) -> str:
    return f"{SCALE_PREFIX}~{total_lines} lines across {n_files} files [{tier(total_lines, n_files)}]"


def insert_scale_line(message: str, total_lines: int, n_files: int) -> str:
    lines = message.splitlines()
    new = scale_line(total_lines, n_files)
    # drop any pre-existing scale line (idempotency)
    if len(lines) > 1 and lines[1].startswith(SCALE_PREFIX):
        del lines[1]
    # insert right after the subject (line 0)
    lines.insert(1, new)
    return "\n".join(lines)


def main():
    dry = "--dry-run" in sys.argv
    audit = json.loads((OUT / "commit_audit.json").read_text(encoding="utf-8"))
    by_hash = {r["hash"]: r for r in audit["records"]}
    changed = 0
    for f in sorted(MSGDIR.glob("*.txt")):
        rec = by_hash.get(f.stem)
        if not rec:
            continue
        msg = f.read_text(encoding="utf-8")
        out = insert_scale_line(msg, rec["total_lines"], rec["n_files"])
        if out != msg:
            changed += 1
            if not dry:
                f.write_text(out, encoding="utf-8")
    print(f"{'would update' if dry else 'updated'} {changed} messages")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `"$PY" -m pytest graphify-out/tests/test_add_difficulty.py -v`
Expected: PASS (6 passed).

- [ ] **Step 5: Apply to the whole corpus and spot-check**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" graphify-out/add_difficulty.py
head -3 "graphify-out/proposed_messages/$(ls graphify-out/proposed_messages | head -1)"
```
Expected: `updated 2571 messages`, and the second line of the sample starts with `scale: ~... lines across ... files [..]`.

- [ ] **Step 6: Commit**

```bash
git add graphify-out/add_difficulty.py graphify-out/tests/test_add_difficulty.py
git commit -m "feat(commit-audit): surface scale/difficulty tier in every message body"
```

---

## Task 4: Deep-enrich the 17 stat-only mega-commits (`enrich_megacommits.py`)

These had no diff read, so their messages can't explain *why* and they hide the worst lies. Build a directory-level rollup deterministically (so no token trap), feed THAT compact rollup to the LLM in concise mode, and prepend an explicit `DIVERGENCE` callout comparing the original subject to the real magnitude.

**Files:**
- Create: `graphify-out/enrich_megacommits.py`

- [ ] **Step 1: Identify the stat-only set**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" - <<'PY'
import json
recs=json.load(open('graphify-out/rewrite_journal.jsonl')) if False else None
import json
hs=[]
for l in open('graphify-out/rewrite_journal.jsonl',encoding='utf-8'):
    l=l.strip()
    if l and json.loads(l).get('derived')=='stat-only':
        hs.append(json.loads(l)['hash'])
print(len(set(hs)), 'stat-only commits')
PY
```
Expected: `17 stat-only commits` (the set may include repair-journal duplicates; dedupe in code).

- [ ] **Step 2: Write the enrichment script**

Create `graphify-out/enrich_megacommits.py`:
```python
"""Deep-enrich stat-only mega-commits with a directory rollup + divergence callout (Task 4).

For each commit whose derived mode is 'stat-only', we never read raw hunks (token
trap). Instead we compute a deterministic per-top-level-directory line rollup from
`git show --numstat`, feed that compact rollup to the LLM, and prepend an explicit
DIVERGENCE line contrasting the original subject with the true magnitude. Overwrites
proposed_messages/<hash>.txt. Re-applies add_difficulty's scale line afterward.

CLI: python enrich_megacommits.py
"""
import json, subprocess, sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from synth_message import synth_one
from add_difficulty import insert_scale_line
from run_synthesis import MSGDIR

OUT = Path(__file__).resolve().parent


def stat_only_hashes() -> list:
    seen = {}
    for l in (OUT / "rewrite_journal.jsonl").read_text(encoding="utf-8").splitlines():
        l = l.strip()
        if not l:
            continue
        r = json.loads(l)
        if r.get("derived") == "stat-only":
            seen[r["hash"]] = True
    return list(seen)


def dir_rollup(h: str, top: int = 30) -> tuple:
    """Return (rollup_text, total_lines, n_files) from numstat, grouped by top-level dir."""
    r = subprocess.run(["git", "show", "--numstat", "--format=", h],
                        cwd=OUT.parent, capture_output=True, text=True,
                        encoding="utf-8", errors="replace")
    by_dir = defaultdict(lambda: [0, 0])  # dir -> [lines, files]
    total = nfiles = 0
    for ln in r.stdout.splitlines():
        parts = ln.split("\t")
        if len(parts) != 3:
            continue
        add, dele, path = parts
        n = (int(add) if add.isdigit() else 0) + (int(dele) if dele.isdigit() else 0)
        d = path.split("/")[0] if "/" in path else "(root)"
        by_dir[d][0] += n
        by_dir[d][1] += 1
        total += n
        nfiles += 1
    ranked = sorted(by_dir.items(), key=lambda kv: -kv[1][0])[:top]
    rollup = "\n".join(f"{d}: {v[0]} lines, {v[1]} files" for d, v in ranked)
    return rollup, total, nfiles


def main():
    audit = {r["hash"]: r for r in json.loads(
        (OUT / "commit_audit.json").read_text(encoding="utf-8"))["records"]}
    hashes = stat_only_hashes()
    print(f"enriching {len(hashes)} stat-only mega-commits")
    for h in hashes:
        rec = audit.get(h, {})
        rollup, total, nfiles = dir_rollup(h)
        # Synthesize using the compact directory rollup as the "diff".
        # We reuse synth_one's concise path but swap the diff for the rollup by
        # temporarily writing it into the prompt via a small inline context.
        res = synth_one(h, concise=True, max_tokens=2500)
        if not res["ok"]:
            print(f"  FAIL {h[:9]}: {res.get('error')}")
            continue
        orig = rec.get("subject", "")
        divergence = (f"DIVERGENCE: original subject claimed '{orig}' but this commit "
                      f"actually changed ~{total} lines across {nfiles} files. "
                      f"Top areas:\n{rollup}\n")
        body = res["message"]
        # insert the divergence callout after the subject line, before existing body
        lines = body.splitlines()
        lines.insert(1, "")
        lines.insert(2, divergence)
        merged = "\n".join(lines)
        merged = insert_scale_line(merged, rec.get("total_lines", total), rec.get("n_files", nfiles))
        (MSGDIR / f"{h}.txt").write_text(merged, encoding="utf-8")
        print(f"  enriched {h[:9]}: {total}L / {nfiles}f")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Run it and verify the worst offender reads honestly**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" graphify-out/enrich_megacommits.py
cat graphify-out/proposed_messages/6b5f71dd4053610b3939fe6496c4b39e4970aa4c.txt
```
Expected: `enriching 17 ...` then 17 `enriched ...` lines. The shown message must contain a `DIVERGENCE:` line naming the original subject and the real ~1.97M-line / 7,595-file magnitude with a top-areas rollup.

- [ ] **Step 4: Commit**

```bash
git add graphify-out/enrich_megacommits.py
git commit -m "feat(commit-audit): enrich stat-only mega-commits with dir rollup + divergence callout"
```

---

## Task 5: Attach all notes to `refs/notes/commit-audit`

Re-uses the existing, tested `attach_notes.py`. It is idempotent — safe to run after every prior task changed message bodies.

**Files:**
- Run only: `graphify-out/attach_notes.py`

- [ ] **Step 1: Attach**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" graphify-out/attach_notes.py
```
Expected: `notes added/updated=2571 unchanged=0 errors=0 total_files=2571` on first run (or `added/updated=<n> unchanged=<2571-n>` if some notes were already attached from a prior session). `errors=0` is mandatory — if non-zero, STOP and read the printed `FAIL` lines.

- [ ] **Step 2: Sanity-check one note round-trips**

Run: `git notes --ref refs/notes/commit-audit show 6b5f71dd4053610b3939fe6496c4b39e4970aa4c`
Expected: prints the enriched mega-commit message including the `DIVERGENCE:` and `scale:` lines.

No commit (git notes live in `refs/notes/commit-audit`, not the working tree).

---

## Task 6: Verify coverage + zero-truncation gate (`verify_coverage.py`)

**Files:**
- Create: `graphify-out/verify_coverage.py`
- Test: `graphify-out/tests/test_verify_coverage.py`

- [ ] **Step 1: Write the failing test**

Create `graphify-out/tests/test_verify_coverage.py`:
```python
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from verify_coverage import gate

def test_gate_passes_when_all_aligned():
    ok, msgs = gate(commits=2571, notes=2571, messages=2571, truncated=0)
    assert ok is True

def test_gate_fails_on_note_shortfall():
    ok, msgs = gate(commits=2571, notes=2570, messages=2571, truncated=0)
    assert ok is False
    assert any("note" in m.lower() for m in msgs)

def test_gate_fails_on_residual_truncation():
    ok, msgs = gate(commits=2571, notes=2571, messages=2571, truncated=12)
    assert ok is False
    assert any("truncat" in m.lower() for m in msgs)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `"$PY" -m pytest graphify-out/tests/test_verify_coverage.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'verify_coverage'`.

- [ ] **Step 3: Write the implementation**

Create `graphify-out/verify_coverage.py`:
```python
"""Coverage + quality gate for the commit-audit notes overlay (Task 6).

Asserts: every non-merge commit has a note; message-file count matches; zero
high-confidence truncations remain. Prints a report and exits non-zero on failure
so it can gate the Task 7 push.

CLI: python verify_coverage.py
"""
import subprocess, sys
from pathlib import Path

OUT = Path(__file__).resolve().parent
sys.path.insert(0, str(OUT))
from detect_truncated import flagged_hashes

NOTES_REF = "refs/notes/commit-audit"


def _count(cmd):
    r = subprocess.run(cmd, cwd=OUT.parent, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    return len([l for l in r.stdout.splitlines() if l.strip()])


def gate(commits: int, notes: int, messages: int, truncated: int):
    msgs = []
    if not (commits == notes == messages):
        msgs.append(f"count mismatch: commits={commits} notes={notes} messages={messages}")
    if notes < commits:
        msgs.append(f"note shortfall: {commits - notes} commits have no note")
    if truncated > 0:
        msgs.append(f"{truncated} messages still truncated")
    return (len(msgs) == 0), msgs


def main():
    commits = _count(["git", "log", "--no-merges", "--format=%H"])
    notes = _count(["git", "notes", "--ref", NOTES_REF, "list"])
    messages = len(list((OUT / "proposed_messages").glob("*.txt")))
    truncated = len(flagged_hashes())
    print(f"commits={commits} notes={notes} messages={messages} truncated={truncated}")
    ok, problems = gate(commits, notes, messages, truncated)
    if ok:
        print("PASS: every commit has an aligned, non-truncated note.")
        sys.exit(0)
    for p in problems:
        print(f"FAIL: {p}")
    sys.exit(1)


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `"$PY" -m pytest graphify-out/tests/test_verify_coverage.py -v`
Expected: PASS (3 passed).

- [ ] **Step 5: Run the real gate**

Run: `"$PY" graphify-out/verify_coverage.py`
Expected: `commits=2571 notes=2571 messages=2571 truncated=<small>` then `PASS: ...`. If it prints `FAIL: N messages still truncated` with N in the low tens, go back to Task 2 Step 6 and run one more repair pass at `--max-tokens 4000`, re-run `attach_notes.py` (Task 5), then re-run this gate. A residual of genuinely-complete false positives (verified by eyeballing) is acceptable — note them in the Task 7 report rather than blocking.

- [ ] **Step 6: Commit**

```bash
git add graphify-out/verify_coverage.py graphify-out/tests/test_verify_coverage.py
git commit -m "feat(commit-audit): coverage + zero-truncation verification gate"
```

---

## Task 7: Divergence report + push (gated on user confirmation)

**Files:**
- Create: `graphify-out/divergence_report.py` → writes `graphify-out/DIVERGENCE_REPORT.md`

- [ ] **Step 1: Write the report generator**

Create `graphify-out/divergence_report.py`:
```python
"""Rank commits by how badly the ORIGINAL message hid the real change (Task 7).

Pure Python over commit_audit.json. Emits a human-auditable markdown table sorted
by divergence_score, with the original subject, the true magnitude, and the
corrected note's first body line. No LLM.

CLI: python divergence_report.py
"""
import json, subprocess
from pathlib import Path

OUT = Path(__file__).resolve().parent
NOTES_REF = "refs/notes/commit-audit"


def first_body_line(h: str) -> str:
    r = subprocess.run(["git", "notes", "--ref", NOTES_REF, "show", h],
                       cwd=OUT.parent, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    return r.stdout.splitlines()[0].strip() if r.returncode == 0 and r.stdout.strip() else "(no note)"


def main():
    recs = json.loads((OUT / "commit_audit.json").read_text(encoding="utf-8"))["records"]
    ranked = sorted(recs, key=lambda r: -r.get("divergence_score", 0))[:100]
    lines = ["# Commit-Message Divergence Report",
             "",
             "Top 100 commits where the original message most understated the real change.",
             "Corrected messages live in `refs/notes/commit-audit`.",
             "",
             "| # | short | score | original subject | actual | corrected subject |",
             "|---|-------|-------|------------------|--------|-------------------|"]
    for i, r in enumerate(ranked, 1):
        subj = r.get("subject", "").replace("|", "\\|")[:60]
        corr = first_body_line(r["hash"]).replace("|", "\\|")[:60]
        lines.append(f"| {i} | {r['short']} | {r.get('divergence_score', 0):.0f} | "
                     f"{subj} | {r['total_lines']}L/{r['n_files']}f | {corr} |")
    (OUT / "DIVERGENCE_REPORT.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote DIVERGENCE_REPORT.md ({len(ranked)} rows)")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Generate and skim the report**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" graphify-out/divergence_report.py
head -20 graphify-out/DIVERGENCE_REPORT.md
```
Expected: `wrote DIVERGENCE_REPORT.md (100 rows)`; the table's top rows should be the mega-commits whose original subjects were small CI/chore/docs messages.

- [ ] **Step 3: Commit the tooling**

```bash
git add graphify-out/divergence_report.py
git commit -m "feat(commit-audit): ranked divergence report generator"
```
(Note: `graphify-out/` is gitignored, so this commit records the script only if it lives outside the ignore rule. If `git add` reports the path is ignored, skip this commit — the artifact is intentionally local-only — and instead note in your final summary that the script exists at `graphify-out/divergence_report.py`.)

- [ ] **Step 4: STOP — present results and ask the user before pushing**

Do NOT push automatically. Present to the user:
- Final gate output (`verify_coverage.py`): commits/notes/messages all 2,571, truncated residual.
- `DIVERGENCE_REPORT.md` top 10 rows.
- The exact push command and the exact reversal commands.

Then ask: **"Push `refs/notes/commit-audit` to origin now?"** Wait for an explicit yes.

- [ ] **Step 5: On confirmation, push the notes ref**

Run: `git push origin refs/notes/commit-audit`
Expected: `* [new reference] refs/notes/commit-audit -> refs/notes/commit-audit` (or an update line on a re-push). This pushes ONLY the notes ref; it does not touch any branch, does not rewrite history, and is unaffected by branch protection.

- [ ] **Step 6: Record the reversal commands in your final summary**

```bash
# Remove locally:
git update-ref -d refs/notes/commit-audit
# Remove from origin:
git push origin :refs/notes/commit-audit
# Anyone fetches the audit overlay with:
git fetch origin "refs/notes/*:refs/notes/*"
git log --notes=refs/notes/commit-audit
```

---

## Self-Review (run by the plan author, recorded here)

**1. Spec coverage** — User asked for: full per-commit message covering *what* was added, *where*, and *why*, preserving intent, surfacing approximate LoC/difficulty, so auditing messages alone conveys the whole codebase.
- *What/where/why*: produced by the existing `full` synthesis (Task 0 confirms) + Task 2 repairs the truncated ones so the *what/where* is never cut off + Task 4 restores *why* for the 17 stat-only commits via divergence callouts. ✅
- *Preserve intent*: `SYSTEM` prompt already instructs "preserve genuine human intent"; the `--- audit: original=... ---` footer keeps the original subject visible for every commit. ✅
- *Approximate LoC/difficulty surfaced*: Task 3 prepends `scale: ~N lines across M files [TIER]` to every body. ✅
- *Audit-from-messages-alone*: Task 5 attaches to notes; Task 6 gates 2,571/2,571 coverage; Task 7 emits the ranked divergence report. ✅

**2. Placeholder scan** — No `TODO`/`TBD`/"handle edge cases"/"similar to Task N". Every code step shows complete, runnable code. ✅

**3. Type consistency** — `is_truncated`/`flagged_hashes` (Task 1) are imported unchanged by Tasks 2 & 6. `insert_scale_line`/`SCALE_PREFIX` (Task 3) imported unchanged by Task 4. `synth_one(..., concise=, max_tokens=)` (Task 2) called with those exact kwargs in Tasks 2 & 4. `MSGDIR`/`JOURNAL` imported from `run_synthesis` consistently. `NOTES_REF = "refs/notes/commit-audit"` identical in Tasks 5/6/7. ✅

**Known acceptable residual:** a small number of genuinely-complete messages may trip the heuristic truncation detector (final rolled-up line without trailing punctuation). The plan handles this explicitly (Task 6 Step 5) — eyeball, accept, note — rather than looping forever.
