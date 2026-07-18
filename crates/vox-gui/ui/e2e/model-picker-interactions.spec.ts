/**
 * Chat model picker apply flow against the stateful tauriMock.
 *
 * Ground truth (re-verified in Step 0 against the landed
 * ChatModelPicker.tsx/App.tsx — NOT the Phase 2 plan's literal sample):
 * the pick is lifted to App state via `onApplied` and threaded into the
 * NEXT chat submit as `submit_orchestrator_task`'s `model_override` input
 * field (TaskEnqueueHints.model_override). The picker itself never calls
 * `set_active_model` — that command only touches the GUI process and is
 * deliberately unused here (see ChatModelPicker.tsx's file-level comment
 * and ChatModelPicker.test.tsx's "never set_active_model" assertion).
 * The dropdown's accessible name is "Pick model for this chat", not
 * "Pick active model". The trigger's initial label is "model: auto-route" —
 * the picker's `activeModel` prop is fed from `App.tsx`'s orchestrator-status
 * query (`get_orchestrator_status_bin`, msgpack), which the tauriMock does not
 * populate, so `active_model` reads null here regardless of the JSON
 * `get_active_model`/`get_routing_summary_live` mock cases (out of this
 * task's scope to wire up).
 */
import { test, expect } from '@playwright/test';
import { installTauriMock } from './lib/tauriMock';
import { addMockInitScript } from './lib/tauriMockShared';

test('picking a model updates the trigger label and threads model_override into the next submit', async ({ page }) => {
  await addMockInitScript(page, installTauriMock, 'chat');
  await page.goto('/');
  await page.waitForSelector('nav', { timeout: 15_000 });

  // Trigger renders the active model from orchestrator status; the tauriMock
  // never populates it (see file header), so it starts as 'auto-route'.
  await page.getByRole('button', { name: 'model: auto-route' }).click();
  await expect(page.getByRole('listbox', { name: 'Pick model for this chat' })).toBeVisible();
  await page.getByRole('option', { name: 'sonnet-4-6' }).click();

  // Product-rendered result: onApplied updates the trigger label immediately,
  // with no invoke required for the pick itself.
  await expect(page.getByRole('button', { name: /model: sonnet-4-6/i })).toBeVisible();

  // The override is only observable on the wire once a chat message is sent.
  const composer = page.getByLabel('Task composer');
  await composer.fill('Use the picked model for this');
  await composer.press('Enter');

  await expect
    .poll(
      () =>
        page.evaluate(() =>
          (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'submit_orchestrator_task').length,
        ),
      { timeout: 10_000 },
    )
    .toBeGreaterThan(0);
  const call = await page.evaluate(() =>
    (window as any).__TAURI_CALLS__.find((c: any) => c.cmd === 'submit_orchestrator_task'),
  );
  expect(call.args.input).toMatchObject({ model_override: 'sonnet-4-6' });

  // The picker itself never calls set_active_model (Resolved decision "Item 4").
  const setActiveModelCalls = await page.evaluate(() =>
    (window as any).__TAURI_CALLS__.filter((c: any) => c.cmd === 'set_active_model').length,
  );
  expect(setActiveModelCalls).toBe(0);
});
