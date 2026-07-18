import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

test.describe('Console workbench tab', () => {
  test('console tab shows terminal or orchestrator error', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'console');
    await page.goto('/#view=console');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await expect(page.getByTestId('workbench-tab-console')).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByTestId('console-root')).toBeVisible();
  });
});
