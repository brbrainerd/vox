import { test, expect } from '@playwright/test';

// Regression test for Task 7 (free-tier onboarding plan): `budget_warn_threshold_pct`
// is registered in the vox-llm-config SSOT (Task 2, vc_key! macro, group "General")
// and Settings' Runtime section renders `get_user_config`'s catalog generically
// (RuntimeConfigSection in SettingsView.tsx), so the field should surface with no
// additional frontend code.
test.describe('Vox Settings — budget warn threshold', () => {
  test('renders budget_warn_threshold_pct from the get_user_config catalog', async ({ page }) => {
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
          if (cmd === 'get_user_config') {
            return [
              {
                key: 'daily_budget_usd', label: 'Daily budget (USD)', hint: 'Soft cap on spend per day',
                group: 'General', kind: 'float', options: [], default: '5', currentValue: '5',
              },
              {
                key: 'per_session_budget_usd', label: 'Per-session budget (USD)', hint: 'Soft cap on spend per session',
                group: 'General', kind: 'float', options: [], default: '1', currentValue: '1',
              },
              {
                key: 'budget_warn_threshold_pct', label: 'Budget warn threshold',
                hint: 'Warn when spend crosses this fraction of a budget cap (0.0-1.0)',
                group: 'General', kind: 'float', options: [], default: '0.8', currentValue: '0.8',
              },
            ];
          }
          if (cmd === 'get_llm_spend') return null;
          // Settings' orchestrator hydration effect indexes into this result with
          // bracket access (cfg[k]); it must be an object, not the generic `null`
          // fallback below, or the effect throws and the whole surface error-boundaries.
          if (cmd === 'get_orchestrator_config') return {};
          return null;
        },
      };
    });

    await page.goto('/');
    await expect(page.getByRole('heading', { name: /settings/i })).toBeVisible({ timeout: 15_000 });

    await page.getByRole('button', { name: /runtime/i }).click();

    await expect(page.getByText('Budget warn threshold')).toBeVisible({ timeout: 15_000 });
  });
});
