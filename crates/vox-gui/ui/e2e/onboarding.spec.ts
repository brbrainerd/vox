import { test, expect } from '@playwright/test';

test.describe('Onboarding wizard', () => {
  test('shows three entry paths for a zero-secret, zero-local-model install', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.removeItem('vox_onboarding_dismissed');
      (window as any).__TAURI_CALLS__ = [];
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args?: Record<string, unknown>) => {
          (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
          if (cmd === 'get_initial_view') return 'chat';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'list_secret_status') return [];
          if (cmd === 'inference_provider_status') return [];
          if (cmd === 'get_command_catalog') return { generated_from: 'e2e-mock', entries: [] };
          if (cmd === 'get_action_manifest') return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
          if (cmd === 'get_routing_summary_live') return { decision_preview: null };
          if (cmd === 'get_gui_preference') return null;
          if (cmd === 'set_gui_preference') return null;
          if (cmd === 'get_orchestrator_status_bin') return new Uint8Array([0x80]);
          return null;
        },
      };
    });

    await page.goto('/');
    await expect(page.getByRole('heading', { name: /get started with vox/i })).toBeVisible({ timeout: 15_000 });
    await expect(page.getByRole('button', { name: /get a free key/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /i already have an api key/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /use a local model/i })).toBeVisible();
  });

  test('does not show when a secret is already configured', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.removeItem('vox_onboarding_dismissed');
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string) => {
          if (cmd === 'get_initial_view') return 'chat';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'list_secret_status') return [{ id: 'OPENROUTER_API_KEY', isPresent: true }];
          if (cmd === 'inference_provider_status') return [];
          if (cmd === 'get_command_catalog') return { generated_from: 'e2e-mock', entries: [] };
          if (cmd === 'get_action_manifest') return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
          if (cmd === 'get_routing_summary_live') return { decision_preview: null };
          if (cmd === 'get_gui_preference') return null;
          if (cmd === 'set_gui_preference') return null;
          if (cmd === 'get_orchestrator_status_bin') return new Uint8Array([0x80]);
          return null;
        },
      };
    });

    await page.goto('/');
    await expect(page.getByRole('heading', { name: /get started with vox/i })).not.toBeVisible();
  });

  test('budget screen saves caps via set_user_config before finishing', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.removeItem('vox_onboarding_dismissed');
      (window as any).__TAURI_CALLS__ = [];
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args?: Record<string, unknown>) => {
          (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
          if (cmd === 'get_initial_view') return 'chat';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'list_secret_status') return [];
          if (cmd === 'inference_provider_status') return [];
          if (cmd === 'get_command_catalog') return { generated_from: 'e2e-mock', entries: [] };
          if (cmd === 'get_action_manifest') return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
          if (cmd === 'get_routing_summary_live') return { decision_preview: null };
          if (cmd === 'get_gui_preference') return null;
          if (cmd === 'set_gui_preference') return null;
          if (cmd === 'get_orchestrator_status_bin') return new Uint8Array([0x80]);
          if (cmd === 'get_user_config') {
            return [
              { key: 'daily_budget_usd', label: 'Daily budget', hint: '', group: 'General', kind: 'float', options: [], default: '5', currentValue: '5' },
              { key: 'per_session_budget_usd', label: 'Per-session budget', hint: '', group: 'General', kind: 'float', options: [], default: '1', currentValue: '1' },
              { key: 'budget_warn_threshold_pct', label: 'Warn threshold', hint: '', group: 'General', kind: 'float', options: [], default: '0.8', currentValue: '0.8' },
            ];
          }
          if (cmd === 'set_user_config') return null;
          return null;
        },
      };
    });
    await page.goto('/');
    await page.getByRole('button', { name: /use a local model/i }).click();
    await page.getByRole('button', { name: /^done$/i }).click();
    await expect(page.getByRole('heading', { name: /set your spending limits/i })).toBeVisible();
    await page.getByRole('button', { name: /save and continue/i }).click();
    await expect(page.getByRole('heading', { name: /you're set up/i })).toBeVisible();
  });
});
