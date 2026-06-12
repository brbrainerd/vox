// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const spawnMock = vi.fn().mockResolvedValue(undefined);
const writeMock = vi.fn().mockResolvedValue(undefined);
vi.mock('../../../transport', () => ({
  ptySpawn: (...a: unknown[]) => spawnMock(...a),
  ptyWrite: (...a: unknown[]) => writeMock(...a),
  ptyClose: vi.fn().mockResolvedValue(undefined),
  listenPtyOutput: vi.fn().mockResolvedValue(() => {}),
  listenPtyExit: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    open() {}
    write() {}
    onData() {}
    dispose() {}
    loadAddon() {}
    get cols() {
      return 80;
    }
    get rows() {
      return 24;
    }
  },
}));
vi.mock('@xterm/addon-fit', () => ({ FitAddon: class { fit() {} } }));

import { TerminalTab } from './TerminalTab';

describe('TerminalTab', () => {
  beforeEach(() => {
    cleanup();
    spawnMock.mockClear();
    writeMock.mockClear();
  });

  it('spawns a PTY for its tab id on mount', async () => {
    render(<TerminalTab tabId="tab-1" pendingLine={null} />);
    await waitFor(() => expect(spawnMock).toHaveBeenCalledWith('tab-1', 80, 24));
  });

  it('writes a submitted line to the PTY', async () => {
    const { rerender } = render(<TerminalTab tabId="tab-1" pendingLine={null} />);
    await waitFor(() => expect(spawnMock).toHaveBeenCalled());
    rerender(<TerminalTab tabId="tab-1" pendingLine={{ text: 'ls', seq: 1 }} />);
    await waitFor(() => expect(writeMock).toHaveBeenCalledWith('tab-1', 'ls\n'));
  });
});
