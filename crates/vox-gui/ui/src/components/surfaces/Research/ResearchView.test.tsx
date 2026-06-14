// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const SESSIONS = [
  { id: 1, status: 'completed', query_text: 'What is Vox?', started_at_ms: 0, finished_at_ms: 1 },
];

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'list_research_sessions') return Promise.resolve(SESSIONS);
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { ResearchView } from './ResearchView';

describe('ResearchView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the Research heading', () => {
    render(<ResearchView pushToast={vi.fn()} />);
    expect(screen.getByText('Research')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<ResearchView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('What is Vox?')).toBeTruthy());
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('the research query input is labeled', () => {
    render(<ResearchView pushToast={vi.fn()} />);
    expect(screen.getByLabelText('Research question')).toBeTruthy();
  });

  it('exposes the session history as role=list', async () => {
    render(<ResearchView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByRole('list').length).toBeGreaterThan(0));
    expect(screen.getAllByRole('listitem').length).toBe(SESSIONS.length);
  });
});
