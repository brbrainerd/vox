/**
 * DockShell layout persistence — mocks gui.layout.v1 via Tauri preference IPC.
 *
 * Run: pnpm exec playwright test dock-layout.spec.ts --project=chromium
 */
import { test, expect } from '@playwright/test';
import { installOperatorShellMock } from './lib/operatorShellMock';

const LAYOUT_KEY = 'gui.layout.v1';
const PANEL_ID = 'main-surface';

/**
 * Minimal dockview serialized layout (branch root + single panel group).
 * Matches DockShell defaults: panel id `main-surface`, component `panel`, title `agents`.
 */
const MINIMAL_DOCK_LAYOUT = JSON.stringify({
  grid: {
    root: {
      type: 'branch',
      data: [
        {
          type: 'leaf',
          data: {
            views: [PANEL_ID],
            activeView: PANEL_ID,
            id: '1',
          },
          size: 100,
        },
      ],
      size: 100,
    },
    width: 800,
    height: 600,
    orientation: 0,
  },
  panels: {
    [PANEL_ID]: {
      id: PANEL_ID,
      contentComponent: 'panel',
      title: 'agents',
    },
  },
  activeGroup: '1',
});

test.describe('DockShell layout persistence', () => {
  test('restores persisted layout from gui.layout.v1', async ({ page }) => {
    await page.addInitScript(installOperatorShellMock, {
      initialView: 'dashboard',
      seedGuiPrefs: { [LAYOUT_KEY]: MINIMAL_DOCK_LAYOUT },
    });

    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await expect(page.locator('.dockview-theme-vox')).toBeVisible();
    await expect(page.locator('.dv-tabs-and-actions-container')).toBeVisible();
    await expect(page.locator('.dv-tab')).toHaveCount(1);
    await expect(page.locator('.dv-tab')).toContainText('agents');

    await page.waitForFunction(
      (key) => {
        const calls = (window as any).__TAURI_CALLS__ ?? [];
        return calls.some((c: { cmd: string; args?: { key?: string } }) => c.cmd === 'get_gui_preference' && c.args?.key === key);
      },
      LAYOUT_KEY,
      { timeout: 10_000 },
    );

    const seededLayout = await page.evaluate((key) => {
      const prefs = (window as any).__GUI_PREFS__ as Record<string, string>;
      return prefs[key] ?? null;
    }, LAYOUT_KEY);

    expect(seededLayout).toBe(MINIMAL_DOCK_LAYOUT);
    const parsed = JSON.parse(seededLayout);
    expect(parsed.panels?.[PANEL_ID]).toMatchObject({
      id: PANEL_ID,
      contentComponent: 'panel',
      title: 'agents',
    });
    expect(parsed.grid?.root?.type).toBe('branch');
  });
});
