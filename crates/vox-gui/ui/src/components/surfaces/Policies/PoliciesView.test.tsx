// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const ROWS = [
  { id: 'fmt.rust', title: 'Rust formatting', domain: 'format', group: 'Formatting' },
  { id: 'lint.clippy', title: 'Clippy', domain: 'lint', group: 'Lint' },
];

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'policy_list') return Promise.resolve(ROWS);
  if (cmd === 'list_branches') return Promise.resolve([{ branch: 'main', isCurrent: true }]);
  if (cmd === 'policy_status') return Promise.resolve([]);
  if (cmd === 'policy_show') {
    return Promise.resolve({
      id: 'fmt.rust', title: 'Rust formatting', domain: 'format', description: 'x',
      blocking: true, runsOn: ['push'], origin: 'builtin', sourceKind: 'lint', sourceRef: 'r',
    });
  }
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { PoliciesView } from './PoliciesView';

describe('PoliciesView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the Policies rail heading', async () => {
    render(<PoliciesView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('Policies')).toBeTruthy());
  });

  it('every button carries an explicit type="button"', async () => {
    render(<PoliciesView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('Policies')).toBeTruthy());
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('gives the rail collapse toggle an aria-label and aria-expanded', async () => {
    render(<PoliciesView pushToast={vi.fn()} />);
    const toggle = await screen.findByRole('button', { name: /collapse policy rail|expand policy rail/i });
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
  });

  it('exposes branch toggle chips with aria-pressed', async () => {
    render(<PoliciesView pushToast={vi.fn()} />);
    const chip = await screen.findByRole('button', { name: /branch: main/i });
    expect(chip.getAttribute('aria-pressed')).toBe('true');
  });
});
