import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

/**
 * Command palette — federated policy hit navigates via keyboard (Phase 3.2 step 4).
 *
 * Run: pnpm exec playwright test e2e/palette-search-navigate.spec.ts --project=chromium
 */
test.describe('Command palette search navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(installOperatorShellMock, { initialView: 'dashboard' });
    await page.setViewportSize({ width: 1400, height: 900 });
  });

  test('keyboard navigate on policy row opens policies surface', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await page.keyboard.press('Control+k');
    const paletteInput = page.getByPlaceholder(/search surfaces, commands/i);
    await expect(paletteInput).toBeVisible();

    await paletteInput.fill('fmt');
    await expect(page.getByText('fmt.rust')).toBeVisible({ timeout: 10_000 });

    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');

    await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=policies');
    await expect(page.getByLabel('Policy tree')).toBeVisible();
  });
});
