# History & Clip Manager (Ditto-style, project-scoped) — Design Spec

**Date:** 2026-06-19
**Status:** Design (decided; pending codebase audit + critique in the next session)
**Author:** Brainstorming session (Claude, Opus 4.8)
**Relates to:** [dockable-workspace-context-memory-ssot](2026-06-19-dockable-workspace-context-memory-ssot-design.md) (the `panelRegistry`/dock workspace this panel plugs into) · the GUI redesign program (specs 1–6).
**Execution target:** Gemini 3.5 Flash inside Antigravity — see [limitations doc](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

---

## 1. Problem

There is no recall layer for the small, high-value text the user and agents touch constantly:
snippets, the commands that were run, and prior chat turns. The OS clipboard holds exactly one
item, is global (leaks across projects), and is invisible/unsearchable. Meanwhile Vox already
*captures* the raw signals — Console command boundaries via **OSC 633**
(`crates/vox-gui/ui/src/components/surfaces/Console/osc633.ts` + `crates/vox-gui/src/commands/pty.rs`),
chat turns in vox-db `conversations*`, and a unified search spine (`SearchCorpus`,
`crates/vox-db-types/src/retrieval.rs:95`) — but none of it is surfaced as a searchable,
clickable, project-scoped history the way a clipboard manager like **Ditto** surfaces clips.

## 2. Goal

A **project/repo-scoped, Vox-owned History & Clip manager**, surfaced identically in **CLI and GUI**:
a generous, configurable, searchable ring of typed entries (clip / command / chat) that you filter
as you type and click to copy-out, re-run, re-insert, pin, or delete — **independent of the OS
clipboard**.

**Decided (prior session, user-confirmed):**
- **One unified store** with a `kind` discriminator (`clip` / `command` / `chat`), per-repo. Commands
  mirror in from Console OSC633 + agent-run + CLI; chat turns mirror from `conversations`.
- **Independent of the OS clipboard:** self-managed ring; explicit capture (hotkey/menu/CLI);
  **copy-out to the OS clipboard on click**; no background OS hook. (Passive Ditto-style auto-capture
  is a future opt-in, default-off — **out of v1**.)
- **Search = fast local filter + optional deep search:** instant in-memory substring/fuzzy over the
  loaded ring (Ditto latency), PLUS a new `SearchCorpus::ClipHistory` variant for ranked full-store
  retrieval that also makes history appear in global unified search.

Non-goals (YAGNI): passive OS-clipboard monitoring; cross-machine/mesh sync of history; rich media
clips (text only in v1); a bespoke fuzzy engine if a trivial scorer suffices.

## 3. Architecture

```
 capture sources                      vox-db: history_entries (per-repo, SSOT)
  ┌─ Console OSC633 (command) ─┐        id, repo_id, kind, text, redacted_text,
  ├─ agent-run command       ─┤──add──► created_at, pinned, source, token_estimate
  ├─ chat turn (conversations)┤        + retention/eviction (per-kind caps, pin-escape)
  └─ user clip (hotkey/CLI)   ─┘                 │
                                                 │  vox://history-changed
        ┌────────────────────────────────────────┼─────────────────────────────┐
   CLI: `vox clip` / `vox history`           Tauri: history_list/add/search/pin/  GUI HistoryPanel
   (Ctrl-R-style fuzzy, exec-policy)          delete/copy_out                      (panelRegistry kind)
        └── fast local filter ──┘            deep search → SearchCorpus::ClipHistory
```

### 3.1 Components

| Unit | File(s) | Responsibility |
|---|---|---|
| `history_entries` table | `vox-db` schema domain (**new**) | Durable per-repo store; columns incl. `kind`, `pinned`, `redacted_text`. |
| History store accessor | `vox-db` (**new** module) | `add` / `list(repo, kind?, limit)` / `pin` / `delete` / `evict(per-kind caps)`. |
| Retention/eviction | same | Per-`kind` caps (config; default: clips longest, command/chat shorter window), pinned never evicted. |
| Secret redaction | a pure `redact(text) -> (display, redacted)` fn | Mask secret patterns on capture via a **small local pattern set** (`sk-…`/`ghp_…`/`gho_…`/AWS `AKIA…`/bearer). NOTE (audit 2026-06-19): NO `vox-clavis` crate exists — do not import Clavis; field-masking prior art = `crates/vox-db/src/socrates_telemetry.rs`. |
| `SearchCorpus::ClipHistory` | `crates/vox-db-types/src/retrieval.rs` (extend enum) + query routing | History joins the unified ranked search. |
| Tauri commands | `crates/vox-gui/src/commands/history.rs` (**new**) | `history_list/add/search/pin/delete` + `vox://history-changed`. (audit 2026-06-19: NO `copy_out` Rust command — no Tauri clipboard plugin; copy-out is a frontend `navigator.clipboard.writeText` action in `HistoryPanel`, as `Console.tsx`/`SearchView.tsx` do.) |
| GUI `HistoryPanel` | `crates/vox-gui/ui/src/components/surfaces/History/HistoryPanel.tsx` (**new**) | Searchable list; local fuzzy filter; per-entry actions; a `panelRegistry` kind. |
| Local fuzzy filter | pure TS `filterEntries(query, entries)` | Instant type-to-filter over the loaded ring. |
| CLI command | `crates/vox-cli/...` `vox clip` / `vox history` (**new**) | Interactive fuzzy search; copy-out / print / re-run; honors `contracts/terminal/exec-policy`. |
| Capture wiring | Console (OSC633), chat append, clip hotkey | Feed `history_add` on the respective events. |

### 3.2 Scope key
`repo_id` derives from the active project/repo via **`vox_orchestrator::lineage::repository_id()`**
(audit-verified `lineage.rs:21`; same `repository_id` scoping `conversations` use). Switching repos
switches the visible ring. There is one store; queries always filter by `repository_id`.

### 3.3 Independence from the OS clipboard
Vox never reads the OS clipboard in the background. Capture is explicit (a GUI "add to history"
hotkey/menu, a CLI `vox clip add`, or auto-on-command via OSC633). **Copy-out** writes the chosen
entry to the OS clipboard on click/select. This keeps secrets from silently flowing in, and keeps
behavior identical cross-platform.

## 4. Data flow & SSOT
vox-db `history_entries` is the single truth. Every surface (CLI, GUI panel, global search) reads it;
mutations emit `vox://history-changed` so an open panel refreshes. The **fast local filter** operates
on the already-loaded page of entries (no round-trip per keystroke); **deep search** ("search all
history") routes through `SearchCorpus::ClipHistory`.

## 5. Error handling
- Capture of empty/whitespace text → ignored (no entry).
- Redaction failure → store the entry but flag `redacted=true` conservatively (never store a raw
  suspected secret if a pattern matched but masking errored).
- `copy_out` when OS clipboard unavailable → surface a toast; entry unchanged.
- Eviction never deletes pinned entries; if all non-pinned are gone and cap exceeded, stop evicting.

## 6. Testing strategy
- **Unit:** store round-trip (add→list by repo+kind); eviction respects per-kind caps + pins; `redact` masks known secret patterns and leaves clean text intact; `filterEntries` ranks substring/subsequence matches deterministically.
- **Integration:** `history_add` emits `vox://history-changed`; `SearchCorpus::ClipHistory` returns history hits in a unified query; OSC633 command boundary creates a `command` entry.

## 7. Decomposition into plan tasks (preview)
1. `history_entries` table (+ schema registration/version bump).
2. Store accessor: `add` + `list`.
3. Retention/eviction (per-kind caps + pin-escape).
4. `redact()` secret-masking.
5. `SearchCorpus::ClipHistory` variant + query routing.
6. Tauri `history_*` commands + `vox://history-changed`.
7. `filterEntries` local fuzzy filter (pure TS).
8. `HistoryPanel` GUI + actions + `panelRegistry` entry.
9. Capture wiring (OSC633 command, chat mirror, clip hotkey).
10. CLI `vox clip` / `vox history` interactive search.
