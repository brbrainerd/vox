import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

/**
 * ⌘K quick action "Submit new task…" opens Chat and focuses composer (Phase 3.2).
 *
 * Run: pnpm exec playwright test e2e/submit-task-palette.spec.ts --project=chromium
 */
test.describe('Submit new task palette action', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(installOperatorShellMock, { initialView: 'dashboard' });
    await page.setViewportSize({ width: 1400, height: 900 });
  });

  test('palette submit new task navigates to chat with composer', async ({ page }) => {
    await page.goto('/#view=dashboard');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await page.keyboard.press('Control+k');
    await expect(page.getByPlaceholder(/search commands/i)).toBeVisible();

    await page.getByRole('button', { name: /Submit new task/i }).click();

    await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=chat');
    await expect(page.getByTestId('loquela-composer')).toBeVisible();
    await expect(page.getByPlaceholder(/describe a task/i)).toBeVisible();
  });
});
