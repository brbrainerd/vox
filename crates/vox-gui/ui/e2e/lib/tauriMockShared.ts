/**
 * Shared environment seeding + bootstrap responses for the Tauri-invoke mocks
 * (tauriMock.ts, tauriMockVariants.ts).
 *
 * `page.addInitScript(fn, arg)` serialises ONLY the function body, so module
 * imports are invisible in the browser. Instead, `addMockInitScript` composes
 * one script string that (1) installs these helpers on
 * `window.__VOX_MOCK_SHARED__` from their `.toString()` source and (2) invokes
 * the installer. Every function here must therefore be self-contained (no
 * captured module scope).
 */
import type { Page } from '@playwright/test';

/** Seed localStorage tabs, call log, event plumbing, and transformCallback. */
export function seedMockEnvironment(viewKey: string): void {
  try {
    window.localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({
        openTabs: Array.from(new Set(['chat', viewKey])),
        activeTab: viewKey,
      }),
    );
    window.localStorage.setItem('vox_sidebar_mode', 'default');
  } catch {
    // sandboxed contexts may deny localStorage
  }
  (window as any).__TAURI_CALLS__ = [];
  (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (_event: string, _eventId: number) => {},
  };
  // Event registry + emit helper: `plugin:event|listen` registers transformed
  // callback ids here; specs drive streams via
  //   page.evaluate(([ev, payload]) => (window as any).__TAURI_EMIT__(ev, payload), [...])
  (window as any).__TAURI_EVENT_LISTENERS__ = {} as Record<string, string[]>;
  (window as any).__TAURI_EMIT__ = (event: string, payload: unknown) => {
    const ids: string[] = ((window as any).__TAURI_EVENT_LISTENERS__ ?? {})[event] ?? [];
    for (const id of ids) {
      const cb = (window as any)[id];
      if (typeof cb === 'function') cb({ event, id: 0, payload });
    }
  };
  (window as any).__TAURI_INTERNALS__ = {
    ...((window as any).__TAURI_INTERNALS__ || {}),
    transformCallback: (cb: (...args: unknown[]) => unknown) => {
      const id = `cb_${Math.random().toString(36).slice(2)}`;
      (window as any)[id] = cb;
      return id;
    },
  };
}

/** Handle the tauri event plugin commands; `undefined` means "not an event cmd". */
export function eventPluginResponse(cmd: string, args: any): number | null | undefined {
  if (cmd === 'plugin:event|listen') {
    const reg = (window as any).__TAURI_EVENT_LISTENERS__ as
      | Record<string, string[]>
      | undefined;
    if (reg && typeof args?.event === 'string' && typeof args?.handler === 'string') {
      (reg[args.event] ??= []).push(args.handler);
    }
    return Math.floor(Math.random() * 10000);
  }
  if (cmd === 'plugin:event|unlisten') return null;
  return undefined;
}

/** Commands that must succeed for the app shell to mount at all (single copy). */
export function bootstrapResponse(cmd: string, viewKey: string): unknown {
  switch (cmd) {
    case 'get_initial_view': return viewKey;
    case 'get_build_info': return { version: '0.6.0', display: '0.6.0+local (dev)' };
    case 'get_orchestrator_status_bin': return new Uint8Array([0x80]);
    case 'get_orchestrator_status': return { agent_count: 0, agents: [], recent_events: [], alerts: [], peers: [] };
    case 'get_action_manifest': return { x_vox_version: 2, schema_version: 1, generated_from: 'mock', actions: [] };
    case 'get_gui_preference': return null;
    case 'get_gamify_settings': return { enabled: false, mode: 'off' };
    case 'get_identity_summary': return { display_name: 'tester@vox', os_user: 'tester' };
    case 'get_active_model': return null;
    case 'get_selection_policy': return { chain: [], free_tier: true };
    case 'vox_docs_index': return [];
    // Non-fresh-install defaults so the first-run OnboardingWizard (Task 15)
    // doesn't cover the shell in specs that don't override these explicitly.
    case 'list_secret_status': return [{ id: 'OPENROUTER_API_KEY', isPresent: true }];
    case 'inference_provider_status': return [];
    default: return null;
  }
}

const SHARED_SNIPPET = [
  'window.__VOX_MOCK_SHARED__ = {',
  `  seedMockEnvironment: ${seedMockEnvironment.toString()},`,
  `  eventPluginResponse: ${eventPluginResponse.toString()},`,
  `  bootstrapResponse: ${bootstrapResponse.toString()},`,
  '};',
].join('\n');

/** Compose the full init script for an installer (exported for unit tests). */
export function mockInitScript(installer: (viewKey: string) => void, viewKey: string): string {
  return `${SHARED_SNIPPET}\n(${installer.toString()})(${JSON.stringify(viewKey)});`;
}

/** The ONLY supported way to inject a mock installer into a Playwright page. */
export async function addMockInitScript(
  page: Page,
  installer: (viewKey: string) => void,
  viewKey: string,
): Promise<void> {
  await page.addInitScript({ content: mockInitScript(installer, viewKey) });
}

/** Vitest helper: run an installer against the (fake) global window. */
export function runInstallerWithShared(
  installer: (viewKey: string) => void,
  viewKey: string,
): void {
  (globalThis as any).window.__VOX_MOCK_SHARED__ = {
    seedMockEnvironment,
    eventPluginResponse,
    bootstrapResponse,
  };
  installer(viewKey);
}
