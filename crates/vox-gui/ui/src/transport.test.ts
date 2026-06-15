import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { voxTransport } from './transport';

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe('VoxTransport new methods', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('logFrontend calls log_frontend with level + message', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await voxTransport.logFrontend('warn', 'test warning');
    expect(mockInvoke).toHaveBeenCalledWith('log_frontend', { level: 'warn', message: 'test warning' });
  });

  it('getGuiPreference returns null when backend returns null', async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await voxTransport.getGuiPreference('dock-layout');
    expect(mockInvoke).toHaveBeenCalledWith('get_gui_preference', { key: 'dock-layout' });
    expect(result).toBeNull();
  });

  it('getGuiPreference returns string value', async () => {
    mockInvoke.mockResolvedValue('{"collapsed":false}');
    const result = await voxTransport.getGuiPreference('dock-layout');
    expect(result).toBe('{"collapsed":false}');
  });

  it('setGuiPreference calls set_gui_preference', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await voxTransport.setGuiPreference('dock-layout', '{"collapsed":true}');
    expect(mockInvoke).toHaveBeenCalledWith('set_gui_preference', { key: 'dock-layout', value: '{"collapsed":true}' });
  });

  it('openLocator forwards the locator object and returns the outcome', async () => {
    mockInvoke.mockResolvedValue({ action: 'opened' });
    const locator = { kind: 'file' as const, value: 'src/main.rs' };
    const outcome = await voxTransport.openLocator(locator);
    expect(mockInvoke).toHaveBeenCalledWith('open_locator', { locator });
    expect(outcome).toEqual({ action: 'opened' });
  });

  it('openLocator passes a web locator through unchanged', async () => {
    mockInvoke.mockResolvedValue({ action: 'spawned' });
    const locator = { kind: 'web' as const, value: 'https://example.test' };
    await voxTransport.openLocator(locator);
    expect(mockInvoke).toHaveBeenCalledWith('open_locator', { locator });
  });
});
