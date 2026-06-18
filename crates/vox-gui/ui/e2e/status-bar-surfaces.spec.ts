import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

/**
 * StatusBar visibility on operator surfaces (Phase 1.4).
 *
 * Run: pnpm exec playwright test e2e/status-bar-surfaces.spec.ts --project=chromium
 */
test.describe('StatusBar on operator surfaces', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(installOperatorShellMock, { initialView: 'dashboard' });
  });

  async function expectStatusBarVisible(page: import('@playwright/test').Page) {
    await expect(page.getByTestId('status-bar')).toBeVisible();
    await expect(page.getByRole('status', { name: /operator status/i })).toBeVisible();
  }

  test('status bar visible on dashboard, chat, policies, and console', async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 900 });

    await page.goto('/#view=dashboard');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expectStatusBarVisible(page);

    await page.goto('/#view=chat');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expectStatusBarVisible(page);

    await page.goto('/#view=policies');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expectStatusBarVisible(page);

    await page.goto('/#view=console');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expectStatusBarVisible(page);
  });

  test('status bar visible when navigating via sidebar', async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 900 });

    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expectStatusBarVisible(page);

    const sidebarNav = page.getByRole('navigation');
    await sidebarNav.getByRole('button', { name: 'Chat', exact: true }).click();
    await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=chat');
    await expectStatusBarVisible(page);

    await sidebarNav.getByRole('button', { name: 'Workspace', exact: true }).click();
    await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=console');
    await expectStatusBarVisible(page);
  });
});
