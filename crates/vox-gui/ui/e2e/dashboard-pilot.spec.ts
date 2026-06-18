import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

/**
 * Wave 1 dashboard pilot — hash deep-link round-trip (dashboard ↔ console).
 *
 * Run: pnpm exec playwright test e2e/dashboard-pilot.spec.ts --project=chromium
 */
test.describe('Dashboard pilot — hash navigation', () => {
  test('loads dashboard hash then navigates to console via workspace sidebar', async ({ page }) => {
    await page.addInitScript(installOperatorShellMock, { initialView: 'dashboard' });

    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    // Shell should render with dashboard as the active view (hash-synced on bootstrap).
    await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=dashboard');

    const hasDashboard = (await page.getByText('Dashboard').count()) > 0;
    const hasBakeOff = (await page.getByText('Vox bake-off — Path A (Tauri-mobile)').count()) > 0;
    expect(hasDashboard || hasBakeOff).toBeTruthy();

    // Workspace parent default child is console (see DEFAULT_CHILD_BY_PARENT in navigation.ts).
    await page.getByRole('button', { name: 'Workspace' }).click();

    await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=console');
    await expect(page).toHaveURL(/#view=console/);
  });
});
