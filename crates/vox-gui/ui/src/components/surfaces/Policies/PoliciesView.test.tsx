// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor, fireEvent } from '@testing-library/react';
import React from 'react';

const ROWS = [
  { id: 'fmt.rust', title: 'Rust formatting', domain: 'format', group: 'Formatting', enabled: true, protected: false, blocking: false, severity: null },
  { id: 'lint.clippy', title: 'Clippy', domain: 'lint', group: 'Lint', enabled: true, protected: false, blocking: false, severity: null },
];

const DETAIL_BASE = {
  id: 'fmt.rust', title: 'Rust formatting', domain: 'format', group: 'Formatting',
  description: 'x', blocking: true, runsOn: ['push'], origin: 'builtin',
  sourceKind: 'lint', sourceRef: 'r', sourceDetail: null, docs: null,
  severity: null, enabled: true, protected: false,
};

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'policy_list') return Promise.resolve(ROWS);
  if (cmd === 'list_branches') return Promise.resolve([{ branch: 'main', isCurrent: true }]);
  if (cmd === 'policy_status') return Promise.resolve([]);
  if (cmd === 'policy_show') return Promise.resolve(DETAIL_BASE);
  if (cmd === 'policy_set_enabled') return Promise.resolve(undefined);
  if (cmd === 'policy_edit') return Promise.resolve(undefined);
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
    // Reset detail to enabled=true for each test
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'policy_list') return Promise.resolve(ROWS);
      if (cmd === 'list_branches') return Promise.resolve([{ branch: 'main', isCurrent: true }]);
      if (cmd === 'policy_status') return Promise.resolve([]);
      if (cmd === 'policy_show') return Promise.resolve(DETAIL_BASE);
      if (cmd === 'policy_set_enabled') return Promise.resolve(undefined);
      if (cmd === 'policy_edit') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
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

  it('renders policy tree rail and detail pane', async () => {
    render(<PoliciesView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByRole('navigation', { name: /policy tree/i })).toBeTruthy());
    expect(screen.getByRole('region', { name: /policy detail/i })).toBeTruthy();
  });

  it('Disable button calls policy_set_enabled(id, false) and is not disabled for non-protected policy', async () => {
    render(<PoliciesView pushToast={vi.fn()} />);
    // Wait for detail to load (enabled=true, protected=false)
    const disableBtn = await screen.findByRole('button', { name: /disable/i });
    expect(disableBtn).toBeTruthy();
    expect(disableBtn.hasAttribute('disabled')).toBe(false);

    fireEvent.click(disableBtn);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('policy_set_enabled', { id: 'fmt.rust', enabled: false });
    });
  });
});
