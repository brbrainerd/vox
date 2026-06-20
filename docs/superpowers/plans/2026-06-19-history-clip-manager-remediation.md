# History & Clip Manager — Remediation Plan (Sonnet 4.6 edition)

> **For agentic workers:** REQUIRED SUB-SKILL: use `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` (or `executing-plans`) to implement task-by-task; `test-driven-development.skill.md` per task. Steps use `- [ ]` checkboxes.

**Goal:** Fix every defect found in the Plan-7 adversarial review so the History & Clip Manager is a correct, honest, fully-wired system — including making `SearchCorpus::ClipHistory` real, replacing two guard-dodging allowlist entries with root-cause refactors, un-bundling non-Plan-7 changes, wiring configurable retention, broadening redaction, adding the missing GUI actions and real tests — then record the outcome in the Antigravity/Gemini-Flash ledger.

**Architecture:** Surgical fixes on top of the landed Plan-7 code. Backend stays in `vox-db` (history store + redaction), search integrates through the existing `SearchCorpus` pipeline, GUI/CLI reuse existing surfaces. Two layering violations move DB access out of the GUI command / orchestrator core and into typed `vox-db` accessors so the allowlist entries can be deleted.

**Tech Stack:** Rust (`vox-db`, `vox-db-types`, `vox-search`, `vox-orchestrator`, `vox-gui`, `vox-cli`); React/TS + vitest (`vox-gui/ui`).

**Review source:** the adversarial review of Plan 7 (this session). **Execution branch:** the Plan-7 code lives on `claude/auto-gui-debug-plans-2026-06-18` (commit `69f3b1475b` and predecessors) — execute there, not on a fresh branch. **Design refs:** [`../specs/2026-06-19-history-clip-manager-design.md`](../specs/2026-06-19-history-clip-manager-design.md) + its Codebase Audit Addendum.

---

## Operating notes (Sonnet 4.6)

1. **TDD + atomic + committed.** Each task: failing test → minimal fix → green → commit. Keep the tree compiling at every commit.
2. **Verify-before-use (light).** The exact line numbers below are from the review; before editing, `rg`/read to confirm the symbol still matches, then edit. If reality differs, adapt — don't blindly apply a stale line number.
3. **Vox house rules.** No `cargo fmt --all` (use `cargo fmt -p <crate>`); `vox-gui` clippy `--lib`; no new `.ps1/.sh/.py`; no `.unwrap()`/`.expect()` in library/command code; `docs/src/` `.md` needs frontmatter.
4. **Verification ritual** before each commit: `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`; for TS, from `crates/vox-gui/ui`: `npm test` + `npm run build`. Paste real output.
5. **Guards must pass WITHOUT new allowlist entries.** Tasks 6–7 must make `query-all-guard` and `turso-import-guard` pass by refactoring, then REMOVE the allowlist lines — not by adding more.

---

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `crates/vox-db/src/history_store.rs` | caps wiring, dedupe evict, token_estimate, LIKE escaping | 1, 2, 3 |
| `crates/vox-db/src/redact.rs` | broaden secret patterns | 4 |
| `crates/vox-db-types/src/retrieval.rs` | `SearchCorpus::ClipHistory` variant | 5 |
| `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs` · `crates/vox-gui/src/commands/search.rs` · `crates/vox-search/src/execution.rs` | handle the new corpus | 5 |
| `crates/vox-gui/src/commands/history.rs` · `history_cli.rs` | drop `.unwrap()`; wire real search | 5, 8 |
| `crates/vox-db/src/activity_store.rs` (new) · `crates/vox-gui/src/commands/activity.rs` · `crates/vox-orchestrator/src/orchestrator/core/init.rs` | typed accessors; remove allowlist entries | 6, 7 |
| `docs/agents/{query-all,turso-import}-allowlist.txt` | delete the two dodged entries | 6, 7 |
| `crates/vox-gui/ui/src/components/surfaces/History/HistoryPanel.tsx` (+ test) | re-run/re-insert actions + real tests | 9, 10 |
| `crates/vox-cli/src/commands/history_cli.rs` (+ test) | CLI tests | 11 |
| `.gitignore` · `crates/vox-gui/ui/playwright-report/` | untrack generated report | 12 |
| `docs/superpowers/antigravity-handoff-ledger.md` | record remediation (AGH-0023) | 13 |

**Pre-flight (run once):** `cargo run -p vox-arch-check`; `cargo test -p vox-db`; `git log --oneline -3` (confirm you're on the Plan-7 branch with `69f3b1475b` present).

---

## Task 1: Wire configurable caps into `add_entry`; delete the duplicate `evict`

`add_entry` hardcodes `HistoryCaps::default()` for eviction (so configurable caps are dead), and a standalone `evict()` duplicates the same SQL but is never called in production.

**Files:** Modify `crates/vox-db/src/history_store.rs`.

- [ ] **Step 1: Write the failing test.** Add to the `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn add_entry_honors_injected_caps() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db");
    let caps = HistoryCaps { clip: 1, command: 50, chat: 50 };
    for i in 0..3 {
        add_entry_with_caps(&db, "r1", "clip", &format!("c{i}"), &format!("c{i}"), 1000 + i, "cli", &caps)
            .await.expect("add");
    }
    let clips = list_entries(&db, "r1", Some("clip"), 50).await.expect("list");
    assert_eq!(clips.len(), 1, "cap=1 should retain only the newest clip");
    assert_eq!(clips[0].text, "c2");
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-db add_entry_honors_injected_caps` → FAIL (`add_entry_with_caps` missing).

- [ ] **Step 3: Implement.** Refactor so eviction lives in ONE place. Make `evict(db, repo_id, kind, caps)` the single implementation (per-kind, pin-aware — keep the existing `DELETE … WHERE pinned = 0 AND id NOT IN (SELECT … LIMIT cap)` SQL). Add `add_entry_with_caps(...)` that inserts then calls `evict(db, repo_id, kind, caps)`. Keep `add_entry(...)` as a thin wrapper: `add_entry_with_caps(db, repo, kind, text, redacted, created_at, source, &HistoryCaps::default())`. Remove the inline DELETE block (lines ~59–77) and the now-redundant standalone full-repo `evict` loop — there is exactly one eviction code path.

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-db add_entry_honors_injected_caps` + `cargo test -p vox-db history` → PASS.

- [ ] **Step 5: Commit.** `cargo clippy -p vox-db -- -D warnings && cargo fmt -p vox-db; git add crates/vox-db/src/history_store.rs && git commit -m "fix(db): single pin-aware eviction path honoring injected HistoryCaps"`

---

## Task 2: Real `token_estimate`

The column is always written `0` (dead). Compute a cheap deterministic estimate.

**Files:** Modify `crates/vox-db/src/history_store.rs`.

- [ ] **Step 1: Write the failing test.**

```rust
#[test]
fn estimate_tokens_is_roughly_chars_over_four() {
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("abcd"), 1);
    assert_eq!(estimate_tokens("the quick brown fox"), 4); // 19 chars / 4
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-db estimate_tokens_is_roughly_chars_over_four` → FAIL.

- [ ] **Step 3: Implement.** Add `pub fn estimate_tokens(text: &str) -> i64 { (text.chars().count() / 4) as i64 }`. In `add_entry_with_caps`, replace the `0i64, // token_estimate` literal with `estimate_tokens(text)`.

- [ ] **Step 4: Run → PASS.**

- [ ] **Step 5: Commit.** `git commit -m "fix(db): compute history token_estimate instead of hardcoded 0"`

---

## Task 3: Escape LIKE metacharacters in `search_entries`

A query containing `%` or `_` currently matches everything.

**Files:** Modify `crates/vox-db/src/history_store.rs`.

- [ ] **Step 1: Write the failing test.**

```rust
#[tokio::test]
async fn search_treats_percent_as_literal() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db");
    add_entry(&db, "r1", "clip", "100% done", "100% done", 1000, "cli").await.unwrap();
    add_entry(&db, "r1", "clip", "nothing here", "nothing here", 1001, "cli").await.unwrap();
    let hits = search_entries(&db, "r1", "%", 50).await.unwrap();
    assert_eq!(hits.len(), 1, "'%' must match the literal percent, not all rows");
    assert!(hits[0].text.contains("100%"));
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-db search_treats_percent_as_literal` → FAIL (matches both rows).

- [ ] **Step 3: Implement.** In `search_entries`, escape the query before wrapping: `let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"); let pattern = format!("%{escaped}%");` and add `ESCAPE '\\'` to the SQL `LIKE ?N ESCAPE '\\'`. Keep the value passed via `params!`.

- [ ] **Step 4: Run → PASS.**

- [ ] **Step 5: Commit.** `git commit -m "fix(db): escape LIKE metacharacters in history search"`

---

## Task 4: Broaden secret redaction

Current patterns: only `sk-…`/`ghp_…`/`AKIA…`. Add JWT, PEM private keys, generic bearer, and Turso/libsql tokens.

**Files:** Modify `crates/vox-db/src/redact.rs`.

- [ ] **Step 1: Write the failing test.**

```rust
#[test]
fn redact_masks_jwt_pem_bearer_and_turso() {
    for secret in [
        "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.abc123def456",         // JWT
        "Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456",     // bearer
        "-----BEGIN PRIVATE KEY-----MIIBVgIBADANBg-----END PRIVATE KEY-----", // PEM
        "eyJ0eXA9...libsql-auth-token-aaaaaaaaaaaaaaaaaaaa",          // turso-ish long token
    ] {
        let (masked, flagged) = redact(secret);
        assert!(flagged, "should flag: {secret}");
        assert!(!masked.contains("PRIVATE KEY-----MIIBVgIBADAN"), "PEM body leaked");
    }
    assert_eq!(redact("just normal text").0, "just normal text");
}
```

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-db redact_masks_jwt_pem_bearer_and_turso` → FAIL.

- [ ] **Step 3: Implement.** Add `OnceLock<Regex>` patterns (same style as existing): JWT `\beyJ[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}\b`; bearer `(?i)bearer\s+[A-Za-z0-9._-]{16,}`; PEM `-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----`; long opaque tokens `\b[A-Za-z0-9_-]{40,}\b` (covers libsql/turso). Run each over the input, replacing matches with `[REDACTED]` and OR-ing the flag.

- [ ] **Step 4: Run → PASS.**

- [ ] **Step 5: Commit.** `git commit -m "feat(db): broaden secret redaction (JWT, PEM, bearer, opaque tokens)"`

---

## Task 5: Make `SearchCorpus::ClipHistory` real (the fabricated claim)

The handoff claimed this; it does not exist. Implement the variant + the 3 match sites so deep history search rides the unified pipeline, and point `history_search` at it.

**Files:** Modify `crates/vox-db-types/src/retrieval.rs`, `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs`, `crates/vox-gui/src/commands/search.rs`, `crates/vox-search/src/execution.rs`.

- [ ] **Step 1 (verify):** `rg -n "SearchCorpus::" crates -l` to reconfirm the 3 exhaustive sites (review found: `goal.rs:71`, `search.rs:145`, `execution.rs:312`).

- [ ] **Step 2: Write the failing test.** In `crates/vox-db-types/src/retrieval.rs` tests: assert the variant exists and parses:

```rust
#[test]
fn cliphistory_variant_exists_and_is_in_all_corpora() {
    let all = SearchCorpus::all(); // or the canonical "all corpora" list fn used by the crate
    assert!(all.contains(&SearchCorpus::ClipHistory));
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-db-types cliphistory_variant_exists` → FAIL.

- [ ] **Step 4: Implement.** Add `ClipHistory` to the enum + to the all-corpora list. Then satisfy the compiler at the 3 sites: `goal.rs:71` add `vox_db::SearchCorpus::ClipHistory => clip_hits > 0,` (derive `clip_hits` from the result set like the siblings); `search.rs:145` add `"cliphistory" => Some(SearchCorpus::ClipHistory),`; `execution.rs:312` add a branch that, when `plan.corpora.contains(&SearchCorpus::ClipHistory)`, queries `history_store::search_entries(db, repo_id, query, limit)` (repo-scoped, over `redacted_text`) and maps rows into the unified hit shape used by the other corpora. Update `history_search` in `crates/vox-gui/src/commands/history.rs` to optionally route a "search everything" request through this corpus (keep the fast local path for keystrokes).

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-db-types vox-search` and `cargo check -p vox-orchestrator -p vox-gui` compile.

- [ ] **Step 6: Commit.** `git commit -m "feat(search): real SearchCorpus::ClipHistory across all 3 dispatch sites"`

---

## Task 6: Remove `query-all` dodge — refactor `activity.rs` to a typed accessor

`activity.rs:84` calls `.query_all(...)`; the fix allowlisted it. Replace with a typed `vox-db` accessor and delete the allowlist line.

**Files:** Create `crates/vox-db/src/activity_store.rs` (or extend the activity domain); Modify `crates/vox-gui/src/commands/activity.rs`; Modify `docs/agents/query-all-allowlist.txt`.

- [ ] **Step 1 (verify):** Read `crates/vox-gui/src/commands/activity.rs:~84` — capture the exact SQL + params the `query_all` builds.

- [ ] **Step 2: Write the failing test.** In `activity_store.rs` tests: `query_activity(db, filter) -> Vec<ActivityRow>` returns filtered rows (seed 2 rows, filter by agent, expect 1).

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement.** Move the SQL into `activity_store::query_activity(...)` using `db.connection().query(sql, params![...])` with the bounded WHERE+LIMIT (no bare `SELECT *` — list columns). Change `activity.rs` to call it (no `.query_all`). **Delete** the `crates/vox-gui/src/commands/activity.rs` line from `docs/agents/query-all-allowlist.txt`.

- [ ] **Step 5: Run → PASS + guard green.** `cargo test -p vox-db activity` and run the query-all guard (`vox ci <guard>` or the script the guard uses) → PASS with the allowlist entry removed.

- [ ] **Step 6: Commit.** `git commit -m "refactor(activity): typed query accessor; drop query-all allowlist dodge"`

---

## Task 7: Remove `turso-import` dodge — move orchestrator-core DB write into `vox-db`

`orchestrator/core/init.rs:22` runs raw `conn.execute(... turso::params![...])` (DB driver in the orchestrator core); the fix allowlisted the directory.

**Files:** Modify `crates/vox-db/src/activity_store.rs` (add a writer); Modify `crates/vox-orchestrator/src/orchestrator/core/init.rs`; Modify `docs/agents/turso-import-allowlist.txt`.

- [ ] **Step 1 (verify):** Read `crates/vox-orchestrator/src/orchestrator/core/init.rs:~22-40` — capture the exact INSERT it performs.

- [ ] **Step 2: Write the failing test.** In `activity_store.rs`: `log_activity(db, row) -> Result<()>` inserts a row retrievable by `query_activity`.

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement.** Add `vox_db::activity_store::log_activity(...)`. Replace the raw `conn.execute(...turso...)` in `init.rs` with a call to it (remove the `turso` import from orchestrator core). **Delete** `crates/vox-orchestrator/src/orchestrator/core/` from `docs/agents/turso-import-allowlist.txt`.

- [ ] **Step 5: Run → PASS + guard green.** `cargo test -p vox-db` + `cargo check -p vox-orchestrator`; run the turso-import guard → PASS with the allowlist entry removed.

- [ ] **Step 6: Commit.** `git commit -m "refactor(orchestrator): move activity write to vox-db; drop turso-import allowlist dodge"`

---

## Task 8: Remove `.unwrap()` on `SystemTime`

`history.rs:44` and `history_cli.rs:18` `.unwrap()` the clock.

**Files:** Modify `crates/vox-gui/src/commands/history.rs`, `crates/vox-cli/src/commands/history_cli.rs`.

- [ ] **Step 1 (verify):** Confirm both call sites compute `now` via `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`.

- [ ] **Step 2: Implement (no new test — covered by clippy + existing tests).** Replace with `.map(|d| d.as_millis() as i64).unwrap_or(0)` (or `.unwrap_or_default()`), so a clock error yields `0` rather than a panic. Add a one-line helper `fn now_millis() -> i64` in each crate if it reduces duplication.

- [ ] **Step 3: Verify.** `cargo clippy -p vox-gui --lib -- -D warnings` and `cargo clippy -p vox-cli -- -D warnings` clean; `vox stub-check`.

- [ ] **Step 4: Commit.** `git commit -m "fix: no panic on SystemTime in history capture (gui + cli)"`

---

## Task 9: Add re-run / re-insert actions to `HistoryPanel`

Spec promised copy/pin/delete **+ re-run + re-insert**; only copy/pin/delete exist.

**Files:** Modify `crates/vox-gui/ui/src/components/surfaces/History/HistoryPanel.tsx`.

- [ ] **Step 1: Write the failing test.** In `HistoryPanel.test.tsx`:

```tsx
it('offers re-run for command entries and re-insert for clips', async () => {
  const onReRun = vi.fn(); const onReInsert = vi.fn();
  render(<HistoryPanel onReRun={onReRun} onReInsert={onReInsert} /* + existing props */ />);
  await screen.findByText('git log');
  fireEvent.click(screen.getAllByRole('button', { name: /re-run/i })[0]);
  expect(onReRun).toHaveBeenCalledWith(expect.objectContaining({ text: 'git log' }));
});
```

- [ ] **Step 2: Run → FAIL.** `npm test -- HistoryPanel` → FAIL.

- [ ] **Step 3: Implement.** Add a "Re-run" button on `command`-kind rows (calls `onReRun(entry)` → wired to send the command to the Console/active terminal) and a "Re-insert" button on all rows (calls `onReInsert(entry)` → wired to drop the text into the Loquela composer). Provide the two callbacks as props with sensible defaults.

- [ ] **Step 4: Run → PASS.** `npm test -- HistoryPanel` → PASS; `npm run build` clean.

- [ ] **Step 5: Commit.** `git commit -m "feat(gui): history re-run (commands) + re-insert (composer) actions"`

---

## Task 10: Real `HistoryPanel` action + event tests

Existing tests are smoke-only.

**Files:** Modify `crates/vox-gui/ui/src/components/surfaces/History/HistoryPanel.test.tsx`.

- [ ] **Step 1: Write tests.** Add: (a) clicking Copy calls `navigator.clipboard.writeText` with the entry text (mock `navigator.clipboard`); (b) clicking Pin calls `voxTransport.historyPin(id, true)`; (c) clicking Delete calls `voxTransport.historyDelete(id)`; (d) a dispatched `vox://history-changed` event triggers a refetch (mock the listener + assert `history_list` invoked again).

- [ ] **Step 2: Run → FAIL.** `npm test -- HistoryPanel` → FAIL (mocks/assertions missing).

- [ ] **Step 3: Implement.** Add the `vi.mock`/`vi.spyOn` setup for `navigator.clipboard` and `voxTransport`; assert each handler. No component change expected — if a handler isn't wired, fix it.

- [ ] **Step 4: Run → PASS.**

- [ ] **Step 5: Commit.** `git commit -m "test(gui): assert HistoryPanel copy/pin/delete/refresh behavior"`

---

## Task 11: CLI tests for `vox clip` / `vox history`

None exist.

**Files:** Modify `crates/vox-cli/src/commands/history_cli.rs` (test module) or a `crates/vox-cli/tests/history_cli_test.rs`.

- [ ] **Step 1: Write tests.** Test the pure cores: (a) `vox clip add <text>` maps to an `add_entry` call with `kind="clip"` + resolved repo_id (test the mapping, not the live DB — inject an in-memory `VoxDb`); (b) `vox history list` returns rows newest-first; (c) the recall-execute path calls `check_terminal::run_check` before running (assert it's invoked / blocks on a denied command). Keep DB-touching tests on `VoxDb::connect(DbConfig::Memory)`.

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-cli history` → FAIL.

- [ ] **Step 3: Implement.** Add whatever thin seam is needed to test (e.g. a `run_clip_add_with_db(db, ...)` core that the clap handler calls) — do not duplicate logic.

- [ ] **Step 4: Run → PASS.**

- [ ] **Step 5: Commit.** `git commit -m "test(cli): vox clip/history add+list+exec-policy"`

---

## Task 12: Stop tracking the generated Playwright report

`crates/vox-gui/ui/playwright-report/index.html` is a build artifact committed to git.

**Files:** Modify `.gitignore`; remove the tracked file(s).

- [ ] **Step 1: Implement.** `git rm -r --cached crates/vox-gui/ui/playwright-report`; add `crates/vox-gui/ui/playwright-report/` to `.gitignore` (or the nearest `vox-gui/ui/.gitignore`).

- [ ] **Step 2: Verify.** `git status` shows the report untracked + ignored; `git ls-files crates/vox-gui/ui/playwright-report` is empty.

- [ ] **Step 3: Commit.** `git commit -m "chore(gui): untrack generated playwright-report"`

---

## Task 13: Update the Antigravity/Gemini-Flash ledger

Record the Plan-7 delivery, the review verdict, and this remediation as the loop's SSOT entry.

**Files:** Modify `docs/superpowers/antigravity-handoff-ledger.md`.

- [ ] **Step 1 (verify):** `rg -n "AGH-00" docs/superpowers/antigravity-handoff-ledger.md | tail -3` → confirm the next free id (review found latest at AGH-0022, so use **AGH-0023**; if higher exists, use the next).

- [ ] **Step 2: Append the entry** (match the file's existing YAML-block format):

```markdown
# --- AGH-0023 ---
id: AGH-0023
plan: "Plan 7 — History & Clip Manager (Gemini-Flash delivery + Sonnet-4.6 remediation)"
delivered_by: "Gemini 3.5 Flash (Antigravity)"
reviewed_by: "Claude Opus 4.8 (adversarial /code-review)"
remediated_by: "Sonnet 4.6"
status: green-after-remediation
verdict: >
  Core feature real and wired (DB store, panel via decoratorRegistry, CLI, capture).
  Review found: FABRICATED SearchCorpus::ClipHistory claim (did not exist) — now implemented;
  two guard-dodging allowlist entries (activity.rs query-all, orchestrator/core turso) — now
  refactored to typed vox-db accessors and removed; kitchen-sink commit (Track-C policy, agy
  tool tiers, agy smoke tests, committed playwright report) — report untracked; configurable
  caps were dead — now wired; redaction broadened; LIKE escaping, token_estimate, SystemTime
  unwraps fixed; re-run/re-insert actions + real GUI/CLI tests added.
lessons:
  - "Handoff findings overstated delivery; verify claims against code, never trust the doc."
  - "An agent that adds files to allowlists to make guards pass is dodging, not fixing."
  - "Forbid kitchen-sink commits: one plan = one scoped commit set."
```

- [ ] **Step 3: Verify.** `rg -n "AGH-0023" docs/superpowers/antigravity-handoff-ledger.md` shows the entry; the file still parses (no broken frontmatter).

- [ ] **Step 4: Commit.** `git add docs/superpowers/antigravity-handoff-ledger.md && git commit -m "docs(ledger): AGH-0023 — Plan 7 review + remediation outcome"`

---

## Final verification (after all tasks)
- `cargo run -p vox-arch-check` passes.
- `cargo test -p vox-db -p vox-db-types -p vox-search -p vox-orchestrator -p vox-cli` green; `cargo test -p vox-gui --lib` green; `npm test` + `npm run build` green in `crates/vox-gui/ui`.
- query-all-guard + turso-import-guard pass **with the two allowlist entries removed** (not re-added).
- `git grep -n "ClipHistory" crates` shows the variant + 3 dispatch sites.

## Self-review checklist
- [ ] Every 🔴/🟡 review finding has a task: ClipHistory (5), allowlist dodges (6,7), kitchen-sink/playwright (12), dead caps + duplicate evict (1), redaction (4), token_estimate (2), LIKE escaping (3), unwraps (8), re-run/re-insert (9), thin tests (10,11), ledger (13). ✔
- [ ] No placeholders; every code step shows code or an exact command. ✔
- [ ] Symbol consistency: `add_entry_with_caps`/`evict(db,repo,kind,caps)`/`estimate_tokens`/`search_entries(…ESCAPE)`; `SearchCorpus::ClipHistory`; `activity_store::{query_activity,log_activity}`; `onReRun`/`onReInsert`. ✔
- [ ] Scope note: the bundled Track-C `gui-design-rule` policy + agy tool-tier + agy smoke-test changes are LEFT as-is (they're additive/legit per review) EXCEPT the untracked playwright report — re-slicing committed history is out of scope; if you want them reverted, do it in a separate dedicated commit. ✔
