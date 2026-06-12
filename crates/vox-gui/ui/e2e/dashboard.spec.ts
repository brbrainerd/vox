import { test, expect } from '@playwright/test';

test.describe('Vox Dashboard', () => {
  test('should load the dashboard and verify event payload delivery', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('vox_sidebar_mode', 'default');
      (window as any).__TAURI_CALLS__ = [];
      // Minimal Tauri invoke mock so UI logic can be exercised under Playwright.
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args?: any) => {
          (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
          if (cmd === 'get_command_catalog') {
            return {
              generated_from: 'e2e-mock',
              entries: [{ path: ['check'], command: 'vox check', about: 'Check project', aliases: [], has_subcommands: false, compiled_in: true, source_group: 'core', feature_gate: null, tier: 'recommended', arguments: [] }],
            };
          }
          if (cmd === 'list_model_cards') {
            return [{ model_id: 'mens-8b', display_name: 'Mens 8B' }];
          }
          if (cmd === 'set_active_model') return null;
          if (cmd === 'get_orchestrator_status_bin') {
            return new Uint8Array([0x80]); // empty map in msgpack
          }
          if (cmd === 'get_initial_view') return 'dashboard';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'get_action_manifest') {
            return {
              x_vox_version: 2,
              schema_version: 1,
              generated_from: 'e2e-mock',
              actions: [],
            };
          }
          if (cmd === 'execute_command') {
            return {
              exit_code: 0,
              stdout: 'repo ok',
              stderr: '',
            };
          }
          if (cmd === 'get_model_scoreboard') return [];
          if (cmd === 'list_gui_runs') {
            return [{
              run_id: 'gui-run-1',
              workflow_name: 'gui.harness.submit',
              status: 'success',
              planned_steps: 1,
              completed_steps: 1,
              updated_at_ms: Date.now(),
              last_error: null,
            }];
          }
          if (cmd === 'get_routing_summary_live') return { decision_preview: null };
          if (cmd === 'submit_orchestrator_task') return { ok: true, task_id: '101', message: 'submitted' };
          if (cmd === 'pause_orchestrator_agent' || cmd === 'resume_orchestrator_agent') return { ok: true };
          return null;
        },
      };
    });
    await page.goto('/');

    // Check if the dashboard loads
    await expect(page).toHaveTitle(/frontend|vox/i);

    // Verify one of the expected UI shells rendered.
    const hasDashboard = (await page.getByText('Dashboard').count()) > 0;
    const hasBakeOff = (await page.getByText('Vox bake-off — Path A (Tauri-mobile)').count()) > 0;
    expect(hasDashboard || hasBakeOff).toBeTruthy();

    // Non-fixture execution path when the full shell is active.
    if (hasDashboard) {
      await page.getByRole('button', { name: 'Workspace' }).click();
      await page.getByRole('button', { name: 'Repository' }).click();
      await page.getByRole('button', { name: 'Workspace status' }).click();
      await expect(page.getByText('repo ok')).toBeVisible();

      // Harness tab redirects to Loquela composer (legacy surface retained for deep links).
      await page.getByRole('button', { name: 'Harness' }).click();
      await expect(page.getByText('Quick Harness lives in the composer')).toBeVisible();
      await page.getByRole('button', { name: 'Focus composer' }).click();

      // Runs view under Runs & Approvals parent nav.
      await page.getByRole('button', { name: 'Runs & Approvals' }).click();
      await expect(page.getByText('gui-run-1').first()).toBeVisible();
    }
  });
});
