/**
 * Multi-state visual audit — empty and error state screenshots for 10 key surfaces.
 *
 * NOT run on standard CI. Opt in with:
 *   VOX_VARIANT_SCREENSHOTS=1 pnpm exec playwright test screenshots-variants.spec.ts --project=chromium
 *
 * A post-merge CI step opts in via the workflow env var (see Task 11) so the
 * error-state assertions below still get exercised on a schedule even though
 * this spec self-skips by default in the standard PR-gating sweep.
 *
 * Output:
 *   e2e/screens/<view>-empty.png  — surface with all list/detail IPC returning empty
 *   e2e/screens/<view>-error.png  — surface with key data-fetch IPC throwing errors
 */
import { test, expect, type Page } from '@playwright/test';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { installEmptyStateMock, installErrorStateMock } from './lib/tauriMockVariants';
import { addMockInitScript } from './lib/tauriMockShared';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SCREENS_DIR = join(__dirname, 'screens');

const RUN_VARIANTS = !!process.env['VOX_VARIANT_SCREENSHOTS'];

const KEY_SURFACES = [
  'dashboard', 'chat', 'runs', 'approvals', 'models',
  'memory', 'vox-search', 'policies', 'gamify', 'settings',
] as const;

const BENIGN_CONSOLE: string[] = ['favicon'];

function captureErrors(page: Page) {
  const consoleErrors: string[] = [];
  const pageErrors: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(`${m.text()} ${m.location()?.url ?? ''}`);
  });
  page.on('pageerror', (e) => pageErrors.push(e.message));
  return { consoleErrors, pageErrors };
}

const meaningfulConsole = (errs: string[]): string[] =>
  errs.filter((t) => !BENIGN_CONSOLE.some((b) => t.includes(b)));

test.describe('GUI visual audit — empty states', () => {
  for (const view of KEY_SURFACES) {
    test(`capture ${view}-empty`, async ({ browser }) => {
      test.skip(!RUN_VARIANTS, 'Set VOX_VARIANT_SCREENSHOTS=1 to run variant screenshots');
      const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      const page = await ctx.newPage();
      const { consoleErrors, pageErrors } = captureErrors(page);
      await addMockInitScript(page, installEmptyStateMock, view);
      await page.goto('/');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await page.waitForTimeout(1600);
      await page.screenshot({ path: join(SCREENS_DIR, `${view}-empty.png`), fullPage: true });

      // Empty responses must NOT crash the error boundary — surfaces should show empty-state UI.
      await expect(
        page.locator('[data-surface-error]'),
        `[${view}-empty] crashed into its error boundary on empty data`,
      ).toHaveCount(0);
      expect(pageErrors, `[${view}-empty] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
      const meaningful = meaningfulConsole(consoleErrors);
      expect(meaningful, `[${view}-empty] console errors:\n${meaningful.join('\n')}`).toEqual([]);
      await ctx.close();
    });
  }
});

test.describe('GUI visual audit — error states', () => {
  for (const view of KEY_SURFACES) {
    test(`capture ${view}-error`, async ({ browser }) => {
      test.skip(!RUN_VARIANTS, 'Set VOX_VARIANT_SCREENSHOTS=1 to run variant screenshots');
      const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
      const page = await ctx.newPage();
      const { pageErrors } = captureErrors(page);
      await addMockInitScript(page, installErrorStateMock, view);
      await page.goto('/');
      await page.waitForSelector('nav', { timeout: 15_000 });
      await page.waitForTimeout(1600);
      await page.screenshot({ path: join(SCREENS_DIR, `${view}-error.png`), fullPage: true });

      // Error-state surfaces MAY render an error boundary — both are valid.
      // What we audit: does the error UI look reasonable (not blank, not garbled)?
      expect(pageErrors, `[${view}-error] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);

      // Visible degradation, not a blank panel: at least one toast item
      // attributable to THIS surface (the global 'Chat sessions' toast fires
      // on every view and must not vacuously satisfy other surfaces), a
      // role=alert region in the main panel, or visible error copy in the
      // main panel. Auto-retrying: toast/alert timing varies on CI runners.
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
          {
            timeout: 10_000,
            message: `[${view}-error] no visible toast/alert/error copy — surface degraded to a blank panel`,
          },
        )
        .toBeGreaterThan(0);

      await ctx.close();
    });
  }
});
