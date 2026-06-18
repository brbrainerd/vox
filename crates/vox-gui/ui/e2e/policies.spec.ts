import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

/**
 * Policies two-rail layout — desktop viewport.
 *
 * Run: pnpm exec playwright test e2e/policies.spec.ts --project=chromium
 */
test.describe('Policies surface', () => {
  test('policy tree rail and detail pane visible on desktop', async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.addInitScript(installOperatorShellMock, { initialView: 'policies' });

    await page.goto('/#view=policies');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await expect(page.getByRole('navigation', { name: /policy tree/i })).toBeVisible();
    await expect(page.getByRole('region', { name: /policy detail/i })).toBeVisible();
  });
});
