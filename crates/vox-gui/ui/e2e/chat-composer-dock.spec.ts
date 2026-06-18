import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

/**
 * Loquela composer dock placement (Appendix D).
 *
 * Composer lives only on the Chat surface; no global Loquela dock on dashboard or elsewhere.
 *
 * Run: pnpm exec playwright test e2e/chat-composer-dock.spec.ts --project=chromium
 */
test.describe('Loquela composer dock placement', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(installOperatorShellMock, { initialView: 'dashboard' });
    await page.setViewportSize({ width: 1400, height: 900 });
  });

  test('console view omits global Loquela composer dock', async ({ page }) => {
    await page.goto('/#view=console');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await expect(page.getByTestId('loquela-dock')).toHaveCount(0);
    await expect(page.getByTestId('loquela-composer')).toHaveCount(0);
    await expect(page.locator('#loquela-composer')).toHaveCount(0);
  });

  test('chat view shows composer when chatDocked is false', async ({ page }) => {
    await page.goto('/#view=chat');
    await page.waitForSelector('nav', { timeout: 15_000 });

    // chatDocked=false → global shell dock hidden; composer lives on Chat surface or inline.
    await expect(page.getByTestId('loquela-dock')).toHaveCount(0);

    const composer = page.getByTestId('loquela-composer');
    await expect(composer).toBeVisible();
    await expect(composer.locator('#loquela-composer')).toBeVisible();
    await expect(composer.getByPlaceholder(/describe a task/i)).toBeVisible();
  });

  test('dashboard omits global Loquela dock and composer', async ({ page }) => {
    await page.goto('/#view=dashboard');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await expect(page.getByTestId('loquela-dock')).toHaveCount(0);
    await expect(page.getByTestId('loquela-composer')).toHaveCount(0);
    await expect(page.getByTestId('open-chat-cta')).toBeVisible();
  });
});
