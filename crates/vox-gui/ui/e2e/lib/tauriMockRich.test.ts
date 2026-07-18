// crates/vox-gui/ui/e2e/lib/tauriMockRich.test.ts
import { describe, it, expect } from 'vitest';
import { decode } from '@msgpack/msgpack';
import { RICH_DATASET, buildRichOrchestratorStatus, richMockInitScript } from './tauriMockRich';

function makeFakeWindow(): any {
  const storage: Record<string, string> = {};
  return {
    localStorage: {
      setItem: (k: string, v: string) => { storage[k] = v; },
      getItem: (k: string) => storage[k] ?? null,
    },
  };
}
async function withFakeWindow<T>(fn: (win: any) => Promise<T> | T): Promise<T> {
  const prev = (global as any).window;
  const win = makeFakeWindow();
  (global as any).window = win;
  try { return await fn(win); } finally { (global as any).window = prev; }
}

describe('RICH_DATASET density', () => {
  it('is dense and overflow-shaped (sparse mocks are why occlusion hid)', () => {
    expect(RICH_DATASET.hopperTasks.length).toBeGreaterThanOrEqual(40);
    expect(RICH_DATASET.chatSessions.length).toBeGreaterThanOrEqual(12);
    expect(RICH_DATASET.models.length).toBeGreaterThanOrEqual(30);
    expect(RICH_DATASET.providers.length).toBeGreaterThanOrEqual(6);
    expect(Math.max(...RICH_DATASET.hopperTasks.map((t) => t.intent.length))).toBeGreaterThanOrEqual(120);
    const all = JSON.stringify(RICH_DATASET);
    expect(all).toMatch(/[Ѐ-ӿ]/);
    expect(all).toMatch(/[֐-׿]/);
  });
  it('dataset shapes stay on the wire contract', () => {
    for (const t of RICH_DATASET.hopperTasks) {
      expect([0, 1, 2]).toContain(t.priority);
      expect(['inbox', 'assigned', 'done']).toContain(t.state);
    }
    expect(RICH_DATASET.models.some((m) => m.id.includes('ollama') || m.id.startsWith('mens/') || m.id.startsWith('mesh/'))).toBe(true);
    expect(RICH_DATASET.models.some((m) => !(m.id.includes('ollama') || m.id.startsWith('mens/') || m.id.startsWith('mesh/')))).toBe(true);
  });
});

describe('richMockInitScript serialization', () => {
  it('composed script is self-contained and answers dense commands on a bare window', async () => {
    await withFakeWindow(async (win) => {
      // eslint-disable-next-line no-new-func -- exercising the exact addInitScript path
      new Function(richMockInitScript('tasks'))();
      expect(win.__VOX_MOCK_SHARED__).toBeDefined();
      expect((await win.__TAURI_INTERNALS__.invoke('hopper_list')).length).toBeGreaterThanOrEqual(40);
      expect((await win.__TAURI_INTERNALS__.invoke('chat_list_sessions', { limit: 40 })).length).toBeGreaterThanOrEqual(12);
      expect(await win.__TAURI_INTERNALS__.invoke('chat_list_sessions', { limit: 1 })).toHaveLength(1);
      expect(await win.__TAURI_INTERNALS__.invoke('get_initial_view')).toBe('tasks');
    });
  });
  it('delegates non-dense commands to the full base mock, not bootstrap nulls', async () => {
    await withFakeWindow(async (win) => {
      new Function(richMockInitScript('vox-search'))();
      const catalog = await win.__TAURI_INTERNALS__.invoke('get_command_catalog');
      expect(catalog.entries.length).toBeGreaterThan(0);
    });
  });
  it('serves a dense msgpack orchestrator snapshot (dashboard/flow are not blank)', async () => {
    await withFakeWindow(async (win) => {
      new Function(richMockInitScript('dashboard'))();
      const bin = await win.__TAURI_INTERNALS__.invoke('get_orchestrator_status_bin');
      const status = decode(bin) as ReturnType<typeof buildRichOrchestratorStatus>;
      expect(status.agents.length).toBeGreaterThanOrEqual(8);
      expect(status.recent_events.length).toBeGreaterThanOrEqual(20);
      expect(status.alerts.length).toBeGreaterThan(0);
    });
  });
});
