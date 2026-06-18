import { describe, it, expect } from 'vitest';
import { installEmptyStateMock, installErrorStateMock } from './tauriMockVariants';

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
  it('sets vox_active_view in localStorage', async () => {
    await withFakeWindow((win) => {
      installEmptyStateMock('dashboard');
      expect(win.localStorage._storage['vox_active_view']).toBe(JSON.stringify('dashboard'));
    });
  });

  it('returns [] for list_gui_runs', async () => {
    await withFakeWindow(async (win) => {
      installEmptyStateMock('runs');
      const result = await win.__TAURI_INTERNALS__.invoke('list_gui_runs');
      expect(result).toEqual([]);
    });
  });

  it('returns the viewKey for get_initial_view', async () => {
    await withFakeWindow(async (win) => {
      installEmptyStateMock('settings');
      const result = await win.__TAURI_INTERNALS__.invoke('get_initial_view');
      expect(result).toBe('settings');
    });
  });

  it('returns typed-empty object for get_memory_status', async () => {
    await withFakeWindow(async (win) => {
      installEmptyStateMock('memory');
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
      installEmptyStateMock('dashboard');
      for (const cmd of bootstrapCmds) {
        await expect(win.__TAURI_INTERNALS__.invoke(cmd)).resolves.not.toThrow();
      }
    });
  });
});

describe('installErrorStateMock', () => {
  it('throws for list_gui_runs', async () => {
    await withFakeWindow(async (win) => {
      installErrorStateMock('runs');
      await expect(win.__TAURI_INTERNALS__.invoke('list_gui_runs')).rejects.toThrow('[mock-error]');
    });
  });

  it('still returns viewKey for get_initial_view', async () => {
    await withFakeWindow(async (win) => {
      installErrorStateMock('models');
      const result = await win.__TAURI_INTERNALS__.invoke('get_initial_view');
      expect(result).toBe('models');
    });
  });

  it('throws for policy_list', async () => {
    await withFakeWindow(async (win) => {
      installErrorStateMock('policies');
      await expect(win.__TAURI_INTERNALS__.invoke('policy_list')).rejects.toThrow('[mock-error]');
    });
  });
});
