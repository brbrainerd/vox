import { test, expect } from '@playwright/test';

test.describe('Browser surface', () => {
  test('loads Browser panel and shows preview controls', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('vox_active_view', 'browser');
      localStorage.setItem('vox_sidebar_mode', 'default');
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string) => {
          if (cmd === 'get_initial_view') return 'browser';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test' };
          if (cmd === 'get_command_catalog') return { generated_from: 'e2e', entries: [] };
          if (cmd === 'get_action_manifest') {
            return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e', actions: [] };
          }
          if (cmd === 'get_orchestrator_status_bin') return new Uint8Array([0x80]);
          if (cmd === 'preview_status') {
            return { active: false, url: null, app_dir: null, source: 'none' };
          }
          if (cmd === 'browser_session_status') {
            return { page_id: null, headless: true, action_log: [] };
          }
          if (cmd === 'browser_list_pages') return [];
          if (cmd === 'browser_page_info') {
            return {
              page_id: 'page-test',
              url: 'https://example.com',
              title: 'Example Domain',
              can_go_back: false,
              can_go_forward: false,
            };
          }
          if (cmd === 'list_model_cards') return [];
          return null;
        },
      };
    });

    await page.goto('/');
    await expect(page.getByRole('heading', { name: 'Browser' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Start preview' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Agent live view' })).toBeVisible();
    await page.getByRole('button', { name: 'Agent live view' }).click();
    await expect(page.getByRole('button', { name: 'Back' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Forward' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Reload' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Stop' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Go' })).toBeVisible();
  });
});
