import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';

test.describe('Console workbench tab', () => {
  test('console tab shows terminal or orchestrator error', async ({ page }) => {
    await page.addInitScript(installTauriMock, 'console');
    await page.goto('/#view=console');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await expect(page.getByTestId('workbench-tab-console')).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByTestId('console-root')).toBeVisible();
  });
});
