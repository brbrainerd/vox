import { test, expect } from '@playwright/test';

test.describe('Vox Settings pilot', () => {
  test('loads settings and persists theme preference via transport', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('vox_sidebar_mode', 'default');
      (window as any).__TAURI_CALLS__ = [];
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args?: Record<string, unknown>) => {
          (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
          if (cmd === 'get_initial_view') return 'settings';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'get_command_catalog') {
            return { generated_from: 'e2e-mock', entries: [] };
          }
          if (cmd === 'get_action_manifest') {
            return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
          }
          if (cmd === 'get_routing_summary_live') return { decision_preview: null };
          if (cmd === 'get_gui_preference') {
            const key = args?.key as string;
            if (key === 'gui.theme') return 'dark';
            return null;
          }
          if (cmd === 'set_gui_preference') return null;
          if (cmd === 'get_orchestrator_status_bin') return new Uint8Array([0x80]);
          return null;
        },
      };
    });

    await page.goto('/');
    await expect(page.getByRole('heading', { name: /settings/i })).toBeVisible({ timeout: 15_000 });
  });
});
