// crates/vox-gui/ui/src/components/gamify/urbs/harnessData.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));

import { fetchHarnessSnapshot } from './harnessData';

beforeEach(() => { invokeMock.mockReset(); });

describe('fetchHarnessSnapshot', () => {
  it('maps successful taps into the snapshot, counting only non-assigned hopper items as queued', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'harness_ci_fleet_status':
          return { runners: [{ name: 'r1', busy: true, online: true }], queued: 2 };
        case 'vcs_town_status':
          return { branches: [{ name: 'main', is_head: true, track: '[ahead 1]' }], prs: [{ number: 1, title: 't', head_ref: 'h' }], prs_available: true };
        case 'hopper_list':
          return [{ id: 'a', state: 'inbox' }, { id: 'b', state: 'assigned' }, { id: 'c', state: 'inbox' }];
        default:
          throw new Error(`unexpected ${cmd}`);
      }
    });
    const s = await fetchHarnessSnapshot();
    expect(s.ci?.runners).toHaveLength(1);
    expect(s.ci?.queued).toBe(2);
    expect(s.vcs?.branches[0]).toEqual({ name: 'main', isHead: true, track: '[ahead 1]' });
    // hopper_list returns inbox + assigned; assigned (in-flight) is NOT queued.
    expect(s.queueLen).toBe(2);
    // No MCP server-list command exists — mcp is unconditionally null (AQVAE unlit).
    expect(s.mcp).toBeNull();
  });

  it('a failing tap yields null for that field only (unlit, not fake)', async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'hopper_list') return [];
      throw new Error('unavailable');
    });
    const s = await fetchHarnessSnapshot();
    expect(s.ci).toBeNull();
    expect(s.vcs).toBeNull();
    expect(s.mcp).toBeNull();
    expect(s.queueLen).toBe(0);
  });
});
