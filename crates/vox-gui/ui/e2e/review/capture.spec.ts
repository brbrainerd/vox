// crates/vox-gui/ui/e2e/review/capture.spec.ts
/**
 * Review-bundle capture matrix @review-capture. Env-gated (VOX_REVIEW_CAPTURE=1).
 * Captures are EVIDENCE: failed state setups record state_ok:false, they do
 * not fail the run. Viewport-clipped screenshots (fullPage explodes on rich
 * lists and downscales to unreadability for the vision model); content
 * height is recorded per entry instead.
 */
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { createHash } from 'node:crypto';
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { SURFACE_REGISTRY } from '../../src/generated/surfaceRegistry.generated';
import { VIEWPORTS, statesFor, AUDIT_THEMES, type ReviewState } from './states';
import { auditIconsInPage, auditOverflowInPage } from './audits';
import { addRichMockInitScript } from '../lib/tauriMockRich';
import { addMockInitScript } from '../lib/tauriMockShared';
import { installEmptyStateMock, installErrorStateMock } from '../lib/tauriMockVariants';

const RUN = process.env.VOX_REVIEW_CAPTURE === '1';
const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'review-bundle', 'latest');
const SURFACES = SURFACE_REGISTRY.filter((e) => e.viewKey != null).map((e) => e.viewKey as string);
// Benign noise filter (mirrors screenshots.spec.ts): favicon fetches etc.
const BENIGN = [/favicon/i];

async function installMock(page: import('@playwright/test').Page, kind: string, surface: string) {
  if (kind === 'empty') return addMockInitScript(page, installEmptyStateMock, surface);
  if (kind === 'error') return addMockInitScript(page, installErrorStateMock, surface);
  if (kind === 'none') return; // true browser mode — Phase A regression
  return addRichMockInitScript(page, surface);
}

async function captureOne(
  page: import('@playwright/test').Page,
  browserName: string,
  surface: string,
  state: ReviewState,
  vpName: string,
  theme: string | null,
) {
  mkdirSync(OUT, { recursive: true });
  const id = [surface, state.name, vpName, browserName, ...(theme ? [`theme-${theme}`] : [])].join('--');
  const consoleErrors: string[] = [];
  const consoleWarnings: string[] = [];
  const pageErrors: string[] = [];
  page.on('console', (m) => {
    const text = m.text();
    if (BENIGN.some((re) => re.test(text) || re.test(m.location()?.url ?? ''))) return;
    if (m.type() === 'error') consoleErrors.push(text);
    else if (m.type() === 'warning') consoleWarnings.push(text);
  });
  page.on('pageerror', (e) => pageErrors.push(e.message));

  const t0 = Date.now();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await installMock(page, state.mock ?? 'rich', surface);
  await page.goto('/');
  await page.waitForSelector('nav', { timeout: 20_000 });
  await page.evaluate(() => (document as any).fonts?.ready);
  if (theme) await page.evaluate((t) => { document.documentElement.dataset.theme = t; }, theme);

  let stateOk = true;
  let stateError = '';
  if (state.setup) {
    try {
      await state.setup(page);
    } catch (e) {
      stateOk = false;
      stateError = String(e);
    }
  }
  await page.waitForTimeout(400); // settle: menus, theme swap, layout

  const file = `${id}.png`;
  await page.screenshot({ path: join(OUT, file), animations: 'disabled' }); // viewport clip
  const sha256 = createHash('sha256').update(readFileSync(join(OUT, file))).digest('hex');

  let axeViolations: unknown[] = [];
  try {
    const axe = await new AxeBuilder({ page }).analyze();
    axeViolations = axe.violations.filter((v) => ['moderate', 'serious', 'critical'].includes(v.impact ?? ''));
  } catch (e) {
    consoleWarnings.push(`axe-failed: ${String(e)}`);
  }
  const iconIssues = await page.evaluate(auditIconsInPage);
  const overflow = await page.evaluate(auditOverflowInPage);

  const entry = {
    id, surface, state: state.name, viewport: vpName, browser: browserName,
    theme: theme ?? 'default', file, sha256,
    state_ok: stateOk, state_error: stateError,
    axe_violations: axeViolations,
    console_errors: consoleErrors.slice(0, 50),
    console_warnings: consoleWarnings.slice(0, 50),
    page_errors: pageErrors,
    icon_issues: iconIssues,
    overflow,
    capture_ms: Date.now() - t0,
    captured_at: new Date().toISOString(),
  };
  appendFileSync(
    join(OUT, `entries-${browserName}-w${test.info().workerIndex}.jsonl`),
    JSON.stringify(entry) + '\n',
  );
  if (state.mock === 'none') {
    // Phase A regression, now automated: banner renders; zero raw TypeErrors.
    await expect(page.getByRole('status', { name: /browser preview/i })).toBeVisible();
  }
  expect(pageErrors.filter((e) => /__TAURI_INTERNALS__/.test(e))).toEqual([]);
}

test.describe('review-bundle capture @review-capture', () => {
  test.skip(!RUN, 'set VOX_REVIEW_CAPTURE=1 to run the capture matrix');

  for (const surface of SURFACES) {
    for (const state of statesFor(surface)) {
      for (const vp of VIEWPORTS) {
        if (state.viewports && !state.viewports.includes(vp.name)) continue;
        test(`${surface} -- ${state.name} -- ${vp.name}`, async ({ page, browserName }) => {
          await page.setViewportSize({ width: vp.width, height: vp.height });
          await captureOne(page, browserName, surface, state, vp.name, null);
        });
      }
    }
  }

  // Theme sub-dimension: default state x wide x chromium only (bounded cost).
  for (const surface of SURFACES) {
    for (const theme of AUDIT_THEMES) {
      test(`${surface} -- default -- wide -- theme:${theme}`, async ({ page, browserName }) => {
        test.skip(browserName !== 'chromium', 'theme captures are chromium-only');
        await page.setViewportSize({ width: 1440, height: 900 });
        await captureOne(page, browserName, surface, { name: 'default' }, 'wide', theme);
      });
    }
  }
});
