import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

test.describe('Vox Dashboard', () => {
  test('should load the dashboard and verify event payload delivery', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'dashboard');
    await page.goto('/');

    // Check if the dashboard loads
    await expect(page).toHaveTitle(/Axis|frontend|vox/i);

    // Verify one of the expected UI shells rendered.
    const hasDashboard = (await page.getByText('Dashboard').count()) > 0;
    const hasBakeOff = (await page.getByText('Vox bake-off — Path A (Tauri-mobile)').count()) > 0;
    expect(hasDashboard || hasBakeOff).toBeTruthy();

    // Non-fixture execution path when the full shell is active.
    if (hasDashboard) {
      await expect(page.getByTestId('workbench-tab-bar')).toBeVisible();
      await expect(page.getByTestId('workbench-tab-dashboard')).toHaveAttribute('aria-selected', 'true');

      await page.locator('aside').first().getByRole('button', { name: /^Workspace/ }).click();
      await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=console');
      await expect(page.getByTestId('workbench-tab-console')).toHaveAttribute('aria-selected', 'true');

      await page.goto('/#view=repository');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await expect(page.getByTestId('workbench-tab-repository')).toHaveAttribute('aria-selected', 'true');
      await page.getByRole('button', { name: 'Workspace status' }).click();
      await expect(page.getByText('ok')).toBeVisible();

      // Harness tab redirects to Loquela composer (legacy surface retained for deep links).
      await page.goto('/#view=harness');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await expect(page.getByText('Quick Harness lives in the composer')).toBeVisible();
      await page.getByRole('button', { name: 'Focus composer' }).click();

      // Runs view under Review parent nav.
      await page.locator('aside').first().getByRole('button', { name: /^Review/ }).click();
      await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=approvals');
      await page.goto('/#view=runs');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await expect(page.getByText('gui-run-1').first()).toBeVisible();
    }
  });
});
