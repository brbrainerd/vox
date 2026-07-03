// @vitest-environment jsdom
// crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx  (full replacement)
// The pragma above is REQUIRED as line 1: vitest.config.ts sets no global
// environment, so render()/screen crash under the default node env without it.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
vi.mock('../../transport', () => ({
  listenAgentEvents: vi.fn().mockRejectedValue(new Error('not in tauri')),
  // useLlmSpend (used for the HUD treasury) reads voxTransport from the same
  // module — the mock must provide it or the hook crashes on mount.
  voxTransport: { getLlmSpend: vi.fn().mockRejectedValue(new Error('unavailable')) },
}));

import { LudusSandbox } from './LudusSandbox';

const SCAN = {
  crates: [{ name: 'a', root: 'crates/a', files: [{ path: 'crates/a/x.rs', lines: 10 }] }],
  root: '/ws', scanned_at_ms: 1, truncated: false,
};

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'workspace_town_scan') return SCAN;
    throw new Error('unavailable');
  });
});

describe('LudusSandbox (Vox Urbs shell)', () => {
  it('renders the town canvas and loads the workspace scan', async () => {
    render(<LudusSandbox />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('workspace_town_scan'));
    expect(screen.getByTestId('urbs-canvas')).toBeTruthy();
  });

  it('shows SIM PAVSED when the agent event stream is unavailable', async () => {
    render(<LudusSandbox />);
    await waitFor(() => expect(screen.getByText(/SIM PAVSED/i)).toBeTruthy());
  });

  it('shows a scan-failed state (not a fake town) when the scan tap fails', async () => {
    invokeMock.mockImplementation(async () => { throw new Error('nope'); });
    render(<LudusSandbox />);
    await waitFor(() => expect(screen.getByText(/scan unavailable/i)).toBeTruthy());
  });

  it('renders the HUD with real-null treasury (em-dash) when spend tap fails', async () => {
    render(<LudusSandbox />);
    await waitFor(() => expect(screen.getByTestId('hud-value').textContent).toBe('—'));
  });
});
