/**
 * Workbench tab navigation — dynamic sweep of every registered leaf surface.
 *
 * Run: pnpm exec playwright test e2e/workbench-tabs.spec.ts --project=chromium
 */
import { test, expect } from '@playwright/test';
import { SURFACE_REGISTRY } from '../src/generated/surfaceRegistry.generated';
import {
  DEFAULT_CHILD_BY_PARENT,
  TOP_LEVEL_VIEWS,
  tabLabelFor,
} from '../src/lib/navigation';
import { sidebarParentLabel } from '../src/lib/lexicon';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

const LEAF_VIEWS: string[] = Array.from(
  new Set(
    SURFACE_REGISTRY.filter((e) => e.viewKey && e.tier !== 'none').map((e) => e.viewKey as string),
  ),
).sort();

const SIDEBAR_PARENTS = TOP_LEVEL_VIEWS.filter((k) => k !== 'settings');

async function expectActiveWorkbenchTab(page: import('@playwright/test').Page, viewKey: string) {
  const tabBar = page.getByTestId('workbench-tab-bar');
  await expect(tabBar).toBeVisible();
  const tab = tabBar.getByTestId(`workbench-tab-${viewKey}`);
  await expect(tab).toBeVisible();
  await expect(tab).toHaveAttribute('aria-selected', 'true');
}

test.describe('Workbench tabs — hash navigation', () => {
  for (const viewKey of LEAF_VIEWS) {
    test(`#view=${viewKey} selects workbench tab`, async ({ page }) => {
      await addMockInitScript(page, installTauriMock, viewKey);
      await page.goto(`/#view=${encodeURIComponent(viewKey)}`);
      await page.waitForSelector('nav', { timeout: 15_000 });
      await expectActiveWorkbenchTab(page, viewKey);
      await expect(page.locator('[data-surface-error]')).toHaveCount(0);
      await expect
        .poll(async () => page.evaluate(() => window.location.hash))
        .toContain(`view=${encodeURIComponent(viewKey)}`);
    });
  }
});

test.describe('Workbench tabs — sidebar parents', () => {
  test.describe.configure({ mode: 'serial' });

  for (const parentKey of SIDEBAR_PARENTS) {
    const defaultChild = DEFAULT_CHILD_BY_PARENT[parentKey] ?? parentKey;
    const sidebarLabel = sidebarParentLabel(parentKey);

    test(`sidebar "${sidebarLabel}" opens default tab ${defaultChild}`, async ({ page }) => {
      await addMockInitScript(page, installTauriMock, 'dashboard');
      await page.goto('/');
      await page.waitForSelector('aside nav', { timeout: 15_000 });

      const sidebar = page.locator('aside').first();
      await sidebar.getByRole('button', { name: new RegExp(`^${sidebarLabel}`) }).click();

      await expect
        .poll(async () => page.evaluate(() => window.location.hash))
        .toContain(`view=${encodeURIComponent(defaultChild)}`);
      await expectActiveWorkbenchTab(page, defaultChild);
      await expect(page.locator('[data-surface-error]')).toHaveCount(0);
    });
  }

  test('footer Settings opens settings tab', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'dashboard');
    await page.goto('/');
    await page.waitForSelector('aside nav', { timeout: 15_000 });
    await page.locator('aside').first().getByRole('button', { name: /^Settings/ }).scrollIntoViewIfNeeded();
    await page.locator('aside').first().getByRole('button', { name: /^Settings/ }).click();
    await expectActiveWorkbenchTab(page, 'settings');
  });

  test('footer Coverage opens coverage tab', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'dashboard');
    await page.goto('/');
    await page.waitForSelector('aside nav', { timeout: 15_000 });
    await page.locator('aside').first().getByRole('button', { name: /^Coverage/ }).scrollIntoViewIfNeeded();
    await page.locator('aside').first().getByRole('button', { name: /^Coverage/ }).click();
    await expectActiveWorkbenchTab(page, 'coverage');
  });
});

test.describe('Workbench tabs — tab bar interactions', () => {
  test('switching tabs updates hash and aria-selected', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'dashboard');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    const tabBar = page.getByTestId('workbench-tab-bar');
    await tabBar.getByRole('tab', { name: tabLabelFor('chat') }).click();
    await expectActiveWorkbenchTab(page, 'chat');
    await expect.poll(async () => page.evaluate(() => window.location.hash)).toContain('view=chat');

    await tabBar.getByRole('tab', { name: tabLabelFor('dashboard') }).click();
    await expectActiveWorkbenchTab(page, 'dashboard');
  });

  test('closing a non-pinned tab removes it from the tab bar', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'console');
    await page.goto('/#view=console');
    await page.waitForSelector('nav', { timeout: 15_000 });

    const tabBar = page.getByTestId('workbench-tab-bar');
    await expect(tabBar.getByTestId('workbench-tab-console')).toBeVisible();
    await tabBar.getByTestId('workbench-tab-close-console').click();
    await expect(tabBar.getByTestId('workbench-tab-console')).toHaveCount(0);
  });

  test('chat tab is pinned and has no close button', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'dashboard');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    const tabBar = page.getByTestId('workbench-tab-bar');
    await expect(tabBar.getByTestId('workbench-tab-chat')).toBeVisible();
    await expect(tabBar.getByTestId('workbench-tab-close-chat')).toHaveCount(0);
  });

  test('help omnibar search opens doc reader tab', async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'dashboard');
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await page.keyboard.press('Control+k');
    const input = page.getByPlaceholder(/Search surfaces/i);
    await expect(input).toBeVisible();
    await input.fill('help cli');
    await expect(page.getByRole('button', { name: /CLI Reference/i })).toBeVisible({ timeout: 15_000 });
    await page.getByRole('button', { name: /CLI Reference/i }).click();

    await expect(page.getByTestId('doc-reader')).toBeVisible();
    await expect(
      tabBarDocTab(page, 'docs/src/reference/cli.md'),
    ).toHaveAttribute('aria-selected', 'true');
  });
});

/** Doc tabs use ids like `doc:docs/src/reference/cli.md`. */
function tabBarDocTab(page: import('@playwright/test').Page, docPath: string) {
  const id = `doc:${docPath.replace(/\\/g, '/')}`;
  return page.getByTestId('workbench-tab-bar').getByTestId(`workbench-tab-${id}`);
}

/** Surfaces with stable smoke testids for canary depth beyond tab selection. */
const SURFACE_SMOKE: Record<string, string> = {
  chat: 'chat-surface-layout',
  console: 'console-root',
};

test.describe('Workbench tabs — surface smoke', () => {
  for (const [viewKey, testId] of Object.entries(SURFACE_SMOKE)) {
    test(`#view=${viewKey} mounts ${testId}`, async ({ page }) => {
      await addMockInitScript(page, installTauriMock, viewKey);
      await page.goto(`/#view=${encodeURIComponent(viewKey)}`);
      await page.waitForSelector('nav', { timeout: 15_000 });
      await expectActiveWorkbenchTab(page, viewKey);
      await expect(page.getByTestId(testId)).toBeVisible();
    });
  }
});

test.describe('Workbench tabs — scroll host', () => {
  test('settings surface scrolls inside surface-scroll-viewport', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 420 });
    await addMockInitScript(page, installTauriMock, 'settings');
    await page.goto('/#view=settings');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expect(page.getByLabel('Search settings')).toBeVisible();

    const viewport = page.getByTestId('surface-scroll-viewport');
    await expect(viewport).toBeVisible();
    const scrollable = await viewport.evaluate((el) => el.scrollHeight > el.clientHeight);
    expect(scrollable).toBe(true);
    const before = await viewport.evaluate((el) => el.scrollTop);
    await viewport.evaluate((el) => {
      el.scrollTop += 400;
    });
    const after = await viewport.evaluate((el) => el.scrollTop);
    expect(after).toBeGreaterThan(before);
  });
});
