/**
 * Session rail rename/archive flows (Phase 2 wiring) against the stateful
 * tauriMock: outgoing IPC contract + product-rendered rail state after the
 * handler's loadSessions() refetch of the stateful mock.
 */
import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

test.describe('Session rail actions', () => {
  test.beforeEach(async ({ page }) => {
    await addMockInitScript(page, installTauriMock, 'chat');
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });
    await expect(page.getByTestId('chat-session-rail')).toBeVisible();
    await expect(page.getByRole('tab', { name: /Mock chat/i })).toBeVisible();
  });

  test('rename flows through chat_rename_session and re-renders the new title', async ({ page }) => {
    await page.getByRole('button', { name: 'Session actions for Mock chat' }).click();
    await page.getByRole('menuitem', { name: /rename/i }).click();
    const input = page.getByRole('textbox', { name: /new session title/i });
    await input.fill('Renamed chat');
    await input.press('Enter');

    await expect
      .poll(
        () =>
          page.evaluate(() =>
            (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'chat_rename_session').length,
          ),
        { timeout: 10_000 },
      )
      .toBe(1);
    const call = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.find((c: any) => c.cmd === 'chat_rename_session'),
    );
    expect(call.args).toMatchObject({ sessionId: 'mock-session-1', title: 'Renamed chat' });
    await expect(page.getByRole('tab', { name: /Renamed chat/i })).toBeVisible();
  });

  test('archive flows through chat_archive_session and removes the session tab', async ({ page }) => {
    await page.getByRole('button', { name: 'Session actions for Mock chat' }).click();
    await page.getByRole('menuitem', { name: /archive/i }).click();

    await expect
      .poll(
        () =>
          page.evaluate(() =>
            (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'chat_archive_session').length,
          ),
        { timeout: 10_000 },
      )
      .toBe(1);
    const call = await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.find((c: any) => c.cmd === 'chat_archive_session'),
    );
    expect(call.args).toMatchObject({ sessionId: 'mock-session-1' });
    await expect(page.getByRole('tab', { name: /Mock chat/i })).toHaveCount(0);
  });
});
