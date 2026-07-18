import { describe, it, expect } from 'vitest';
import { installEmptyStateMock, installErrorStateMock } from './tauriMockVariants';
import { runInstallerWithShared, mockInitScript } from './tauriMockShared';

/**
 * The factories are designed to be injected via page.addInitScript (serialised + re-executed
 * in browser context). We test their logic directly by simulating the browser globals.
 */
function makeFakeWindow(): any {
  const storage: Record<string, string> = {};
  return {
    localStorage: {
      setItem: (k: string, v: string) => { storage[k] = v; },
      getItem: (k: string) => storage[k] ?? null,
      _storage: storage,
    },
    __TAURI_INTERNALS__: undefined,
    __TAURI_CALLS__: undefined,
    // Required so unlisten() calls triggered during React cleanup don't throw.
    __TAURI_EVENT_PLUGIN_INTERNALS__: undefined,
  };
}

async function withFakeWindow<T>(fn: (win: any) => Promise<T> | T): Promise<T> {
  const realWindow = (global as any).window;
  const fakeWin = makeFakeWindow();
  (global as any).window = fakeWin;
  try {
    return await fn(fakeWin);
  } finally {
    (global as any).window = realWindow;
  }
}

describe('installEmptyStateMock', () => {
  it('sets vox_workbench_tabs.v1 in localStorage', async () => {
    await withFakeWindow((win) => {
      runInstallerWithShared(installEmptyStateMock, 'dashboard');
      const raw = win.localStorage._storage['vox_workbench_tabs.v1'];
      expect(raw).toBeTruthy();
      const parsed = JSON.parse(raw);
      expect(parsed.activeTab).toBe('dashboard');
      expect(parsed.openTabs).toContain('dashboard');
      expect(parsed.openTabs).toContain('chat');
    });
  });

  it('returns [] for list_gui_runs', async () => {
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installEmptyStateMock, 'runs');
      const result = await win.__TAURI_INTERNALS__.invoke('list_gui_runs');
      expect(result).toEqual([]);
    });
  });

  it('returns the viewKey for get_initial_view', async () => {
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installEmptyStateMock, 'settings');
      const result = await win.__TAURI_INTERNALS__.invoke('get_initial_view');
      expect(result).toBe('settings');
    });
  });

  it('returns typed-empty object for get_memory_status', async () => {
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installEmptyStateMock, 'memory');
      const result = await win.__TAURI_INTERNALS__.invoke('get_memory_status');
      expect(result).toMatchObject({ corpus_counts: {}, shards: [] });
    });
  });

  it('does not throw for any bootstrap command', async () => {
    const bootstrapCmds = [
      'get_build_info', 'get_orchestrator_status_bin', 'get_action_manifest',
      'get_gui_preference', 'get_gamify_settings', 'get_identity_summary',
    ];
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installEmptyStateMock, 'dashboard');
      for (const cmd of bootstrapCmds) {
        await expect(win.__TAURI_INTERNALS__.invoke(cmd)).resolves.not.toThrow();
      }
    });
  });

  it('sets __TAURI_EVENT_PLUGIN_INTERNALS__ with unregisterListener stub', () => {
    withFakeWindow((win) => {
      runInstallerWithShared(installEmptyStateMock, 'dashboard');
      expect(win.__TAURI_EVENT_PLUGIN_INTERNALS__).toBeDefined();
      expect(typeof win.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener).toBe('function');
    });
  });
});

describe('installErrorStateMock', () => {
  it('throws for list_gui_runs', async () => {
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installErrorStateMock, 'runs');
      await expect(win.__TAURI_INTERNALS__.invoke('list_gui_runs')).rejects.toThrow('[mock-error]');
    });
  });

  it('still returns viewKey for get_initial_view', async () => {
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installErrorStateMock, 'models');
      const result = await win.__TAURI_INTERNALS__.invoke('get_initial_view');
      expect(result).toBe('models');
    });
  });

  it('throws for policy_list', async () => {
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installErrorStateMock, 'policies');
      await expect(win.__TAURI_INTERNALS__.invoke('policy_list')).rejects.toThrow('[mock-error]');
    });
  });

  it('sets __TAURI_EVENT_PLUGIN_INTERNALS__ with unregisterListener stub', () => {
    withFakeWindow((win) => {
      runInstallerWithShared(installErrorStateMock, 'runs');
      expect(win.__TAURI_EVENT_PLUGIN_INTERNALS__).toBeDefined();
      expect(typeof win.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener).toBe('function');
    });
  });
});

describe('mockInitScript serialization', () => {
  it('composed script is self-contained and runs against a bare window', async () => {
    await withFakeWindow(async (win) => {
      // eslint-disable-next-line no-new-func -- exercising the exact addInitScript path
      new Function(mockInitScript(installErrorStateMock, 'runs'))();
      expect(win.__VOX_MOCK_SHARED__).toBeDefined();
      await expect(win.__TAURI_INTERNALS__.invoke('list_gui_runs')).rejects.toThrow('[mock-error]');
      expect(await win.__TAURI_INTERNALS__.invoke('get_initial_view')).toBe('runs');
    });
  });
  it('emit helper dispatches to listeners registered via plugin:event|listen', async () => {
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installEmptyStateMock, 'dashboard');
      const seen: unknown[] = [];
      const handler = win.__TAURI_INTERNALS__.transformCallback((e: any) => seen.push(e.payload));
      await win.__TAURI_INTERNALS__.invoke('plugin:event|listen', { event: 'vox://agent-events', handler });
      win.__TAURI_EMIT__('vox://agent-events', { id: 1, kind: { type: 'task_started' } });
      expect(seen).toHaveLength(1);
    });
  });
});
