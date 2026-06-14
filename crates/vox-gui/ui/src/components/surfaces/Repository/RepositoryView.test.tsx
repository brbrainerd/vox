// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'get_vcs_isolation') {
    return Promise.resolve({ strategy_default: 'shared_branch', per_agent: {}, active_conflicts: [] });
  }
  return Promise.resolve({ exit_code: 0, stdout: 'ok', stderr: '' });
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { RepositoryView } from './RepositoryView';

describe('RepositoryView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the Repository Harness heading', () => {
    render(<RepositoryView pushToast={vi.fn()} />);
    expect(screen.getByText('Repository Harness')).toBeTruthy();
  });

  it('every action button carries an explicit type="button"', async () => {
    render(<RepositoryView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('Workspace status')).toBeTruthy());
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('marks the command output region as a polite live region', () => {
    render(<RepositoryView pushToast={vi.fn()} />);
    const out = screen.getByLabelText('Command output');
    expect(out.getAttribute('aria-live')).toBe('polite');
  });
});
