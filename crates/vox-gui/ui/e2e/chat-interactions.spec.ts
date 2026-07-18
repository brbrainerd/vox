/**
 * Chat submit -> stream -> persist against the tauriMock, driving the
 * `vox://agent-events` stream with the __TAURI_EMIT__ helper (tauriMockShared).
 * Guards two distinct contracts:
 *  - frontend double-dispatch (e.g. duplicate Enter handling): exactly ONE
 *    `submit_orchestrator_task` per submit;
 *  - the C2 `already_submitted` contract: the persisted user row must carry
 *    `already_submitted: true` — that flag is what stops the Rust backend's
 *    secretary re-submit (the re-submit itself happens daemon-side and is
 *    invisible to this mock, so the flag IS the observable C2 guard here).
 */
import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

test('submit streams tokens into the transcript and persists the assistant row', async ({ page }) => {
  await addMockInitScript(page, installTauriMock, 'chat');
  await page.goto('/');
  await page.waitForSelector('nav', { timeout: 15_000 });

  const composer = page.getByLabel('Task composer');
  await composer.fill('Summarize the repository layout');
  await composer.press('Enter');

  // Optimistic user bubble + exactly one dispatch (guards FRONTEND
  // double-dispatch, e.g. duplicate Enter handling — NOT C2; the C2
  // re-submit is daemon-side and never crosses the Tauri invoke boundary).
  await expect(page.getByText('Summarize the repository layout')).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'submit_orchestrator_task').length,
      ),
    )
    .toBe(1);
  // User row persisted on submit, carrying the C2 contract flag: Phase 1
  // makes App.tsx send already_submitted: true, which is exactly what stops
  // the backend secretary from re-submitting — the only mock-visible C2 guard.
  expect(
    await page.evaluate(() =>
      (window as any).__TAURI_CALLS__.some(
        (c: any) =>
          c.cmd === 'chat_append_message' &&
          c.args?.input?.role === 'user' &&
          c.args?.input?.content === 'Summarize the repository layout' &&
          c.args?.input?.already_submitted === true,
      ),
    ),
  ).toBe(true);

  // Drive the stream for task 101 (mock submit_orchestrator_task returns task_id '101').
  const emit = (kind: Record<string, unknown>, id: number) =>
    page.evaluate(
      ([k, i]) =>
        (window as any).__TAURI_EMIT__('vox://agent-events', {
          id: i,
          timestamp_ms: Date.now(),
          kind: k,
        }),
      [kind, id] as const,
    );
  await emit({ type: 'task_started', agent_id: 7, task_id: 101 }, 1);
  await emit({ type: 'token_streamed', agent_id: 7, text: 'Hello from the mock stream.' }, 2);
  // Scoped to the message bubble itself (id="msg-<id>"): the same streamed
  // text also renders as the execution rail's "agent stream item" button,
  // which lives inside the same role="log" region, so even scoping to the
  // log is a Playwright strict-mode violation (matches 2 elements).
  const messageBubble = page.locator('[id^="msg-"]').filter({ hasText: 'Hello from the mock stream.' });
  await expect(messageBubble).toBeVisible();
  await emit({ type: 'task_completed', task_id: 101 }, 3);

  // Completed assistant bubble persists exactly once, tagged with the task id.
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__TAURI_CALLS__.filter(
          (c: any) => c.cmd === 'chat_append_message' && c.args?.input?.role === 'assistant',
        ).length,
      ),
    )
    .toBe(1);
  const persisted = await page.evaluate(() =>
    (window as any).__TAURI_CALLS__.find(
      (c: any) => c.cmd === 'chat_append_message' && c.args?.input?.role === 'assistant',
    ),
  );
  expect(persisted.args.input.content).toContain('Hello from the mock stream.');
  expect(String(persisted.args.input.task_id)).toBe('101');
});
