/**
 * IPC-failure degradation audit. Runs in the DEFAULT Playwright sweep (no env
 * gate), i.e. inside the loud post-merge CI step: when a surface's data IPC
 * throws, the app shell must stay up and the surface must show visible error
 * UI — never a blank panel, never an uncaught rejection.
 */
import { test, expect } from '@playwright/test';
import { installErrorStateMock } from './lib/tauriMockVariants';
import { addMockInitScript } from './lib/tauriMockShared';

// TODO(phase3-followup): tasks — useAttentionInbox swallows hopper_list
// rejections with `.catch(() => [])` (useAttentionInbox.ts:34) and TasksView's
// setError path never runs in shared-attention mode, so the surface renders an
// EMPTY list (no affordance) on IPC failure. Re-add once the inbox surfaces
// fetch errors.
// TODO(phase3-followup): dashboard — consumes only bootstrap orchestrator
// status; its widgets + useAgentApprovals swallow errors, so the error mock
// exercises no dashboard failure path. Re-add once dashboard has a real
// data-error affordance.
const KEY_SURFACES = ['chat', 'runs', 'approvals', 'models'] as const;

test.describe('IPC-failure degradation', () => {
  for (const view of KEY_SURFACES) {
    test(`${view} degrades visibly when data IPC throws`, async ({ browser }) => {
      const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      const page = await ctx.newPage();
      const pageErrors: string[] = [];
      page.on('pageerror', (e) => pageErrors.push(e.message));
      await addMockInitScript(page, installErrorStateMock, view);
      await page.goto('/');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await expect(page.getByTestId('workbench-tab-bar')).toBeVisible();
      // Short bounded settle so async uncaught rejections have time to surface
      // before assertion 1 (the affordance check below is auto-retrying and
      // needs no sleep).
      await page.waitForTimeout(1200);

      // 1. Failures are HANDLED — no uncaught exceptions/rejections.
      expect(pageErrors, `[${view}] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
      // 2. Not blank: the page body renders substantive content (shell + surface chrome).
      const bodyText = (await page.locator('body').innerText()).trim();
      expect(bodyText.length, `[${view}] rendered blank on IPC failure`).toBeGreaterThan(0);
      // 3. Visible error affordance attributable to THIS surface. Scoped to the
      // workbench main panel (surface content inside SurfaceErrorBoundary,
      // AppShell.tsx:143-148) so static chrome elsewhere can't satisfy it, and
      // excluding the global 'Chat sessions' toast (App.tsx:396-406 fires it on
      // EVERY view because chat_list_sessions is in ERROR_CMDS) so the count
      // can actually be 0 on a blank panel. Auto-retrying: no fixed-sleep race.
      const mainPanel = page.getByTestId('surface-scroll-host');
      const toastItems =
        view === 'chat'
          ? page.getByRole('status').locator('.pointer-events-auto')
          : page.getByRole('status').locator('.pointer-events-auto').filter({ hasNotText: /chat sessions/i });
      const alerts = mainPanel.getByRole('alert');
      const errorCopy = mainPanel.getByText(/error|failed|unavailable|could not|retry/i);
      await expect
        .poll(
          async () =>
            (await toastItems.count()) + (await alerts.count()) + (await errorCopy.count()),
          { timeout: 10_000, message: `[${view}] no visible error affordance` },
        )
        .toBeGreaterThan(0);
      await ctx.close();
    });
  }
});
