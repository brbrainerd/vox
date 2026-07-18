/** Approvals approve/reject interaction flow against the stateful tauriMock. */
import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

test.describe('Approvals interactions', () => {
  test.beforeEach(async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'approvals');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expect(page.getByText('#AP-000001')).toBeVisible();
  });

  test('approve resolves, toasts, and removes the row', async ({ page }) => {
    await page.getByRole('button', { name: 'Approve rm -rf build' }).click();
    await expect(page.getByRole('status').getByText('Approved')).toBeVisible();
    await expect(page.getByText('#AP-000001')).toHaveCount(0);
    const resolveCalls = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.filter(
        (c: any) => c.cmd === 'invoke_mcp_tool' && String(c.args?.tool ?? '').includes('resolve_approval'),
      ),
    );
    expect(resolveCalls).toHaveLength(1);
    expect(resolveCalls[0].args.args).toMatchObject({ approval_id: 'AP-000001', outcome: 'approved' });
  });

  test('reject resolves, toasts, and removes the row', async ({ page }) => {
    await page.getByRole('button', { name: 'Reject rm -rf build' }).click();
    await expect(page.getByRole('status').getByText('Rejected')).toBeVisible();
    await expect(page.getByText('#AP-000001')).toHaveCount(0);
    const resolveCalls = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.filter(
        (c: any) => c.cmd === 'invoke_mcp_tool' && String(c.args?.tool ?? '').includes('resolve_approval'),
      ),
    );
    expect(resolveCalls[0].args.args).toMatchObject({ approval_id: 'AP-000001', outcome: 'rejected' });
  });
});
