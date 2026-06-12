/**
 * DockShell layout persistence — mocks gui.layout.v1 via Tauri preference IPC.
 *
 * Run: pnpm exec playwright test dock-layout.spec.ts --project=chromium
 */
import { test, expect } from '@playwright/test';

const LAYOUT_KEY = 'gui.layout.v1';
const PANEL_ID = 'main-surface';

/**
 * Minimal dockview serialized layout (branch root + single panel group).
 * Matches DockShell defaults: panel id `main-surface`, component `panel`, title `agents`.
 */
const MINIMAL_DOCK_LAYOUT = JSON.stringify({
  grid: {
    root: {
      type: 'branch',
      data: [
        {
          type: 'leaf',
          data: {
            views: [PANEL_ID],
            activeView: PANEL_ID,
            id: '1',
          },
          size: 100,
        },
      ],
      size: 100,
    },
    width: 800,
    height: 600,
    orientation: 0,
  },
  panels: {
    [PANEL_ID]: {
      id: PANEL_ID,
      contentComponent: 'panel',
      title: 'agents',
    },
  },
  activeGroup: '1',
});

test.describe('DockShell layout persistence', () => {
  test('restores persisted layout from gui.layout.v1', async ({ page }) => {
    await page.addInitScript((layoutJson: string | null) => {
      const layoutKey = 'gui.layout.v1';
      localStorage.setItem('vox_active_view', JSON.stringify('dashboard'));
      localStorage.setItem('vox_sidebar_mode', 'default');
      (window as any).__TAURI_CALLS__ = [];
      (window as any).__GUI_PREFS__ = layoutJson ? { [layoutKey]: layoutJson } : {};

      (window as any).__TAURI_INTERNALS__ = {
        transformCallback: (cb: (...args: unknown[]) => unknown) => {
          const id = `cb_${Math.random().toString(36).slice(2)}`;
          (window as any)[id] = cb;
          return id;
        },
        invoke: async (cmd: string, args?: Record<string, unknown>) => {
          (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
          const prefs = (window as any).__GUI_PREFS__ as Record<string, string>;

          switch (cmd) {
            case 'get_gui_preference':
              return prefs[args?.key as string] ?? null;
            case 'set_gui_preference': {
              prefs[args?.key as string] = args?.value as string;
              return null;
            }
            case 'get_initial_view':
              return 'dashboard';
            case 'get_build_info':
              return { version: '0.6.0', display: '0.6.0+build.test (e2e)' };
            case 'get_command_catalog':
              return { generated_from: 'e2e-mock', entries: [] };
            case 'get_action_manifest':
              return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
            case 'get_orchestrator_status_bin':
              return new Uint8Array([0x80]);
            case 'get_orchestrator_status':
              return { agent_count: 0, agents: [], recent_events: [], alerts: [], peers: [] };
            case 'list_model_cards':
              return [{ model_id: 'mens-8b', display_name: 'Mens 8B', id: 'mens-8b' }];
            case 'get_active_model':
              return 'mens-8b';
            case 'get_routing_summary':
            case 'get_routing_summary_live':
              return {
                active_model: 'mens-8b',
                exploration_spent_usd: 0,
                exploration_budget_usd: 50,
                arm_count: 1,
                model_count: 1,
                decision_preview: null,
              };
            case 'get_selection_policy':
              return { chain: ['mens-8b'], free_tier: true };
            case 'get_model_scoreboard':
              return [];
            case 'list_gui_runs':
              return [];
            case 'get_identity_summary':
              return { display_name: 'tester@vox', os_user: 'tester' };
            case 'chat_list_sessions':
              return [];
            case 'chat_get_messages':
              return [];
            case 'get_ludus_profile':
              return {
                user_id: 'local',
                level: 1,
                xp: 0,
                xp_to_next_level: 100,
                xp_progress: 0,
                total_xp_earned: 0,
                crystals: 0,
                lumens: 0,
                energy: 100,
                max_energy: 100,
                current_streak: 0,
                prestige_level: 0,
                title: 'Initiate',
                full_title: 'Initiate',
                trust_tier: 'New',
              };
            case 'list_ludus_notifications':
              return [];
            case 'get_gamify_settings':
              return { enabled: false, mode: 'off' };
            default:
              return null;
          }
        },
      };
    }, MINIMAL_DOCK_LAYOUT);

    await page.goto('/');
    await page.waitForSelector('nav', { timeout: 15_000 });

    await expect(page.locator('.dockview-theme-vox')).toBeVisible();
    await expect(page.locator('.dv-tabs-and-actions-container')).toBeVisible();
    await expect(page.locator('.dv-tab')).toHaveCount(1);
    await expect(page.locator('.dv-tab')).toContainText('agents');

    await page.waitForFunction(
      (key) => {
        const calls = (window as any).__TAURI_CALLS__ ?? [];
        return calls.some((c: { cmd: string; args?: { key?: string } }) => c.cmd === 'get_gui_preference' && c.args?.key === key);
      },
      LAYOUT_KEY,
      { timeout: 10_000 },
    );

    const seededLayout = await page.evaluate((key) => {
      const prefs = (window as any).__GUI_PREFS__ as Record<string, string>;
      return prefs[key] ?? null;
    }, LAYOUT_KEY);

    expect(seededLayout).toBe(MINIMAL_DOCK_LAYOUT);
    const parsed = JSON.parse(seededLayout);
    expect(parsed.panels?.[PANEL_ID]).toMatchObject({
      id: PANEL_ID,
      contentComponent: 'panel',
      title: 'agents',
    });
    expect(parsed.grid?.root?.type).toBe('branch');
  });
});
