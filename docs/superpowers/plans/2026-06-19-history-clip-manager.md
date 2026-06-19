# History & Clip Manager (Ditto-style) — Implementation Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `crates/vox-skills/skills/superpowers/test-driven-development.skill.md`. Steps use `- [ ]` checkboxes.

> **🤖 EXECUTION TARGET — READ FIRST.** Gemini 3.5 Flash inside Google Antigravity (~48% real-world completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context). Engineered against those modes. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md). Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

> **⚠️ PRE-EXECUTION NOTE:** this plan was written before its dedicated codebase-audit session. Treat **every** Step-1 `rg`/read as authoritative over the plan text: if a signature/path differs, STOP and report (the audit pass will reconcile). Do not code against the plan's assumed names without confirming them.

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** A crash between tasks leaves a compiling, tested tree. Never split a compile-breaking change across two commits.
2. **Verify-before-use.** Every Step-1 `rg`/read is a BLOCKING gate — paste output before any code step; reality differs → STOP.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Two failures → STOP + handoff note.
5. **Parallel dispatch** per tags; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all` (`cargo fmt -p <crate>`); `.vox` automation only; `docs/src/` `.md` needs frontmatter. `vox-gui` clippy `--lib`.
7. **Verification ritual** before commit: Rust → `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`; TS → from `crates/vox-gui/ui`: `npm test` + `npm run build`. Paste output.
8. **Rollback on broken tree:** `git reset --hard HEAD` to last green; re-attempt the one task.
9. **Split-on-overrun:** an Implement step touching >1 file or adding >1 new function → one atomic green commit per sub-bullet, in order.
10. **Rust:** no `.unwrap()` in lib code; inject params in tests; deterministic. DB test ctor is `vox_db::VoxDb::connect(vox_db::DbConfig::Memory)` (`local` feature) — there is NO `open_in_memory`. Tauri commands register in `crates/vox-gui/src/main.rs`'s `tauri::generate_handler![…]`.

**Goal:** A project-scoped, OS-clipboard-independent History & Clip manager (unified `clip`/`command`/`chat` entries) with CLI + GUI parity: searchable (fast local filter + a `ClipHistory` deep-search corpus), clickable (copy-out / re-run / re-insert / pin / delete), generous + configurable retention.

**Architecture:** A vox-db `history_entries` store (per-repo SSOT) with per-kind eviction + secret redaction; a `SearchCorpus::ClipHistory` variant for unified deep search; Tauri `history_*` commands + `vox://history-changed`; a `HistoryPanel` registered in the spec-6 `panelRegistry`; and a `vox clip`/`vox history` CLI. Capture feeds from Console OSC633, chat append, and explicit user clips.

**Tech Stack:** Rust (`vox-db`, `vox-db-types`, `vox-gui`, `vox-cli`); React/TS + vitest (`vox-gui/ui`).

**Design:** [`../specs/2026-06-19-history-clip-manager-design.md`](../specs/2026-06-19-history-clip-manager-design.md). **Depends on** the spec-6 `panelRegistry`/dock workspace for the GUI panel (Task 8).

---

## Codebase Audit Addendum (2026-06-19 — VERIFIED against real code; OVERRIDES plan text where it conflicts)

Two parallel audits confirmed/corrected the anchors below. These are authoritative; the per-task snippets further down are illustrative and must be reconciled to these.

**🔴 vox-db query API (Tasks 1–3, 6).** There is NO `db.execute(sql, ())` / `db.query(sql, ())`. The real API (see `crates/vox-db/src/codex_chat.rs`) is:
```rust
use turso::params;
self.connection().execute("INSERT ... VALUES (?1, ?2)", params![a, b]).await?;          // writes
let mut rows = self.connection().query("SELECT x FROM t WHERE repository_id = ?1", params![rid]).await?;
while let Some(row) = rows.next().await? { let v: String = row.get(0)?; /* map */ }       // reads
```
All DB calls are `async .await?`; params use the `turso::params!` macro; rows map via `row.get::<T>(idx)`. A history store module takes the `VoxDb`/`Codex` handle and calls `.connection()`. Test ctor `VoxDb::connect(DbConfig::Memory)` (local feature) is correct. **Rewrite every test/impl snippet in Tasks 1–3 to this shape.**

**🔴 Schema registration (Task 1) — has a GAP.** Current `BASELINE_VERSION = 78` (`crates/vox-db/src/schema/manifest.rs`). Steps: (a) create `schema/domains/history.rs` with `pub const SCHEMA_HISTORY: &str = "CREATE TABLE …"`; (b) `pub mod history;` in `domains/mod.rs`; (c) add a `SchemaFragment { name: "history_entries", sql: domains::history::SCHEMA_HISTORY }` to `SCHEMA_FRAGMENTS` and bump `BASELINE_VERSION` to **79**; (d) **MISSING FROM THE ORIGINAL TASK — update `contracts/db/baseline-version-policy.yaml`** (`repository_baseline_integer: 79` + the new `repository_baseline_digest_hex`). A contract test (`baseline_policy_matches_compiled_schema`) FAILS until the YAML matches — run the test, copy the expected digest from its failure message, paste it in. This is one atomic commit.

**🔴 SearchCorpus::ClipHistory (Task 5) — 3 NAMED sites, no single executor.** Adding the variant breaks these exhaustive matches; fix all in the same commit: (1) `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs:~70-77` (add an arm); (2) `crates/vox-gui/src/commands/search.rs:~145-150` (string→corpus, add `"cliphistory" => Some(SearchCorpus::ClipHistory)`); (3) `crates/vox-search/src/execution.rs:~312+` (the corpus→rows routing — add a branch SELECTing from `history_entries` over `redacted_text`, repo-scoped). The enum is in `crates/vox-db-types/src/retrieval.rs:95`.

**🟡 Redaction (Task 4) — there is NO Clavis crate.** `crates/vox-clavis/src` does not exist (Clavis is an external vault referenced only in comments). There is no centralized redact lib. So `redact()` is a **small local secret-pattern set** (e.g. `sk-…`, `ghp_…`, `gho_…`, AWS `AKIA…`, bearer tokens); prior art for field masking is `crates/vox-db/src/socrates_telemetry.rs`. Do NOT import a non-existent Clavis API.

**🟡 copy-out (Tasks 6, 8) — no Tauri clipboard plugin.** Neither `tauri-plugin-clipboard-manager` (Rust) nor `@tauri-apps/plugin-clipboard-manager` (JS) is a dependency. Existing surfaces (`Console.tsx`, `SearchView.tsx`) copy via the browser `navigator.clipboard.writeText(...)` inside the webview. So **copy-out is a frontend action in `HistoryPanel` using `navigator.clipboard.writeText`**, NOT a Rust `history_copy_out` Tauri command. Drop `history_copy_out` from Task 6; implement copy-out in Task 8 (toast on success/failure, matching Console.tsx).

**🟡 panelRegistry not landed (Task 8) — register via the CURRENT surface system, do NOT block.** Spec-6's `panelRegistry.ts` does not exist yet. Ship the panel now via: (a) add `'history'` to the `View` union in `crates/vox-gui/ui/src/App.tsx` (+ `LEGACY_VIEWS`); (b) add a `case 'history': return <HistoryPanel .../>;` to the `childRenderer` switch in `components/layout/surfaceComponents.tsx`; (c) add a `surface-registry.v1.yaml` entry. Leave a comment: migrate to `panelRegistry` when spec-6 lands. (Supersedes Task 8's BLOCKED-path.)

**🟢 Verified-correct anchors:** repo scope = `vox_orchestrator::lineage::repository_id() -> String` (`lineage.rs:21`; same `repository_id` field `conversations` uses) — use it everywhere, not "cwd". Tauri commands register in `crates/vox-gui/src/main.rs` `generate_handler![…]` (module declared in `commands/mod.rs`). FE calls use raw `invoke()` (`@tauri-apps/api/core`) + `listen()` (`@tauri-apps/api/event`) per `transport.ts` — no custom wrapper. CLI subcommand pattern = module in `vox-cli/src/commands/` + `mod.rs` + `Cli` enum variant + dispatch (examples: `codex.rs`, `db.rs`). Test stack = vitest 3.2.6 + @testing-library/react 16.3.2.

**Capture hooks (Task 9):** 9a → `Console.tsx` `handleBlock` callback (fires with a finalized `Block { command, exitCode, output }` from `osc633.ts`; note `block.output` may be filled async by `TerminalTab` — guard on `exitCode !== null`). 9c → after the DB append in `chat_append_message` (`crates/vox-gui/src/commands/chat.rs` → `db.chat_append_workspace_message`, `codex_chat.rs:~97-137`); mirror assistant turns.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-db/src/schema/domains/history.rs` (+ `domains/mod.rs`, `schema/manifest.rs`) | `history_entries` table + registration + version bump | Create/Modify (Task 1) |
| `crates/vox-db/src/history_store.rs` (+ lib re-export) | add/list/pin/delete/evict accessor | Create (Tasks 2–3) |
| `crates/vox-db/src/redact.rs` (or nearest existing secret-pattern util) | `redact()` | Create/Modify (Task 4) |
| `crates/vox-db-types/src/retrieval.rs` | `SearchCorpus::ClipHistory` + planning | Modify (Task 5) |
| `crates/vox-gui/src/commands/history.rs` (+ `main.rs`) | Tauri `history_*` + `vox://history-changed` | Create/Modify (Task 6) |
| `crates/vox-gui/ui/src/lib/historyFilter.ts` | `filterEntries` local fuzzy | Create (Task 7) |
| `crates/vox-gui/ui/src/components/surfaces/History/HistoryPanel.tsx` (+ `lib/panelRegistry.ts`) | GUI panel + actions | Create/Modify (Task 8) |
| Console `osc633.ts` / chat append / clip hotkey | capture → `history_add` | Modify (Task 9) |
| `crates/vox-cli/src/...` | `vox clip` / `vox history` | Create/Modify (Task 10) |

**Pre-flight (run once, paste output):**
- `rg -n "BASELINE_VERSION|SchemaFragment|pub mod " crates/vox-db/src/schema/manifest.rs crates/vox-db/src/schema/domains/mod.rs` — table registration + version pattern.
- `rg -n "VoxDb::connect\(DbConfig::Memory\)" crates/vox-db/src/local_tests.rs` — confirm the test DB ctor.
- `rg -n "pub enum SearchCorpus" -A 12 crates/vox-db-types/src/retrieval.rs` — confirm variants + where corpora lists are built.
- `rg -n "generate_handler!" crates/vox-gui/src/main.rs` — Tauri registration site.
- `rg -n "panelRegistry|PANEL_KINDS|PanelKind" crates/vox-gui/ui/src/lib/panelRegistry.ts` — confirm the spec-6 registry exists (if NOT, the GUI panel Task 8 is BLOCKED on the dockable-workspace plan; build the rest, mark 8 blocked).
- `rg -n "secret|redact|Clavis|mask" crates/vox-db/src crates/vox-clavis/src -l` — find an existing secret-pattern source to reuse for redaction.
- `rg -n "CommandFinished|command|osc633|prompt" crates/vox-gui/ui/src/components/surfaces/Console/osc633.ts` — confirm the command-boundary event to hook for capture.
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task 1 `[SEQUENTIAL]`: `history_entries` table

**Files:** Create `crates/vox-db/src/schema/domains/history.rs`; Modify `schema/domains/mod.rs` + `schema/manifest.rs`.

- [ ] **Step 1 (verify-before-use):** From Pre-flight, copy the exact registration pattern (an existing domain const + its `SchemaFragment` entry + the `BASELINE_VERSION` constant). Note the current version number.

- [ ] **Step 2: Write the failing test.** Add a vox-db test:

```rust
#[tokio::test]
async fn history_entries_round_trip() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db"); // VERIFIED ctor
    db.execute(
        "INSERT INTO history_entries (repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate)
         VALUES ('r1','clip','hello','hello',1000,0,'cli',1)", ()
    ).await.expect("insert");
    let rows = db.query("SELECT kind FROM history_entries WHERE repo_id='r1'", ()).await.expect("q");
    assert_eq!(rows.len(), 1);
}
```

(Replace `execute`/`query` with the real vox-db API confirmed in Step 1.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-db history_entries_round_trip` → FAIL (no table).

- [ ] **Step 4: Implement (one commit — schema is multi-anchor).** Create the domain const, register the fragment, bump `BASELINE_VERSION`, add `pub mod history;`:

```sql
CREATE TABLE IF NOT EXISTS history_entries (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id        TEXT NOT NULL,
    kind           TEXT NOT NULL,            -- 'clip' | 'command' | 'chat'
    text           TEXT NOT NULL,
    redacted_text  TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    pinned         INTEGER NOT NULL DEFAULT 0,
    source         TEXT,                      -- 'cli' | 'gui' | 'osc633' | 'agent' | 'chat'
    token_estimate INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_history_repo_kind ON history_entries(repo_id, kind, created_at);
CREATE INDEX IF NOT EXISTS idx_history_pinned    ON history_entries(repo_id, pinned);
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-db history_entries_round_trip` → PASS.

- [ ] **Step 6: Commit.** `cargo clippy -p vox-db -- -D warnings && cargo fmt -p vox-db; git add crates/vox-db/src/schema/ && git commit -m "feat(db): history_entries table"`

---

## Task 2 `[SEQUENTIAL]`: store accessor — `add` + `list`

**Files:** Create `crates/vox-db/src/history_store.rs` (+ lib re-export).

- [ ] **Step 1 (verify-before-use):** `rg -n "pub async fn|impl VoxDb|execute\(|query\(" crates/vox-db/src/codex_chat.rs | head` — copy the real query/execute API + row-mapping pattern used by an existing store.

- [ ] **Step 2: Write the failing test.** In `history_store.rs`:

```rust
#[tokio::test]
async fn add_then_list_by_kind() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db");
    add_entry(&db, "r1", "clip", "snippet A", "snippet A", 1000, "cli").await.expect("add");
    add_entry(&db, "r1", "command", "cargo test", "cargo test", 1001, "osc633").await.expect("add");
    let clips = list_entries(&db, "r1", Some("clip"), 50).await.expect("list");
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].text, "snippet A");
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-db add_then_list_by_kind` → FAIL.

- [ ] **Step 4: Implement.** `pub struct HistoryEntry { id, repo_id, kind, text, redacted_text, created_at, pinned, source, token_estimate }`; `pub async fn add_entry(db, repo, kind, text, redacted, created_at, source) -> Result<i64>` (INSERT, returns id); `pub async fn list_entries(db, repo, kind: Option<&str>, limit) -> Result<Vec<HistoryEntry>>` (SELECT … ORDER BY created_at DESC LIMIT, kind filter optional). Re-export from `vox-db` lib.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-db add_then_list_by_kind` → PASS.

- [ ] **Step 6: Commit.** `git commit -m "feat(db): history store add_entry + list_entries"`

---

## Task 3 `[SEQUENTIAL]`: retention / eviction (per-kind caps + pin-escape)

**Files:** Modify `crates/vox-db/src/history_store.rs`.

- [ ] **Step 1 (verify-before-use):** Re-read `history_store.rs` from Task 2.

- [ ] **Step 2: Write the failing test.** Insert N clips beyond a cap of 2 (one pinned); `evict(db, repo, caps)` keeps the pinned + the 2 newest unpinned, drops the rest:

```rust
#[tokio::test]
async fn evict_respects_caps_and_pins() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db");
    for i in 0..5 { add_entry(&db, "r1", "clip", &format!("c{i}"), &format!("c{i}"), 1000+i, "cli").await.unwrap(); }
    pin_entry(&db, /*id of c0*/ 1, true).await.unwrap();
    let caps = HistoryCaps { clip: 2, command: 50, chat: 50 };
    evict(&db, "r1", &caps).await.unwrap();
    let clips = list_entries(&db, "r1", Some("clip"), 50).await.unwrap();
    // pinned c0 + 2 newest unpinned (c4,c3) = 3
    assert_eq!(clips.len(), 3);
    assert!(clips.iter().any(|c| c.text == "c0"));
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-db evict_respects_caps_and_pins` → FAIL.

- [ ] **Step 4: Implement.** `pub struct HistoryCaps { clip, command, chat }` (defaults via a `Default`/config: clip generous, command/chat shorter); `pub async fn pin_entry(db, id, pinned) -> Result<()>`; `pub async fn delete_entry(db, id) -> Result<()>`; `pub async fn evict(db, repo, caps) -> Result<()>` — per kind, keep all `pinned=1` + newest `cap` unpinned, DELETE the remainder. Call `evict` after each `add_entry` (or expose for the caller).

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-db evict_respects_caps_and_pins` → PASS.

- [ ] **Step 6: Commit.** `git commit -m "feat(db): history retention (per-kind caps, pin-escape) + pin/delete"`

---

## Task 4 `[PARALLEL-SAFE]` (own file): `redact()` secret-masking

**Files:** Create `crates/vox-db/src/redact.rs` (or extend the existing secret-pattern util from Pre-flight).

- [ ] **Step 1 (verify-before-use):** Paste the Pre-flight `rg` for existing secret patterns (Clavis). If a pattern set exists, REUSE it; do not invent a competing regex set.

- [ ] **Step 2: Write the failing test.**

```rust
#[test]
fn redact_masks_known_secrets_keeps_clean_text() {
    assert_eq!(redact("just text").0, "just text");          // (display, redacted_flag)
    let (masked, flagged) = redact("token sk-ABC123DEF456GHI789");
    assert!(flagged);
    assert!(!masked.contains("sk-ABC123DEF456GHI789"));
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-db redact_masks_known_secrets_keeps_clean_text` → FAIL.

- [ ] **Step 4: Implement.** `pub fn redact(text: &str) -> (String, bool)` returning masked text + a flag, using the reused secret-pattern set (e.g. `sk-…`, `ghp_…`, AWS keys, `VAULT_TOKEN`-style). On any regex error, conservatively flag + mask the whole entry. Wire `add_entry` to store `text` (or masked, per source) into `redacted_text`.

- [ ] **Step 5: Run → PASS.**

- [ ] **Step 6: Commit.** `git commit -m "feat(db): secret redaction for history capture"`

---

## Task 5 `[SEQUENTIAL]`: `SearchCorpus::ClipHistory` deep-search variant

**Files:** Modify `crates/vox-db-types/src/retrieval.rs` (+ the query path that resolves a corpus to rows).

- [ ] **Step 1 (verify-before-use):** Paste `rg -n "pub enum SearchCorpus" -A 12 crates/vox-db-types/src/retrieval.rs` and find every exhaustive `match SearchCorpus` (run `rg -n "SearchCorpus::" crates/ -l`). Adding a variant breaks them — list the sites first.

- [ ] **Step 2: Write the failing test.** A test asserting `SearchCorpus::ClipHistory` exists and is included in the "all corpora" planning list (or the appropriate default set).

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement.** Add `ClipHistory` to the enum; add a `Cancelled`-style arm to **every** matched site found in Step 1; wire the query executor so a `ClipHistory` search SELECTs from `history_entries` (repo-scoped, over `redacted_text`) and maps to the unified hit shape. Keep it in the same single commit so the tree compiles.

- [ ] **Step 5: Run → PASS.** Full `cargo test -p vox-db-types` (and the search crate) PASS.

- [ ] **Step 6: Commit.** `git commit -m "feat(search): ClipHistory corpus over history_entries"`

---

## Task 6 `[SEQUENTIAL]`: Tauri `history_*` commands + `vox://history-changed`

**Files:** Create `crates/vox-gui/src/commands/history.rs`; Modify `commands/mod.rs` + `main.rs`.

- [ ] **Step 1 (verify-before-use):** `rg -n "generate_handler!" crates/vox-gui/src/main.rs` + read an existing command that obtains the DB + active repo id.

- [ ] **Step 2: Write the failing test.** Unit test for a pure DTO mapper `entry_to_dto(&HistoryEntry) -> HistoryEntryDto` (id, kind, text=redacted_text, created_at, pinned, source).

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-gui entry_to_dto` → FAIL.

- [ ] **Step 4: Implement.** `HistoryEntryDto` + mapper; `#[tauri::command]` fns `history_list(kind?, limit)`, `history_add(kind, text)`, `history_search(query, limit)` (→ `ClipHistory` corpus), `history_pin(id, pinned)`, `history_delete(id)`, `history_copy_out(id)` (writes the entry's text to the OS clipboard via Tauri's clipboard API). Add `pub const HISTORY_CHANGED_EVENT: &str = "vox://history-changed";`, emit on mutations. Register all in `main.rs` `generate_handler!`.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-gui entry_to_dto` → PASS; `cargo check -p vox-gui`.

- [ ] **Step 6: Commit.** `cargo clippy -p vox-gui --lib -- -D warnings && cargo fmt -p vox-gui; git commit -m "feat(gui): history_* Tauri commands + vox://history-changed"`

---

## Task 7 `[PARALLEL-SAFE]` (own file): `filterEntries` local fuzzy filter

**Files:** Create `crates/vox-gui/ui/src/lib/historyFilter.ts` (+ test).

- [ ] **Step 1 (verify-before-use):** none (pure new file). Confirm no `historyFilter.ts` exists: `rg -n "historyFilter" crates/vox-gui/ui/src`.

- [ ] **Step 2: Write the failing test.**

```ts
import { describe, it, expect } from 'vitest';
import { filterEntries } from './historyFilter';

describe('filterEntries', () => {
  it('ranks subsequence matches, drops non-matches', () => {
    const e = [{id:1,text:'cargo test'},{id:2,text:'git commit'},{id:3,text:'cargo build'}];
    const out = filterEntries('crgo', e as any);
    expect(out.map(x => x.id)).toEqual([1,3]);     // both 'cargo' lines, deterministic order
  });
  it('empty query returns all in original order', () => {
    const e = [{id:1,text:'a'},{id:2,text:'b'}];
    expect(filterEntries('', e as any).map(x=>x.id)).toEqual([1,2]);
  });
});
```

- [ ] **Step 3: Run → FAIL.** `npm test -- historyFilter` → FAIL.

- [ ] **Step 4: Implement.** `export function filterEntries<T extends {text:string}>(query: string, entries: T[]): T[]` — case-insensitive subsequence match + a simple score (contiguous/earlier matches rank higher); empty query → input unchanged. Pure, deterministic.

- [ ] **Step 5: Run → PASS.** `npm test -- historyFilter` → PASS; build clean.

- [ ] **Step 6: Commit.** `git commit -m "feat(gui): local fuzzy filter for history (instant type-to-filter)"`

---

## Task 8 `[SEQUENTIAL]`: `HistoryPanel` GUI + actions

**Files:** Create `crates/vox-gui/ui/src/components/surfaces/History/HistoryPanel.tsx` (+ test); Modify `lib/panelRegistry.ts`.

- [ ] **Step 1 (verify-before-use):** Confirm `panelRegistry` exists (spec-6). If NOT, STOP and report this task BLOCKED on the dockable-workspace plan; build Tasks 1–7, 9–10 regardless. Read an existing surface for the `invoke` + event-subscribe pattern.

- [ ] **Step 2: Write the failing test.** `HistoryPanel.test.tsx`: renders entries (rows with `data-testid="history-row"`); typing in the search box filters via `filterEntries`; clicking a row's Copy calls `onCopy(id)`; Pin/Delete/Re-run/Re-insert call their handlers.

- [ ] **Step 3: Run → FAIL.** `npm test -- HistoryPanel` → FAIL.

- [ ] **Step 4: Implement.** `HistoryPanel`: loads via `invoke('history_list')`, search box runs `filterEntries` over the loaded ring (instant) with a "search all history" button → `invoke('history_search', …)`; per-row actions copy-out (`history_copy_out`) / re-run (commands → send to Console/composer) / re-insert (→ composer) / pin (`history_pin`) / delete (`history_delete`); subscribe `vox://history-changed`. Register a `history` panel kind in `panelRegistry`.

- [ ] **Step 5: Run → PASS.** `npm test -- HistoryPanel` → PASS; build clean.

- [ ] **Step 6: Commit.** `git commit -m "feat(gui): HistoryPanel (searchable, clickable) + panelRegistry entry"`

---

## Task 9 `[SEQUENTIAL]`: capture wiring

**Files:** Modify Console `osc633.ts` (command capture); chat append path; a clip hotkey/menu.

- [ ] **Step 1 (verify-before-use):** From Pre-flight, confirm the OSC633 command-finished event shape in `osc633.ts`. Read the chat-append site (where a user/assistant turn is persisted).

- [ ] **Step 2–6 (split, one commit each):**
  - **9a:** on an OSC633 command boundary, call `history_add('command', <command text>, source:'osc633')`; test the parser→add hook with a fixture. Commit.
  - **9b:** a "Add selection to history" hotkey/menu in the GUI → `history_add('clip', …)`; commit.
  - **9c:** mirror chat turns → `history_add('chat', …)` at the append site (guard against duplicates by source); commit.

```bash
git commit -m "feat(gui): capture <source> into history"
```

---

## Task 10 `[SEQUENTIAL]`: CLI `vox clip` / `vox history`

**Files:** Create/Modify `crates/vox-cli/src/...` (a new subcommand).

- [ ] **Step 1 (verify-before-use):** `rg -n "Subcommand|fn run|clap" crates/vox-cli/src -l` then read how an existing subcommand is declared + dispatched (clap). Honor `contracts/terminal/exec-policy` for any interactive prompt.

- [ ] **Step 2: Write the failing test.** Unit-test the pure command core: `history_cli_search(entries, query) -> Vec<…>` reuses the same subsequence logic (or a Rust port) and returns ranked ids; `vox clip add <text>` maps to an `add_entry` call (test the arg→call mapping, not the DB).

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-cli history_cli_search` → FAIL.

- [ ] **Step 4: Implement.** `vox clip add <text>` / `vox clip list` / `vox clip copy <id>` (prints to stdout for shell capture) and `vox history [search <q>]` (interactive fuzzy list; non-TTY → plain list). Reuse the store accessors; resolve `repo_id` from cwd. No new `.ps1/.sh` — pure Rust CLI.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-cli history_cli_search` → PASS.

- [ ] **Step 6: Commit.** `cargo clippy -p vox-cli -- -D warnings && cargo fmt -p vox-cli; git commit -m "feat(cli): vox clip / vox history"`

---

## Parallel waves
- **Wave 1 (sequential, vox-db):** Task 1 → 2 → 3 (shared `history_store.rs`). Task 4 (`redact.rs`, own file) is `[PARALLEL-SAFE]` alongside 2/3.
- **Wave 2 (sequential):** Task 5 (search) — touches shared retrieval enum + many match sites.
- **Wave 3:** Task 6 (Tauri) then — **parallel** — Task 7 (`historyFilter.ts`, own file). Task 8 (panel) after 6+7. Task 10 (CLI) is independent of GUI and can run alongside Wave 3 (own crate).
- **Wave 4:** Task 9 (capture) last — it depends on `history_add` (Task 6) + the panel to observe results.

## Self-review checklist
- [ ] Spec §7 covered: table (1), accessor (2), eviction (3), redact (4), corpus (5), Tauri (6), filter (7), panel (8), capture (9), CLI (10). ✔
- [ ] OS-clipboard independence: only `history_copy_out` writes OUT to the OS clipboard; nothing reads it in the background. ✔
- [ ] Per-repo scope everywhere (`repo_id` filter); per-kind eviction caps; pins never evicted. ✔
- [ ] Secret redaction on capture; `redacted_text` is what search + display use. ✔
- [ ] Flash: every code step has a verify gate; schema is one commit; SearchCorpus match-sites enumerated before edit; oversized capture task split (9a/9b/9c); Task 8 has a BLOCKED path if panelRegistry absent. ✔
- [ ] Symbol consistency: `history_entries`/`HistoryEntry`/`add_entry`/`list_entries`/`evict`/`HistoryCaps`/`pin_entry`/`delete_entry`; `redact`; `SearchCorpus::ClipHistory`; `history_*` commands/`HISTORY_CHANGED_EVENT`/`HistoryEntryDto`; `filterEntries`; `HistoryPanel`. ✔

---

> **For the next session (audit + critique):** verify against real code — the exact vox-db query API + row mapping, the secret-pattern source to reuse (Clavis vs vox-db), every `SearchCorpus` match site, the OSC633 command-finished event name, the Tauri clipboard API for `copy_out`, and whether `panelRegistry` (spec-6) has landed. Then split/clarify any task the audit shows is too big or assumes a missing symbol, exactly as the prior plans' Flash hardening did.
