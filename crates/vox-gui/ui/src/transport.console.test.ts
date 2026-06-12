// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import { discoverySuggest, ptyWrite } from './transport';

describe('console transport', () => {
  beforeEach(() => invokeMock.mockReset());

  it('discoverySuggest forwards typed + limit to invoke', async () => {
    invokeMock.mockResolvedValue([
      { action_id: 'vox.config.show', completion: 'config show', about: '' },
    ]);
    const out = await discoverySuggest('config', 5);
    expect(invokeMock).toHaveBeenCalledWith('discovery_suggest', { typed: 'config', limit: 5 });
    expect(out[0].action_id).toBe('vox.config.show');
  });

  it('ptyWrite forwards tab id + data', async () => {
    invokeMock.mockResolvedValue(undefined);
    await ptyWrite('tab-1', 'ls\n');
    expect(invokeMock).toHaveBeenCalledWith('pty_write', { tabId: 'tab-1', data: 'ls\n' });
  });
});
