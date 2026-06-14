// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, cleanup, waitFor, screen } from '@testing-library/react';
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

// Capture the OSC 633 handler the component registers so tests can drive markers.
let oscHandler: ((data: string) => boolean) | null = null;
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    parser = {
      registerOscHandler: (id: number, cb: (data: string) => boolean) => {
        if (id === 633) oscHandler = cb;
        return { dispose() {} };
      },
    };
    buffer = { active: { cursorY: 0, baseY: 0 } };
    open() {}
    write() {}
    onData() {}
    dispose() {}
    loadAddon() {}
    registerMarker() {
      return { dispose() {} };
    }
    registerDecoration() {
      return { onRender() {}, dispose() {} };
    }
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
    oscHandler = null;
  });

  it('exposes the host as a labeled application region', () => {
    render(<TerminalTab tabId="tab-1" pendingLine={null} />);
    const host = screen.getByLabelText('terminal');
    expect(host.getAttribute('role')).toBe('application');
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

  it('registers an OSC 633 handler and fires onBlock when a block completes', async () => {
    const onBlock = vi.fn();
    render(<TerminalTab tabId="tab-1" pendingLine={null} onBlock={onBlock} />);
    await waitFor(() => expect(oscHandler).toBeTruthy());
    oscHandler!('A');
    oscHandler!('E;git status');
    oscHandler!('C');
    oscHandler!('D;0');
    expect(onBlock).toHaveBeenCalledTimes(1);
    expect(onBlock.mock.calls[0][0].command).toBe('git status');
    expect(onBlock.mock.calls[0][0].exitCode).toBe(0);
  });
});
