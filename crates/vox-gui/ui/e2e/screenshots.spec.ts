/**
 * Visual-audit screenshot sweep.
 *
 * Extends the existing dashboard.spec Tauri-bridge mock (window.__TAURI_INTERNALS__.invoke)
 * into a full capture of every GUI surface. For each view it spins up a fresh page with the
 * target view pre-selected (localStorage + get_initial_view) and a rich invoke mock so panels
 * render with representative data, then writes a full-page PNG to e2e/screens/<view>.png.
 *
 * Run: pnpm exec playwright test screenshots.spec.ts --project=chromium
 */
import { test, expect, type Page } from '@playwright/test';
import { SURFACE_REGISTRY } from '../src/generated/surfaceRegistry.generated';
import { installTauriMock } from './lib/tauriMock';

/**
 * Every screenshot-able surface is derived from the generated SURFACE_REGISTRY — the same
 * SSOT the sidebar renders from. This keeps the sweep drift-proof: when a surface is added,
 * renamed, combined, or removed, `vox ci gui-surface-registry --write` regenerates the
 * registry and this list follows automatically. There is no hand-maintained view list to
 * fall out of date.
 */
const VIEWS: string[] = Array.from(
  new Set(
    SURFACE_REGISTRY.filter((e) => e.viewKey && e.tier !== 'none').map((e) => e.viewKey as string),
  ),
).sort();

/**
 * Console-error substrings that are environmental noise rather than surface defects. Kept
 * deliberately narrow — only favicon requests, matched by their URL (the console text for a
 * failed resource is just "Failed to load resource: …404") — so a real missing asset or any
 * other 404 still fails the audit.
 */
const BENIGN_CONSOLE: string[] = ['favicon'];

/** Capture console-error + pageerror streams for a page. Console entries carry their resource
 *  URL so favicon noise can be filtered by URL without masking other failures. */
function captureErrors(page: Page) {
  const consoleErrors: string[] = [];
  const pageErrors: string[] = [];
  page.on('console', m => { if (m.type() === 'error') consoleErrors.push(`${m.text()} ${m.location()?.url ?? ''}`); });
  page.on('pageerror', e => pageErrors.push(e.message));
  return { consoleErrors, pageErrors };
}

const meaningfulConsole = (errs: string[]) => errs.filter(t => !BENIGN_CONSOLE.some(b => t.includes(b)));

test.describe('GUI visual audit', () => {
  for (const view of VIEWS) {
    test(`capture ${view}`, async ({ browser }) => {
      const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      const page = await ctx.newPage();
      const { consoleErrors, pageErrors } = captureErrors(page);
      await page.addInitScript(installTauriMock, view);
      await page.goto('/');
      // The app shell (sidebar nav) must mount before we judge the surface itself.
      await page.waitForSelector('nav', { timeout: 15_000 });
      await page.waitForTimeout(1600);
      await page.screenshot({ path: `e2e/screens/${view}.png`, fullPage: true });

      // ── Visual-audit assertions ─────────────────────────────────────────
      // 1. The surface rendered without tripping its error boundary.
      await expect(
        page.locator('[data-surface-error]'),
        `[${view}] crashed into its error boundary`,
      ).toHaveCount(0);
      // 2. No uncaught exceptions during render.
      expect(pageErrors, `[${view}] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
      // 3. No console errors (React key/prop warnings, failed IPC, …) beyond the benign allowlist.
      const meaningful = meaningfulConsole(consoleErrors);
      expect(meaningful, `[${view}] console errors:\n${meaningful.join('\n')}`).toEqual([]);

      await ctx.close();
    });
  }

  test('capture command-palette', async ({ browser }) => {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    await page.addInitScript(installTauriMock, 'dashboard');
    await page.goto('/');
    await page.waitForTimeout(1000);
    await page.keyboard.press('Control+k');
    await page.waitForTimeout(400);
    await page.keyboard.type('search');
    await page.waitForTimeout(600);
    await page.screenshot({ path: 'e2e/screens/_command-palette.png', fullPage: false });
    await ctx.close();
  });

  // Capture the sidebar in its non-default widths so the grouped/collapsed rendering is audited
  // in every mode, not just 'default'.
  for (const sbMode of ['rail', 'wide'] as const) {
    test(`capture sidebar-${sbMode}`, async ({ browser }) => {
      const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      const page = await ctx.newPage();
      const { consoleErrors, pageErrors } = captureErrors(page);
      await page.addInitScript(installTauriMock, 'dashboard');
      // useLocalStorage JSON-parses its value, so the mode must be stored as JSON.
      await page.addInitScript((m: string) => localStorage.setItem('vox_sidebar_mode', JSON.stringify(m)), sbMode);
      await page.goto('/');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await page.waitForTimeout(1200);
      await page.screenshot({ path: `e2e/screens/_sidebar-${sbMode}.png`, fullPage: false });
      await expect(page.locator('[data-surface-error]')).toHaveCount(0);
      expect(pageErrors, `[sidebar-${sbMode}] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
      const meaningful = meaningfulConsole(consoleErrors);
      expect(meaningful, `[sidebar-${sbMode}] console errors:\n${meaningful.join('\n')}`).toEqual([]);
      await ctx.close();
    });
  }
});
