# Axis GUI Remediation Phase 3 (Tests & CI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute spec Phase 3 (Test & CI buildout) of `docs/superpowers/specs/2026-07-16-axis-gui-audit-remediation-design.md` with fork **F2 resolved = post-merge only, NO PR gating**: make the asserting screenshot sweep fail the post-merge `gui-playwright-smoke` job loudly, close the surface-registry escape (guard test + MissionControl fix), un-skip the empty/error variant sweep as an advisory CI step with toast/alert assertions, add an IPC-failure degradation spec plus three interaction specs (approvals, tasks, chat) against the existing tauriMock — with a fourth, post-Phase-2 interaction task (Task 13: model picker apply, session rename/archive, hopper mark-done) — dedupe the duplicated mock bootstrap into a shared module, fix the visual-review cache key (model id + prompt version + schema_version 1 + dead-viewKey prune), and delete the orphan `playwright.screens.config.ts`.

**Architecture:** Frontend guards are vitest source-scanning tests in `crates/vox-gui/ui/src/guards/` (idiom: `ipcBoundaries.test.ts`, `surfaceHonesty.guard.test.ts`). E2e specs are Playwright specs in `crates/vox-gui/ui/e2e/` driven by init-script Tauri mocks (`e2e/lib/tauriMock.ts` / `tauriMockVariants.ts`); because `page.addInitScript(fn, arg)` serializes the function body, shared mock code is injected as a composed script string via a new `e2e/lib/tauriMockShared.ts`. The visual-review cache lives in `crates/vox-orchestrator-mcp/src/visus_review/` behind feature `gui-visual-review`. CI edits touch only the `gui-playwright-smoke` job in `.github/workflows/ci.yml` (lines ~1620-1734); the required `ci-summary` needs list (line 1449) is **never** touched.

**Tech Stack:** TypeScript + React 19 + Playwright `@playwright/test` + vitest (pnpm-managed, `crates/vox-gui/ui`); Rust (`vox-orchestrator-mcp`, serde/tokio); GitHub Actions YAML.

**Ground rules (Windows / repo policy):**
- All frontend commands run from `C:\Users\Owner\vox\crates\vox-gui\ui` via **pnpm** (never npm). Script names verified from `package.json`: `pnpm typecheck`, `pnpm test` (vitest run), `pnpm test:e2e` (playwright test). Playwright's `playwright.config.ts` auto-boots the Vite dev server on port 1420 (`webServer` block, `reuseExistingServer: !process.env.CI`).
- Rust: `cargo test -p vox-orchestrator-mcp --features gui-visual-review visus_review`. **NEVER `cargo fmt --all`** — use `cargo fmt -p vox-orchestrator-mcp`. **Never pipe cargo output to `head`/`grep`** — redirect to a file in the scratchpad and read it.
- The staged file `contracts/reports/gui-visual-review/0000-00-00.json` is a Phase 1 concern (unstage + guard) — do NOT touch it in this plan.
- YAML edits are surgical: keep existing job/step structure, insert/annotate steps only.

---

## Task 1: Delete the orphan `playwright.screens.config.ts`

Spec Phase 3 item 5 (tail). Verified orphan: `grep -rn "playwright.screens.config"` over the repo matches **only** documentation (`docs/superpowers/specs/*`, `docs/superpowers/plans/2026-06-25-*, 2026-06-26-*`, `docs/src/architecture/vox-gui-visual-audit-2026-06-03.md`, `docs/agents/doc-inventory.json`) — no npm script, no CI step, no Rust invocation references it. The committed `playwright.config.ts` already serves the sweep (same 1420 baseURL, plus a `webServer` block the orphan lacks).

**Files:**
- Delete: `crates/vox-gui/ui/playwright.screens.config.ts` (19 lines)
- Edit: `docs/src/architecture/vox-gui-visual-audit-2026-06-03.md:16-18` (stale reference)

**Steps:**

- [ ] Re-verify orphanhood (expect matches only under `docs/`):
  ```
  cd C:\Users\Owner\vox
  git grep -n "playwright.screens.config" -- ':!docs'
  ```
  Expected output: *no matches* (exit code 1). If any non-docs match appears, STOP and report — the config is not orphaned.
- [ ] `git rm crates/vox-gui/ui/playwright.screens.config.ts`
- [ ] Update the stale architecture doc. Current text at `docs/src/architecture/vox-gui-visual-audit-2026-06-03.md:16-18`:
  ```
    `crates/vox-gui/ui/playwright.screens.config.ts`.
  ...
    `pnpm --dir crates/vox-gui/ui exec playwright test --config=playwright.screens.config.ts`.
  ```
  Replace both mentions so the doc points at the surviving config (do not touch the file's frontmatter):
  - `crates/vox-gui/ui/playwright.config.ts` (the committed config; boots the dev server itself)
  - `pnpm --dir crates/vox-gui/ui exec playwright test screenshots.spec.ts --project=chromium`
  Add one sentence: `The standalone playwright.screens.config.ts was removed 2026-07-16 (orphan — no script or CI referenced it).`
- [ ] Sanity: `cd crates/vox-gui/ui && pnpm exec playwright test --list screenshots.spec.ts` still lists the `GUI visual audit` tests (config resolution unaffected).
- [ ] Commit:
  ```
  git add -A crates/vox-gui/ui/playwright.screens.config.ts docs/src/architecture/vox-gui-visual-audit-2026-06-03.md
  git commit -m "chore(gui): delete orphan playwright.screens.config.ts (no script/CI referent)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 2: Registry-escape guard test (RED first)

Spec Phase 3 item 2 / finding B8. A vitest guard that extracts every routed `case 'x':` from the `childRenderer` switch, every `surfaceDecorators` key, and every special tab render in `App.tsx`, and fails when a routed surface is missing from `SURFACE_REGISTRY` without an explicit, justified allowlist entry.

Ground truth (verified 2026-07-16):
- `ui/src/components/layout/surfaceComponents.tsx:84-210` — the only `switch (viewKey)` in the file; case keys: `dashboard, flow, catalog, memory, vox-search, graphify, mercatus, models, runs, tasks, settings, repository, mesh, gamify, harness, browser, console, approvals, coderabbit, activity, needs-you, mission-control, policies, skills, chat`.
- `SURFACE_REGISTRY` (`ui/src/generated/surfaceRegistry.generated.ts`) contains all of those **except** `graphify` (deliberate one-release alias, lines 117-118 of surfaceComponents: `case 'vox-search': /* fall-through */ case 'graphify':`) and **except** `mission-control` (the B8 escape).
- `surfaceDecorators` keys (`ui/src/components/surfaces/decoratorRegistry.ts:39-58`): `scientia, coverage, mens, populi, research, publications, oratio, sub-agents` — all present in the registry (no violation).
- Special tab render, `ui/src/App.tsx:1123-1125`:
  ```tsx
  const mainSurface = isDocTab(activeTab ?? '')
    ? <DocReader tabId={activeTab!} />
    : renderSurfaceContent(activeView, surfaceProps);
  ```
  `doc:*` tabs are keyed by document path (`useWorkbenchTabs.ts:28-36`), not surface viewKey — allowlisted by design.

**Files:**
- Create: `crates/vox-gui/ui/src/guards/surfaceRegistryEscape.test.ts`

**Steps:**

- [ ] Create `crates/vox-gui/ui/src/guards/surfaceRegistryEscape.test.ts` with exactly:
  ```ts
  import { describe, it, expect } from 'vitest';
  import { readFileSync } from 'node:fs';
  import { join } from 'node:path';
  import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
  import { surfaceDecorators } from '../components/surfaces/decoratorRegistry';

  const SRC_ROOT = join(import.meta.dirname, '..');

  const REGISTERED = new Set(
    SURFACE_REGISTRY.filter((e) => e.viewKey != null).map((e) => e.viewKey as string),
  );

  /**
   * Routed render paths that intentionally have NO SURFACE_REGISTRY entry.
   * Every entry needs a justification — an unexplained addition here defeats
   * the guard. Registry escapes silently skip ALL screenshot/visual coverage
   * (the sweep derives its view list from SURFACE_REGISTRY), so the default
   * answer is: register the surface, don't allowlist it.
   */
  const ALLOWLIST = new Set<string>([
    // One-release deep-link alias for 'vox-search' (surfaceComponents.tsx
    // fall-through case). The canonical key 'vox-search' IS registered and
    // screenshot-covered; the alias renders the identical panel.
    'graphify',
  ]);

  function childRendererCaseKeys(): string[] {
    const src = readFileSync(
      join(SRC_ROOT, 'components/layout/surfaceComponents.tsx'),
      'utf8',
    );
    return [...src.matchAll(/^\s*case '([^']+)':/gm)].map((m) => m[1]);
  }

  describe('surface registry escape guard (B8)', () => {
    it('every childRenderer case key has a SURFACE_REGISTRY entry or a justified allowlist entry', () => {
      const escaped = childRendererCaseKeys().filter(
        (k) => !REGISTERED.has(k) && !ALLOWLIST.has(k),
      );
      expect(
        escaped,
        `Routed in surfaceComponents.tsx but missing from SURFACE_REGISTRY ` +
          `(register via contracts/gui/surface-registry.v1.yaml + ` +
          `\`vox ci gui-surface-registry --write\`, or remove the route): ` +
          JSON.stringify(escaped),
      ).toEqual([]);
    });

    it('every surfaceDecorators key has a SURFACE_REGISTRY entry', () => {
      const escaped = Object.keys(surfaceDecorators).filter((k) => !REGISTERED.has(k));
      expect(escaped, `Decorator surfaces missing from SURFACE_REGISTRY: ${escaped}`).toEqual([]);
    });

    it('DocReader is the only registry-exempt special tab render in App.tsx', () => {
      const app = readFileSync(join(SRC_ROOT, 'App.tsx'), 'utf8');
      const specialRenders = [...app.matchAll(/isDocTab\([^)]*\)\s*\?\s*<(\w+)/g)].map(
        (m) => m[1],
      );
      // doc:* tabs are keyed by document path, not a surface viewKey — a new
      // special tab TYPE must either register a surface or extend this guard
      // with its own justification.
      expect(specialRenders).toEqual(['DocReader']);
    });

    it('allowlist entries stay minimal and still routed (no stale entries)', () => {
      const cases = new Set(childRendererCaseKeys());
      const stale = [...ALLOWLIST].filter((k) => !cases.has(k));
      expect(stale, `Allowlisted keys no longer routed — delete them: ${stale}`).toEqual([]);
    });
  });
  ```
- [ ] Run it RED:
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm exec vitest run src/guards/surfaceRegistryEscape.test.ts
  ```
  Expected: **1 failed / 3 passed** — the first test fails with `escaped` = `["mission-control"]` (message names surfaceComponents.tsx and the regen command). `graphify` must NOT appear (allowlisted). If anything else appears, the registry drifted since this plan was written — investigate before proceeding.
- [ ] Commit the red guard (it documents the escape; Task 3 turns it green):
  ```
  git add crates/vox-gui/ui/src/guards/surfaceRegistryEscape.test.ts
  git commit -m "test(gui): registry-escape guard for routed surfaces (B8) — red on mission-control" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 3: Fix MissionControl (remove the dead route) + delete dead `DiscoveryReviewView` + record the Matrix verdict

**Investigation result (decided; re-verify in steps):**
- `MissionControlPanel` is routed at `surfaceComponents.tsx:178-179` under `'mission-control'`, which appears in the `View` union (`App.tsx:122`) and `LEGACY_VIEWS` (`App.tsx:130`) but has **no** registry entry, **no** sidebar nav, and **no** `navigateTo('mission-control')` caller anywhere in `ui/src` — reachable only by hand-crafted deep link. The panel duplicates already-registered surfaces: subagent tree (= `sub-agents` decorator `SubAgentsView`), pending approvals with `vox_resolve_approval` (= `approvals` / `ApprovalsView`), mesh policy controls (= `mesh` / `MeshView`). Its own header comment (`MissionControlPanel.tsx:1`) says it was parked awaiting a spec that never landed: `// TODO: register in panelRegistry once dockable-workspace spec lands (spec-6)`. **Decision: REMOVE the route and the component** — do not register it in `contracts/gui/surface-registry.v1.yaml`; registering would add a redundant nav surface and permanent screenshot cost for duplicated functionality.
- `Matrix` (`ui/src/components/surfaces/Matrix/Matrix.tsx`) is **NOT dead code — do not remove it.** It is unrouted in `childRenderer`, but `ChatSurface.tsx:20` imports it and renders it at `ChatSurface.tsx:329` as the chat rail's folded Routing panel (`ChatExecutionRail.tsx:32`: "folded Matrix surface — gui-ia-blueprint: matrix → chat rail"). It also has a live unit test (`Matrix.test.tsx`). No action beyond this recorded verdict.
- `Scientia/DiscoveryReviewView.tsx` is the one genuinely dead spec-listed candidate (spec P2 "Dead code candidates"): its only non-test references are its own test file and the `ipcBoundaries.test.ts` allowlist entry (line 69). **Decision: REMOVE it too** (same discipline as MissionControl). The other three spec-listed candidates are NOT dead — `DiscoveryReview.tsx` is live via `DiscoverySurface.tsx:66`, `ScientiaDashboard.tsx` via `ScientiaSurface.tsx:40` (decoratorRegistry), `Settings/PriorityChainEditor.tsx` via `SettingsView.tsx:1407` — record that verdict in the commit body, do not touch them.

**Files:**
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx:18` (import), `:178-179` (case)
- `crates/vox-gui/ui/src/App.tsx:122` (`| 'mission-control'` in the View union), `:130` (`'mission-control'` in `LEGACY_VIEWS`)
- `crates/vox-gui/ui/src/guards/ipcBoundaries.test.ts:60` (MissionControl allowlist entry), `:69` (DiscoveryReviewView allowlist entry)
- `crates/vox-gui/ui/src/lib/lexicon.ts:52` (`'mc-mission'` label)
- Delete: `crates/vox-gui/ui/src/components/surfaces/MissionControl/` (`MissionControlPanel.tsx`, `MissionControlPanel.test.tsx`)
- Delete: `crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReviewView.tsx` + `DiscoveryReviewView.test.tsx`

**Steps:**

- [ ] Re-verify Matrix is reachable (expect the ChatSurface import + render):
  ```
  git grep -n "from '../Matrix/Matrix'" crates/vox-gui/ui/src
  ```
  Expected: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx:20`. If gone, Matrix became dead — flag it in the commit message but do not expand this task.
- [ ] Re-verify nothing navigates to mission-control:
  ```
  git grep -n "mission-control" crates/vox-gui/ui/src
  ```
  Expected matches ONLY: `App.tsx:122`, `App.tsx:130`, `surfaceComponents.tsx:178`, `ipcBoundaries.test.ts:60`, files under `surfaces/MissionControl/`, and the new guard test's failure message (if any). Any other hit (a real caller) = STOP, switch to the register-instead path: add a `mission-control` entry to `contracts/gui/surface-registry.v1.yaml` and run `vox ci gui-surface-registry --write` instead of deleting.
- [ ] `surfaceComponents.tsx`: delete line 18
  (`import { MissionControlPanel } from '../surfaces/MissionControl/MissionControlPanel';`)
  and lines 178-179:
  ```tsx
      case 'mission-control':
        return <MissionControlPanel pushToast={props.pushToast} />;
  ```
- [ ] `App.tsx:122`: delete the union member line `  | 'mission-control'`.
- [ ] `App.tsx:130`: change
  ```ts
    'review', 'tasks', 'mission-control', 'sub-agents',
  ```
  to
  ```ts
    'review', 'tasks', 'sub-agents',
  ```
- [ ] Delete the component directory: `git rm -r crates/vox-gui/ui/src/components/surfaces/MissionControl`
- [ ] `ipcBoundaries.test.ts:60`: delete the allowlist line
  `'components/surfaces/MissionControl/MissionControlPanel.tsx',` (the allowlist is documented as shrink-only).
- [ ] `lexicon.ts:52`: delete the now-unreferenced label entry
  `'mc-mission': { en: 'Mission Control', la: 'Praefectura' },`
  then `git grep -n "mc-mission" crates/vox-gui/ui/src` — expect no matches. If a lexicon completeness test fails in the next step, restore the entry and note why in the commit.
- [ ] Note (do NOT act): the Tauri command `list_mc_approvals` may now be frontend-dead (`list_subagent_tree` is still used by `SubAgents/subAgentClient.ts`). Record this in the commit body as a Phase-2/cleanup candidate; backend command removal is out of Phase 3 scope.
- [ ] Re-verify `DiscoveryReviewView` is dead (expect matches ONLY in its own test and the ipcBoundaries allowlist):
  ```
  git grep -n "DiscoveryReviewView" crates/vox-gui/ui
  ```
  Expected matches ONLY: `src/guards/ipcBoundaries.test.ts:69` (allowlist entry), `src/components/surfaces/Scientia/DiscoveryReviewView.tsx` (its own definition), and `src/components/surfaces/Scientia/DiscoveryReviewView.test.tsx`. Any other hit (an import from a live component) = STOP for this sub-item, keep the file, and record why in the commit body.
- [ ] `git rm crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReviewView.tsx crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReviewView.test.tsx`
- [ ] `ipcBoundaries.test.ts:69`: delete the allowlist line
  `'components/surfaces/Scientia/DiscoveryReviewView.tsx',` (shrink-only allowlist, same as the MissionControl entry). Do NOT touch `DiscoveryReview.tsx` (no "View" suffix) — it is live via `DiscoverySurface.tsx:66`.
- [ ] Verify green:
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm typecheck
  pnpm exec vitest run src/guards/surfaceRegistryEscape.test.ts src/guards/ipcBoundaries.test.ts
  pnpm test
  ```
  Expected: typecheck clean; the registry-escape guard now **4 passed / 0 failed**; ipcBoundaries passes; full vitest suite passes (the deleted `MissionControlPanel.test.tsx` and `DiscoveryReviewView.test.tsx` no longer run).
- [ ] Commit:
  ```
  git add -A crates/vox-gui/ui/src
  git commit -m "fix(gui): remove unregistered mission-control route + panel and dead DiscoveryReviewView; registry-escape guard green (B8)" -m "Matrix confirmed live (ChatSurface routing rail) - kept. DiscoveryReview/ScientiaDashboard/PriorityChainEditor confirmed live - kept. list_mc_approvals now a backend-dead-command candidate." -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 4: Visual-review cache correctness (model id + prompt version + schema_version 1 + prune)

Spec Phase 3 item 5. Ground truth in `crates/vox-orchestrator-mcp/src/visus_review/` (feature-gated `gui-visual-review`, `lib.rs:136-137`):
- `mod.rs:40-46` — `decide_status` keys ONLY on `screenshot_sha256`:
  ```rust
  pub fn decide_status(cache: &CacheIndex, view_key: &str, fresh_sha: &str) -> ReviewDecision {
      match cache.entries.get(view_key) {
          None => ReviewDecision::New,
          Some(e) if e.screenshot_sha256 == fresh_sha => ReviewDecision::Cached,
          Some(_) => ReviewDecision::Changed,
      }
  }
  ```
- `types.rs:77-86` — `CacheIndex` `#[derive(Default)]` yields `schema_version: 0` (the serde `default_schema()` = 1 applies only on deserialize-when-missing), which is why the committed `contracts/reports/gui-visual-review/cache.v1.json` says `"schema_version": 0`. Nothing ever checks it.
- No prompt version exists anywhere (`prompt.rs` has only `RUBRIC`/`system_prompt`/`user_prompt`).
- Dead viewKeys (e.g. `agents`, `archive-panel` — no longer registry viewKeys) persist in the committed cache forever; `run()` (mod.rs:131+) only ever inserts.
- Existing unit tests: `mod.rs:48-86` (`decide_tests`), `types.rs:88-126`.

**Files:**
- `crates/vox-orchestrator-mcp/src/visus_review/types.rs` (CacheEntry, CacheIndex Default, tests)
- `crates/vox-orchestrator-mcp/src/visus_review/prompt.rs` (new `PROMPT_VERSION`)
- `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (decide_status:40-46, run() load ~139-142 / call site ~175 / insert ~234-243 / persist ~267-280, decide_tests:48-86, new `prune_dead_views`)

**Steps:**

- [ ] `prompt.rs` — add above `RUBRIC`:
  ```rust
  /// Cache-busting prompt version. BUMP whenever `RUBRIC`, `system_prompt`, or
  /// `user_prompt` change meaning: a verdict produced under an older prompt must
  /// not satisfy the new one (decide_status compares this against each cache entry).
  pub const PROMPT_VERSION: &str = "2026-07-16.1";
  ```
  Extend the existing `prompt::tests` module with:
  ```rust
      #[test]
      fn prompt_version_is_set() {
          assert!(!PROMPT_VERSION.trim().is_empty());
      }
  ```
- [ ] `types.rs` — `CacheEntry` (lines 68-75) gains a serde-defaulted field so legacy entries deserialize (and then miss the cache exactly once):
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct CacheEntry {
      pub screenshot_sha256: String,
      pub score: u32,
      pub verdict: String,
      pub model: String,
      pub reviewed_at: String,
      /// Prompt version the verdict was produced under (empty on legacy entries).
      #[serde(default)]
      pub prompt_version: String,
  }
  ```
- [ ] `types.rs` — fix the `schema_version: 0` bug: remove `Default` from `CacheIndex`'s derive list and add a manual impl (keep `default_schema()` for serde):
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct CacheIndex {
      #[serde(default = "default_schema")]
      pub schema_version: u32,
      #[serde(default)]
      pub entries: BTreeMap<String, CacheEntry>,
  }

  impl Default for CacheIndex {
      fn default() -> Self {
          Self { schema_version: default_schema(), entries: BTreeMap::new() }
      }
  }
  ```
  Update `types.rs` test `cache_roundtrips` (line 100) to construct the entry with `prompt_version: "2026-07-16.1".into(),` and add:
  ```rust
      #[test]
      fn default_cache_index_is_schema_1_not_0() {
          assert_eq!(CacheIndex::default().schema_version, 1);
      }
      #[test]
      fn legacy_entry_without_prompt_version_deserializes_empty() {
          let json = r#"{ "screenshot_sha256":"aa", "score":90, "verdict":"pass", "model":"m", "reviewed_at":"t" }"#;
          let e: CacheEntry = serde_json::from_str(json).unwrap();
          assert_eq!(e.prompt_version, "");
      }
  ```
- [ ] `mod.rs` — replace `decide_status` (lines 40-46) with:
  ```rust
  /// Cache schema this build reads/writes. A mismatched on-disk cache is
  /// discarded wholesale (one-time full re-review) rather than trusted.
  pub const CACHE_SCHEMA_VERSION: u32 = 1;

  pub fn decide_status(
      cache: &CacheIndex,
      view_key: &str,
      fresh_sha: &str,
      model: &str,
      prompt_version: &str,
  ) -> ReviewDecision {
      match cache.entries.get(view_key) {
          None => ReviewDecision::New,
          Some(e)
              if e.screenshot_sha256 == fresh_sha
                  && e.model == model
                  && e.prompt_version == prompt_version =>
          {
              ReviewDecision::Cached
          }
          Some(_) => ReviewDecision::Changed,
      }
  }

  /// Drop cache entries whose viewKey is absent from the current capture
  /// manifest (dead surfaces). No-op on an empty manifest so a missing/unreadable
  /// manifest never wipes a good cache.
  pub fn prune_dead_views(cache: &mut CacheIndex, manifest: &Manifest) {
      if manifest.surfaces.is_empty() {
          return;
      }
      let live: std::collections::BTreeSet<&str> =
          manifest.surfaces.iter().map(|s| s.view_key.as_str()).collect();
      cache.entries.retain(|k, _| live.contains(k.as_str()));
  }
  ```
  Note: `Manifest` is declared below the tests in the current file — move nothing; Rust items are order-independent at module scope.
- [ ] `mod.rs` `run()` — after the cache load (current lines 139-142 ending `.unwrap_or_default();`), add the schema check:
  ```rust
      if cache.schema_version != CACHE_SCHEMA_VERSION {
          eprintln!(
              "::warning::gui-visual-review: cache schema_version {} != {} — discarding cache (one-time full re-review)",
              cache.schema_version, CACHE_SCHEMA_VERSION
          );
          cache = CacheIndex::default();
      }
  ```
- [ ] `mod.rs` `run()` — update the single `decide_status` call (current line 175):
  ```rust
          let decision = decide_status(&cache, &entry.view_key, &entry.sha256, &model, prompt::PROMPT_VERSION);
  ```
  (`model` is the `String` chosen at lines 156-164; add `use crate::visus_review::prompt;` is unnecessary — `prompt` is a sibling module, reference as `prompt::PROMPT_VERSION`.)
- [ ] `mod.rs` `run()` — the cache insert (current lines 234-243) gains the field:
  ```rust
                          CacheEntry {
                              screenshot_sha256: entry.sha256.clone(),
                              score: report.score.unwrap_or(0),
                              verdict: report.verdict.clone().unwrap_or_default(),
                              model: report.model.clone().unwrap_or_else(|| model.clone()),
                              reviewed_at: args.now_iso.clone(),
                              prompt_version: prompt::PROMPT_VERSION.to_string(),
                          },
  ```
- [ ] `mod.rs` `run()` — in the persist block (`if args.do_ai {` at current line 268), prune + stamp before serializing:
  ```rust
      if args.do_ai {
          prune_dead_views(&mut cache, &manifest);
          cache.schema_version = CACHE_SCHEMA_VERSION;
          if let Some(parent) = args.cache_path.parent() {
  ```
  (rest of the block unchanged).
- [ ] `mod.rs` — extend `decide_tests` (lines 48-86). Update the helper and existing tests to the new signature, and add the new cases:
  ```rust
  #[cfg(test)]
  mod decide_tests {
      use super::*;
      const M: &str = "google/gemini-3-flash-preview";
      const PV: &str = "2026-07-16.1";
      fn cache_with(view: &str, sha: &str, model: &str, prompt_version: &str) -> CacheIndex {
          let mut c = CacheIndex::default();
          c.entries.insert(
              view.into(),
              CacheEntry {
                  screenshot_sha256: sha.into(),
                  score: 90,
                  verdict: "pass".into(),
                  model: model.into(),
                  reviewed_at: "t".into(),
                  prompt_version: prompt_version.into(),
              },
          );
          c
      }
      #[test]
      fn new_surface_is_new() {
          assert_eq!(decide_status(&CacheIndex::default(), "x", "aa", M, PV), ReviewDecision::New);
      }
      #[test]
      fn same_hash_model_and_prompt_is_cached() {
          assert_eq!(decide_status(&cache_with("x", "aa", M, PV), "x", "aa", M, PV), ReviewDecision::Cached);
      }
      #[test]
      fn different_hash_is_changed() {
          assert_eq!(decide_status(&cache_with("x", "aa", M, PV), "x", "bb", M, PV), ReviewDecision::Changed);
      }
      #[test]
      fn different_model_is_changed_even_with_same_hash() {
          assert_eq!(decide_status(&cache_with("x", "aa", "old/model", PV), "x", "aa", M, PV), ReviewDecision::Changed);
      }
      #[test]
      fn different_prompt_version_is_changed_even_with_same_hash() {
          assert_eq!(decide_status(&cache_with("x", "aa", M, "2026-01-01.1"), "x", "aa", M, PV), ReviewDecision::Changed);
      }
      #[test]
      fn legacy_entry_empty_prompt_version_is_changed() {
          assert_eq!(decide_status(&cache_with("x", "aa", M, ""), "x", "aa", M, PV), ReviewDecision::Changed);
      }
      #[test]
      fn prune_drops_views_absent_from_manifest() {
          let mut c = cache_with("dead-view", "aa", M, PV);
          c.entries.extend(cache_with("live-view", "bb", M, PV).entries);
          let manifest = Manifest {
              total_capture_ms: 0,
              surfaces: vec![ManifestEntry {
                  view_key: "live-view".into(),
                  file: "live-view.png".into(),
                  sha256: "bb".into(),
                  capture_ms: 1,
              }],
          };
          prune_dead_views(&mut c, &manifest);
          assert!(c.entries.contains_key("live-view"));
          assert!(!c.entries.contains_key("dead-view"));
      }
      #[test]
      fn prune_is_noop_on_empty_manifest() {
          let mut c = cache_with("x", "aa", M, PV);
          prune_dead_views(&mut c, &Manifest { total_capture_ms: 0, surfaces: vec![] });
          assert_eq!(c.entries.len(), 1);
      }
  }
  ```
- [ ] Build + test (redirect, never pipe cargo; PowerShell syntax):
  ```
  cd C:\Users\Owner\vox
  cargo test -p vox-orchestrator-mcp --features gui-visual-review visus_review > "$env:TEMP\visus_test.log" 2>&1
  ```
  Read the log. Expected: all `visus_review` tests pass (decide_tests 8 — the eight `#[test]` fns in the replacement module above — verdict_tests 2, types tests 5, prompt tests 2, plus model_select/spike modules untouched). Then:
  ```
  cargo clippy -p vox-orchestrator-mcp --features gui-visual-review -- -D warnings > "$env:TEMP\visus_clippy.log" 2>&1
  cargo fmt -p vox-orchestrator-mcp
  ```
- [ ] Note in the commit body: the committed `contracts/reports/gui-visual-review/cache.v1.json` still says `schema_version: 0` — the next post-merge AI run discards it, fully re-reviews once, prunes dead viewKeys, and the CI bot commits the migrated v1 cache (`ci.yml` step "Commit visual-review cache + report"). Deliberate one-time cost; do not hand-edit the JSON.
- [ ] Commit:
  ```
  git add crates/vox-orchestrator-mcp/src/visus_review
  git commit -m "fix(visual-review): cache keys on model+prompt version, schema_version 1 enforced, dead viewKeys pruned" -m "Committed cache.v1.json (schema 0) self-migrates via one full re-review on the next post-merge AI run." -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 5: Dedupe the mock bootstrap into `tauriMockShared.ts` (+ event-emit support)

Spec Phase 3 item 4 (tail). The duplication (verified): `tauriMock.ts:10-26` (localStorage seed + `__TAURI_CALLS__` + event-plugin internals + transformCallback) is copied in `tauriMockVariants.ts:74-104` and again at `:139-170`; `bootstrapResponse` is duplicated verbatim inside `tauriMockVariants.ts` (`:57-72` and `:122-137`, each carrying a "keep both copies in sync" NOTE); `tauriMock.ts`'s big switch re-implements the same bootstrap cases inline (`get_initial_view`, `get_build_info`, `get_orchestrator_status_bin`, `get_orchestrator_status`, `get_identity_summary`, `get_gui_preference`, `get_action_manifest`, `plugin:event|listen`, `plugin:event|unlisten`).

**Constraint that forced the duplication:** `page.addInitScript(fn, arg)` serializes only `fn`'s body — module-scope imports are not available in the browser. **Solution:** compose a single init-script *string* that first defines `window.__VOX_MOCK_SHARED__` from the shared functions' `.toString()` source, then invokes the installer. Installers read the shared helpers off `window`. This also adds the event-listener registry + `window.__TAURI_EMIT__` needed by Task 10's chat stream spec.

**Files:**
- Create: `crates/vox-gui/ui/e2e/lib/tauriMockShared.ts`
- Edit: `crates/vox-gui/ui/e2e/lib/tauriMock.ts` (lines 10-26 seed block, duplicated switch cases, `default:` arm)
- Edit: `crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts` (both installers; delete both `bootstrapResponse` copies + seed blocks)
- Edit: `crates/vox-gui/ui/e2e/lib/tauriMockVariants.test.ts` (installers now need the shared object)
- Edit call sites (all `page.addInitScript(<installer>, view)` → `addMockInitScript(page, <installer>, view)`):
  `e2e/screenshots.spec.ts:54,82,100` · `e2e/screenshots-variants.spec.ts:48,74` · `e2e/workbench-tabs.spec.ts:35,55,71,80,91,105,116,126,159,171` · `e2e/axis-brand.spec.ts:18,26,49` · `e2e/console-workbench.spec.ts:6` · `e2e/dashboard.spec.ts:6` · `e2e/visual-review.spec.ts:28`
- NOT touched: `e2e/lib/operatorShellMock.ts` (self-contained different mock; out of scope — note only).

**Steps:**

- [ ] Create `crates/vox-gui/ui/e2e/lib/tauriMockShared.ts`:
  ```ts
  /**
   * Shared environment seeding + bootstrap responses for the Tauri-invoke mocks
   * (tauriMock.ts, tauriMockVariants.ts).
   *
   * `page.addInitScript(fn, arg)` serialises ONLY the function body, so module
   * imports are invisible in the browser. Instead, `addMockInitScript` composes
   * one script string that (1) installs these helpers on
   * `window.__VOX_MOCK_SHARED__` from their `.toString()` source and (2) invokes
   * the installer. Every function here must therefore be self-contained (no
   * captured module scope).
   */
  import type { Page } from '@playwright/test';

  /** Seed localStorage tabs, call log, event plumbing, and transformCallback. */
  export function seedMockEnvironment(viewKey: string): void {
    try {
      window.localStorage.setItem(
        'vox_workbench_tabs.v1',
        JSON.stringify({
          openTabs: Array.from(new Set(['chat', viewKey])),
          activeTab: viewKey,
        }),
      );
      window.localStorage.setItem('vox_sidebar_mode', 'default');
    } catch {
      // sandboxed contexts may deny localStorage
    }
    (window as any).__TAURI_CALLS__ = [];
    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (_event: string, _eventId: number) => {},
    };
    // Event registry + emit helper: `plugin:event|listen` registers transformed
    // callback ids here; specs drive streams via
    //   page.evaluate(([ev, payload]) => (window as any).__TAURI_EMIT__(ev, payload), [...])
    (window as any).__TAURI_EVENT_LISTENERS__ = {} as Record<string, string[]>;
    (window as any).__TAURI_EMIT__ = (event: string, payload: unknown) => {
      const ids: string[] = ((window as any).__TAURI_EVENT_LISTENERS__ ?? {})[event] ?? [];
      for (const id of ids) {
        const cb = (window as any)[id];
        if (typeof cb === 'function') cb({ event, id: 0, payload });
      }
    };
    (window as any).__TAURI_INTERNALS__ = {
      ...((window as any).__TAURI_INTERNALS__ || {}),
      transformCallback: (cb: (...args: unknown[]) => unknown) => {
        const id = `cb_${Math.random().toString(36).slice(2)}`;
        (window as any)[id] = cb;
        return id;
      },
    };
  }

  /** Handle the tauri event plugin commands; `undefined` means "not an event cmd". */
  export function eventPluginResponse(cmd: string, args: any): number | null | undefined {
    if (cmd === 'plugin:event|listen') {
      const reg = (window as any).__TAURI_EVENT_LISTENERS__ as
        | Record<string, string[]>
        | undefined;
      if (reg && typeof args?.event === 'string' && typeof args?.handler === 'string') {
        (reg[args.event] ??= []).push(args.handler);
      }
      return Math.floor(Math.random() * 10000);
    }
    if (cmd === 'plugin:event|unlisten') return null;
    return undefined;
  }

  /** Commands that must succeed for the app shell to mount at all (single copy). */
  export function bootstrapResponse(cmd: string, viewKey: string): unknown {
    switch (cmd) {
      case 'get_initial_view': return viewKey;
      case 'get_build_info': return { version: '0.6.0', display: '0.6.0+local (dev)' };
      case 'get_orchestrator_status_bin': return new Uint8Array([0x80]);
      case 'get_orchestrator_status': return { agent_count: 0, agents: [], recent_events: [], alerts: [], peers: [] };
      case 'get_action_manifest': return { x_vox_version: 2, schema_version: 1, generated_from: 'mock', actions: [] };
      case 'get_gui_preference': return null;
      case 'get_gamify_settings': return { enabled: false, mode: 'off' };
      case 'get_identity_summary': return { display_name: 'tester@vox', os_user: 'tester' };
      case 'get_active_model': return null;
      case 'get_selection_policy': return { chain: [], free_tier: true };
      case 'vox_docs_index': return [];
      default: return null;
    }
  }

  const SHARED_SNIPPET = [
    'window.__VOX_MOCK_SHARED__ = {',
    `  seedMockEnvironment: ${seedMockEnvironment.toString()},`,
    `  eventPluginResponse: ${eventPluginResponse.toString()},`,
    `  bootstrapResponse: ${bootstrapResponse.toString()},`,
    '};',
  ].join('\n');

  /** Compose the full init script for an installer (exported for unit tests). */
  export function mockInitScript(installer: (viewKey: string) => void, viewKey: string): string {
    return `${SHARED_SNIPPET}\n(${installer.toString()})(${JSON.stringify(viewKey)});`;
  }

  /** The ONLY supported way to inject a mock installer into a Playwright page. */
  export async function addMockInitScript(
    page: Page,
    installer: (viewKey: string) => void,
    viewKey: string,
  ): Promise<void> {
    await page.addInitScript({ content: mockInitScript(installer, viewKey) });
  }

  /** Vitest helper: run an installer against the (fake) global window. */
  export function runInstallerWithShared(
    installer: (viewKey: string) => void,
    viewKey: string,
  ): void {
    (globalThis as any).window.__VOX_MOCK_SHARED__ = {
      seedMockEnvironment,
      eventPluginResponse,
      bootstrapResponse,
    };
    installer(viewKey);
  }
  ```
- [ ] Rewrite `tauriMockVariants.ts`: delete both `bootstrapResponse` copies (lines 52-72 and 119-137), both seed blocks (74-95 and 139-160), and both sync NOTEs. Each installer begins:
  ```ts
  export function installEmptyStateMock(viewKey: string): void {
    const shared = (window as any).__VOX_MOCK_SHARED__;
    if (!shared) {
      throw new Error(
        'installEmptyStateMock must be injected via addMockInitScript (e2e/lib/tauriMockShared.ts)',
      );
    }
    // ... LIST_CMDS / DETAIL_CMDS / emptyDetailResponse stay verbatim ...
    shared.seedMockEnvironment(viewKey);
    (window as any).__TAURI_INTERNALS__ = {
      ...((window as any).__TAURI_INTERNALS__ || {}),
      invoke: async (cmd: string, args?: any) => {
        (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
        if (LIST_CMDS.has(cmd)) return [];
        if (DETAIL_CMDS.has(cmd)) return emptyDetailResponse(cmd);
        const ev = shared.eventPluginResponse(cmd, args);
        if (ev !== undefined) return ev;
        return shared.bootstrapResponse(cmd, viewKey);
      },
    };
  }
  ```
  `installErrorStateMock` mirrors it (guard message names itself; `ERROR_CMDS` check replaces the LIST/DETAIL checks; same event/bootstrap tail). Update the file header comment: usage is now `await addMockInitScript(page, installErrorStateMock, viewKey)`.
- [ ] Rewrite `tauriMock.ts` head (current lines 10-26): same `shared` guard + `shared.seedMockEnvironment(viewKey)` replacing the seed block and the inline `transformCallback` (keep the rich data constants and the `invoke` switch). Delete these now-shared switch cases: `get_initial_view` (127), `get_build_info` (128), `get_orchestrator_status_bin` (129), `get_orchestrator_status` (130), `get_identity_summary` (228), `get_gui_preference` (270), `plugin:event|listen` (305), `plugin:event|unlisten` (306). KEEP the rich overrides that intentionally differ from bootstrap: `get_gamify_settings` (enabled:true), `get_active_model` ('opus-4-8'), `get_selection_policy` (chain), `vox_docs_index` (CLI Reference entry), `get_action_manifest` — delete this one too (identical after Task's `generated_from: 'mock'` unification in shared). Replace `default: return null;` (line 310) with:
  ```ts
          default: {
            const ev = shared.eventPluginResponse(cmd, args);
            if (ev !== undefined) return ev;
            return shared.bootstrapResponse(cmd, viewKey);
          }
  ```
  Update the doc comment (lines 1-9): injection is via `addMockInitScript`, not raw `addInitScript`.
- [ ] Migrate ALL call sites listed in **Files** above. Mechanical change per site, e.g. `screenshots.spec.ts:54`:
  ```ts
  // before
  await page.addInitScript(installTauriMock, view);
  // after
  await addMockInitScript(page, installTauriMock, view);
  ```
  plus `import { addMockInitScript } from './lib/tauriMockShared';` in each spec. Secondary `addInitScript` calls that are NOT mock installers (e.g. `screenshots.spec.ts:102` sidebar-mode localStorage, `workbench-tabs.spec.ts` equivalents) stay as-is. Verify no stragglers **among the three migrated installers only**:
  ```
  git grep -nE "addInitScript\((installTauriMock|installEmptyStateMock|installErrorStateMock)" crates/vox-gui/ui/e2e
  ```
  Expected: no matches. Note: a broader `addInitScript(install` grep still matches by design — the nine `installOperatorShellMock` spec call sites (chat-composer-dock, chat-session-rail, coderabbit, dashboard-pilot, dock-layout, palette-search-navigate, policies, status-bar-surfaces, submit-task-palette) plus doc-comment lines in `operatorShellMock.ts` and `tauriMockVariants.ts` are expected leftovers. `operatorShellMock` is out of scope for this task; do NOT "migrate" those sites — the installer is self-contained and does not read `window.__VOX_MOCK_SHARED__`.
- [ ] Update `tauriMockVariants.test.ts`: import `runInstallerWithShared, mockInitScript` from `./tauriMockShared`; every direct `installEmptyStateMock('x')` / `installErrorStateMock('x')` call becomes `runInstallerWithShared(installEmptyStateMock, 'x')` (11 call sites, lines 37-118). Add a serialization regression test at the end:
  ```ts
  describe('mockInitScript serialization', () => {
    it('composed script is self-contained and runs against a bare window', async () => {
      await withFakeWindow(async (win) => {
        // eslint-disable-next-line no-new-func -- exercising the exact addInitScript path
        new Function(mockInitScript(installErrorStateMock, 'runs'))();
        expect(win.__VOX_MOCK_SHARED__).toBeDefined();
        await expect(win.__TAURI_INTERNALS__.invoke('list_gui_runs')).rejects.toThrow('[mock-error]');
        expect(await win.__TAURI_INTERNALS__.invoke('get_initial_view')).toBe('runs');
      });
    });
    it('emit helper dispatches to listeners registered via plugin:event|listen', async () => {
      await withFakeWindow(async (win) => {
        runInstallerWithShared(installEmptyStateMock, 'dashboard');
        const seen: unknown[] = [];
        const handler = win.__TAURI_INTERNALS__.transformCallback((e: any) => seen.push(e.payload));
        await win.__TAURI_INTERNALS__.invoke('plugin:event|listen', { event: 'vox://agent-events', handler });
        win.__TAURI_EMIT__('vox://agent-events', { id: 1, kind: { type: 'task_started' } });
        expect(seen).toHaveLength(1);
      });
    });
  });
  ```
  (`new Function` bodies reference bare `window`; the existing `withFakeWindow` swap of `(global as any).window` covers it — the shared functions consistently use `window.localStorage`, never bare `localStorage`.)
- [ ] Verify:
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm typecheck
  pnpm exec vitest run e2e/lib/tauriMockVariants.test.ts
  pnpm exec playwright test dashboard.spec.ts workbench-tabs.spec.ts --project=chromium
  pnpm exec playwright test screenshots.spec.ts --project=chromium --workers=4
  ```
  Expected: all green (the sweep behaves identically — the delegated bootstrap answers match the deleted inline cases; `get_action_manifest.generated_from` changes 'mock'→'mock' for tauriMock and 'mock-empty'→'mock' for variants, which nothing asserts).
- [ ] Commit:
  ```
  git add crates/vox-gui/ui/e2e
  git commit -m "refactor(gui-e2e): dedupe mock bootstrap/seed into tauriMockShared + __TAURI_EMIT__ event support" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 6: Variant error runs assert visible toast/alert (+ fix the dead `search` key)

Spec Phase 3 item 3 (assertions half). `e2e/screenshots-variants.spec.ts` error loop (lines 67-86) currently only screenshots and checks `pageErrors`. Also: `KEY_SURFACES` (line 21-24) contains `'search'`, which is **not** a `childRenderer` case (the registry key is `'vox-search'`) — its variant screenshots capture a null (blank) surface today.

The global toast container (`ui/src/components/ui/Toasts.tsx:28-33`) is an always-rendered `role="status"` div; individual toasts are its `.pointer-events-auto` children — so assert on children, never on the container.

**Files:**
- `crates/vox-gui/ui/e2e/screenshots-variants.spec.ts` (lines 21-24, 67-86)

**Steps:**

- [ ] Line 21-24: replace `'search'` with `'vox-search'`:
  ```ts
  const KEY_SURFACES = [
    'dashboard', 'chat', 'runs', 'approvals', 'models',
    'memory', 'vox-search', 'policies', 'gamify', 'settings',
  ] as const;
  ```
- [ ] In the error-states loop, after the existing `expect(pageErrors, ...)` at line 82, add the affordance assertion. Two hardening rules (both load-bearing): (1) use `expect.poll` (auto-retrying), never a fixed sleep + one-shot count — a toast appearing at 1300ms on a loaded runner must not flip the result; (2) scope the alert/copy checks to the workbench main panel and exclude the global "Chat sessions" toast — `chat_list_sessions` is already in `ERROR_CMDS` and its failure fires an app-level toast from `App.tsx:396-406` on EVERY view, so an unscoped count is always >= 1 and the assertion could never fail. The main-panel container is the surface content rendered inside `SurfaceErrorBoundary` (`AppShell.tsx:143-148`), stable locator `[data-testid="surface-scroll-host"]` (`SurfaceScrollHost.tsx:5`).
  ```ts
        // Visible degradation, not a blank panel: at least one toast item
        // attributable to THIS surface (the global 'Chat sessions' toast fires
        // on every view and must not vacuously satisfy other surfaces), a
        // role=alert region in the main panel, or visible error copy in the
        // main panel. Auto-retrying: toast/alert timing varies on CI runners.
        const mainPanel = page.getByTestId('surface-scroll-host');
        const toastItems =
          view === 'chat'
            ? page.getByRole('status').locator('.pointer-events-auto')
            : page.getByRole('status').locator('.pointer-events-auto').filter({ hasNotText: /chat sessions/i });
        const alerts = mainPanel.getByRole('alert');
        const errorCopy = mainPanel.getByText(/error|failed|unavailable|could not|retry/i);
        await expect
          .poll(
            async () =>
              (await toastItems.count()) + (await alerts.count()) + (await errorCopy.count()),
            {
              timeout: 10_000,
              message: `[${view}-error] no visible toast/alert/error copy — surface degraded to a blank panel`,
            },
          )
          .toBeGreaterThan(0);
  ```
  Keep the env gate (`test.skip(!RUN_VARIANTS, ...)`, lines 44 and 70) — CI opts in via the workflow env var (Task 11); local default stays skipped. Update the file header comment (lines 3-5) to mention the post-merge CI step.
- [ ] Verify locally (opt in via env — PowerShell syntax; `set X=1` is a cmd.exe-ism that silently fails to export in PowerShell, making every test self-skip and the run a false green):
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  $env:VOX_VARIANT_SCREENSHOTS = '1'
  pnpm exec playwright test screenshots-variants.spec.ts --project=chromium --workers=2
  Remove-Item Env:VOX_VARIANT_SCREENSHOTS
  ```
  Expected: **20 passed, 0 skipped** (10 empty + 10 error). Any `skipped` count > 0 means the env gate never opened — the run verified nothing; fix the env var before trusting it. Empty-state tests pass as before; error-state tests pass for surfaces that render error affordances. **If a surface fails the new assertion**, that is a real Phase-3 finding: check whether Task 7's `ERROR_CMDS` additions (`invoke_mcp_tool`, `hopper_list`) cover it; if the surface genuinely renders blank on error, keep the assertion (variants are advisory in CI per F2) and list the failing surface(s) in the commit body as remediation candidates — do NOT weaken the assertion.
- [ ] Commit:
  ```
  git add crates/vox-gui/ui/e2e/screenshots-variants.spec.ts
  git commit -m "test(gui-e2e): variant error runs assert visible toast/alert affordance; fix dead 'search' key -> 'vox-search'" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 7: IPC-failure degradation spec (`error-states.spec.ts`) + `ERROR_CMDS` gaps

Spec Phase 3 item 3 (new-spec half): a spec using `installErrorStateMock` (`e2e/lib/tauriMockVariants.ts:107-171` pre-Task-5; same export after) that runs in the **default asserting sweep** (no env gate → executes inside the loud CI sweep step, Task 11) and asserts key surfaces degrade with visible error UI, not blank panels.

Coverage gaps found in `ERROR_CMDS` (lines 109-117): Approvals loads via `invoke_mcp_tool` (`ApprovalsView.tsx:119` → `vox_pending_approvals`) and Tasks via `hopper_list` (`transport.ts:797-799`) — neither command is in `ERROR_CMDS`, so today the error mock exercises neither surface's failure path.

**Reality check on which surfaces can actually show error UI (verified 2026-07-16):**
- **tasks:** in the shipped app TasksView runs in shared-attention mode — App always passes `attention` (`App.tsx:1120`, `surfaceComponents.tsx:127`), and the actual `hopper_list` caller is `useAttentionInbox`, which **silently swallows** rejections: `Promise.resolve(hopperList()).catch(() => [] as HopperTaskDto[])` (`useAttentionInbox.ts:34`). `TasksView.setError` (`TasksView.tsx:61-62`) fires only in the self-fetch mode the app never uses. So with `hopper_list` in `ERROR_CMDS`, the tasks surface renders an **empty list, not error UI** — a real silent-swallow defect, excluded from `KEY_SURFACES` below with a TODO trail.
- **dashboard:** consumes only the bootstrap `get_orchestrator_status_bin` payload (which the error mock always answers) — its default widgets and `useAgentApprovals` swallow their own errors, so the error mock exercises no dashboard failure path at all. Also excluded with a TODO trail.
- **Vacuity hazard for all remaining surfaces:** `chat_list_sessions` (already in `ERROR_CMDS`) fails at App mount on EVERY view and pushes a global "Chat sessions" warn toast (`App.tsx:396-406`, ~5s lifetime) — an unscoped toast/text count is therefore always >= 1 and assertion 3 could never fail. The spec below scopes alerts/error-copy to the workbench main panel (`SurfaceErrorBoundary` content, `AppShell.tsx:143-148` → `[data-testid="surface-scroll-host"]`) and excludes the chat-sessions toast on non-chat views.

**Files:**
- Edit: `crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts` (`ERROR_CMDS` set)
- Create: `crates/vox-gui/ui/e2e/error-states.spec.ts`

**Steps:**

- [ ] Add to `ERROR_CMDS` in `installErrorStateMock`:
  ```ts
      'invoke_mcp_tool', 'hopper_list',
  ```
  (`invoke_mcp_tool` failures are caught by `ApprovalsView.refresh` → "Approvals load failed" toast, and by `feedbackList().catch` consumers. `hopper_list` is added **knowingly exercising a silent-swallow defect**: `useAttentionInbox.ts:34` converts the rejection to `[]` with no toast/error state, so no affordance reaches the tasks surface today — the entry documents the defect and becomes meaningful the moment a follow-up gives the attention inbox an error affordance.)
- [ ] Create `crates/vox-gui/ui/e2e/error-states.spec.ts`:
  ```ts
  /**
   * IPC-failure degradation audit. Runs in the DEFAULT Playwright sweep (no env
   * gate), i.e. inside the loud post-merge CI step: when a surface's data IPC
   * throws, the app shell must stay up and the surface must show visible error
   * UI — never a blank panel, never an uncaught rejection.
   */
  import { test, expect } from '@playwright/test';
  import { installErrorStateMock } from './lib/tauriMockVariants';
  import { addMockInitScript } from './lib/tauriMockShared';

  // TODO(phase3-followup): tasks — useAttentionInbox swallows hopper_list
  // rejections with `.catch(() => [])` (useAttentionInbox.ts:34) and TasksView's
  // setError path never runs in shared-attention mode, so the surface renders an
  // EMPTY list (no affordance) on IPC failure. Re-add once the inbox surfaces
  // fetch errors.
  // TODO(phase3-followup): dashboard — consumes only bootstrap orchestrator
  // status; its widgets + useAgentApprovals swallow errors, so the error mock
  // exercises no dashboard failure path. Re-add once dashboard has a real
  // data-error affordance.
  const KEY_SURFACES = ['chat', 'runs', 'approvals', 'models'] as const;

  test.describe('IPC-failure degradation', () => {
    for (const view of KEY_SURFACES) {
      test(`${view} degrades visibly when data IPC throws`, async ({ browser }) => {
        const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
        const page = await ctx.newPage();
        const pageErrors: string[] = [];
        page.on('pageerror', (e) => pageErrors.push(e.message));
        await addMockInitScript(page, installErrorStateMock, view);
        await page.goto('/');
        await page.waitForSelector('nav', { timeout: 15_000 });
        await expect(page.getByTestId('workbench-tab-bar')).toBeVisible();
        // Short bounded settle so async uncaught rejections have time to surface
        // before assertion 1 (the affordance check below is auto-retrying and
        // needs no sleep).
        await page.waitForTimeout(1200);

        // 1. Failures are HANDLED — no uncaught exceptions/rejections.
        expect(pageErrors, `[${view}] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
        // 2. Not blank: the page body renders substantive content (shell + surface chrome).
        const bodyText = (await page.locator('body').innerText()).trim();
        expect(bodyText.length, `[${view}] rendered blank on IPC failure`).toBeGreaterThan(0);
        // 3. Visible error affordance attributable to THIS surface. Scoped to the
        // workbench main panel (surface content inside SurfaceErrorBoundary,
        // AppShell.tsx:143-148) so static chrome elsewhere can't satisfy it, and
        // excluding the global 'Chat sessions' toast (App.tsx:396-406 fires it on
        // EVERY view because chat_list_sessions is in ERROR_CMDS) so the count
        // can actually be 0 on a blank panel. Auto-retrying: no fixed-sleep race.
        const mainPanel = page.getByTestId('surface-scroll-host');
        const toastItems =
          view === 'chat'
            ? page.getByRole('status').locator('.pointer-events-auto')
            : page.getByRole('status').locator('.pointer-events-auto').filter({ hasNotText: /chat sessions/i });
        const alerts = mainPanel.getByRole('alert');
        const errorCopy = mainPanel.getByText(/error|failed|unavailable|could not|retry/i);
        await expect
          .poll(
            async () =>
              (await toastItems.count()) + (await alerts.count()) + (await errorCopy.count()),
            { timeout: 10_000, message: `[${view}] no visible error affordance` },
          )
          .toBeGreaterThan(0);
        await ctx.close();
      });
    }
  });
  ```
- [ ] Run it:
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm exec playwright test error-states.spec.ts --project=chromium
  ```
  Expected: **4 passed** (chat, runs, approvals, models — tasks and dashboard are pre-declared assertion-3 gaps, see the TODO trail above `KEY_SURFACES`). **If one of the four fails**: assertion 1 or 2 failing is a real defect — apply the minimal handler fix in that surface (idiom: `.catch` + `setError`/`pushToast`, see `TasksView.tsx:50-66`) and keep the test; assertion 3 failing means the surface swallows errors silently — if the minimal fix is not obvious, move that view to the TODO trail with a `// TODO(phase3-followup): <view> renders no error affordance — <one-line finding>` comment above the array. Never ship a weakened assertion without the TODO trail.
- [ ] Re-run the variant vitest suite (ERROR_CMDS changed): `pnpm exec vitest run e2e/lib/tauriMockVariants.test.ts` — expected green.
- [ ] Commit:
  ```
  git add crates/vox-gui/ui/e2e/error-states.spec.ts crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts
  git commit -m "test(gui-e2e): IPC-failure degradation spec in the asserting sweep; error-mock covers invoke_mcp_tool + hopper_list" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 8: Interaction spec — Approvals approve/reject

Spec Phase 3 item 4. Ground truth: `ApprovalsView.tsx:119` loads via `voxTransport.invokeMcpTool('vox_pending_approvals', {})`; resolution at `:161` calls `invokeMcpTool('vox_resolve_approval', { approval_id, outcome })`, checks `unwrapMcpEnvelope(res.result)` for `resolved === false`, toasts `Approved`/`Rejected` (`:166-171`), and filters the row out. Buttons carry `aria-label={'Approve ' + r.summary}` / `aria-label={'Reject ' + r.summary}` (`:245,255`). The tauriMock's `mcpResult` (`tauriMock.ts:97-115`) currently returns a single hardcoded pending approval and `{ ok: true }` for everything else — it must become stateful.

**Files:**
- Edit: `crates/vox-gui/ui/e2e/lib/tauriMock.ts` (`mcpResult`, `invoke_mcp_tool` case, seed)
- Create: `crates/vox-gui/ui/e2e/approvals-interactions.spec.ts`

**Steps:**

- [ ] In `installTauriMock`, seed mutable approval state next to the other constants (after the `models` array):
  ```ts
    (window as any).__MOCK_APPROVALS__ = [
      {
        approval_id: 'AP-000001',
        tool: 'vox_run_shell',
        summary: 'rm -rf build',
        requested_at_ms: 1717400000000,
        resolved: false,
      },
    ];
  ```
- [ ] Make `mcpResult` stateful — replace the current function with:
  ```ts
    const mcpResult = (tool: string, targs?: any) => {
      if (tool.includes('mesh_nodes')) return { nodes: [{ id: 'node-a', status: 'online', vram_gb: 24 }, { id: 'node-b', status: 'online', vram_gb: 12 }], edges: [] };
      if (tool.includes('resolve_approval')) {
        const id = String(targs?.approval_id ?? '');
        const hit = ((window as any).__MOCK_APPROVALS__ as any[]).find(a => a.approval_id === id);
        if (hit) hit.resolved = true;
        return { success: true, data: { resolved: !!hit } };
      }
      if (tool.includes('pending_approval')) {
        const pending = ((window as any).__MOCK_APPROVALS__ as any[])
          .filter(a => !a.resolved)
          .map(({ resolved: _r, ...a }) => a);
        return { success: true, data: { approvals: pending } };
      }
      if (tool.includes('git_diff')) return { success: true, data: 'diff --git a/README.md b/README.md\n' };
      if (tool.includes('skill') || tool.includes('plugin')) return { skills: [{ id: 'superpowers', name: 'Superpowers', enabled: true }], plugins: [{ id: 'design', name: 'Design' }] };
      return { ok: true };
    };
  ```
  and thread the tool args through the `invoke_mcp_tool` case (current line 276):
  ```ts
          case 'invoke_mcp_tool': return { tool: args?.tool ?? 'unknown', is_error: false, result: mcpResult(args?.tool ?? '', args?.args) };
  ```
  (Order matters: `resolve_approval` before `pending_approval` — both contain the substring `approval`... they do not, but `vox_resolve_approval` does NOT contain `pending_approval`; the explicit ordering above is still the safe shape.)
- [ ] Create `crates/vox-gui/ui/e2e/approvals-interactions.spec.ts`:
  ```ts
  /** Approvals approve/reject interaction flow against the stateful tauriMock. */
  import { test, expect } from '@playwright/test';
  import { installTauriMock } from './lib/tauriMock';
  import { addMockInitScript } from './lib/tauriMockShared';

  test.describe('Approvals interactions', () => {
    test.beforeEach(async ({ page }) => {
      await addMockInitScript(page, installTauriMock, 'approvals');
      await page.goto('/');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await expect(page.getByText('#AP-000001')).toBeVisible();
    });

    test('approve resolves, toasts, and removes the row', async ({ page }) => {
      await page.getByRole('button', { name: 'Approve rm -rf build' }).click();
      await expect(page.getByRole('status').getByText('Approved')).toBeVisible();
      await expect(page.getByText('#AP-000001')).toHaveCount(0);
      const resolveCalls = await page.evaluate(() =>
        (window as any).__TAURI_CALLS__.filter(
          (c: any) => c.cmd === 'invoke_mcp_tool' && String(c.args?.tool ?? '').includes('resolve_approval'),
        ),
      );
      expect(resolveCalls).toHaveLength(1);
      expect(resolveCalls[0].args.args).toMatchObject({ approval_id: 'AP-000001', outcome: 'approved' });
    });

    test('reject resolves, toasts, and removes the row', async ({ page }) => {
      await page.getByRole('button', { name: 'Reject rm -rf build' }).click();
      await expect(page.getByRole('status').getByText('Rejected')).toBeVisible();
      await expect(page.getByText('#AP-000001')).toHaveCount(0);
      const resolveCalls = await page.evaluate(() =>
        (window as any).__TAURI_CALLS__.filter(
          (c: any) => c.cmd === 'invoke_mcp_tool' && String(c.args?.tool ?? '').includes('resolve_approval'),
        ),
      );
      expect(resolveCalls[0].args.args).toMatchObject({ approval_id: 'AP-000001', outcome: 'rejected' });
    });
  });
  ```
  Caveat to verify while running: `voxTransport.invokeMcpTool` may canonicalize/route the call (transport.ts:404-440 consults the action manifest; the mock manifest has `actions: []`, so the raw `invoke_mcp_tool` fallback path is expected). If the recorded `cmd` differs, adjust the `__TAURI_CALLS__` filter to the observed command — the UI assertions (toast + row removal) are the primary contract.
- [ ] Run:
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm exec playwright test approvals-interactions.spec.ts --project=chromium
  ```
  Expected: 2 passed. Also re-run `pnpm exec playwright test screenshots.spec.ts --project=chromium --workers=4` — the approvals screenshot still renders the pending row (mcpResult shape for `pending_approval` unchanged: `{ success, data: { approvals } }`).
- [ ] Commit:
  ```
  git add crates/vox-gui/ui/e2e/approvals-interactions.spec.ts crates/vox-gui/ui/e2e/lib/tauriMock.ts
  git commit -m "test(gui-e2e): approvals approve/reject interaction spec + stateful approvals mock" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 9: Interaction spec — Tasks create → reprioritize → cancel

Spec Phase 3 item 4. Ground truth (`TasksView.tsx`): create = `invoke('hopper_submit', { intent, affinity: [] })` (`:120`) from `TaskComposer` (textarea `aria-label="Add a task"`, `Add` button, Enter submits — `TaskComposer.tsx:22-44`); cancel = `invoke('hopper_cancel', { itemId: String(id) })` (`:125`, button `title="Cancel task"` at `:238`); reprioritize = `invoke('hopper_reprioritize', { itemId, priority })` (`:127-128`) via the row `<select>` (values 2/1/0, `:144-157`). Rows come from `hopper_list` (`transport.ts:797-799`, DTO `{ item_id, intent, priority, state, task_id }`) mapped by `mapHopperTasksToRows` (`tasksHelpers.ts:75-96`; `row.id = dto.item_id`, `state: 'inbox'` → `queued`). The mock has NO `hopper_*` cases today (`list_orchestrator_tasks: []` at tauriMock.ts:307 is a different store).

**Staleness caveat (Phase-2 interaction):** this ground truth was verified against PRE-Phase-2 `TasksView.tsx`. Phase 2 Task 10 rewrites `remove`/`reprioritize` into per-origin handlers (orchestrator rows go to `cancel_orchestrator_task`/`reorder_orchestrator_task`; hopper rows keep `hopper_cancel`/`hopper_reprioritize`) and replaces the priority option-value literals with `TASK_PRIORITY_WIRE` constants. If Phase 2 has landed when this task executes (the spec's rollout order says it has), **re-verify the call shapes and line refs against the landed `TasksView.tsx` before trusting the quotes above** — the spec below stays valid because it creates hopper-origin rows only, but the cited line numbers and literal option values will have drifted. Task 13 owns the post-Phase-2 additions (mark-done, origin-aware actions).

**Files:**
- Edit: `crates/vox-gui/ui/e2e/lib/tauriMock.ts` (stateful hopper cases)
- Create: `crates/vox-gui/ui/e2e/tasks-interactions.spec.ts`

**Steps:**

- [ ] Seed hopper state in `installTauriMock` (next to `__MOCK_APPROVALS__`):
  ```ts
    (window as any).__MOCK_HOPPER__ = [] as any[];
  ```
  and add switch cases (next to `list_orchestrator_tasks`):
  ```ts
          case 'hopper_list':
            return ((window as any).__MOCK_HOPPER__ as any[]).map(t => ({ ...t }));
          case 'hopper_submit': {
            const items = (window as any).__MOCK_HOPPER__ as any[];
            const n = items.length + 1;
            items.push({
              item_id: `hop-${n}`,
              intent: String(args?.intent ?? ''),
              priority: 1,
              state: 'inbox',
              task_id: 9000 + n,
            });
            return { item_id: `hop-${n}` };
          }
          case 'hopper_cancel': {
            const items = (window as any).__MOCK_HOPPER__ as any[];
            (window as any).__MOCK_HOPPER__ = items.filter(t => t.item_id !== String(args?.itemId));
            return null;
          }
          case 'hopper_reprioritize': {
            const items = (window as any).__MOCK_HOPPER__ as any[];
            const hit = items.find(t => t.item_id === String(args?.itemId));
            if (hit) hit.priority = Number(args?.priority ?? 1);
            return null;
          }
  ```
- [ ] Create `crates/vox-gui/ui/e2e/tasks-interactions.spec.ts`:
  ```ts
  /** Tasks (hopper to-do) create -> reprioritize -> cancel against the stateful tauriMock. */
  import { test, expect } from '@playwright/test';
  import { installTauriMock } from './lib/tauriMock';
  import { addMockInitScript } from './lib/tauriMockShared';

  test('create -> reprioritize -> cancel round-trip', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'tasks');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    // Create via the composer (Enter submits; TaskComposer.tsx).
    const composer = page.getByLabel('Add a task');
    await composer.fill('Ship the release notes');
    await composer.press('Enter');
    await expect(page.getByText('Ship the release notes')).toBeVisible();
    expect(
      await page.evaluate(() =>
        (window as any).__TAURI_CALLS__.some(
          (c: any) => c.cmd === 'hopper_submit' && c.args?.intent === 'Ship the release notes',
        ),
      ),
    ).toBe(true);

    // Reprioritize to Urgent via the row's priority select (values: 2/1/0).
    const row = page.locator('tr', { hasText: 'Ship the release notes' });
    const prioritySelect = row.getByRole('combobox').first();
    await prioritySelect.selectOption('2');
    await expect
      .poll(() =>
        page.evaluate(() =>
          (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'hopper_reprioritize').length,
        ),
      )
      .toBe(1);
    await expect(prioritySelect).toHaveValue('2'); // survives the post-action refresh (stateful mock)

    // Cancel removes the row.
    await row.getByTitle('Cancel task').click();
    await expect(page.getByText('Ship the release notes')).toHaveCount(0);
    expect(
      await page.evaluate(() =>
        (window as any).__TAURI_CALLS__.some(
          (c: any) => c.cmd === 'hopper_cancel' && c.args?.itemId === 'hop-1',
        ),
      ),
    ).toBe(true);
  });
  ```
  Selector caveats to verify while running: `DataTable` may render `div` rows rather than `tr` — if `page.locator('tr', ...)` matches nothing, scope with `page.locator('[role="row"]', { hasText: ... })` or fall back to `page.getByRole('combobox').first()` / `page.getByTitle('Cancel task').first()` (the test creates exactly one task, so `.first()` is unambiguous). The reprioritize args key is `itemId` (camelCase) as written at `TasksView.tsx:128`.
- [ ] Run:
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm exec playwright test tasks-interactions.spec.ts --project=chromium
  ```
  Expected: 1 passed. Note: TasksView runs in shared-attention mode (App passes `attention`; `useAttentionInbox` polls `hopper_list`), and every action calls `refresh()` → `attention.refresh()` → immediate re-fetch of the stateful mock, so no poll-interval waits are needed beyond `expect.poll`.
- [ ] Commit:
  ```
  git add crates/vox-gui/ui/e2e/tasks-interactions.spec.ts crates/vox-gui/ui/e2e/lib/tauriMock.ts
  git commit -m "test(gui-e2e): hopper task create/reprioritize/cancel interaction spec + stateful hopper mock" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 10: Interaction spec — Chat submit → stream → persist

Spec Phase 3 item 4. Ground truth:
- Submit (`App.tsx:670-755`): persists the user row via `invoke('chat_append_message', { input: { session_id, role: 'user', content, task_id: null, already_submitted: true } })` — post-Phase-1 Task 1, App.tsx sends `already_submitted: true` on the user persist; that flag is the C2 contract that stops the backend secretary re-submit (re-verify against the landed App.tsx) — then ONE `submit_orchestrator_task` dispatch (`allow_duplicate: false`); mock returns `{ ok: true, task_id: '101' }` (`tauriMock.ts:289`).
- Stream correlation (`lib/chatCorrelation.ts:140-167`): frames arrive on Tauri event `vox://agent-events` (`transport.ts:34`), shape `{ id, timestamp_ms, kind: { type, ... } }`; `task_started {agent_id, task_id}` seeds agent→task; `token_streamed {agent_id, text}` appends to the assistant bubble; `task_completed {task_id}` marks it `done`.
- Persist (`App.tsx:836-859`): each completed assistant bubble persists once via `chat_append_message` with `role: 'assistant'` and `task_id`.
- Composer: textarea `aria-label="Task composer"` (`Loquela.tsx:502`), plain Enter submits (`:456`). Sessions: mock `chat_list_sessions` returns `mock-session-1`.
- Event emission: `window.__TAURI_EMIT__` + listener registration built in Task 5.

**Files:**
- Create: `crates/vox-gui/ui/e2e/chat-interactions.spec.ts`

**Steps:**

- [ ] Create `crates/vox-gui/ui/e2e/chat-interactions.spec.ts`:
  ```ts
  /**
   * Chat submit -> stream -> persist against the tauriMock, driving the
   * `vox://agent-events` stream with the __TAURI_EMIT__ helper (tauriMockShared).
   * Guards two distinct contracts:
   *  - frontend double-dispatch (e.g. duplicate Enter handling): exactly ONE
   *    `submit_orchestrator_task` per submit;
   *  - the C2 `already_submitted` contract: the persisted user row must carry
   *    `already_submitted: true` — that flag is what stops the Rust backend's
   *    secretary re-submit (the re-submit itself happens daemon-side and is
   *    invisible to this mock, so the flag IS the observable C2 guard here).
   */
  import { test, expect } from '@playwright/test';
  import { installTauriMock } from './lib/tauriMock';
  import { addMockInitScript } from './lib/tauriMockShared';

  test('submit streams tokens into the transcript and persists the assistant row', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'chat');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    const composer = page.getByLabel('Task composer');
    await composer.fill('Summarize the repository layout');
    await composer.press('Enter');

    // Optimistic user bubble + exactly one dispatch (guards FRONTEND
    // double-dispatch, e.g. duplicate Enter handling — NOT C2; the C2
    // re-submit is daemon-side and never crosses the Tauri invoke boundary).
    await expect(page.getByText('Summarize the repository layout')).toBeVisible();
    await expect
      .poll(() =>
        page.evaluate(() =>
          (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'submit_orchestrator_task').length,
        ),
      )
      .toBe(1);
    // User row persisted on submit, carrying the C2 contract flag: Phase 1
    // makes App.tsx send already_submitted: true, which is exactly what stops
    // the backend secretary from re-submitting — the only mock-visible C2 guard.
    expect(
      await page.evaluate(() =>
        (window as any).__TAURI_CALLS__.some(
          (c: any) =>
            c.cmd === 'chat_append_message' &&
            c.args?.input?.role === 'user' &&
            c.args?.input?.content === 'Summarize the repository layout' &&
            c.args?.input?.already_submitted === true,
        ),
      ),
    ).toBe(true);

    // Drive the stream for task 101 (mock submit_orchestrator_task returns task_id '101').
    const emit = (kind: Record<string, unknown>, id: number) =>
      page.evaluate(
        ([k, i]) =>
          (window as any).__TAURI_EMIT__('vox://agent-events', {
            id: i,
            timestamp_ms: Date.now(),
            kind: k,
          }),
        [kind, id] as const,
      );
    await emit({ type: 'task_started', agent_id: 7, task_id: 101 }, 1);
    await emit({ type: 'token_streamed', agent_id: 7, text: 'Hello from the mock stream.' }, 2);
    await expect(page.getByText('Hello from the mock stream.')).toBeVisible();
    await emit({ type: 'task_completed', task_id: 101 }, 3);

    // Completed assistant bubble persists exactly once, tagged with the task id.
    await expect
      .poll(() =>
        page.evaluate(() =>
          (window as any).__TAURI_CALLS__.filter(
            (c: any) => c.cmd === 'chat_append_message' && c.args?.input?.role === 'assistant',
          ).length,
        ),
      )
      .toBe(1);
    const persisted = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.find(
        (c: any) => c.cmd === 'chat_append_message' && c.args?.input?.role === 'assistant',
      ),
    );
    expect(persisted.args.input.content).toContain('Hello from the mock stream.');
    expect(String(persisted.args.input.task_id)).toBe('101');
  });
  ```
- [ ] Run:
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm exec playwright test chat-interactions.spec.ts --project=chromium
  ```
  Expected: 1 passed. Debug notes if red: (a) tokens dropped → the `plugin:event|listen` registration isn't recording handlers — confirm Task 5's `eventPluginResponse` is reached (add a temporary `page.evaluate(() => (window as any).__TAURI_EVENT_LISTENERS__)` dump; `vox://agent-events` must have ≥1 handler after load); (b) two `submit_orchestrator_task` calls → a FRONTEND double-dispatch (duplicate Enter/submit handling), while a missing or `false` `already_submitted` on the user persist → the Phase-1 C2 fix regressed in App.tsx — in either case report, don't paper over; (c) no assistant persist → `task_completed` must carry the SAME numeric task_id the mock returned (`'101'`; `chatCorrelation.ts` normalizes via `String()`).
- [ ] Commit:
  ```
  git add crates/vox-gui/ui/e2e/chat-interactions.spec.ts
  git commit -m "test(gui-e2e): chat submit->stream->persist interaction spec over __TAURI_EMIT__ (one-dispatch + already_submitted C2 contract guards)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 11: CI post-merge hardening (fork F2: loud sweep, advisory variants, NO PR gate)

Spec Phase 3 items 1 + 3 (workflow half). All edits inside the `gui-playwright-smoke` job, `.github/workflows/ci.yml:1620-1734`. **Audit result (verified against the current file):** `continue-on-error: true` appears in this job ONLY on the three advisory steps — visual-review capture (line 1677), AI review (line 1686), cache commit (line 1711). The asserting sweep step (lines 1658-1662) **already has no `continue-on-error`** — i.e. the F2 mitigation is already structurally true; this task pins it with an explicit comment (so a future edit can't quietly soften it) and adds the variants step. Do NOT add `continue-on-error` anywhere new except the variants step, do NOT change any `if:`/`needs:`/`runs-on:` lines, and do NOT touch `ci-summary` (`needs: [guards-fast, lints, compiler-gates, tests, audits]`, line 1449).

**Files:**
- `.github/workflows/ci.yml` (job `gui-playwright-smoke` only: comment at ~1653-1657, new step after ~1672, artifact paths at ~1693-1696)

**Steps:**

- [ ] Update the sweep step's comment block (currently lines 1653-1657) — replace with:
  ```yaml
        # Registry-driven visual-audit sweep: screenshots every GUI surface (derived from
        # SURFACE_REGISTRY, so new surfaces are covered automatically) and asserts no error-
        # boundary trip / uncaught error / console error. Also collects every other asserting
        # e2e spec (error-states, interaction specs). Fork F2 (2026-07-16): this job stays
        # post-merge/full-ci only (no PR gate), and THIS step is its loud failure signal —
        # it must NEVER get `continue-on-error`; main-branch breakage fails the job visibly
        # (CI-monitor) instead of passing silently. Variants + AI review below stay advisory.
  ```
  The `- name: GUI visual-audit sweep (Playwright)` step itself (run block, lines 1658-1662) is unchanged.
- [ ] Insert the advisory variants step immediately after the `Upload GUI visual-audit screenshots` step (i.e. after line 1672's `if-no-files-found: ignore`), before `GUI visual-review capture (manifest)`:
  ```yaml
        # Empty/error variant sweep (screenshots-variants.spec.ts is env-gated and
        # self-skips in the asserting sweep above). Advisory per fork F2: failures
        # surface in logs + artifacts but never fail the post-merge job.
        - name: GUI variant states sweep (empty/error, advisory)
          working-directory: crates/vox-gui/ui
          env:
            VOX_VARIANT_SCREENSHOTS: "1"
          run: pnpm exec playwright test screenshots-variants.spec.ts --project=chromium --workers=2
          continue-on-error: true
  ```
  (Indentation: 6 spaces to `- name:`, matching sibling steps. `working-directory` matches the idiom already used by the capture step at line 1675.)
- [ ] Extend the `Upload visual-review report + screens (always)` step's `path:` list (currently lines 1693-1696) so advisory variant failures are inspectable even when the job succeeds:
  ```yaml
            path: |
              contracts/reports/gui-visual-review/
              crates/vox-gui/ui/e2e/screens/manifest.json
              crates/vox-gui/ui/e2e/screens/*-empty.png
              crates/vox-gui/ui/e2e/screens/*-error.png
  ```
- [ ] Guard-rail checks (all must hold before committing):
  ```
  cd C:\Users\Owner\vox
  git diff .github/workflows/ci.yml
  ```
  - The diff touches ONLY the `gui-playwright-smoke` job (between `gui-playwright-smoke:` and `all-features-matrix:`).
  - `git diff .github/workflows/ci.yml | grep "^[+-].*needs:"` → no output (ci-summary needs untouched).
  - Exactly ONE added `continue-on-error: true` (the variants step).
  - No added `pull_request` triggers / no change to the job `if:` at line 1625.
  - YAML parses: `python -c "import yaml,io; yaml.safe_load(io.open('.github/workflows/ci.yml', encoding='utf-8'))"` (or `pnpm exec js-yaml` if python is unavailable) — expect silence.
- [ ] Commit:
  ```
  git add .github/workflows/ci.yml
  git commit -m "ci(gui): pin loud post-merge sweep (F2, no PR gate) + advisory empty/error variants step with artifacts" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Task 12: Whole-phase verification sweep

Run the full local equivalent of the post-merge job plus every suite this plan touched, in one sitting, before declaring Phase 3 done. Scope: Tasks 1-11 only — Task 13 (post-Phase-2 interaction specs) is gated on the Phase 2 series having landed and carries its own verification steps; if it has already been executed, include its three specs in the sweep expectations below.

**Files:** none (verification only).

**Steps:**

- [ ] Frontend static + unit:
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm typecheck
  pnpm test
  ```
  Expected: 0 type errors; full vitest suite green (includes `surfaceRegistryEscape`, `ipcBoundaries`, `surfaceHonesty`, `tauriMockVariants` with the serialization/emit tests).
- [ ] Full asserting Playwright sweep (what the loud CI step runs):
  ```
  pnpm exec playwright test --project=chromium --workers=4
  ```
  Expected: green — including the three Task 8-10 interaction specs and `error-states.spec.ts` (4 passed; tasks/dashboard are pre-declared TODO(phase3-followup) gaps, see Task 7); `screenshots-variants.spec.ts` reports its tests as *skipped* (env gate off).
- [ ] Variant sweep opt-in (what the advisory CI step runs — PowerShell syntax; cmd.exe's `set X=1` does NOT export in PowerShell and every test would silently self-skip):
  ```
  $env:VOX_VARIANT_SCREENSHOTS = '1'
  pnpm exec playwright test screenshots-variants.spec.ts --project=chromium --workers=2
  Remove-Item Env:VOX_VARIANT_SCREENSHOTS
  ```
  Expected: **20 passed, 0 skipped** (any `skipped` > 0 = the env gate never opened; the run verified nothing), or documented advisory failures matching Task 6's commit-body findings list (nothing new/unexplained).
- [ ] Rust (redirect, never pipe; PowerShell syntax):
  ```
  cd C:\Users\Owner\vox
  cargo test -p vox-orchestrator-mcp --features gui-visual-review visus_review > "$env:TEMP\phase3_rust.log" 2>&1
  cargo clippy -p vox-orchestrator-mcp --features gui-visual-review -- -D warnings > "$env:TEMP\phase3_clippy.log" 2>&1
  ```
  Read both logs — expected: all tests pass, zero clippy warnings.
- [ ] Negative test of the registry guard (proves the guard guards — spec "Testing strategy"): temporarily add a bogus routed case to `surfaceComponents.tsx`:
  ```tsx
      case 'not-a-registered-surface':
        return null;
  ```
  run `pnpm exec vitest run src/guards/surfaceRegistryEscape.test.ts` — expected: FAILS naming `not-a-registered-surface`. Revert the temporary case (`git checkout -- src/components/layout/surfaceComponents.tsx` from the ui dir) and re-run — green.
- [ ] Confirm the working tree contains no stray artifacts: `git status` shows only intended commits, no unstaged `e2e/screens/*.png` churn committed (screenshots are gitignored output — verify none were `git add`ed by earlier tasks; if any were, unstage them).
- [ ] Final: `git log --oneline -12` — expect the 11 Phase-3 commits from Tasks 1-11 in order (one per task; Task 13 adds its own commits later, once Phase 2 has landed), each independently revertable, on top of the starting commit.

---

## Task 13: Post-Phase-2 interaction specs (model picker, session rename/archive, task mark-done)

Spec Phase 3 item 4 (remaining three of the five named interaction specs). **Sequencing gate:** this task executes only AFTER the Phase 2 series (`2026-07-16-axis-gui-remediation-phase2-wiring.md`) has landed — it drives UI that Phase 2 creates (`ChatModelPicker`, the session-rail kebab menu, the Tasks mark-done button) and relies on the tauriMock command cases the Phase 2 plan (after its own review fixes) adds for `set_active_model`, `chat_rename_session`, `chat_archive_session`, and `hopper_mark_done`. Per the spec's rollout order (bugs → wiring → tests/CI) Phase 2 has landed by the time this plan runs, so execute Task 13 in normal order; if Phase 2 has NOT landed, skip this task, leave its checkboxes unticked, and revisit — do not write specs against wiring that does not exist. Tasks 1-12 have no dependency on this task.

Ground truth is split between landed code and Phase-2-plan artifacts:
- **Verified against current code:** the session rail has `data-testid="chat-session-rail"` and renders sessions as `role="tab"` with the title (`chat-session-rail.spec.ts:19-22`); the tauriMock's `chat_list_sessions` returns one session `mock-session-1` / "Mock chat" (`tauriMock.ts:224`); `get_active_model` returns `'opus-4-8'` and `list_model_cards` returns ids including `'sonnet-4-6'` (`tauriMock.ts:28-43,131-132`); Task 9's hopper mock provides `__MOCK_HOPPER__` + `hopper_submit`/`hopper_list`.
- **Keyed to the Phase 2 plan's stated roles/testids (artifacts that do not exist pre-P2 — re-verify each against the LANDED code in Step 0):** `ChatModelPicker` trigger button accessible-name `model: <activeModel>`, dropdown `role="listbox"` `aria-label="Pick active model"`, entries `role="option"` named by model id, apply = `invoke('set_active_model', { modelId })` (phase2 plan Task 7 Steps 5-6); session-rail kebab `aria-label="Session actions for <title>"`, `role="menuitem"` Rename/Archive, rename input `aria-label="New session title"` with Enter-commit, handlers `invoke('chat_rename_session', { sessionId, title })` / `invoke('chat_archive_session', { sessionId })` (phase2 plan Task 11); Tasks mark-done button `title="Mark done"` on hopper-origin non-completed rows calling `hopperMarkDone(String(r.id))` → `invoke('hopper_mark_done', { itemId })`, done rows grouped under 'Completed' (phase2 plan Tasks 9-10). **Caveat:** phase2's adversarial-review fix for its F4 finding may re-route the picker's apply so the pick also (or instead) threads a `model_override` into the chat submit payload — Step 0 must pin the landed IPC contract before the assertion is trusted.

**Files:**
- Edit: `crates/vox-gui/ui/e2e/lib/tauriMock.ts` (stateful sessions; verify/extend the Phase-2-added `hopper_mark_done` + session cases)
- Create: `crates/vox-gui/ui/e2e/model-picker-interactions.spec.ts`
- Create: `crates/vox-gui/ui/e2e/session-rail-actions.spec.ts`
- Edit: `crates/vox-gui/ui/e2e/tasks-interactions.spec.ts` (append the mark-done test)

**Steps:**

- [ ] **Step 0 — re-verify the Phase-2 landed surface** (the plan-stated shapes above may have drifted in review):
  ```
  git grep -n "Pick active model\|Session actions for\|New session title\|Mark done" crates/vox-gui/ui/src
  git grep -n "set_active_model\|chat_rename_session\|chat_archive_session\|hopper_mark_done" crates/vox-gui/ui/src crates/vox-gui/ui/e2e/lib
  ```
  Expected: hits in `ChatModelPicker.tsx`, `ChatSessionRail.tsx`, `ChatSurface.tsx`, `TasksView.tsx`, and `e2e/lib/tauriMock.ts` (the Phase-2-added mock cases). If any selector/command differs from the ground truth above, use the landed form in the specs below. Also pin the picker's apply contract: read the landed `ChatModelPicker.tsx` — if the pick threads `model_override` into the chat submit instead of (or in addition to) calling `set_active_model`, assert THAT payload in Step 2. Re-verify TasksView's per-origin action shapes (Task 9 caveat): hopper rows must still call `hopper_cancel`/`hopper_reprioritize`.
- [ ] **Step 1 — make the sessions mock stateful** (skip any part Phase 2 already made stateful). In `installTauriMock`, seed next to `__MOCK_APPROVALS__`/`__MOCK_HOPPER__`:
  ```ts
    (window as any).__MOCK_SESSIONS__ = [
      { session_id: 'mock-session-1', title: 'Mock chat', updated_at: 'now', message_count: 2, conversation_id: 1 },
    ];
  ```
  and replace/add the switch cases (Tauri camelCase arg mapping — the frontend sends `{ sessionId, title }`):
  ```ts
          case 'chat_list_sessions':
            return ((window as any).__MOCK_SESSIONS__ as any[]).map(s => ({ ...s }));
          case 'chat_rename_session': {
            const hit = ((window as any).__MOCK_SESSIONS__ as any[]).find(
              s => s.session_id === String(args?.sessionId),
            );
            if (hit) hit.title = String(args?.title ?? hit.title);
            return null;
          }
          case 'chat_archive_session': {
            (window as any).__MOCK_SESSIONS__ = ((window as any).__MOCK_SESSIONS__ as any[]).filter(
              s => s.session_id !== String(args?.sessionId),
            );
            return null;
          }
  ```
  and verify the Phase-2-added `hopper_mark_done` case is stateful against `__MOCK_HOPPER__`; if it is missing or stateless, use:
  ```ts
          case 'hopper_mark_done': {
            const items = (window as any).__MOCK_HOPPER__ as any[];
            const hit = items.find(t => t.item_id === String(args?.itemId));
            if (hit) hit.state = 'done';
            return hit ? { ...hit } : null;
          }
  ```
  (Task 9's `hopper_list` case already returns everything in `__MOCK_HOPPER__`, so done rows keep flowing — matching Phase 2's "hopper_list includes terminal done items" read.)
- [ ] **Step 2 — create `crates/vox-gui/ui/e2e/model-picker-interactions.spec.ts`:**
  ```ts
  /**
   * Chat model picker apply flow (Phase 2 wiring) against the stateful tauriMock.
   * Selectors keyed to the Phase 2 plan's stated roles (re-verified in Step 0).
   */
  import { test, expect } from '@playwright/test';
  import { installTauriMock } from './lib/tauriMock';
  import { addMockInitScript } from './lib/tauriMockShared';

  test('picking a model applies it and updates the trigger label', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'chat');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    // Trigger renders the active model (mock get_active_model = 'opus-4-8').
    await page.getByRole('button', { name: /model: opus-4-8/i }).click();
    await expect(page.getByRole('listbox', { name: 'Pick active model' })).toBeVisible();
    await page.getByRole('option', { name: 'sonnet-4-6' }).click();

    // Outgoing IPC contract. If Step 0 found the landed picker threads
    // model_override into the submit payload instead, assert that payload here.
    await expect
      .poll(
        () =>
          page.evaluate(() =>
            (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'set_active_model').length,
          ),
        { timeout: 10_000 },
      )
      .toBeGreaterThan(0);
    const call = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.find((c: any) => c.cmd === 'set_active_model'),
    );
    expect(call.args).toMatchObject({ modelId: 'sonnet-4-6' });

    // Product-rendered result: onApplied updates the trigger label.
    await expect(page.getByRole('button', { name: /model: sonnet-4-6/i })).toBeVisible();
  });
  ```
- [ ] **Step 3 — create `crates/vox-gui/ui/e2e/session-rail-actions.spec.ts`:**
  ```ts
  /**
   * Session rail rename/archive flows (Phase 2 wiring) against the stateful
   * tauriMock: outgoing IPC contract + product-rendered rail state after the
   * handler's loadSessions() refetch of the stateful mock.
   */
  import { test, expect } from '@playwright/test';
  import { installTauriMock } from './lib/tauriMock';
  import { addMockInitScript } from './lib/tauriMockShared';

  test.describe('Session rail actions', () => {
    test.beforeEach(async ({ page }) => {
      await addMockInitScript(page, installTauriMock, 'chat');
      await page.setViewportSize({ width: 1400, height: 900 });
      await page.goto('/');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await expect(page.getByTestId('chat-session-rail')).toBeVisible();
      await expect(page.getByRole('tab', { name: /Mock chat/i })).toBeVisible();
    });

    test('rename flows through chat_rename_session and re-renders the new title', async ({ page }) => {
      await page.getByRole('button', { name: 'Session actions for Mock chat' }).click();
      await page.getByRole('menuitem', { name: /rename/i }).click();
      const input = page.getByRole('textbox', { name: /new session title/i });
      await input.fill('Renamed chat');
      await input.press('Enter');

      await expect
        .poll(
          () =>
            page.evaluate(() =>
              (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'chat_rename_session').length,
            ),
          { timeout: 10_000 },
        )
        .toBe(1);
      const call = await page.evaluate(() =>
        (window as any).__TAURI_CALLS__.find((c: any) => c.cmd === 'chat_rename_session'),
      );
      expect(call.args).toMatchObject({ sessionId: 'mock-session-1', title: 'Renamed chat' });
      await expect(page.getByRole('tab', { name: /Renamed chat/i })).toBeVisible();
    });

    test('archive flows through chat_archive_session and removes the session tab', async ({ page }) => {
      await page.getByRole('button', { name: 'Session actions for Mock chat' }).click();
      await page.getByRole('menuitem', { name: /archive/i }).click();

      await expect
        .poll(
          () =>
            page.evaluate(() =>
              (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'chat_archive_session').length,
            ),
          { timeout: 10_000 },
        )
        .toBe(1);
      const call = await page.evaluate(() =>
        (window as any).__TAURI_CALLS__.find((c: any) => c.cmd === 'chat_archive_session'),
      );
      expect(call.args).toMatchObject({ sessionId: 'mock-session-1' });
      await expect(page.getByRole('tab', { name: /Mock chat/i })).toHaveCount(0);
    });
  });
  ```
- [ ] **Step 4 — append the mark-done test to `crates/vox-gui/ui/e2e/tasks-interactions.spec.ts`:**
  ```ts
  test('create -> mark done calls hopper_mark_done and retires the affordance', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'tasks');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    const composer = page.getByLabel('Add a task');
    await composer.fill('Write the changelog');
    await composer.press('Enter');
    await expect(page.getByText('Write the changelog')).toBeVisible();

    const row = page.locator('tr', { hasText: 'Write the changelog' });
    await row.getByTitle('Mark done').click();
    await expect
      .poll(
        () =>
          page.evaluate(() =>
            (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'hopper_mark_done').length,
          ),
        { timeout: 10_000 },
      )
      .toBe(1);
    const call = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.find((c: any) => c.cmd === 'hopper_mark_done'),
    );
    expect(call.args).toMatchObject({ itemId: 'hop-1' });

    // Done rows stay listed (Phase 2: hopper_list includes terminal done items)
    // but lose the mark-done affordance (hopper-origin + not-completed guard).
    await expect(page.getByText('Write the changelog')).toBeVisible();
    await expect(row.getByTitle('Mark done')).toHaveCount(0);
  });
  ```
  Same selector caveats as the existing test in this file (`tr` vs `[role="row"]`); additionally re-verify that the landed `mapHopperTasksToRows` maps `state: 'done'` → `lifecycle: 'completed'` (Phase 2 Task 10 groups such rows under 'Completed') — if done rows are filtered out of the view instead, replace the last two assertions with `await expect(page.getByText('Write the changelog')).toHaveCount(0);` and note the landed behavior in the commit body.
- [ ] **Step 5 — run everything this task touched:**
  ```
  cd C:\Users\Owner\vox\crates\vox-gui\ui
  pnpm exec playwright test model-picker-interactions.spec.ts session-rail-actions.spec.ts tasks-interactions.spec.ts --project=chromium
  ```
  Expected: 4 passed (1 picker + 2 rail + the extended tasks spec now counts 2). Then re-run the sweep the mock edits could affect: `pnpm exec playwright test screenshots.spec.ts chat-interactions.spec.ts --project=chromium --workers=4` — green (the chat screenshot still renders 'Mock chat'; the sessions/hopper cases changed shape only from literal to stateful).
- [ ] **Step 6 — commit:**
  ```
  git add crates/vox-gui/ui/e2e
  git commit -m "test(gui-e2e): post-Phase-2 interaction specs - model picker apply, session rename/archive, hopper mark-done" -m "Completes spec Phase 3 item 4's five interaction flows; stateful session/hopper mock cases." -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
  ```

---

## Out of scope (explicitly deferred, do not implement here)

- **Model picker apply / session rename/archive / tasks mark-done interaction specs are NOT deferred out of this plan** — they are owned by **Task 13 above**, explicitly gated on the Phase 2 series having landed. (Phase 2 adds the wiring and the tauriMock command cases but contains no e2e interaction specs of its own; pointing the deferral at "the Phase 2 series" would orphan these specs between the two plans.)
- **Workbench doc-tab open/close spec**: already covered by `e2e/workbench-tabs.spec.ts` — skip (verified: 10 tests including doc-tab persistence).
- **PR gating of any GUI job**: fork F2 resolved as post-merge only. Nothing in this plan may add `gui-playwright-smoke` (or any new job) to `ci-summary.needs` or change PR triggers.
- **Unstaging `contracts/reports/gui-visual-review/0000-00-00.json`** and the `--date` default fix: Phase 1 item 7.
- **`list_mc_approvals` backend command removal** (orphaned by Task 3): cleanup candidate, needs its own small change.
