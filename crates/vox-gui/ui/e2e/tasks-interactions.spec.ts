/** Tasks (hopper to-do) create -> reprioritize -> cancel against the stateful tauriMock. */
import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

test('create -> reprioritize -> cancel round-trip', async ({ page }) => {
  await addMockInitScript(page, installTauriMock, 'tasks');
  await page.goto('/');
  await page.waitForSelector('nav', { timeout: 15_000 });

  // Create via the composer (Enter submits; TaskComposer.tsx).
  const composer = page.getByLabel('Add a task');
  await composer.fill('Ship the release notes');
  await composer.press('Enter');
  await expect(page.getByText('Ship the release notes')).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.some(
        (c: any) => c.cmd === 'hopper_submit' && c.args?.intent === 'Ship the release notes',
      ),
    ),
  ).toBe(true);

  // Reprioritize to Urgent via the row's priority select (values: 2/1/0).
  const row = page.locator('tr', { hasText: 'Ship the release notes' });
  const prioritySelect = row.getByRole('combobox').first();
  await prioritySelect.selectOption('2');
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'hopper_reprioritize').length,
      ),
    )
    .toBe(1);
  await expect(prioritySelect).toHaveValue('2'); // survives the post-action refresh (stateful mock)

  // Cancel removes the row.
  await row.getByTitle('Cancel task').click();
  await expect(page.getByText('Ship the release notes')).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.some(
        (c: any) => c.cmd === 'hopper_cancel' && c.args?.itemId === 'hop-1',
      ),
    ),
  ).toBe(true);
});
