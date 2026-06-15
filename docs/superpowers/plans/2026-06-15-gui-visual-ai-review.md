# GUI Visual AI Adversarial Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dynamically screenshot every vox-gui surface, content-hash each shot, and run an AI adversarial design-principles review **only when a surface's appearance actually changes** — non-gating in CI, performance-budgeted with spike detection, using the existing OpenRouter vision pipeline with a config-driven (not hard-pinned) vision model.

**Architecture:** Three layers, each reusing what already exists. (1) **Capture** extends the existing Playwright sweep (`e2e/screenshots.spec.ts`, driven by the `SURFACE_REGISTRY` SSOT + Tauri-invoke mock) to capture high-DPI PNGs and emit a `manifest.json` with per-surface sha256 + timing. (2) **Change-detection** diffs each surface's sha256 against a committed `cache.v1.json`; unchanged → reuse cached verdict (no AI call), changed/new → warn + queue. (3) **Review** runs a Rust reviewer (in `vox-orchestrator-mcp`, which already owns the `image → base64 image_url → OpenAI-compat → OpenRouter` transport) that critiques only queued surfaces against the front-end design principles, writes a versioned report + a JSONL ledger (trend/spike), and updates the cache. Everything is advisory: the reviewer always exits 0; CI runs it in the already-non-gating `gui-playwright-smoke` job with `continue-on-error: true`.

**Tech Stack:** Playwright (`@playwright/test`), Node crypto (sha256), Rust (`vox-orchestrator-mcp` llm_bridge + `vox-openai` multimodal wire types + `vox-config`/`vox-secrets` for OpenRouter), `serde_json`, the `vox-audit` versioned-report convention.

---

## Design decisions & rationale (read first)

| Decision | Choice | Why |
|---|---|---|
| **Auto-discovery** | Reuse `src/generated/surfaceRegistry.generated.ts` (`SURFACE_REGISTRY`), filter `viewKey != null && tier !== 'none'`. | Already the SSOT; the existing sweep uses it; new surfaces are auto-covered when the registry regenerates. **Do not build a second surface list.** |
| **Rendering** | Reuse the existing Tauri-invoke mock in `e2e/screenshots.spec.ts` (extract it to `e2e/lib/tauriMock.ts`). | Surfaces hard-depend on `window.__TAURI_INTERNALS__.invoke`; they crash in a bare browser. The mock is the only way to render them. |
| **Cache key** | `sha256(PNG bytes)` per surface. Unchanged sha → skip AI. | "Don't re-review when appearance hasn't changed." Content hash = appearance fingerprint. |
| **Change ≠ failure** | A changed sha emits a **warning** and triggers re-review; it never fails the build. No pixel-diff gate. | "We don't want regression tests to auto-gate when appearance changes — but warn and re-run AI review." |
| **Non-gating** | Reviewer always `exit 0`; CI step `continue-on-error: true`; lives in the post-merge/`full-ci` `gui-playwright-smoke` job, never the required Rust gate. | "We don't want to gate CICD by it." Audit confirmed this job is already advisory. |
| **Vision transport** | Reuse `vox-orchestrator-mcp` llm_bridge path (`vox_openai::ChatMessageContent::Parts` + `ImageUrl{ url: data:image/png;base64,... }` → OpenAI-compat adapter → OpenRouter). | Audit confirmed base64 images already flow to OpenRouter today. Zero new wire code. |
| **Model** | **Config-driven preference list**, not hard-pinned. Default `google/gemini-3-flash-preview`; fallbacks `google/gemini-2.5-flash`, `anthropic/claude-opus-4.8`. Selected via registry filtered on `capabilities.supports_vision`. | Web research: Gemini native-multimodal family leads UI-screenshot critique; Flash is cheap+strong; Opus for escalation. "We may want to learn and test over different AIs" → config + per-surface model recorded in cache for later A/B. |
| **Image fidelity** | `deviceScaleFactor: 2`, full-page PNG, viewport 1440×900 (→ 2880px physical), longest-edge cap 2880px. | "Big enough and detailed enough to be adversarially reviewed." OpenRouter accepts png/base64 with no hard size limit; tokens scale with resolution, so cap to control cost. |
| **Cost/time control** | Only review changed/new surfaces; concurrency cap; per-run wall-clock budget; defer overflow (record `deferred`, never block). | "Aggressive time… don't slow CICD." Typical PR touches 1–2 surfaces → 1–2 vision calls. |
| **Spike detection** | Append `{ts, surfaces_reviewed, total_ms, cost}` to `ledger.jsonl`; compare this run vs trailing median; `spiked: true` if > 1.5× median or over absolute budget. Warn only. | "Tracking overall the difference in time… noticing spikes." Mirrors the existing CR-P2 JSONL-append-then-rollup pattern. |
| **The "rule"** | Advisory `vox ci gui-visual-review` wrapper (always exit 0) + a documented convention doc. | "Consider what you can add as a rule and how." Non-failing CI check + SSOT doc. |
| **Cache provenance** | `cache.v1.json` + latest report committed back on `main` post-merge via the existing `SSOT_AUTOREGEN_TOKEN` bot. | Cross-run cache needs to persist in-repo. Reuses the established autoregen pattern (memory: ssot-autoregen PR bot). |

**Front-end design principles rubric (SSOT):** `docs/src/architecture/gui-frontend-design-principles-2026-06-14.md` (the 360-principle catalog) + the 24-item per-surface checklist in `docs/superpowers/specs/2026-06-14-vox-gui-design-principles-application-design.md`. The review prompt embeds a distilled rubric (Task 9).

**Verified existing APIs this plan calls (from the audit):**
- `SURFACE_REGISTRY: SurfaceRegistryEntry[]` — `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`.
- Tauri mock + capture reference — `crates/vox-gui/ui/e2e/screenshots.spec.ts:47-307` (mock), `:320,348,365` (captures).
- Multimodal wire types — `crates/vox-openai/src/chat_completion.rs:86-103` (`ChatMessageContent`, `ChatMessagePart::ImageUrl`, `ImageUrl`).
- Image→base64→OpenRouter reference — `crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs:468-496`.
- OpenAI-compat transport — `crates/vox-orchestrator-mcp/src/llm_bridge/providers/openai.rs:33-55` (`http_openai_compatible_with_headers`).
- OpenRouter URL + key — `vox_config::openrouter_chat_completions_url()` (`crates/vox-config/src/inference.rs:117`), `vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey)`.
- Vision capability flag — `crates/vox-orchestrator/src/models/spec.rs:38-85` (`ModelCapabilities::supports_vision`), registry filter `best_for_with_filter(..., pred)` in `models/registry.rs`.
- Report convention — `crates/vox-audit/src/bin/cr-e1.rs` (percentile-vs-budget), `cr-p2.rs` (JSONL append → rollup).
- CI job — `.github/workflows/ci.yml:1088-1161` (`gui-playwright-smoke`, advisory).

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `crates/vox-gui/ui/e2e/lib/tauriMock.ts` | **Create** | Extracted Tauri-invoke mock (from `screenshots.spec.ts`) so capture specs reuse one mock. |
| `crates/vox-gui/ui/e2e/lib/screenshotManifest.ts` | **Create** | Pure helpers: `sha256Png(buf)`, `ManifestEntry`/`Manifest` types, `writeManifest(dir, entries)`. |
| `crates/vox-gui/ui/e2e/lib/screenshotManifest.test.ts` | **Create** | Vitest unit tests for the hashing/manifest helpers. |
| `crates/vox-gui/ui/e2e/visual-review.spec.ts` | **Create** | Playwright sweep: for every registry surface, capture hi-DPI PNG + timing, emit `e2e/screens/manifest.json`. |
| `crates/vox-gui/ui/vitest.config.ts` | **Modify** | Ensure `e2e/lib/*.test.ts` is in the vitest include (unit-testable pure helpers). |
| `contracts/orchestration/visual-review.config.v1.json` | **Create** | Pluggable model preference list + budgets (not hard-pinned). |
| `contracts/reports/gui-visual-review/cache.v1.json` | **Create** | Committed cache: `viewKey → {screenshot_sha256, score, verdict, model, reviewed_at}`. |
| `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` | **Create** | Reviewer library: load manifest+cache, diff, call vision model, parse verdict, write report+ledger, update cache. |
| `crates/vox-orchestrator-mcp/src/visus_review/prompt.rs` | **Create** | The adversarial review system prompt embedding the design-principles rubric + the strict JSON output schema. |
| `crates/vox-orchestrator-mcp/src/visus_review/types.rs` | **Create** | `ReviewVerdict`, `Finding`, `SurfaceReport`, `RunReport`, `CacheIndex`, `VisualReviewConfig` serde types. |
| `crates/vox-orchestrator-mcp/src/visus_review/vision_call.rs` | **Create** | `call_vision_model(model, system, user, png_bytes) -> Result<(String, Usage)>` — builds base64 `ImageUrl` part, POSTs to OpenRouter, returns content+usage. |
| `crates/vox-orchestrator-mcp/src/visus_review/model_select.rs` | **Create** | Vision-aware model resolver: config list ∩ registry `supports_vision`, with fallback. |
| `crates/vox-orchestrator-mcp/src/visus_review/spike.rs` | **Create** | Ledger append + trailing-median spike detection. |
| `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs` | **Create** | Thin CLI: parse args, call `visus_review::run`, always exit 0. |
| `crates/vox-orchestrator-mcp/src/lib.rs` | **Modify** | `pub mod visus_review;`. |
| `crates/vox-cli/src/commands/ci/...` | **Modify** | Add advisory `vox ci gui-visual-review` subcommand (wraps the binary, exit 0). |
| `.github/workflows/ci.yml` | **Modify** | In `gui-playwright-smoke`: emit manifest, run reviewer (`continue-on-error`), upload report always; post-merge commit cache via bot. |
| `docs/src/architecture/gui-visual-ai-review.md` | **Create** | SSOT doc: the rule, the cache/warn-not-gate model, model config, perf budget. |

**Phasing (each phase = working software):**
- **Phase 1 (Tasks 1–4):** Capture + hashing + manifest (TS only, no AI). Delivers per-surface appearance fingerprints + timing.
- **Phase 2 (Tasks 5–7):** Reviewer skeleton + cache diff + change warnings + report writer (Rust, **no AI call yet** — `status: new|changed|cached` only). Delivers change-detection + reports.
- **Phase 3 (Tasks 8–11):** Vision call + model selection + prompt + verdict parsing. Delivers actual AI review of changed surfaces.
- **Phase 4 (Tasks 12–15):** Spike detection + CI wiring + advisory `vox ci` rule + docs.

---

## Task 1: Extract the Tauri-invoke mock into a reusable helper

**Files:**
- Create: `crates/vox-gui/ui/e2e/lib/tauriMock.ts`
- Reference (do not break): `crates/vox-gui/ui/e2e/screenshots.spec.ts:47-307`

- [ ] **Step 1: Read the existing mock**

Run: open `crates/vox-gui/ui/e2e/screenshots.spec.ts` and copy the `installMock` function (the `~60-command switch` that sets `window.__TAURI_INTERNALS__ = { invoke: async (cmd, args) => {...} }`) verbatim.

- [ ] **Step 2: Create the extracted helper**

Create `crates/vox-gui/ui/e2e/lib/tauriMock.ts`:

```ts
// Reusable Tauri-invoke mock for Playwright captures. Surfaces hard-depend on
// window.__TAURI_INTERNALS__.invoke and crash in a bare browser, so every capture
// must inject this before navigation via page.addInitScript(installTauriMock, viewKey).
//
// This is the single source of truth for the mock — extracted verbatim from the
// original screenshots.spec.ts installMock so both specs share one implementation.

/** Installs a fake Tauri bridge that returns representative data for ~60 commands. */
export function installTauriMock(viewKey: string): void {
  // PASTE the body of installMock from screenshots.spec.ts here verbatim,
  // using `viewKey` as the forced-surface argument (it overrides
  // localStorage['vox_active_view'] and the get_initial_view command result).
  // Keep the exact command switch and return values — do not trim commands.
}
```

> The body is mechanical copy from the existing spec. Keep every command case; trimming any will crash that surface during capture.

- [ ] **Step 3: Repoint the original spec at the helper**

Modify `crates/vox-gui/ui/e2e/screenshots.spec.ts`: delete the inline `installMock`, add `import { installTauriMock } from './lib/tauriMock';`, and replace `page.addInitScript(installMock, view)` with `page.addInitScript(installTauriMock, view)`.

- [ ] **Step 4: Verify the original sweep still works**

Run (from `crates/vox-gui/ui`, with the dev server discoverable per the existing config):
```
pnpm exec playwright test --config playwright.config.ts -g screenshots --project=chromium --workers=4
```
Expected: same pass/fail as before the refactor (no new failures); `e2e/screens/<view>.png` still produced.

- [ ] **Step 5: Commit**

```
git add crates/vox-gui/ui/e2e/lib/tauriMock.ts crates/vox-gui/ui/e2e/screenshots.spec.ts
git commit -m "refactor(vox-gui/e2e): extract reusable Tauri-invoke mock to e2e/lib/tauriMock.ts"
```

---

## Task 2: Pure manifest + hashing helpers (TDD)

**Files:**
- Create: `crates/vox-gui/ui/e2e/lib/screenshotManifest.ts`
- Test: `crates/vox-gui/ui/e2e/lib/screenshotManifest.test.ts`
- Modify: `crates/vox-gui/ui/vitest.config.ts` (include `e2e/lib/**/*.test.ts`)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/e2e/lib/screenshotManifest.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { sha256Png, buildManifest, type ManifestEntry } from './screenshotManifest';

describe('screenshotManifest', () => {
  it('sha256Png is deterministic and 64 hex chars', () => {
    const buf = Buffer.from('fake-png-bytes');
    const a = sha256Png(buf);
    const b = sha256Png(Buffer.from('fake-png-bytes'));
    expect(a).toBe(b);
    expect(a).toMatch(/^[0-9a-f]{64}$/);
  });

  it('different bytes produce different hashes', () => {
    expect(sha256Png(Buffer.from('a'))).not.toBe(sha256Png(Buffer.from('b')));
  });

  it('buildManifest sorts entries by viewKey and stamps schema_version', () => {
    const entries: ManifestEntry[] = [
      { viewKey: 'settings', file: 'settings.png', sha256: 'x'.repeat(64), bytes: 10, width: 2880, height: 1800, captureMs: 120 },
      { viewKey: 'agents', file: 'agents.png', sha256: 'y'.repeat(64), bytes: 20, width: 2880, height: 1800, captureMs: 90 },
    ];
    const m = buildManifest(entries, 42);
    expect(m.schema_version).toBe(1);
    expect(m.total_capture_ms).toBe(42);
    expect(m.surfaces.map((s) => s.viewKey)).toEqual(['agents', 'settings']);
  });
});
```

- [ ] **Step 2: Run it (fails — module missing)**

Run: `pnpm vitest run e2e/lib/screenshotManifest.test.ts`
Expected: FAIL — `Cannot find module './screenshotManifest'`.

- [ ] **Step 3: Implement the helper**

Create `crates/vox-gui/ui/e2e/lib/screenshotManifest.ts`:

```ts
import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';

export interface ManifestEntry {
  viewKey: string;
  file: string;        // relative filename, e.g. "agents.png"
  sha256: string;      // sha256 of the PNG bytes — the appearance fingerprint
  bytes: number;
  width: number;
  height: number;
  captureMs: number;   // wall-clock ms to navigate + capture this surface
}

export interface Manifest {
  schema_version: 1;
  generated_at_unix_ms: number | null; // stamped by the writer (passed in, not Date.now() at import)
  total_capture_ms: number;
  surfaces: ManifestEntry[];
}

/** sha256 hex of raw PNG bytes — the per-surface appearance fingerprint. */
export function sha256Png(buf: Buffer): string {
  return createHash('sha256').update(buf).digest('hex');
}

/** Sort entries by viewKey for stable diffs; stamp schema + total timing. */
export function buildManifest(entries: ManifestEntry[], totalCaptureMs: number, nowMs: number | null = null): Manifest {
  const surfaces = [...entries].sort((a, b) => a.viewKey.localeCompare(b.viewKey));
  return { schema_version: 1, generated_at_unix_ms: nowMs, total_capture_ms: totalCaptureMs, surfaces };
}

/** Write manifest.json into the screenshot output dir. */
export function writeManifest(dir: string, manifest: Manifest): string {
  const path = join(dir, 'manifest.json');
  writeFileSync(path, JSON.stringify(manifest, null, 2) + '\n', 'utf8');
  return path;
}
```

- [ ] **Step 4: Run tests (pass)**

Run: `pnpm vitest run e2e/lib/screenshotManifest.test.ts`
Expected: PASS (3 tests). If vitest does not pick up the file, edit `vitest.config.ts` `test.include` to add `'e2e/lib/**/*.test.ts'` and re-run.

- [ ] **Step 5: Commit**

```
git add crates/vox-gui/ui/e2e/lib/screenshotManifest.ts crates/vox-gui/ui/e2e/lib/screenshotManifest.test.ts crates/vox-gui/ui/vitest.config.ts
git commit -m "feat(vox-gui/e2e): add sha256 screenshot-manifest helpers (TDD)"
```

---

## Task 3: The hi-DPI capture sweep that emits the manifest

**Files:**
- Create: `crates/vox-gui/ui/e2e/visual-review.spec.ts`

- [ ] **Step 1: Write the sweep**

Create `crates/vox-gui/ui/e2e/visual-review.spec.ts`:

```ts
import { test, expect } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { installTauriMock } from './lib/tauriMock';
import { sha256Png, buildManifest, writeManifest, type ManifestEntry } from './lib/screenshotManifest';
import { SURFACE_REGISTRY } from '../src/generated/surfaceRegistry.generated';

const OUT_DIR = join(__dirname, 'screens');
const VIEWS = Array.from(
  new Set(
    SURFACE_REGISTRY
      .filter((e) => e.viewKey != null && e.tier !== 'none')
      .map((e) => e.viewKey as string),
  ),
).sort();

// Capture at 2x for review fidelity; 1440x900 logical -> 2880x1800 physical.
test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

test('visual-review: capture every surface + emit manifest', async ({ page }, testInfo) => {
  mkdirSync(OUT_DIR, { recursive: true });
  const entries: ManifestEntry[] = [];
  const sweepStart = Date.now();

  for (const view of VIEWS) {
    await page.addInitScript(installTauriMock, view);
    const t0 = Date.now();
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });
    // Settle async surface fetches without an arbitrary long sleep.
    await page.waitForLoadState('networkidle').catch(() => {});
    const file = `${view}.png`;
    await page.screenshot({ path: join(OUT_DIR, file), fullPage: true });
    const captureMs = Date.now() - t0;
    const buf = readFileSync(join(OUT_DIR, file));
    const { width, height } = await page.evaluate(() => ({ width: document.documentElement.scrollWidth, height: document.documentElement.scrollHeight }));
    entries.push({ viewKey: view, file, sha256: sha256Png(buf), bytes: buf.length, width, height, captureMs });
    // Reset init scripts between surfaces so the next mock's viewKey wins.
    await page.context().clearCookies();
  }

  const manifest = buildManifest(entries, Date.now() - sweepStart, sweepStart);
  const manifestPath = writeManifest(OUT_DIR, manifest);
  testInfo.attachments.push({ name: 'manifest.json', path: manifestPath, contentType: 'application/json' });

  // The sweep itself is advisory: it must not fail the build on a slow/odd surface.
  // We only assert the manifest is non-empty so a totally broken sweep is visible.
  expect(manifest.surfaces.length).toBeGreaterThan(0);
});
```

> Note on `addInitScript` accumulation: Playwright keeps init scripts across `goto` within a context. If repeated mocks conflict, switch to a fresh context per surface (`browser.newContext()` in a loop) — keep the simpler shared-page form unless Task 4 reveals cross-surface bleed.

- [ ] **Step 2: Run the sweep**

Run (dev server per existing config; from `crates/vox-gui/ui`):
```
pnpm exec playwright test --config playwright.config.ts visual-review --project=chromium --workers=1
```
Expected: PASS; `e2e/screens/manifest.json` exists with one entry per surface (sha256, bytes, captureMs). Inspect: `node -e "console.log(require('./e2e/screens/manifest.json').surfaces.length)"` prints the surface count.

- [ ] **Step 3: Commit**

```
git add crates/vox-gui/ui/e2e/visual-review.spec.ts
git commit -m "feat(vox-gui/e2e): hi-DPI capture sweep emitting sha256 manifest.json"
```

---

## Task 4: Verify capture isolation + timing sanity

**Files:** none (verification + a fix only if needed).

- [ ] **Step 1: Confirm each surface renders distinctly**

Run the sweep, then check that distinct surfaces have distinct hashes (a bleed bug shows up as many identical hashes):
```
node -e "const m=require('./e2e/screens/manifest.json'); const h=new Set(m.surfaces.map(s=>s.sha256)); console.log('surfaces',m.surfaces.length,'unique',h.size)"
```
Expected: `unique` is close to `surfaces` (a few legitimately-identical empty states are fine; if `unique` << `surfaces`, init-script bleed is happening).

- [ ] **Step 2: If bleed is detected, switch to per-surface contexts**

Modify `visual-review.spec.ts` to create a fresh context per surface:
```ts
test('visual-review: capture every surface + emit manifest', async ({ browser }, testInfo) => {
  // ...as above, but inside the loop:
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  const page = await context.newPage();
  await page.addInitScript(installTauriMock, view);
  // ...goto, capture...
  await context.close();
});
```
Re-run Step 1; `unique` should now track `surfaces`.

- [ ] **Step 3: Record the capture-time baseline**

Run the sweep and note `total_capture_ms`. This is the Phase-1 timing baseline the spike detector (Task 12) compares against. No code change.

- [ ] **Step 4: Commit (only if Step 2 changed code)**

```
git add crates/vox-gui/ui/e2e/visual-review.spec.ts
git commit -m "fix(vox-gui/e2e): per-surface browser context to prevent capture bleed"
```

---

## Task 5: Reviewer types + config (Rust, TDD)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/visus_review/types.rs`
- Create: `contracts/orchestration/visual-review.config.v1.json`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs` (add `pub mod visus_review;`)
- Create: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (declares submodules)

> Crate choice: the reviewer lives in `vox-orchestrator-mcp` because that crate already owns the working `image → OpenRouter` transport and can call its internal llm_bridge functions. Before writing, run `cargo run -p vox-arch-check` after Task 7 to confirm no layering violation; if it flags the new binary, the fallback is a dedicated leaf crate `vox-gui-visus-review` that depends on `vox-orchestrator-mcp` — but start in-crate.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-orchestrator-mcp/src/visus_review/types.rs` test module first (test only):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_model_preference_and_budgets() {
        let json = r#"{
          "schema_version": 1,
          "model_preference": ["google/gemini-3-flash-preview", "google/gemini-2.5-flash"],
          "escalation_model": "anthropic/claude-opus-4.8",
          "per_surface_review_budget_ms": 8000,
          "total_review_budget_ms": 90000,
          "max_concurrent_reviews": 3,
          "max_image_edge_px": 2880,
          "spike_factor": 1.5
        }"#;
        let cfg: VisualReviewConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.model_preference[0], "google/gemini-3-flash-preview");
        assert_eq!(cfg.total_review_budget_ms, 90_000);
        assert_eq!(cfg.spike_factor, 1.5);
    }

    #[test]
    fn cache_roundtrips() {
        let mut idx = CacheIndex::default();
        idx.entries.insert("dashboard".into(), CacheEntry {
            screenshot_sha256: "a".repeat(64),
            score: 82,
            verdict: "pass_with_notes".into(),
            model: "google/gemini-3-flash-preview".into(),
            reviewed_at: "2026-06-15T00:00:00Z".into(),
        });
        let s = serde_json::to_string(&idx).unwrap();
        let back: CacheIndex = serde_json::from_str(&s).unwrap();
        assert_eq!(back.entries["dashboard"].score, 82);
    }
}
```

- [ ] **Step 2: Run it (fails — types missing)**

Run: `cargo test -p vox-orchestrator-mcp visus_review::types 2>&1 | head`
Expected: FAIL — types not found.

- [ ] **Step 3: Implement the types**

Prepend to `crates/vox-orchestrator-mcp/src/visus_review/types.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Pluggable review config (contracts/orchestration/visual-review.config.v1.json).
#[derive(Debug, Clone, Deserialize)]
pub struct VisualReviewConfig {
    pub schema_version: u32,
    /// Vision model slugs in priority order. NOT hard-pinned in code.
    pub model_preference: Vec<String>,
    /// Higher-tier model used when a Flash review flags a high-severity finding.
    pub escalation_model: String,
    pub per_surface_review_budget_ms: u64,
    pub total_review_budget_ms: u64,
    pub max_concurrent_reviews: usize,
    pub max_image_edge_px: u32,
    pub spike_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub principle: String,     // e.g. "#65 visual hierarchy"
    pub severity: String,      // "low" | "medium" | "high"
    pub region: String,        // free-text region hint, e.g. "top-right toolbar"
    pub critique: String,
}

/// Strict JSON the vision model must return (parsed in Task 11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub score: u32,            // 0..=100
    pub verdict: String,       // "pass" | "pass_with_notes" | "fail"
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SurfaceReport {
    pub view_key: String,
    pub screenshot_sha256: String,
    pub status: String,        // "reviewed" | "cached" | "new" | "changed" | "deferred"
    pub score: Option<u32>,
    pub verdict: Option<String>,
    pub findings: Vec<Finding>,
    pub model: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub review_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub default_model: String,
    pub surfaces: Vec<SurfaceReport>,
    pub total_capture_ms: u64,
    pub total_review_ms: u64,
    pub surfaces_reviewed: usize,
    pub surfaces_cached: usize,
    pub surfaces_deferred: usize,
    pub spiked: bool,
    pub spike_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub screenshot_sha256: String,
    pub score: u32,
    pub verdict: String,
    pub model: String,
    pub reviewed_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheIndex {
    #[serde(default = "default_schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, CacheEntry>,
}

fn default_schema() -> u32 { 1 }
```

Create `crates/vox-orchestrator-mcp/src/visus_review/mod.rs`:

```rust
//! GUI visual AI adversarial review. Advisory: never gates CI.
pub mod types;
pub mod prompt;
pub mod model_select;
pub mod vision_call;
pub mod spike;

pub use types::*;
```

Add to `crates/vox-orchestrator-mcp/src/lib.rs`: `pub mod visus_review;`

Create the config `contracts/orchestration/visual-review.config.v1.json`:

```json
{
  "schema_version": 1,
  "model_preference": ["google/gemini-3-flash-preview", "google/gemini-2.5-flash"],
  "escalation_model": "anthropic/claude-opus-4.8",
  "per_surface_review_budget_ms": 8000,
  "total_review_budget_ms": 90000,
  "max_concurrent_reviews": 3,
  "max_image_edge_px": 2880,
  "spike_factor": 1.5
}
```

> Create empty stub files so `mod.rs` compiles: `prompt.rs`, `model_select.rs`, `vision_call.rs`, `spike.rs` each with a `// filled in a later task` line + the minimal `pub fn`/types referenced below. (Real bodies land in Tasks 8–12; a compiling stub here is a module declaration, not a hollow feature — fill before use.)

- [ ] **Step 4: Run tests (pass)**

Run: `cargo test -p vox-orchestrator-mcp visus_review::types`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-mcp/src/visus_review crates/vox-orchestrator-mcp/src/lib.rs contracts/orchestration/visual-review.config.v1.json
git commit -m "feat(visus-review): config + report/cache serde types (TDD)"
```

---

## Task 6: Cache diff — decide which surfaces need review (TDD)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (add `decide_status`)

- [ ] **Step 1: Write the failing test**

Add to `mod.rs`:

```rust
/// Per-surface review decision based on screenshot hash vs the cache.
#[derive(Debug, PartialEq, Eq)]
pub enum ReviewDecision { New, Changed, Cached }

/// Returns the decision for `view_key` given its fresh screenshot hash.
pub fn decide_status(cache: &CacheIndex, view_key: &str, fresh_sha: &str) -> ReviewDecision {
    match cache.entries.get(view_key) {
        None => ReviewDecision::New,
        Some(e) if e.screenshot_sha256 == fresh_sha => ReviewDecision::Cached,
        Some(_) => ReviewDecision::Changed,
    }
}

#[cfg(test)]
mod decide_tests {
    use super::*;
    fn cache_with(view: &str, sha: &str) -> CacheIndex {
        let mut c = CacheIndex::default();
        c.entries.insert(view.into(), CacheEntry {
            screenshot_sha256: sha.into(), score: 90, verdict: "pass".into(),
            model: "m".into(), reviewed_at: "t".into() });
        c
    }
    #[test] fn new_surface_is_new() {
        assert_eq!(decide_status(&CacheIndex::default(), "x", "aa"), ReviewDecision::New);
    }
    #[test] fn same_hash_is_cached() {
        assert_eq!(decide_status(&cache_with("x", "aa"), "x", "aa"), ReviewDecision::Cached);
    }
    #[test] fn different_hash_is_changed() {
        assert_eq!(decide_status(&cache_with("x", "aa"), "x", "bb"), ReviewDecision::Changed);
    }
}
```

- [ ] **Step 2: Run (fails then passes)**

Run: `cargo test -p vox-orchestrator-mcp visus_review::mod::decide_tests` (it compiles the impl above too).
Expected: PASS (3 tests). (The function and test land together; if you prefer strict red-first, comment the function body to `unimplemented!()` first, run to see FAIL, then restore.)

- [ ] **Step 3: Commit**

```
git add crates/vox-orchestrator-mcp/src/visus_review/mod.rs
git commit -m "feat(visus-review): sha-vs-cache review decision (New/Changed/Cached)"
```

---

## Task 7: Reviewer skeleton — load, diff, write report (NO AI yet)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (add `run`)
- Create: `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs`

- [ ] **Step 1: Implement `run` (change-detection only)**

Add to `mod.rs`:

```rust
use std::path::Path;

/// Minimal manifest entry shape mirroring the TS manifest.json (Task 2/3).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ManifestEntry {
    #[serde(rename = "viewKey")] pub view_key: String,
    pub file: String,
    pub sha256: String,
    #[serde(rename = "captureMs")] pub capture_ms: u64,
}
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Manifest {
    pub total_capture_ms: u64,
    pub surfaces: Vec<ManifestEntry>,
}

pub struct RunArgs<'a> {
    pub manifest_path: &'a Path,
    pub screens_dir: &'a Path,
    pub cache_path: &'a Path,
    pub report_dir: &'a Path,
    pub now_iso: String,        // passed in (no clock in lib)
    pub do_ai: bool,            // false in Phase 2; true from Task 11
}

/// Loads manifest + cache, decides per-surface status, (optionally) reviews, writes report.
/// Always returns Ok(report) — advisory, never errors the build out.
pub fn run(args: &RunArgs<'_>) -> RunReport {
    let manifest: Manifest = std::fs::read_to_string(args.manifest_path)
        .ok().and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Manifest { total_capture_ms: 0, surfaces: vec![] });
    let cache: CacheIndex = std::fs::read_to_string(args.cache_path)
        .ok().and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut surfaces = Vec::new();
    let (mut reviewed, mut cached, mut deferred) = (0usize, 0usize, 0usize);

    for entry in &manifest.surfaces {
        let decision = decide_status(&cache, &entry.view_key, &entry.sha256);
        match decision {
            ReviewDecision::Cached => {
                cached += 1;
                let c = &cache.entries[&entry.view_key];
                surfaces.push(SurfaceReport {
                    view_key: entry.view_key.clone(), screenshot_sha256: entry.sha256.clone(),
                    status: "cached".into(), score: Some(c.score), verdict: Some(c.verdict.clone()),
                    findings: vec![], model: Some(c.model.clone()),
                    prompt_tokens: None, completion_tokens: None, cost_usd: None, review_ms: None,
                });
            }
            ReviewDecision::New | ReviewDecision::Changed => {
                let status = if decision == ReviewDecision::New { "new" } else { "changed" };
                // Phase 2: record the change/warn; Phase 3 replaces this block with a real AI review.
                eprintln!("::warning::gui-visual-review: surface '{}' {} — appearance review needed", entry.view_key, status);
                if args.do_ai {
                    reviewed += 1;
                    // Filled in Task 11.
                    surfaces.push(review_surface(args, entry));
                } else {
                    surfaces.push(SurfaceReport {
                        view_key: entry.view_key.clone(), screenshot_sha256: entry.sha256.clone(),
                        status: status.into(), score: None, verdict: None, findings: vec![],
                        model: None, prompt_tokens: None, completion_tokens: None, cost_usd: None, review_ms: None,
                    });
                }
            }
        }
        let _ = deferred; // becomes nonzero once the budget gate (Task 12) defers overflow.
    }

    RunReport {
        schema_version: 1, generated_at: args.now_iso.clone(),
        default_model: String::new(), // set in Task 11
        surfaces, total_capture_ms: manifest.total_capture_ms, total_review_ms: 0,
        surfaces_reviewed: reviewed, surfaces_cached: cached, surfaces_deferred: deferred,
        spiked: false, spike_detail: String::new(),
    }
}

// Phase-2 placeholder so `run` compiles; replaced wholesale in Task 11.
fn review_surface(_args: &RunArgs<'_>, entry: &ManifestEntry) -> SurfaceReport {
    SurfaceReport {
        view_key: entry.view_key.clone(), screenshot_sha256: entry.sha256.clone(),
        status: "deferred".into(), score: None, verdict: None, findings: vec![],
        model: None, prompt_tokens: None, completion_tokens: None, cost_usd: None, review_ms: None,
    }
}

/// Writes the run report to `<report_dir>/<date>.json`. Returns the path.
pub fn write_report(report_dir: &Path, date: &str, report: &RunReport) -> std::io::Result<std::path::PathBuf> {
    std::fs::create_dir_all(report_dir)?;
    let path = report_dir.join(format!("{date}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(report).unwrap() + "\n")?;
    Ok(path)
}
```

- [ ] **Step 2: Write the binary**

Create `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs`:

```rust
//! Advisory GUI visual review CLI. ALWAYS exits 0 — never gates CI.
use std::path::Path;
use vox_orchestrator_mcp::visus_review::{run, write_report, RunArgs};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Args: --manifest <p> --screens <dir> --cache <p> --report-dir <dir> --date <YYYY-MM-DD> --now <iso> [--ai]
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let manifest = get("--manifest").unwrap_or_else(|| "crates/vox-gui/ui/e2e/screens/manifest.json".into());
    let screens = get("--screens").unwrap_or_else(|| "crates/vox-gui/ui/e2e/screens".into());
    let cache = get("--cache").unwrap_or_else(|| "contracts/reports/gui-visual-review/cache.v1.json".into());
    let report_dir = get("--report-dir").unwrap_or_else(|| "contracts/reports/gui-visual-review".into());
    let date = get("--date").unwrap_or_else(|| "0000-00-00".into());
    let now = get("--now").unwrap_or_default();
    let do_ai = a.iter().any(|x| x == "--ai");

    let args = RunArgs {
        manifest_path: Path::new(&manifest), screens_dir: Path::new(&screens),
        cache_path: Path::new(&cache), report_dir: Path::new(&report_dir),
        now_iso: now, do_ai,
    };
    let report = run(&args); // Task 11 makes this async; adjust to `.await` then.
    match write_report(Path::new(&report_dir), &date, &report) {
        Ok(p) => eprintln!("gui-visual-review: wrote {}", p.display()),
        Err(e) => eprintln!("::warning::gui-visual-review: report write failed: {e}"),
    }
    eprintln!(
        "gui-visual-review: {} reviewed, {} cached, {} deferred{}",
        report.surfaces_reviewed, report.surfaces_cached, report.surfaces_deferred,
        if report.spiked { " (TIME SPIKE)" } else { "" },
    );
    std::process::exit(0); // advisory — never non-zero
}
```

- [ ] **Step 3: Build + run against a fixture**

Run:
```
cargo build -p vox-orchestrator-mcp --bin gui-visual-review
cargo run -p vox-orchestrator-mcp --bin gui-visual-review -- --date 2026-06-15 --now 2026-06-15T00:00:00Z
```
Expected: exits 0; prints `::warning::` lines for new/changed surfaces; writes `contracts/reports/gui-visual-review/2026-06-15.json` with `status: new` for every surface (cache is empty on first run).

- [ ] **Step 4: Commit**

```
git add crates/vox-orchestrator-mcp/src/visus_review/mod.rs crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs
git commit -m "feat(visus-review): change-detection skeleton + report writer + advisory CLI (no AI yet)"
```

---

## Task 8: Vision-aware model selection (TDD)

**Files:**
- Replace stub: `crates/vox-orchestrator-mcp/src/visus_review/model_select.rs`

- [ ] **Step 1: Write the failing test**

Create `model_select.rs`:

```rust
//! Pick a vision-capable model: first config-preference that the registry marks
//! supports_vision; fall back to the first preference if the registry is unavailable.

/// Minimal trait so the resolver is unit-testable without a full registry.
pub trait VisionCatalog {
    fn supports_vision(&self, model_id: &str) -> Option<bool>;
}

/// Returns the chosen model id. Never panics; falls back to preference[0].
pub fn choose_vision_model(preference: &[String], catalog: &dyn VisionCatalog) -> String {
    for m in preference {
        if catalog.supports_vision(m) == Some(true) {
            return m.clone();
        }
    }
    preference.first().cloned().unwrap_or_else(|| "google/gemini-2.5-flash".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    struct Fake(HashMap<String, bool>);
    impl VisionCatalog for Fake {
        fn supports_vision(&self, m: &str) -> Option<bool> { self.0.get(m).copied() }
    }
    #[test] fn picks_first_vision_capable() {
        let c = Fake(HashMap::from([("a".into(), false), ("b".into(), true)]));
        assert_eq!(choose_vision_model(&["a".into(), "b".into()], &c), "b");
    }
    #[test] fn falls_back_to_first_when_registry_silent() {
        let c = Fake(HashMap::new());
        assert_eq!(choose_vision_model(&["x".into(), "y".into()], &c), "x");
    }
}
```

- [ ] **Step 2: Run (pass)** — `cargo test -p vox-orchestrator-mcp visus_review::model_select` → 2 pass.

- [ ] **Step 3: Wire the real registry adapter**

Append a registry-backed `VisionCatalog` impl that consults `vox_orchestrator::models::registry::ModelRegistry` (`get(model_id)` → `spec.capabilities.supports_vision`, per audit `spec.rs:38-85`). If `vox-orchestrator-mcp` already depends on `vox-orchestrator` (it does — the bridge uses the registry), import it directly:

```rust
pub struct RegistryCatalog<'a>(pub &'a vox_orchestrator::models::registry::ModelRegistry);
impl<'a> VisionCatalog for RegistryCatalog<'a> {
    fn supports_vision(&self, model_id: &str) -> Option<bool> {
        self.0.get(model_id).map(|spec| spec.capabilities.supports_vision)
    }
}
```

> Verify the exact field path (`spec.capabilities.supports_vision`) and `ModelRegistry::get` signature against `crates/vox-orchestrator/src/models/{registry.rs,spec.rs}` before compiling; adjust the accessor if the registry returns a reference/option wrapper.

- [ ] **Step 4: Build** — `cargo build -p vox-orchestrator-mcp` → clean.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-mcp/src/visus_review/model_select.rs
git commit -m "feat(visus-review): config+registry vision-model selection with fallback (TDD)"
```

---

## Task 9: The adversarial review prompt + rubric

**Files:**
- Replace stub: `crates/vox-orchestrator-mcp/src/visus_review/prompt.rs`

- [ ] **Step 1: Author the prompt**

Create `prompt.rs`:

```rust
//! System prompt for adversarial GUI screenshot review against the vox-gui
//! front-end design principles. The model MUST return ONLY the JSON object
//! described below (ReviewVerdict shape).

/// Distilled rubric drawn from docs/src/architecture/gui-frontend-design-principles-2026-06-14.md
/// and the 24-item per-surface checklist. Kept terse so it fits the system turn.
pub const RUBRIC: &str = r#"
Review this desktop-app surface SCREENSHOT adversarially against these principles. Hunt for real defects; do not flatter.
1 Visual hierarchy: exactly one primary action; scale/weight/contrast rank elements (#65-73).
2 Tokens/consistency: consistent color meaning, spacing rhythm, iconography; no ad-hoc visual noise (#20-24,#247-255).
3 Typography & spacing: readable measure/line-height; aligned to a spacing scale (#74-98).
4 Loading/empty/error: deliberate states, not silent blanks; errors actionable (#1.1,#47-52,#163-168).
5 Accessibility (visual): text contrast >=4.5:1, UI/focus >=3:1; targets >=24px; icon-only controls look labeled (#178-211).
6 Affordance & feedback: interactive elements look interactive; current location obvious (#132-162).
7 Minimalism: progressive disclosure; remove clutter (#42-46).
8 Error prevention: destructive actions visually distinct/guarded (#25-31).
"#;

/// Full system prompt: role + rubric + strict output contract.
pub fn system_prompt() -> String {
    format!(
        "You are a senior product-design reviewer performing an ADVERSARIAL critique of a desktop \
GUI surface screenshot. Be specific and skeptical: every finding must cite a visible region and a \
principle number.\n\nRUBRIC:\n{RUBRIC}\n\nOUTPUT CONTRACT: Respond with ONLY a single JSON object, no \
prose, no markdown fence:\n{{\n  \"score\": <integer 0-100, lower = more/worse defects>,\n  \"verdict\": \
\"pass\" | \"pass_with_notes\" | \"fail\",\n  \"findings\": [ {{ \"principle\": \"#NN short-name\", \
\"severity\": \"low\"|\"medium\"|\"high\", \"region\": \"<where on screen>\", \"critique\": \"<what is wrong and why>\" }} ]\n}}\n\
If the surface is clean, return an empty findings array, verdict \"pass\", score >=90."
    )
}

/// Per-surface user turn (the text part that precedes the image part).
pub fn user_prompt(view_key: &str) -> String {
    format!("Surface: '{view_key}'. Critique the attached screenshot per the rubric and output the JSON verdict.")
}
```

- [ ] **Step 2: Smoke test the prompt shape**

Add a test asserting the contract words are present (cheap guard against accidental edits):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn prompt_states_json_only_contract() {
        let p = system_prompt();
        assert!(p.contains("ONLY a single JSON object"));
        assert!(p.contains("\"findings\""));
        assert!(RUBRIC.contains("Visual hierarchy"));
    }
}
```

Run: `cargo test -p vox-orchestrator-mcp visus_review::prompt` → pass.

- [ ] **Step 3: Commit**

```
git add crates/vox-orchestrator-mcp/src/visus_review/prompt.rs
git commit -m "feat(visus-review): adversarial design-principles review prompt + JSON contract"
```

---

## Task 10: The vision call — base64 image → OpenRouter

**Files:**
- Replace stub: `crates/vox-orchestrator-mcp/src/visus_review/vision_call.rs`

> Reuse the exact base64/data-URL construction from `llm_bridge/infer.rs:468-496` and the OpenAI-compat transport. First inspect whether `http_openai_compatible_with_headers` (`providers/openai.rs:33-55`) is callable from this module (same crate → yes for `pub(crate)`); if its signature is awkward for a one-shot call, do a direct `reqwest` POST as below (it mirrors what that function does).

- [ ] **Step 1: Implement the call**

Create `vision_call.rs`:

```rust
//! One-shot vision completion to OpenRouter using a base64 PNG image part.
//! Mirrors llm_bridge/infer.rs:468-496 (data-URL build) and the OpenAI-compat POST.

use base64::Engine as _;
use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct Usage { pub prompt_tokens: u64, pub completion_tokens: u64, pub cost_usd: Option<f64> }

#[derive(Deserialize)]
struct OrUsage { #[serde(default)] prompt_tokens: u64, #[serde(default)] completion_tokens: u64,
    #[serde(default)] total_cost: Option<f64>, #[serde(default)] cost: Option<f64> }
#[derive(Deserialize)]
struct OrMsg { content: String }
#[derive(Deserialize)]
struct OrChoice { message: OrMsg }
#[derive(Deserialize)]
struct OrResp { choices: Vec<OrChoice>, #[serde(default)] usage: Option<OrUsage> }

/// POST text+image to OpenRouter /chat/completions. Returns (content, usage).
pub async fn call_vision_model(
    model: &str, system: &str, user_text: &str, png_bytes: &[u8],
) -> Result<(String, Usage), String> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);
    let data_url = format!("data:image/png;base64,{b64}");
    let url = vox_config::openrouter_chat_completions_url();
    let key = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenRouterApiKey)
        .map_err(|e| format!("no OpenRouter key: {e}"))?;

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": [
                { "type": "text", "text": user_text },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]}
        ],
        "temperature": 0.2,
        "usage": { "include": true }
    });

    let client = reqwest::Client::new();
    let resp = client.post(&url)
        .bearer_auth(key)
        .header("HTTP-Referer", "https://github.com/vox-foundation/vox")
        .header("X-Title", "vox-gui-visual-review")
        .json(&body).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("openrouter {}: {}", resp.status(), resp.text().await.unwrap_or_default()));
    }
    let parsed: OrResp = resp.json().await.map_err(|e| e.to_string())?;
    let content = parsed.choices.into_iter().next().map(|c| c.message.content).unwrap_or_default();
    let usage = parsed.usage.map(|u| Usage {
        prompt_tokens: u.prompt_tokens, completion_tokens: u.completion_tokens,
        cost_usd: u.total_cost.or(u.cost),
    }).unwrap_or_default();
    Ok((content, usage))
}
```

> Confirm `vox-orchestrator-mcp` already has `base64`, `reqwest`, `serde_json` deps (the bridge uses all three — audit cites `base64::engine` in infer.rs). If `vox-secrets` / `vox-config` aren't direct deps, add them to `crates/vox-orchestrator-mcp/Cargo.toml` (the bridge already resolves the OpenRouter URL+key, so they should be present).

- [ ] **Step 2: Build** — `cargo build -p vox-orchestrator-mcp` → clean.

- [ ] **Step 3: Manual live smoke (optional, needs key)**

With `OPENROUTER_API_KEY` set, add a `#[ignore]` test that sends a tiny solid-color PNG and asserts a non-empty response, run with `cargo test -p vox-orchestrator-mcp visus_review::vision_call -- --ignored --nocapture`. Keep it `#[ignore]` so CI/offline runs skip it.

- [ ] **Step 4: Commit**

```
git add crates/vox-orchestrator-mcp/src/visus_review/vision_call.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "feat(visus-review): one-shot base64-image vision call to OpenRouter"
```

---

## Task 11: Wire AI review into `run` + verdict parsing + cache update

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs`
- Modify: `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs` (make `run` async)

- [ ] **Step 1: Verdict parser (TDD)**

Add to `mod.rs`:

```rust
/// Parse the model's JSON verdict, tolerating a leading/trailing code fence or prose.
pub fn parse_verdict(raw: &str) -> Result<ReviewVerdict, String> {
    let start = raw.find('{').ok_or("no JSON object in response")?;
    let end = raw.rfind('}').ok_or("no closing brace")?;
    serde_json::from_str(&raw[start..=end]).map_err(|e| format!("verdict parse: {e}"))
}

#[cfg(test)]
mod verdict_tests {
    use super::*;
    #[test] fn parses_fenced_json() {
        let raw = "```json\n{\"score\":80,\"verdict\":\"pass_with_notes\",\"findings\":[]}\n```";
        let v = parse_verdict(raw).unwrap();
        assert_eq!(v.score, 80);
        assert_eq!(v.verdict, "pass_with_notes");
    }
    #[test] fn errors_on_garbage() { assert!(parse_verdict("no json here").is_err()); }
}
```

Run: `cargo test -p vox-orchestrator-mcp visus_review::mod::verdict_tests` → pass.

- [ ] **Step 2: Replace `review_surface` with the real async review**

Make `run` async and rewrite `review_surface`:

```rust
async fn review_surface(
    args: &RunArgs<'_>, entry: &ManifestEntry, model: &str, cfg: &VisualReviewConfig,
) -> SurfaceReport {
    use crate::visus_review::{prompt, vision_call};
    let png = match std::fs::read(args.screens_dir.join(&entry.file)) {
        Ok(b) => b, Err(e) => return failed_surface(entry, &format!("read png: {e}")),
    };
    let t0 = std::time::Instant::now();
    let res = vision_call::call_vision_model(model, &prompt::system_prompt(), &prompt::user_prompt(&entry.view_key), &png).await;
    let review_ms = t0.elapsed().as_millis() as u64;
    let _ = cfg; // per_surface budget consulted by the caller (Task 12)
    match res {
        Ok((content, usage)) => match parse_verdict(&content) {
            Ok(v) => SurfaceReport {
                view_key: entry.view_key.clone(), screenshot_sha256: entry.sha256.clone(),
                status: "reviewed".into(), score: Some(v.score), verdict: Some(v.verdict),
                findings: v.findings, model: Some(model.to_string()),
                prompt_tokens: Some(usage.prompt_tokens), completion_tokens: Some(usage.completion_tokens),
                cost_usd: usage.cost_usd, review_ms: Some(review_ms),
            },
            Err(e) => failed_surface(entry, &e),
        },
        Err(e) => failed_surface(entry, &e),
    }
}

fn failed_surface(entry: &ManifestEntry, why: &str) -> SurfaceReport {
    eprintln!("::warning::gui-visual-review: '{}' review failed: {}", entry.view_key, why);
    SurfaceReport {
        view_key: entry.view_key.clone(), screenshot_sha256: entry.sha256.clone(),
        status: "deferred".into(), score: None, verdict: None, findings: vec![],
        model: None, prompt_tokens: None, completion_tokens: None, cost_usd: None, review_ms: None,
    }
}
```

In `run`: load `VisualReviewConfig` from `contracts/orchestration/visual-review.config.v1.json`; build the registry, choose the model via `model_select::choose_vision_model`; set `report.default_model`. For each New/Changed surface call `review_surface(...).await`; on `status == "reviewed"` insert/update the cache entry. After the loop, persist the cache with `serde_json::to_string_pretty` back to `args.cache_path`, and total `review_ms`.

- [ ] **Step 3: Update the binary to await**

In `gui-visual-review.rs`, change `let report = run(&args);` → `let report = run(&args).await;` and keep `#[tokio::main]`.

- [ ] **Step 4: Build + offline run (no key → graceful)**

Run (no key): `cargo run -p vox-orchestrator-mcp --bin gui-visual-review -- --ai --date 2026-06-15 --now 2026-06-15T00:00:00Z`
Expected: exits 0; surfaces with no key resolve to `status: deferred` with a `::warning::` (not a crash); report still written.

- [ ] **Step 5: Build + live run (with key)**

With `OPENROUTER_API_KEY` set and a fresh `manifest.json`: same command. Expected: changed/new surfaces get `status: reviewed` with `score`/`findings`/`cost_usd`; `cache.v1.json` updated; a second run with the same screenshots shows all `status: cached` and **zero** vision calls.

- [ ] **Step 6: Commit**

```
git add crates/vox-orchestrator-mcp/src/visus_review/mod.rs crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs
git commit -m "feat(visus-review): AI review of changed surfaces + verdict parse + cache update"
```

---

## Task 12: Time budget + spike detection (TDD)

**Files:**
- Replace stub: `crates/vox-orchestrator-mcp/src/visus_review/spike.rs`
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (enforce budget + append ledger)

- [ ] **Step 1: Spike math (TDD)**

Create `spike.rs`:

```rust
//! Trailing-median spike detection over a JSONL ledger. Mirrors the CR-P2 pattern.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerRow {
    pub ts: String,
    pub surfaces_reviewed: usize,
    pub total_review_ms: u64,
    pub total_cost_usd: f64,
    pub model: String,
}

/// True if `this_ms` exceeds `factor` * median(history) (history = prior totals).
pub fn is_spike(history_ms: &[u64], this_ms: u64, factor: f64) -> (bool, String) {
    if history_ms.is_empty() { return (false, "no baseline".into()); }
    let mut v: Vec<u64> = history_ms.to_vec();
    v.sort_unstable();
    let median = v[v.len() / 2] as f64;
    let threshold = median * factor;
    let spiked = (this_ms as f64) > threshold;
    (spiked, format!("this={this_ms}ms median={median:.0}ms threshold={threshold:.0}ms"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn no_history_no_spike() { assert!(!is_spike(&[], 9999, 1.5).0); }
    #[test] fn within_threshold_ok() { assert!(!is_spike(&[100,100,100], 140, 1.5).0); }
    #[test] fn over_threshold_spikes() { assert!(is_spike(&[100,100,100], 200, 1.5).0); }
}
```

Run: `cargo test -p vox-orchestrator-mcp visus_review::spike` → 3 pass.

- [ ] **Step 2: Enforce per-run wall-clock budget in `run`**

In `run`, before each `review_surface` call, check elapsed against `cfg.total_review_budget_ms`; if exceeded, push a `status: "deferred"` surface (increment `deferred`) and `eprintln!("::warning::… deferred (review budget exhausted)")` instead of calling the model. This bounds the time cost so CI never balloons.

- [ ] **Step 3: Append ledger + set spike fields**

After the loop: read `contracts/reports/gui-visual-review/ledger.jsonl` (each line a `LedgerRow`), collect prior `total_review_ms` into `history`, compute `is_spike(history, total_review_ms, cfg.spike_factor)`, set `report.spiked`/`report.spike_detail`, then append this run's `LedgerRow`. On spike, `eprintln!("::warning::gui-visual-review: TIME SPIKE — {detail}")`.

- [ ] **Step 4: Build + run twice**

Run the binary twice (with key). Expected: 2nd `ledger.jsonl` line appended; `report.spiked=false` on a normal second run; if you artificially inflate (review many surfaces), `spiked=true` with detail, still exit 0.

- [ ] **Step 5: Commit**

```
git add crates/vox-orchestrator-mcp/src/visus_review/spike.rs crates/vox-orchestrator-mcp/src/visus_review/mod.rs
git commit -m "feat(visus-review): per-run time budget + trailing-median spike warning (TDD)"
```

---

## Task 13: CI wiring (non-gating)

**Files:**
- Modify: `.github/workflows/ci.yml` (`gui-playwright-smoke` job, around lines 1088-1161)

- [ ] **Step 1: Add capture + review steps after the existing sweep**

In `gui-playwright-smoke`, after the visual-audit playwright step, add:

```yaml
      - name: GUI visual-review capture (manifest)
        working-directory: crates/vox-gui/ui
        run: pnpm exec playwright test --config playwright.config.ts visual-review --project=chromium --workers=1
        continue-on-error: true

      - name: GUI visual AI review (advisory, non-gating)
        env:
          OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
        run: |
          DATE=$(date -u +%F)
          NOW=$(date -u +%FT%TZ)
          cargo run -p vox-orchestrator-mcp --bin gui-visual-review -- \
            --ai --date "$DATE" --now "$NOW"
        continue-on-error: true

      - name: Upload visual-review report + screens (always)
        if: ${{ always() }}
        uses: actions/upload-artifact@v4
        with:
          name: gui-visual-review-${{ github.run_id }}
          path: |
            contracts/reports/gui-visual-review/
            crates/vox-gui/ui/e2e/screens/manifest.json
          if-no-files-found: ignore
          retention-days: 14
```

> The job is already advisory (post-merge / `full-ci` label, per audit). `continue-on-error: true` + the binary's hard `exit 0` make double-sure it never fails the job. `OPENROUTER_API_KEY` must exist as a repo/org secret on the `[self-hosted, linux, x64, browser]` runner; if absent, the review step logs warnings and defers (Task 11 Step 4) — still green.

- [ ] **Step 2: Persist cache on main (post-merge) via the autoregen bot**

Add a conditional commit-back step (only on push to `main`), reusing the `SSOT_AUTOREGEN_TOKEN` pattern (memory: ssot-autoregen PR bot):

```yaml
      - name: Commit visual-review cache + latest report (main only)
        if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}
        env:
          GH_TOKEN: ${{ secrets.SSOT_AUTOREGEN_TOKEN }}
        run: |
          if ! git diff --quiet -- contracts/reports/gui-visual-review/; then
            git config user.name "vox-ssot-bot"
            git config user.email "bot@vox-foundation"
            git add contracts/reports/gui-visual-review/cache.v1.json contracts/reports/gui-visual-review/*.json contracts/reports/gui-visual-review/ledger.jsonl
            git commit -m "chore(visual-review): update cache + report [skip ci]"
            git push origin HEAD:main || echo "::warning::visual-review cache push skipped (non-fast-forward)"
          fi
        continue-on-error: true
```

- [ ] **Step 3: Validate the workflow file**

Run: `vox ci workflow-lint` (or `actionlint .github/workflows/ci.yml` if available). Expected: no errors. Confirm `gui-playwright-smoke` is still NOT in the required-checks `needs:` of the gating job (`grep -n "gui-playwright-smoke" .github/workflows/ci.yml` — it must not appear as a dependency of "Check, Build, and Test (Rust)").

- [ ] **Step 4: Commit**

```
git add .github/workflows/ci.yml
git commit -m "ci(visual-review): non-gating capture+AI-review steps in gui-playwright-smoke"
```

---

## Task 14: The advisory `vox ci gui-visual-review` rule

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/` (add a subcommand; follow an existing advisory check like `runner-policy-check`)

- [ ] **Step 1: Add the subcommand**

Mirror an existing advisory `vox ci` check (e.g. `runner-policy-check`, which prints warnings and returns Ok even with findings). Add `gui-visual-review` that shells `cargo run -p vox-orchestrator-mcp --bin gui-visual-review -- --ai` (or `--report-only` without `--ai` when `--no-ai`/offline), prints a one-line summary from the written report, and **always returns `Ok(())`**. Register it in the `vox ci` subcommand enum + dispatch, matching the pattern of the sibling checks in `crates/vox-cli/src/commands/ci/`.

> Read one existing advisory check end-to-end first (subcommand enum variant + dispatch arm + the check fn) and copy its shape exactly — do not invent a new pattern.

- [ ] **Step 2: Build + run**

Run: `cargo run -p vox-cli -- ci gui-visual-review` (offline). Expected: prints `gui-visual-review: N reviewed, M cached, K deferred`, exits 0 even when surfaces are flagged.

- [ ] **Step 3: Confirm it is NOT added to the pre-push/required gate**

Run: `grep -rn "gui-visual-review" .github/workflows scripts lefthook.yml 2>/dev/null`. It must appear ONLY in the advisory `gui-playwright-smoke` steps from Task 13 — never in pre-push hooks or the required gate.

- [ ] **Step 4: Commit**

```
git add crates/vox-cli/src/commands/ci
git commit -m "feat(ci): advisory `vox ci gui-visual-review` wrapper (never gates)"
```

---

## Task 15: SSOT documentation

**Files:**
- Create: `docs/src/architecture/gui-visual-ai-review.md`

- [ ] **Step 1: Write the doc (with required frontmatter)**

Create `docs/src/architecture/gui-visual-ai-review.md` starting with the mandatory frontmatter (per CLAUDE.md, `category` must be a canonical label):

```markdown
---
title: GUI Visual AI Adversarial Review
description: Non-gating, cache-driven AI review of Playwright screenshots of every vox-gui surface against the front-end design principles.
category: "Architecture SSOTs"
---

# GUI Visual AI Adversarial Review

## The rule
Every vox-gui surface is screenshotted on the post-merge / `full-ci` Playwright sweep. A surface is
AI-reviewed against the design principles ONLY when its screenshot content hash changes (or it is new).
This is **advisory** — it never gates CI; it warns and collects reports.

## How it works
- **Discover**: `SURFACE_REGISTRY` (SSOT) → every `viewKey` with `tier != 'none'`.
- **Capture**: `e2e/visual-review.spec.ts` shoots hi-DPI PNGs (2x), emits `manifest.json` with per-surface `sha256` + timing.
- **Cache**: `contracts/reports/gui-visual-review/cache.v1.json` maps `viewKey → screenshot_sha256 + verdict`. Same hash → reuse verdict, no AI call. Changed hash → warn + re-review.
- **Review**: `gui-visual-review` (in `vox-orchestrator-mcp`) sends the PNG to an OpenRouter vision model and parses a JSON verdict.
- **Report**: versioned `contracts/reports/gui-visual-review/<date>.json` + `ledger.jsonl` (trend/spike). Spike = run time > 1.5x trailing median → warning only.

## Model (pluggable, not pinned)
`contracts/orchestration/visual-review.config.v1.json` lists vision models in priority order (default Gemini Flash family; Opus for escalation). The resolver picks the first config entry the model registry marks `supports_vision`. Per-surface model is recorded for future A/B learning.

## What it never does
- Never fails CI (binary exits 0; CI step `continue-on-error`).
- Never gates on appearance change — it warns and re-reviews, it does not pixel-diff-gate.
```

- [ ] **Step 2: Validate the doc pipeline**

Run: `vox ci ssot-drift` (or the doc-pipeline lint). Expected: passes — the `category: "Architecture SSOTs"` frontmatter satisfies the gate (this is the exact value that blocked an earlier push when omitted).

- [ ] **Step 3: Commit**

```
git add docs/src/architecture/gui-visual-ai-review.md
git commit -m "docs(architecture): GUI visual AI review SSOT (the rule + cache + model config)"
```

---

## Self-Review

**Spec coverage:**
- "AI adversarial evaluation of Playwright screenshots" → Tasks 9–11 (prompt + vision call + verdict). ✅
- "screenshots of all endpoints, auto-explored as surfaces change" → Task 3 drives off `SURFACE_REGISTRY` (auto-updates). ✅
- "performantly, aggressive time, track time delta, notice spikes" → Task 12 (budget + trailing-median spike), Task 3/4 (capture timing). ✅
- "don't gate CICD" → Tasks 7 (exit 0), 13 (`continue-on-error`, advisory job), 14 (advisory rule). ✅
- "collect reports, especially newly generated sites" → Tasks 7/11 (report), `status: new`, ledger. ✅
- "don't re-review when appearance unchanged (cache)" → Tasks 5/6/11 (sha cache, `Cached` skips AI). ✅
- "warn + re-run AI when appearance changes" → Task 7 `::warning::` + Task 11 review on `Changed`. ✅
- "OpenRouter pipeline, model good at GUI, not necessarily pinned, learnable" → Task 8 config+registry selection, config file, per-surface model recorded; research picked Gemini family default. ✅
- "screenshots big/detailed enough" → Task 3 `deviceScaleFactor: 2`, fullPage, `max_image_edge_px` cap. ✅
- "add a rule" → Task 14 advisory `vox ci` check + Task 15 SSOT doc. ✅
- "audit/integrate/wire with what we have" → reuses SURFACE_REGISTRY, the existing mock, the llm_bridge vision transport, the registry vision flag, the CR report convention, the advisory CI job. ✅

**Placeholder scan:** The only intentional forward-references are the Task-5 module stubs (`prompt.rs`/`model_select.rs`/`vision_call.rs`/`spike.rs`), each filled in a named later task before it is called — flagged inline, not hollow shipped code. The Phase-2 `review_surface` placeholder is explicitly replaced in Task 11.

**Type consistency:** `ManifestEntry` (TS, Task 2) and `ManifestEntry` (Rust, Task 7) share field meaning; the Rust side uses `#[serde(rename)]` for `viewKey`/`captureMs`. `ReviewVerdict`/`Finding` (Task 5) are produced by `parse_verdict` (Task 11) and consumed in `SurfaceReport`. `VisualReviewConfig` fields (Task 5) match the JSON (Task 5) and are read in Tasks 11–12. `choose_vision_model` (Task 8) feeds `review_surface`'s `model` arg (Task 11).

**Open verification items the executor MUST confirm before relying on them** (called out at point of use): (1) `vox-arch-check` accepts the reviewer living in `vox-orchestrator-mcp` (Task 5 note); (2) exact `ModelRegistry::get(...).capabilities.supports_vision` accessor (Task 8 Step 3); (3) `vox-orchestrator-mcp` already has `base64`/`reqwest`/`vox-secrets`/`vox-config` deps (Task 10 note); (4) `OPENROUTER_API_KEY` exists on the self-hosted browser runner (Task 13 Step 1).

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-15-gui-visual-ai-review.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration. Phases 1–2 (Tasks 1–7) deliver change-detection + reports with zero AI cost; Phase 3+ adds the vision review.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
