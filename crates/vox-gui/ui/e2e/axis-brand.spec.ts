/**
 * Axis brand visual + assertion sweep.
 *
 * Confirms the Vox Axis rebrand is perceivable end-to-end in the running UI:
 *  - the document title is "Axis"
 *  - the sidebar renders the AxisMark gimbal glyph (aria-label="Axis"), the "AXIS"
 *    wordmark, and the "Vox Axis" footer — and NOT the old "VOX"/"V" letterform
 *  - the mark is still present when the sidebar is collapsed (rail mode)
 * and captures screenshots a human (and CI) can eyeball.
 *
 * Run: pnpm exec playwright test axis-brand.spec.ts --project=chromium
 */
import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

test.describe('Axis brand', () => {
  test('document title is Axis', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'dashboard');
    await page.goto('/');
    await expect(page).toHaveTitle('Axis');
  });

  test('sidebar shows the AxisMark + AXIS wordmark + Vox Axis footer', async ({ browser }) => {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    await addMockInitScript(page, installTauriMock, 'dashboard');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    // brand glyph (the gimbal AxisMark) is present and labeled
    await expect(page.locator('svg[aria-label="Axis"]').first()).toBeVisible();
    // wordmark, full brand, and no legacy letterform
    await expect(page.getByText('AXIS', { exact: true }).first()).toBeVisible();
    await expect(page.getByText(/Vox Axis/).first()).toBeVisible();
    await expect(page.getByText('VOX', { exact: true })).toHaveCount(0);
    // topbar (TopHud) lockup is rebranded too — guards the second brand surface
    await expect(page.getByText('axis operator console')).toBeVisible();
    await expect(page.getByText('vox operator console')).toHaveCount(0);
    // no lingering legacy brand strings anywhere on the surface
    await expect(page.getByText(/\bIMPERIUM\b/)).toHaveCount(0);

    await page.screenshot({ path: 'e2e/screens/_axis-brand-sidebar.png', fullPage: false });
    await ctx.close();
  });

  test('mark stays visible in rail (collapsed) mode', async ({ browser }) => {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
    const page = await ctx.newPage();
    await addMockInitScript(page, installTauriMock, 'dashboard');
    // useLocalStorage JSON-parses its value, so the mode must be stored as JSON.
    await page.addInitScript(() => localStorage.setItem('vox_sidebar_mode', JSON.stringify('rail')));
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expect(page.locator('svg[aria-label="Axis"]').first()).toBeVisible();
    await page.screenshot({ path: 'e2e/screens/_axis-brand-rail.png', fullPage: false });
    await ctx.close();
  });
});
