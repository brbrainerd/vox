# Handoff — Vox language features, then vox-mental-tracker completion

> **For agentic workers:** REQUIRED SUB-SKILLS: superpowers:using-git-worktrees, superpowers:writing-plans (review only — plans are pre-written), superpowers:subagent-driven-development (for parallel execution), superpowers:executing-plans (sequential fallback), superpowers:test-driven-development (per task), superpowers:verification-before-completion, superpowers:requesting-code-review.

This document is a complete, stand-alone handoff. The receiving session has no prior context. Read this whole file before taking any action.

---

## 0. Where you are

- **Repo:** `C:\Users\Owner\vox\` (vox-foundation/vox).
- **Current app branch with in-progress work:** `claude/vox-mental-tracker-baseline` ([PR #70](https://github.com/vox-foundation/vox/pull/70)).
- **Sibling open PR (platform-side, not blocking this work for the parts called out below):** [PR #68](https://github.com/vox-foundation/vox/pull/68) on branch `claude/silly-wright-065314`.
- **Main branch for new compiler PRs:** `main`.
- **Build target:** Use `target/release/vox.exe` after `cargo build --release -p vox-cli` rather than the globally-installed `~/.cargo/bin/vox` — that one is stale (the source has features the install lacks; rebuilding the install requires stopping any running `vox.exe` processes which we should not assume is safe).

### What's already landed on `claude/vox-mental-tracker-baseline`

These are the commits the app PR (#70) carries beyond `main`:

- App baseline: `apps/vox-mental-tracker/` source tree (Capacitor scaffold, contracts, app-owned docs, vitest + Playwright tests, CI workflow), plus minor additive platform shims under `crates/vox-cli`, `crates/vox-compiler/src/builtin/std/mobile.vox`, `crates/vox-compiler/src/codegen_ts/`, `crates/vox-compiler/src/typeck/`. Two pointer docs in `docs/src/how-to/`. Examples under `examples/oratio/`.
- **Phase 1 (just-landed):**
  - `apps/vox-mental-tracker/src/ts/materializer.ts` — pure-function materialization SSOT: `resolveCorrections` (collapses `correction_of` chains, latest wins, surfaces `effective_event_id` as chain root), `groupByDay` (deterministic UTC date buckets), `weeklyAggregate` (configurable window with per-kind counts).
  - `apps/vox-mental-tracker/tests/materializer.test.ts` — 11 vitest cases proving order-independence, chain collapse (A→A'→A''), orphan-correction handling, window filtering, is_backdated propagation. All green; run with `pnpm test` from the app dir.
  - `apps/vox-mental-tracker/src/main.vox` — `weekly_summary_json` now does per-kind aggregation (iterates rows, filters out non-empty `correction_of`); new `timeline_events_json()` endpoint emits a JSON array shaped for the TS materializer.
  - `apps/vox-mental-tracker/docs/architecture/data-model-ssot.md` — updated to name the materializer as the derived-state authority.

### What's NOT yet landed (the work this handoff is for)

- **Six language/compiler features** that the app's remaining phases want. All have written plans under `docs/superpowers/plans/language/2026-05-08-language-*.md`, indexed by `docs/superpowers/plans/language/2026-05-08-language-features-tracker-index.md`.
- **App phases 2-6** (voice E2E, native STT parity, clinician-grade export, hourglass verification, release-readiness). Once the language work lands, the app PR rebases on the new `main` and the remaining phases finish on `claude/vox-mental-tracker-baseline`.

---

## 1. Phase A — Land the language features

Each language plan is its own PR off `main`. **Do not commingle compiler changes with the app branch.** Use git worktrees.

### Plans (read these in this order)

| # | Plan | Path |
|---|---|---|
| 1 | struct types | `docs/superpowers/plans/language/2026-05-08-language-struct-types.md` |
| 2 | JSON parse + access stdlib | `docs/superpowers/plans/language/2026-05-08-language-json-stdlib.md` |
| 3 | match-arm statement bodies | `docs/superpowers/plans/language/2026-05-08-language-match-arm-statements.md` |
| 4 | string utilities (split/slice/char_at/index_of/starts_with/ends_with) | `docs/superpowers/plans/language/2026-05-08-language-string-utils.md` |
| 5 | regex stdlib | `docs/superpowers/plans/language/2026-05-08-language-regex-stdlib.md` |
| 6 | TS-source FFI from Vox components | `docs/superpowers/plans/language/2026-05-08-language-ts-source-ffi.md` |

Index document with rationale + ordering: `docs/superpowers/plans/language/2026-05-08-language-features-tracker-index.md`.

### Dependency graph

```
1 (structs) ─┬─> 2 (json)  (D1 of (2) wraps parsed values into structs)
             └─> 6 (ts-ffi)  (extern signatures want struct param types)
3 (match-arm stmts)   independent
4 (str utils)         independent
5 (regex)             independent of (1/2/6); pairs nicely with (4)
```

### Recommended execution: parallel where safe

Use `superpowers:subagent-driven-development`. From `main`:

**Wave 1 (in parallel — independent):**
- Subagent A: plan (1) struct types → branch `claude/lang-struct-types` → PR off main.
- Subagent B: plan (3) match-arm statement bodies → branch `claude/lang-match-arm-stmts`.
- Subagent C: plan (4) string utilities → branch `claude/lang-str-utils`.
- Subagent D: plan (5) regex stdlib → branch `claude/lang-regex`.

Wait for wave 1 to land (or at least for plan (1) to land — that's the only blocker downstream).

**Wave 2 (parallel — depend on (1)):**
- Subagent E: plan (2) JSON parse + access → branch `claude/lang-json-stdlib`. Once `main` carries struct types, `JSON.parse` D1 (`std.json.parse_typed[T]`) becomes reachable; for the initial cut leave the typed wrapper out (already noted as out of scope in the plan).
- Subagent F: plan (6) TS-source FFI → branch `claude/lang-ts-ffi`. Extern signatures can reference structs declared in the calling module.

### Per-plan workflow (each subagent)

1. Read the plan top-to-bottom. The plans use `- [ ]` checkbox syntax — track each task as a TodoWrite item.
2. Use `superpowers:using-git-worktrees` to create a fresh worktree off `main`. Do NOT touch the `claude/vox-mental-tracker-baseline` branch during compiler work.
3. Use `superpowers:test-driven-development`: write the failing test (parser test, golden snapshot, or compiler unit test) before the implementation. Most plans specify what the failing-first artifact should be (typically `examples/golden/<feature>.vox` running through the existing golden harness).
4. Implement task-by-task in the order the plan lists. Each task ends with a working `cargo nextest run -p vox-compiler`.
5. Use `superpowers:verification-before-completion` before claiming the plan done — actually run:
   - `cargo build --release -p vox-cli` (must succeed — picks up source changes for use in step 6)
   - `target/release/vox.exe check examples/golden/<the new golden>.vox` (must pass)
   - `cargo nextest run -p vox-compiler` (must pass — including any new unit tests)
   - For language features that touch Rust codegen: `cargo build --workspace` (so existing crates that consume codegen still link).
   - For language features that touch TS codegen: build a tiny example with `vox build` and `node --check` the emitted TS.
6. Use `superpowers:requesting-code-review` after self-verification. Open the PR with `gh pr create --base main`.
7. Update `docs/superpowers/plans/language/2026-05-08-language-features-tracker-index.md` to mark the plan landed (replace its row's bare title with `[landed in #NNN](url)`).

### Existing repo conventions to respect

- **Pre-push hook** (`.git/hooks/pre-push`) runs line-endings + doc-pipeline regen checks. If you add new platform-facing docs, also regenerate `docs/src/SUMMARY.md` and `docs/src/feed.xml` (`vox docs regen` or however it's wired — see the existing `pnpm docs:*` scripts).
- **Line endings:** repo enforces LF on tracked files (Windows worktree, but Git config normalizes). Don't introduce CRLF.
- **VoxCI gates:** existing CI lanes are split into guards-fast / lints / compiler-gates / tests / audits per `2026-05-03-local-ci-pre-push-and-job-split.md`. Keep added tests in the right lane (compiler tests under `crates/vox-compiler/tests/`; golden examples under `examples/golden/`).
- **Don't commit the temporary install.** The `cargo install --path crates/vox-cli` step is per-machine; don't add `target/` to commits.

---

## 2. Phase B — Complete the vox-mental-tracker app

Once **all six language plans** are merged into `main`, return to the app.

### B1. Rebase the app branch

```
git fetch origin
git checkout claude/vox-mental-tracker-baseline
git rebase origin/main
# Expect conflicts under crates/vox-cli, crates/vox-compiler/src/codegen_ts,
# and crates/vox-compiler/src/typeck — both branches added to the same areas.
# Resolve by keeping main's compiler features and re-applying the app-side
# additive shims around them.
git push --force-with-lease
```

If [PR #68](https://github.com/vox-foundation/vox/pull/68) (vox-mobile platform Phase 1) has also landed, expect a second wave of conflicts in the same files. Resolve in the same direction.

### B2. App phase plans (write these next, then execute)

These do NOT exist yet. The receiving session should write them using `superpowers:writing-plans`, one per phase, modeled on the existing `2026-05-03-local-ci-pre-push-and-job-split.md` style. Source material lives in:

- `apps/vox-mental-tracker/docs/architecture/data-model-ssot.md`
- `apps/vox-mental-tracker/docs/architecture/failure-modes-research-2026.md`
- `apps/vox-mental-tracker/docs/how-to/clinical-export.md`
- The PR #70 description itself (`gh pr view 70 --json body`).

**Phase 2 — Voice E2E (parser/confirm/edit/save).**
- Now that struct types + JSON stdlib + regex are in `main`, replace the planned `voice_*` extractor stubs with a single `type ParsedVoice { kind: str, payload_json: str, confidence: float }` and `@endpoint(kind: query) fn parse_voice(transcript: str) to ParsedVoice` in `apps/vox-mental-tracker/src/main.vox`.
- Bring `preview_voice_parse` to parity with `apps/vox-mental-tracker/src/ts/intent_parser.ts` — extract real mood scores (regex captures the digit), exercise duration (regex captures the minutes), meal description (regex captures the noun phrase). The TS parser is the reference; share fixtures via `apps/vox-mental-tracker/tests/fixtures/parser_cases.json`.
- Wire the `VoicePage` component (currently in `apps/vox-mental-tracker/src/main.vox`) end-to-end: Transcribe → Parse preview → Confirm-and-edit (state vars for kind/payload/confidence the user can tweak) → Save (calls `record_raw_transcript` first, then `record_event` with the returned `transcript_id`). Use match-arm statement bodies in error paths now that they're available.
- New tests: vitest cases proving the Vox parser and TS parser produce identical output for every fixture in `parser_cases.json`. Add a Playwright test for the full UI flow.

**Phase 3 — Native STT parity.**
- Blocked on PR #68 + a future platform Phase 2 (host-shell FFI). The skeleton plan is at `docs/superpowers/plans/2026-05-08-vox-mobile-phase2-host-shell-contract-skeleton.md`. Once that lands, replace the `SpeechRecognizer` placeholder in the Android Kotlin (`apps/vox-mental-tracker/plugins/.../SherpaTranscribe.kt`) with a JNI bridge into the cdylib's `vox-oratio` sherpa-onnx backend. iOS gets a real implementation through the same path.
- Out of scope for this handoff IF platform Phase 2 hasn't landed; defer.

**Phase 4 — Clinician-grade export completion.**
- With JSON stdlib + TS-source FFI available, `export_health_json_bundle` can call into the materializer through an extern, hash the full row dump (not just the header), include weekly aggregate metadata, and emit deterministic CSV ordering aligned with `apps/vox-mental-tracker/contracts/export/csv-columns.v1.yaml`.
- Add PDF/HTML rendering. The existing `export_clinical_html` is a stub; expand it with the materialized timeline + weekly aggregates.
- Tests: deterministic snapshot of CSV + JSON bundle for a fixed fixture.

**Phase 5 — Hourglass verification + CI lanes.**
- Expand vitest + Playwright. Add policy-runner labels per `apps/vox-mental-tracker/docs/architecture/failure-modes-research-2026.md` to the existing `.github/workflows/vox-mental-tracker.yml` (use the lane shape from `docs/superpowers/plans/ci/2026-05-03-local-ci-pre-push-and-job-split.md`).
- Add a `vox check` CI step that depends on the language features being in `main`.

**Phase 6 — Release-readiness gate.**
- Final checklist with evidence links per gate, modeled on `apps/vox-mental-tracker/docs/architecture/data-model-ssot.md` style. Add a release how-to under `apps/vox-mental-tracker/docs/how-to/`.

### B3. Verification of full app completion

When phases 2 / 4 / 5 / 6 are done (3 may stay deferred per its blocker):

- `cd apps/vox-mental-tracker && pnpm test` — all vitest suites green.
- `cd apps/vox-mental-tracker && pnpm exec playwright install && pnpm e2e` — full Playwright suite green.
- `target/release/vox.exe check apps/vox-mental-tracker/src/main.vox` — passes.
- `cargo nextest run --workspace` — passes.
- `gh pr view 70` — Test plan checkboxes all checked.
- Manual: build the Capacitor app for Android per `apps/vox-mental-tracker/docs/how-to/build-android.md` and exercise the voice flow on a device.

---

## 3. Branch hygiene rules

- One language plan = one branch off `main` = one PR. **Never** mix two plans on one branch.
- The app branch (`claude/vox-mental-tracker-baseline`) only receives:
  1. The Phase 1 commits already there.
  2. A rebase onto the new `main` after each language PR lands.
  3. The Phase 2 / 4 / 5 / 6 app commits.
- If a compiler change is small enough to squeeze into the app PR, it isn't — split it. The existing app PR is already large; reviewers won't thank you for adding a parser change to it.
- When merging language PRs, prefer **rebase-merge** to keep `main` linear. Squash-merge is fine for small plans (3, 4) but loses TDD provenance for larger ones (1, 2, 6).

---

## 4. Useful commands cheat-sheet

```powershell
# Build the dev-ready compiler binary (use this — the global install is stale)
cd C:\Users\Owner\vox; cargo build --release -p vox-cli

# Type-check a Vox file with the fresh binary
.\target\release\vox.exe check <path-to-vox-file>

# Run the app's TS test suite
cd C:\Users\Owner\vox\apps\vox-mental-tracker; pnpm test

# Inspect the open PRs
gh pr list --state open

# Inspect a plan's tasks
Get-Content docs\superpowers\plans\2026-05-08-language-struct-types.md | Select-String "^- \[ \]"
```

---

## 5. Out-of-scope reminders

These are real concerns but **NOT in this handoff**:

- **Pre-existing failing tests in `vox-corpus`** (4 failures in `vox-corpus/tests/synthetic_gen_test.rs` on origin/main, unrelated fixture-path issues). Do not let these block your work; they were broken before and they belong to a different track.
- **PR #68 (platform vox-mobile Phase 1)** and the platform Phase 2 skeleton — separate ownership.
- **PR #69 (Vox Dashboard Phase 1–3)** — separate concern.
- **`vox-corpus`, `vox-orchestrator`, `vox-populi`** changes — none expected; the language plans only touch `vox-compiler` (and possibly the runtime crate for JSON / regex helpers).
- **MCP / agent SDK work** — out of scope.

---

## 6. If something is unclear

- The plans (1)–(6) are detailed enough to execute task-by-task without further input. If a task feels under-specified, read the *referenced* file in the plan's "Files" section before asking — the plan was written with file paths so the answer is usually one read away.
- If a plan turns out to be infeasible as written (e.g., the parser's lookahead doesn't permit the proposed disambiguation), update the plan in-place with the alternative approach and a one-line "amended:" note, then proceed.
- The receiving session is ALSO authorized to discover additional language features that block app work and add them as new plan files using the same naming convention (`2026-05-08-language-<feature>.md`) and update the index. Do not start app Phase 2 with known-needed language pieces unwritten.
