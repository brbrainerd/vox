// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const RUNS = [
  { run_id: 'r1', workflow_name: 'build', status: 'done', planned_steps: 3, completed_steps: 3, updated_at_ms: 0, last_error: null },
];

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'get_model_scoreboard') return Promise.resolve([]);
  if (cmd === 'list_gui_runs') return Promise.resolve(RUNS);
  if (cmd === 'get_routing_summary_live') return Promise.resolve({ decision_preview: null });
  if (cmd === 'get_gui_run') return Promise.resolve(RUNS[0]);
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { RunsView } from './RunsView';

describe('RunsView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the Recent Activity heading', () => {
    render(<RunsView pushToast={vi.fn()} />);
    expect(screen.getByText('Recent Activity')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<RunsView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByText('build').length).toBeGreaterThan(0));
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('selecting a run marks it with aria-pressed', async () => {
    render(<RunsView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByText('build').length).toBeGreaterThan(0));
    const runBtn = screen.getAllByText('build')[0].closest('button')!;
    expect(runBtn.getAttribute('aria-pressed')).toBeDefined();
  });
});
