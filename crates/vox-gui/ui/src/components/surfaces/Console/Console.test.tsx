// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

vi.mock('../../../transport', () => ({
  discoverySuggest: vi.fn().mockResolvedValue([]),
  discoveryHelp: vi.fn().mockResolvedValue(null),
  discoveryRecord: vi.fn().mockResolvedValue(undefined),
  ptySpawn: vi.fn().mockResolvedValue(undefined),
  ptyWrite: vi.fn().mockResolvedValue(undefined),
  ptyClose: vi.fn().mockResolvedValue(undefined),
  listenPtyOutput: vi.fn().mockResolvedValue(() => {}),
  listenPtyExit: vi.fn().mockResolvedValue(() => {}),
  listenOrchStatus: vi.fn().mockRejectedValue(new Error('not in tauri')),
  sendToAgent: vi.fn().mockResolvedValue('msg-1'),
}));
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    parser = { registerOscHandler: () => ({ dispose() {} }) };
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

import { Console } from './Console';

describe('Console', () => {
  beforeEach(() => cleanup());

  it('renders the terminal, input, and discovery rail', async () => {
    render(<Console pushToast={vi.fn()} />);
    expect(screen.getByLabelText('terminal')).toBeTruthy();
    expect(screen.getByRole('textbox')).toBeTruthy();
    await waitFor(() => expect(screen.getByLabelText('discovery')).toBeTruthy());
  });

  it('submitting a line in the input forwards it to the terminal write path', async () => {
    const t = await import('../../../transport');
    render(<Console pushToast={vi.fn()} />);
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'echo hi' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(t.ptyWrite).toHaveBeenCalledWith('console-1', 'echo hi\n'));
  });

  it('gives toolbar controls explicit button type', () => {
    render(<Console pushToast={vi.fn()} />);
    expect(screen.getByText('copy last block').getAttribute('type')).toBe('button');
    expect(screen.getByText('send to agent').getAttribute('type')).toBe('button');
  });
});
