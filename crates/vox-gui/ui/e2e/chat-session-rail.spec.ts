import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

/**
 * Chat session rail collapse/expand (Phase 3.2).
 *
 * Run: pnpm exec playwright test e2e/chat-session-rail.spec.ts --project=chromium
 */
test.describe('Chat session rail', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(installOperatorShellMock, { initialView: 'chat' });
    await page.setViewportSize({ width: 1400, height: 900 });
  });

  test('session rail collapses and expands', async ({ page }) => {
    await page.goto('/#view=chat');
    await page.waitForSelector('nav', { timeout: 15_000 });

    const rail = page.getByTestId('chat-session-rail');
    await expect(rail).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Sessions' })).toBeVisible();
    await expect(page.getByRole('tab', { name: /Mock chat/i })).toBeVisible();

    await page.getByRole('button', { name: 'Collapse sessions rail' }).click();
    await expect(rail).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Sessions' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Expand sessions rail' })).toBeVisible();

    await page.getByRole('button', { name: 'Expand sessions rail' }).click();
    await expect(page.getByRole('heading', { name: 'Sessions' })).toBeVisible();
    await expect(page.getByRole('tab', { name: /Mock chat/i })).toBeVisible();
  });
});
