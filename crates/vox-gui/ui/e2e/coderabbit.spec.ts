import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

/**
 * CodeRabbit review panel — plan a date-scoped sweep and render the slice manifest,
 * overlaying run-state status onto planned chunks.
 *
 * Run: pnpm exec playwright test e2e/coderabbit.spec.ts --project=chromium
 */
test.describe('CodeRabbit review panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(installOperatorShellMock, { initialView: 'coderabbit' });
    await page.setViewportSize({ width: 1400, height: 900 });
  });

  test('plan renders slice rows with run-state status overlaid', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    // Panel mounts; token presence resolved from the (mocked) secrets-backed command.
    await expect(page.getByRole('heading', { name: 'CodeRabbit review' })).toBeVisible();
    await expect(page.getByText(/token: present/)).toBeVisible();

    // No plan until the user asks for one.
    await expect(page.getByText(/No plan yet/)).toBeVisible();

    await page.getByRole('button', { name: 'Plan sweep' }).click();

    // Slice rows from the planned manifest.
    await expect(page.getByText('crate_vox_db')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('06_docs_src')).toBeVisible();
    await expect(page.getByText('2 files')).toBeVisible();

    // run-state overlay: crate_vox_db is completed with PR #7; docs chunk is still planned.
    await expect(page.getByText('completed', { exact: true })).toBeVisible();
    await expect(page.getByText('#7')).toBeVisible();
    await expect(page.getByText('planned', { exact: true })).toBeVisible();

    // Run is enabled once a plan exists.
    await expect(page.getByRole('button', { name: 'Run', exact: true })).toBeEnabled();
  });
});
