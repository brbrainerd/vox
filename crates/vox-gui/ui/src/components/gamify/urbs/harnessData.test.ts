// crates/vox-gui/ui/src/components/gamify/urbs/harnessData.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest';

const harnessCiFleetStatus = vi.fn();
const vcsTownStatus = vi.fn();
const hopperList = vi.fn();
vi.mock('../../../transport', () => ({
  voxTransport: {
    harnessCiFleetStatus: (...a: unknown[]) => harnessCiFleetStatus(...a),
    vcsTownStatus: (...a: unknown[]) => vcsTownStatus(...a),
    hopperList: (...a: unknown[]) => hopperList(...a),
  },
}));

import { fetchHarnessSnapshot } from './harnessData';

beforeEach(() => {
  harnessCiFleetStatus.mockReset();
  vcsTownStatus.mockReset();
  hopperList.mockReset();
});

describe('fetchHarnessSnapshot', () => {
  it('maps successful taps into the snapshot, counting only non-assigned hopper items as queued', async () => {
    harnessCiFleetStatus.mockResolvedValue({ runners: [{ name: 'r1', busy: true, online: true }], queued: 2 });
    vcsTownStatus.mockResolvedValue({ branches: [{ name: 'main', is_head: true, track: '[ahead 1]' }], prs: [{ number: 1, title: 't', head_ref: 'h' }], prs_available: true });
    hopperList.mockResolvedValue([{ id: 'a', state: 'inbox' }, { id: 'b', state: 'assigned' }, { id: 'c', state: 'inbox' }]);
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
    harnessCiFleetStatus.mockRejectedValue(new Error('unavailable'));
    vcsTownStatus.mockRejectedValue(new Error('unavailable'));
    hopperList.mockResolvedValue([]);
    const s = await fetchHarnessSnapshot();
    expect(s.ci).toBeNull();
    expect(s.vcs).toBeNull();
    expect(s.mcp).toBeNull();
    expect(s.queueLen).toBe(0);
  });
});
