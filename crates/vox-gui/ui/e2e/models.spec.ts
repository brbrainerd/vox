import { test, expect } from '@playwright/test';

test.describe('Vox Models view', () => {
  test('free-tier filter hides non-free models when toggled on', async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.setItem('vox_sidebar_mode', 'default');
      localStorage.setItem('vox_onboarding_dismissed', 'true');
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string) => {
          if (cmd === 'get_initial_view') return 'models';
          if (cmd === 'list_model_cards') {
            return [
              { id: 'free/model-a', provider: 'openrouter', tier: 'Free', cost_per_1k: 0, max_tokens: 8000, is_free: true, latency_p50_ms: 400, quality_score: 0.8 },
              { id: 'paid/model-b', provider: 'openrouter', tier: 'Pro', cost_per_1k: 0.01, max_tokens: 8000, is_free: false, latency_p50_ms: 300, quality_score: 0.9 },
            ];
          }
          if (cmd === 'get_routing_summary_live') return { decision_preview: null };
          if (cmd === 'get_active_model') return null;
          if (cmd === 'inference_provider_status') return [];
          return null;
        },
      };
    });

    await page.goto('/');
    await expect(page.getByText('paid/model-b')).toBeVisible();
    await page.getByRole('checkbox', { name: /free only/i }).check();
    await expect(page.getByText('paid/model-b')).not.toBeVisible();
    await expect(page.getByText('free/model-a')).toBeVisible();
  });
});
