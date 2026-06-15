# Retroactive Commit-Message Accuracy (git-notes overlay) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **HANDOFF NOTE (Opus → Sonnet 4.6):** Phase 0 is already built **and verified** by
> the planning agent (Opus). The graph, the per-commit audit, and the bounded diff
> extractor exist on disk and have been run successfully. Your job (Sonnet 4.6) is
> Phases 1–3: build the synthesis runner, generate accurate messages for all 2,571
> commits via OpenRouter, attach them as git notes, verify, and push. Do **not**
> rebuild Phase 0 — verify it with Task 0 and move on.

**Goal:** Attach an accurate, complete, auto-generated message describing what each
of the 2,571 non-merge commits *actually changed* as a non-destructive git note on
`refs/notes/commit-audit`, with zero history rewrite.

**Architecture:** A deterministic commit×module graph + per-commit audit (already
built, no LLM) identifies what every commit touched. A bounded diff extractor
(already built) produces a budget-capped LLM context per commit so even the 1.97M-line
mega-commits stay cheap. A resumable synthesis runner calls OpenRouter once per commit
to write a corrected Conventional-Commits message, then `git notes add` attaches it.
Notes are reversible in one command and bypass branch protection (they touch no branch).

**Tech Stack:** Python 3.12 (the graphify interpreter at `graphify-out/.graphify_python`),
git notes, OpenRouter (`google/gemini-2.5-flash`, key in `$OPENROUTER_API_KEY`).

---

## Environment constants (used in every task)

- `PY="$(cat graphify-out/.graphify_python)"` — the Python that has graphify installed
  (`C:\Users\Owner\AppData\Roaming\uv\tools\graphifyy\Scripts\python.exe`).
- Working dir: the repo worktree root (where `graphify-out/` lives).
- `NOTES_REF=refs/notes/commit-audit` — dedicated notes ref (never the default).
- OpenRouter: `base_url=https://openrouter.ai/api/v1`, `model=google/gemini-2.5-flash`,
  `env_key=OPENROUTER_API_KEY` (already exported in this environment).

## File Structure

- `graphify-out/build_commit_graph.py` — **EXISTS (Phase 0).** Builds the graph + audit. Do not modify.
- `graphify-out/bounded_diff.py` — **EXISTS (Phase 0).** `commit_context(hash, budget)` → budget-capped context. Do not modify.
- `graphify-out/commit_audit.json` — **EXISTS.** 2,571 records with flags. Read-only input.
- `graphify-out/synth_message.py` — **CREATE (Task 1–2).** One-commit synthesis via OpenRouter.
- `graphify-out/run_synthesis.py` — **CREATE (Task 3).** Resumable loop over all commits.
- `graphify-out/attach_notes.py` — **CREATE (Task 4).** Idempotent notes attach from generated files.
- `graphify-out/rewrite_journal.jsonl` — **GENERATED.** One line per completed commit (resume marker).
- `graphify-out/proposed_messages/<hash>.txt` — **GENERATED.** Final message text per commit.
- `graphify-out/tests/` — **CREATE.** pytest tests for the runner pieces.

---

## Task 0: Verify Phase 0 foundation (do not rebuild)

**Files:**
- Read: `graphify-out/commit_audit.json`, `graphify-out/bounded_diff.py`, `graphify-out/graph.json`

- [ ] **Step 1: Confirm the audit exists and is well-formed**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" -c "import json; d=json.load(open('graphify-out/commit_audit.json')); print('commits', d['total_commits'], 'flagged', d['flagged_count'], 'rec0', d['records'][0]['short'])"
```
Expected: `commits 2571 flagged 292 rec0 <somehash>`

- [ ] **Step 2: Confirm the bounded extractor enforces the budget on the worst mega-commit**

Run:
```bash
"$PY" graphify-out/bounded_diff.py 6b5f71dd4 2>&1 | tail -2
```
Expected: a line like `[context chars: stat=8617 diff=0]` — `diff=0` proves the
1.97M-line commit is stat-only (no raw hunks). If `diff` is large, STOP — the
extractor is broken; re-read `bounded_diff.py`.

- [ ] **Step 3: Confirm OpenRouter is reachable**

Run:
```bash
"$PY" -c "import os; print('KEY', 'set' if os.environ.get('OPENROUTER_API_KEY') else 'MISSING')"
```
Expected: `KEY set`. If MISSING, stop and ask the user — synthesis cannot run.

---

## Task 1: Synthesis prompt + OpenRouter call (single commit, happy path)

**Files:**
- Create: `graphify-out/synth_message.py`
- Test: `graphify-out/tests/test_synth_message.py`

- [ ] **Step 1: Write the failing test (prompt assembles the bounded context, no network)**

```python
# graphify-out/tests/test_synth_message.py
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from synth_message import build_prompt

def test_prompt_includes_stat_and_original_and_caps_size():
    ctx = {
        "hash": "abc123def", "date": "2026-01-01", "subject": "chore: stuff",
        "body": "", "stat_block": "  100+ 5- crates/vox-cli/src/main.rs",
        "diff_block": "+fn main() {}", "derived": "full",
        "total_lines": 105, "n_files": 1,
    }
    p = build_prompt(ctx)
    assert "chore: stuff" in p          # original message preserved for intent
    assert "vox-cli/src/main.rs" in p   # real stat present
    assert "Conventional Commits" in p  # instruction present
    assert len(p) < 20000               # never unbounded
```

- [ ] **Step 2: Run it to verify it fails**

Run: `"$PY" -m pytest graphify-out/tests/test_synth_message.py -q`
Expected: FAIL with `ModuleNotFoundError: No module named 'synth_message'`

- [ ] **Step 3: Write minimal implementation**

```python
# graphify-out/synth_message.py
import json, os, sys, time, urllib.request
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from bounded_diff import commit_context

MODEL = os.environ.get("SYNTH_MODEL", "google/gemini-2.5-flash")
BASE_URL = "https://openrouter.ai/api/v1/chat/completions"

SYSTEM = (
    "You rewrite git commit messages to accurately describe what a commit ACTUALLY "
    "changed, based on its real diff and file stats. Output Conventional Commits "
    "format. Be specific and complete; never invent changes not in the diff. "
    "Preserve any genuine human intent from the original message but correct the "
    "type/scope and fill in everything significant that the original omitted."
)

def build_prompt(ctx: dict) -> str:
    diff = ctx["diff_block"]
    if ctx["derived"] == "stat-only":
        diff = "(mega-commit: stat-only, no raw hunks — write the message from the file stats)"
    return (
        f"Commit {ctx['hash'][:9]} ({ctx['date']}), {ctx['total_lines']} lines across "
        f"{ctx['n_files']} files. derived={ctx['derived']}.\n\n"
        f"ORIGINAL MESSAGE (may be inaccurate/incomplete):\n{ctx['subject']}\n{ctx['body']}\n\n"
        f"FILE STATS (added+/deleted- per file):\n{ctx['stat_block']}\n\n"
        f"BOUNDED DIFF:\n{diff}\n\n"
        "Write a corrected message in Conventional Commits format:\n"
        "- First line: <type>(<scope>): <imperative subject ≤72 chars>\n"
        "- Blank line, then a body: one '- ' bullet per significant module/area changed, "
        "naming what changed there. Cover every area shown in the stats.\n"
        "Return ONLY the commit message text, no preamble, no code fences."
    )

def synth_one(h: str, budget: int = 12000, retries: int = 4) -> dict:
    ctx = commit_context(h, budget)
    prompt = build_prompt(ctx)
    payload = json.dumps({
        "model": MODEL,
        "messages": [{"role": "system", "content": SYSTEM},
                     {"role": "user", "content": prompt}],
        "temperature": 0.2, "max_tokens": 1200,
    }).encode()
    last = None
    for attempt in range(retries):
        try:
            req = urllib.request.Request(
                BASE_URL, data=payload,
                headers={"Authorization": f"Bearer {os.environ['OPENROUTER_API_KEY']}",
                         "Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=90) as r:
                data = json.loads(r.read())
            msg = data["choices"][0]["message"]["content"].strip()
            msg = msg.strip("`").replace("```", "").strip()
            footer = (f"\n\n--- audit: original={ctx['subject']!r} | "
                      f"actual={ctx['total_lines']}L across {ctx['n_files']} files | "
                      f"derived={ctx['derived']} ---")
            return {"hash": ctx["hash"], "message": msg + footer, "derived": ctx["derived"], "ok": True}
        except Exception as e:  # network / rate-limit — backoff and retry
            last = str(e); time.sleep(2 ** attempt)
    return {"hash": ctx["hash"], "message": "", "derived": ctx["derived"], "ok": False, "error": last}

if __name__ == "__main__":
    print(synth_one(sys.argv[1])["message"])
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `"$PY" -m pytest graphify-out/tests/test_synth_message.py -q`
Expected: PASS (1 passed)

- [ ] **Step 5: Commit (notes-tooling lives in gitignored graphify-out/, so commit only the test scaffold marker)**

graphify-out/ is gitignored — there is nothing to commit for this task. Skip the
commit; the journal + generated files are local artifacts by design. (If the user
later wants the runner tracked, move it under `tools/commit-audit/` in a follow-up.)

---

## Task 2: Live single-commit synthesis smoke test

**Files:**
- Use: `graphify-out/synth_message.py`

- [ ] **Step 1: Synthesize one real flagged commit end-to-end (hits OpenRouter)**

Run:
```bash
PY="$(cat graphify-out/.graphify_python)"
"$PY" graphify-out/synth_message.py 044b68cb9
```
Expected: a Conventional-Commits message that (a) is NOT typed `chore`, (b) names
multiple modules (this commit touched 85 files / 45K lines under a bare
`chore: stabilize diagnostics...` subject), and (c) ends with the `--- audit: ...`
footer. If the output is empty or errors, debug the network call before scaling up.

- [ ] **Step 2: Synthesize the mega-commit (stat-only path)**

Run: `"$PY" graphify-out/synth_message.py 6b5f71dd4`
Expected: a message derived from stats with `derived=stat-only` in the footer, no crash.

---

## Task 3: Resumable batch runner over all 2,571 commits

**Files:**
- Create: `graphify-out/run_synthesis.py`
- Test: `graphify-out/tests/test_journal_resume.py`

- [ ] **Step 1: Write the failing test (already-journaled commits are skipped)**

```python
# graphify-out/tests/test_journal_resume.py
import json, sys, os, tempfile
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from run_synthesis import pending_hashes

def test_pending_excludes_journaled(tmp_path):
    all_hashes = ["a", "b", "c"]
    journal = tmp_path / "j.jsonl"
    journal.write_text(json.dumps({"hash": "b", "ok": True}) + "\n", encoding="utf-8")
    assert pending_hashes(all_hashes, str(journal)) == ["a", "c"]
```

- [ ] **Step 2: Run it to verify it fails**

Run: `"$PY" -m pytest graphify-out/tests/test_journal_resume.py -q`
Expected: FAIL with `ModuleNotFoundError: No module named 'run_synthesis'`

- [ ] **Step 3: Write minimal implementation**

```python
# graphify-out/run_synthesis.py
import json, sys, os, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from synth_message import synth_one

OUT = Path(__file__).resolve().parent
JOURNAL = OUT / "rewrite_journal.jsonl"
MSGDIR = OUT / "proposed_messages"

def pending_hashes(all_hashes, journal_path):
    done = set()
    p = Path(journal_path)
    if p.exists():
        for ln in p.read_text(encoding="utf-8").splitlines():
            if ln.strip():
                rec = json.loads(ln)
                if rec.get("ok"):
                    done.add(rec["hash"])
    return [h for h in all_hashes if h not in done]

def main():
    MSGDIR.mkdir(exist_ok=True)
    audit = json.loads((OUT / "commit_audit.json").read_text(encoding="utf-8"))
    # newest-first: records are already in `git log` order (newest first)
    all_hashes = [r["hash"] for r in audit["records"]]
    todo = pending_hashes(all_hashes, JOURNAL)
    print(f"{len(all_hashes)} total, {len(all_hashes)-len(todo)} done, {len(todo)} pending")
    failures = []
    with JOURNAL.open("a", encoding="utf-8") as jf:
        for i, h in enumerate(todo, 1):
            res = synth_one(h)
            if res["ok"]:
                (MSGDIR / f"{h}.txt").write_text(res["message"], encoding="utf-8")
            else:
                failures.append(h)
            jf.write(json.dumps({"hash": h, "ok": res["ok"], "derived": res["derived"]}) + "\n")
            jf.flush()
            if i % 25 == 0:
                print(f"  {i}/{len(todo)} ({len(failures)} fail)")
            time.sleep(0.15)  # gentle pacing; OpenRouter paid tier has no hard RPM
    print(f"done. failures={len(failures)} -> {failures[:20]}")
    if failures:
        (OUT / "synthesis_failures.json").write_text(json.dumps(failures), encoding="utf-8")
        print("Re-run this script to retry failures (journal skips successes).")

if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `"$PY" -m pytest graphify-out/tests/test_journal_resume.py -q`
Expected: PASS (1 passed)

- [ ] **Step 5: Dry-run on a 5-commit slice before the full run**

Run:
```bash
"$PY" -c "
import json, sys; sys.path.insert(0,'graphify-out')
from run_synthesis import MSGDIR; from synth_message import synth_one
MSGDIR.mkdir(exist_ok=True)
audit=json.load(open('graphify-out/commit_audit.json'))
for r in audit['records'][:5]:
    res=synth_one(r['hash'])
    (MSGDIR/(r['hash']+'.txt')).write_text(res['message'],encoding='utf-8')
    print(r['short'], 'ok' if res['ok'] else 'FAIL', '->', res['message'].split(chr(10))[0])
"
```
Expected: 5 lines, each `ok` with a corrected first line. Eyeball that the new
subjects describe the real change. If good, proceed to the full run.

- [ ] **Step 6: Full run (all 2,571, resumable)**

Run: `"$PY" graphify-out/run_synthesis.py`
Expected: progress every 25 commits, ending `done. failures=N`. If interrupted,
just re-run — the journal skips completed commits. Re-run once more if `failures>0`.
Acceptance: `ls graphify-out/proposed_messages/*.txt | wc -l` == 2571.

---

## Task 4: Attach generated messages as git notes (idempotent)

**Files:**
- Create: `graphify-out/attach_notes.py`
- Test: `graphify-out/tests/test_attach_notes.py`

- [ ] **Step 1: Write the failing test (re-attaching the same message is a no-op)**

```python
# graphify-out/tests/test_attach_notes.py
import sys, os, subprocess, tempfile
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from attach_notes import note_matches

def test_note_matches_detects_identical(tmp_path, monkeypatch):
    # note_matches(existing, candidate) -> bool; whitespace-insensitive compare
    assert note_matches("feat: x\n\n- y\n", "feat: x\n\n- y") is True
    assert note_matches("feat: x", "fix: x") is False
```

- [ ] **Step 2: Run it to verify it fails**

Run: `"$PY" -m pytest graphify-out/tests/test_attach_notes.py -q`
Expected: FAIL with `ModuleNotFoundError: No module named 'attach_notes'`

- [ ] **Step 3: Write minimal implementation**

```python
# graphify-out/attach_notes.py
import subprocess, sys
from pathlib import Path

OUT = Path(__file__).resolve().parent
MSGDIR = OUT / "proposed_messages"
NOTES_REF = "refs/notes/commit-audit"

def note_matches(existing: str, candidate: str) -> bool:
    norm = lambda s: "\n".join(l.rstrip() for l in s.strip().splitlines())
    return norm(existing) == norm(candidate)

def _git(args, **kw):
    return subprocess.run(["git"] + args, cwd=OUT.parent, capture_output=True,
                          text=True, encoding="utf-8", errors="replace", **kw)

def existing_note(h: str):
    r = _git(["notes", "--ref", NOTES_REF, "show", h])
    return r.stdout if r.returncode == 0 else None

def main():
    files = sorted(MSGDIR.glob("*.txt"))
    added = skipped = 0
    for f in files:
        h = f.stem
        candidate = f.read_text(encoding="utf-8")
        cur = existing_note(h)
        if cur is not None and note_matches(cur, candidate):
            skipped += 1
            continue
        r = _git(["notes", "--ref", NOTES_REF, "add", "-f", "-F", str(f), h])
        if r.returncode != 0:
            print(f"FAIL {h}: {r.stderr.strip()}")
        else:
            added += 1
    print(f"notes added/updated={added} unchanged={skipped} total_files={len(files)}")

if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `"$PY" -m pytest graphify-out/tests/test_attach_notes.py -q`
Expected: PASS (1 passed)

- [ ] **Step 5: Attach all notes**

Run: `"$PY" graphify-out/attach_notes.py`
Expected: `notes added/updated=2571 unchanged=0 total_files=2571` (on a re-run:
`added/updated=0 unchanged=2571`, proving idempotency).

---

## Task 5: Verify coverage and visual correctness

**Files:** none (verification only)

- [ ] **Step 1: Assert every non-merge commit has a note**

Run:
```bash
commits=$(git log --no-merges --format=%H | wc -l)
notes=$(git notes --ref refs/notes/commit-audit list | wc -l)
echo "commits=$commits notes=$notes"
```
Expected: `commits=2571 notes=2571` (equal). If notes < commits, re-run Task 3
(synthesis) then Task 4 (attach) — some commits failed synthesis.

- [ ] **Step 2: Eyeball the overlay on recent history**

Run: `git log --notes=refs/notes/commit-audit -3`
Expected: each commit shows its original message AND a `Notes (commit-audit):`
block with the corrected message + audit footer.

- [ ] **Step 3: Spot-check a known offender**

Run: `git notes --ref refs/notes/commit-audit show 933a6f69b`
Expected: a `feat`/`refactor`-typed message naming the native-installer migration
and the modules involved (the original was `chore: migrate to native OS installers`
hiding 443K lines / 861 files).

---

## Task 6: Publish (and document reversal)

**Files:** none

- [ ] **Step 1: Confirm with the user before pushing**

This pushes a new ref to `origin`. Ask the user to confirm. (It is non-destructive
and touches no branch, so branch protection / merge queue are unaffected.)

- [ ] **Step 2: Push the notes ref**

Run: `git push origin refs/notes/commit-audit`
Expected: `* [new reference]  refs/notes/commit-audit -> refs/notes/commit-audit`

- [ ] **Step 3: Record the reversal command (for the user)**

To fully undo, locally and remotely:
```bash
git update-ref -d refs/notes/commit-audit
git push origin :refs/notes/commit-audit
```
Anyone fetching the overlay does: `git fetch origin "refs/notes/*:refs/notes/*"`
then `git log --notes=refs/notes/commit-audit`.

---

## Self-Review (completed by planning agent)

- **Spec coverage:** Phase 0 (build+verify) → Task 0. P1 synthesis (all 2,571,
  bounded, resumable, throttled, newest-first) → Tasks 1–3. P2 notes attach
  (idempotent, dedicated ref, audit footer) → Task 4. P3 verify+publish+reversal →
  Tasks 5–6. The optional P4 post-commit hook is intentionally deferred (YAGNI until
  the one-time backfill is accepted).
- **Placeholder scan:** none — every code step is complete and runnable; the two
  Phase-0 scripts referenced (`build_commit_graph.py`, `bounded_diff.py`) exist and
  were executed during planning.
- **Type consistency:** `commit_context(hash, budget)` returns the exact dict keys
  consumed by `build_prompt`/`synth_one`; `synth_one` returns `{hash,message,derived,ok}`
  consumed by `run_synthesis`; `proposed_messages/<hash>.txt` is the contract between
  Task 3 (writer) and Task 4 (reader); `note_matches(existing, candidate)` signature
  is identical in test and impl.

## Risks & guardrails

- **Token trap (top risk):** mitigated by `bounded_diff.py` — verified `diff=0` on the
  1.97M-line commit. Do not raise `--budget` above ~16000 without re-checking mega-commits.
- **Rate limits:** OpenRouter paid tier has no hard RPM, but the runner backs off
  exponentially on any error and the journal makes the whole run resumable.
- **Accuracy:** the system prompt forbids inventing changes; Task 2 + Task 5 spot-checks
  catch hallucinated modules. The audit footer preserves the original message verbatim,
  so nothing is ever lost.
- **Reversibility:** the entire overlay is two commands to delete (Task 6 Step 3).
