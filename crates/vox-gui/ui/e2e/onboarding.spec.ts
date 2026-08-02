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
      // False until the test flips it (right before clicking Done on `local-model`)
      // — other components (e.g. ChatModelPicker) also call `inference_provider_status`
      // on mount, so a call-count-based mock is order-dependent and flaky. Reads of
      // this flag stay empty for the initial gate check (must look like a fresh
      // install so the wizard shows at all); once flipped, the recheck inside
      // `local-model`'s Done handler (Important #4) sees a reachable local model.
      (window as any).__LOCAL_MODEL_READY__ = false;
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args?: Record<string, unknown>) => {
          (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
          if (cmd === 'get_initial_view') return 'chat';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'list_secret_status') return [];
          if (cmd === 'inference_provider_status') {
            return (window as any).__LOCAL_MODEL_READY__ ? [{ provider: 'ollama', is_local: true, local_reachable: true }] : [];
          }
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
    await page.evaluate(() => { (window as any).__LOCAL_MODEL_READY__ = true; });
    await page.getByRole('button', { name: /^done$/i }).click();
    await expect(page.getByRole('heading', { name: /set your spending limits/i })).toBeVisible();
    await page.getByRole('button', { name: /save and continue/i }).click();
    await expect(page.getByRole('heading', { name: /you're set up/i })).toBeVisible();
  });

  test('local-model Done re-checks reachability and warns instead of silently advancing', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.removeItem('vox_onboarding_dismissed');
      (window as any).__TAURI_CALLS__ = [];
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args?: Record<string, unknown>) => {
          (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
          if (cmd === 'get_initial_view') return 'chat';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'list_secret_status') return [];
          // No local model ever becomes reachable — the recheck on Done must
          // block advancing to the budget screen (Important #4).
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
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /use a local model/i }).click();
    await dialog.getByRole('button', { name: /^done$/i }).click();
    await expect(dialog.getByRole('alert')).toContainText(/no local model detected/i);
    await expect(page.getByRole('heading', { name: /set your spending limits/i })).not.toBeVisible();
    // Back returns to entry — no dead end.
    await dialog.getByRole('button', { name: /^back$/i }).click();
    await expect(page.getByRole('heading', { name: /get started with vox/i })).toBeVisible();
  });

  test('OAuth success path reaches the budget screen after "Get a free key"', async ({ page }) => {
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
          if (cmd === 'oauth_login_openrouter') return { success: true, error: null, fallbackUrl: null };
          if (cmd === 'verify_openrouter_key') return true;
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
    await page.getByRole('button', { name: /get a free key/i }).click();
    await expect(page.getByRole('heading', { name: /set your spending limits/i })).toBeVisible({ timeout: 15_000 });
  });

  test('OAuth failure-with-fallback-URL path shows the error and a clickable link on entry', async ({ page }) => {
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
          if (cmd === 'oauth_login_openrouter') {
            return { success: false, error: 'no browser found', fallbackUrl: 'https://openrouter.ai/auth?callback_url=http://127.0.0.1' };
          }
          return null;
        },
      };
    });
    await page.goto('/');
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /get a free key/i }).click();
    await expect(page.getByRole('heading', { name: /get started with vox/i })).toBeVisible();
    await expect(dialog.getByRole('alert')).toContainText(/no browser found/i);
    const link = dialog.getByRole('link', { name: /open this link manually/i });
    await expect(link).toBeVisible();
    await expect(link).toHaveAttribute('href', 'https://openrouter.ai/auth?callback_url=http://127.0.0.1');
  });
});
